//! aster-vec backed durable memory store — the convergence target for Aster
//! memory.
//!
//! One system, Aster's own engine: memory items live in an in-process aster-vec
//! collection with a full-text index over their content, so recall is a real
//! relevance query instead of loading every item. Importance and type ride
//! alongside as typed fields; vector (semantic) retrieval slots into the same
//! collection once an embedding source is wired.
//!
//! Primary records and full-text search use `aster-vec`. Semantic recall shares
//! the caller-owned vector primitives used by typed memory and workspace retrieval.

use anyhow::{anyhow, Context as _, Result};
use aster_durable_memory::vector::{
    InMemoryVectorIndex, VectorIndex, VectorIndexDescriptor, VectorRecord, VectorSearchRequest,
};
use aster_durable_memory::{MemoryItem, MemoryStore, MemoryType, ValidatedPrunePolicy};
use aster_vec::{
    Collection, CollectionOptions, CollectionSchema, DataType, Doc, Durability, FieldSchema,
    IndexParams, SearchQuery,
};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};

pub mod management;

mod hygiene;
use hygiene::MemoryHygiene;

static ENGINE_INIT: OnceLock<Result<(), String>> = OnceLock::new();

fn ensure_initialized() -> Result<(), String> {
    // The embedding host owns process-wide I/O and storage defaults.
    ENGINE_INIT
        .get_or_init(|| {
            if aster_vec::is_initialized() {
                Ok(())
            } else {
                aster_vec::initialize(None).map_err(|error| error.to_string())
            }
        })
        .clone()
}

fn display_error(error: impl std::fmt::Display) -> anyhow::Error {
    anyhow!(error.to_string())
}

/// True when the engine error is the cross-process exclusive lock holding
/// the collection (the multi-session recall stall of #17).
fn is_collection_lock_held(error: &aster_vec::Error) -> bool {
    let text = error.to_string();
    text.contains("collection is locked by another process")
}

#[derive(Debug, thiserror::Error)]
#[error("memory collection is locked by another Aster session; recall is skipped without stalling the first token (#17)")]
struct MemoryCollectionBusy(#[source] std::io::Error);

/// Fast refusal when a live peer holds the collection's exclusive lock.
/// Uses the engine's own lock file directly so the check precedes the
/// engine initialization entirely.
fn probe_collection_lock(root: &Path) -> Result<()> {
    let lock_path = root.join(COLLECTION_NAME).join(".aster-vec.lock");
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
    else {
        return Ok(()); // first use or unreadable — the normal open path names it
    };
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            drop(file); // probe only; the real open takes it for the lifetime
            Ok(())
        }
        Err(error) => Err(MemoryCollectionBusy(error).into()),
    }
}

/// Open with a shared (read-only) lock so concurrent readers coexist.
fn open_read_only(collection_path: &str) -> Result<Collection> {
    let mut options = memory_collection_options()?;
    options.set_read_only(true)?;
    Ok(Collection::open(collection_path, Some(&options))?)
}

fn memory_collection_options() -> Result<CollectionOptions> {
    let mut options = CollectionOptions::new()?;
    options.set_durability(Durability::Always)?;
    options.set_wal_checkpoint_thresholds(128, 8 * 1024 * 1024)?;
    Ok(options)
}

/// One durable memory record as stored in the aster-vec collection.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub importance: f32,
    pub memory_type: String,
    /// RFC3339 timestamp (lexicographic order == chronological for UTC).
    pub timestamp: String,
    /// Serialized auxiliary state (metadata map, access counters).
    pub meta: String,
}

impl VecMemoryStore {
    /// Stored record count from collection metadata, without reading memory bodies.
    pub fn record_count(&self) -> Result<usize> {
        self.collection.count().map_err(display_error)
    }
}

/// One vector per input text (specs/013 P3+P5 wiring seam). The host
/// injects any embedding provider; core stays dependency-clean.
pub trait SemanticEmbed: Send + Sync {
    fn embed(&self, text: &str) -> Option<Vec<f32>>;
}

/// An aster-vec collection holding durable memory items, opened at a fixed path.
pub struct VecMemoryStore {
    collection: Collection,
    path: PathBuf,
    /// Host-injected embedding callback for this store generation.
    semantic: Option<Arc<dyn SemanticEmbed>>,
    semantic_index: std::sync::Mutex<Option<InMemoryVectorIndex>>,
    /// Set when an existing collection already has one non-null fp32
    /// `embedding` column. Inserts fill it with zeros and recall stays FTS.
    embedding_dimension: Option<usize>,
    /// True when the exclusive lock was held by a live peer and the store
    /// opened with a shared (read-only) lock instead (#17).
    read_only: bool,
    mutation: std::sync::Mutex<MemoryHygiene>,
    /// Same-process exclusion. The engine file lock does not stop a second open here.
    /// Held so `Drop` releases the directory; the field itself is not read.
    _open_lease: OpenLease,
}

static OPEN_DIRECTORIES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

struct OpenLease(PathBuf);

impl Drop for OpenLease {
    fn drop(&mut self) {
        if let Ok(mut open) = OPEN_DIRECTORIES.lock() {
            open.remove(&self.0);
        }
    }
}

fn register_open_directory(path: PathBuf) -> Result<OpenLease> {
    let mut open = OPEN_DIRECTORIES
        .lock()
        .map_err(|_| anyhow!("memory open registry is poisoned"))?;
    if !open.insert(path.clone()) {
        return Err(anyhow!(
            "durable memory collection is already open at {}",
            path.display()
        ));
    }
    Ok(OpenLease(path))
}

const COLLECTION_NAME: &str = "durable_memory";
const CONTENT_FIELD: &str = "content";
const SEMANTIC_SOURCE_ID: &str = "aster.memory.legacy.id";

fn memory_schema() -> Result<CollectionSchema> {
    let mut content =
        FieldSchema::new(CONTENT_FIELD, DataType::String, false, 0).map_err(display_error)?;
    let fts = IndexParams::fts(Some("whitespace"), None, None).map_err(display_error)?;
    content.set_index_params(&fts).map_err(display_error)?;
    CollectionSchema::builder(COLLECTION_NAME)
        // The engine primary key (set on every Doc) IS the memory id; no
        // separate id field is declared.
        .add_field(content)
        .add_field(FieldSchema::new("tags", DataType::String, true, 0).map_err(display_error)?)
        .add_field(FieldSchema::new("importance", DataType::Float, true, 0).map_err(display_error)?)
        .add_field(
            FieldSchema::new("memory_type", DataType::String, true, 0).map_err(display_error)?,
        )
        .add_field(FieldSchema::new("timestamp", DataType::String, true, 0).map_err(display_error)?)
        .add_field(FieldSchema::new("meta", DataType::String, true, 0).map_err(display_error)?)
        .build()
        .map_err(display_error)
}

fn record_to_doc(record: &MemoryRecord) -> Result<Doc> {
    let mut doc = Doc::new()?;
    doc.set_pk(&record.id);
    doc.add_string(CONTENT_FIELD, &record.content)?;
    doc.add_string("tags", &record.tags.join(" "))?;
    doc.add_f32("importance", record.importance)?;
    doc.add_string("memory_type", &record.memory_type)?;
    doc.add_string("timestamp", &record.timestamp)?;
    doc.add_string("meta", &record.meta)?;
    Ok(doc)
}

/// `None` is a full-text collection this store created. `Some` is an existing
/// collection with one fp32 `embedding` column. Any other shape is refused
/// before this store writes.
fn accepted_embedding_dimension(collection: &Collection) -> Result<Option<usize>> {
    let schema = collection.schema().map_err(display_error)?;
    let mut names: Vec<&str> = schema.fields.iter().map(FieldSchema::name).collect();
    names.sort_unstable();
    const EXPECTED: [&str; 6] = [
        "content",
        "importance",
        "memory_type",
        "meta",
        "tags",
        "timestamp",
    ];
    if names != EXPECTED {
        return Err(anyhow!(
            "memory collection scalar fields do not match the durable memory set"
        ));
    }
    match schema.vectors() {
        [] => Ok(None),
        [vector]
            if vector.name() == "embedding"
                && vector.data_type() == DataType::VectorFp32
                && vector.dimension() > 0 =>
        {
            Ok(Some(vector.dimension() as usize))
        }
        _ => Err(anyhow!(
            "memory collection vector fields must be empty or one fp32 embedding column"
        )),
    }
}

fn memory_id(document: &Doc) -> Result<String> {
    document
        .get_pk()
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("aster-vec memory result omitted its primary key"))
}

impl VecMemoryStore {
    /// Open the memory collection at `root`, creating it on first use.
    /// When another live process holds the exclusive lock, the store opens
    /// read-only (shared lock) so concurrent sessions recall without the
    /// first-token stall of a failed open (#17).
    pub fn open(root: &Path) -> Result<Self> {
        // The lock probe costs microseconds; the engine init it precedes can
        // cost seconds on a cold cache. On a locked collection (#17) the
        // probe names the peer immediately instead of stalling the init.
        probe_collection_lock(root)?;
        ensure_initialized().map_err(|error| anyhow!("aster-vec engine init failed: {error}"))?;
        fs_err_create_dir_all(root)?;
        let canonical = root
            .canonicalize()
            .with_context(|| format!("canonicalize memory root {}", root.display()))?;
        // Register before opening. A refused second open must not close a file
        // descriptor that would release the first store's engine lock.
        let open_lease = register_open_directory(canonical)?;
        let collection_path = root.join(COLLECTION_NAME);
        let collection_path = collection_path
            .to_str()
            .ok_or_else(|| anyhow!("memory collection path is not UTF-8"))?;
        let options = memory_collection_options()?;
        let (collection, embedding_dimension, read_only) =
            if collection_path_exists(collection_path) {
                match Collection::open(collection_path, Some(&options)) {
                    Ok(collection) => {
                        let embedding_dimension = accepted_embedding_dimension(&collection)?;
                        (collection, embedding_dimension, false)
                    }
                    Err(error) if is_collection_lock_held(&error) => {
                        let read_only = open_read_only(collection_path)?;
                        let embedding_dimension = accepted_embedding_dimension(&read_only)?;
                        (read_only, embedding_dimension, true)
                    }
                    Err(error) => return Err(error.into()),
                }
            } else {
                let schema = memory_schema()?;
                match Collection::create_and_open(collection_path, &schema, Some(&options)) {
                    Ok(collection) => (collection, None, false),
                    // Both sessions raced the create; the winner holds the
                    // exclusive lock and made the collection exist — read it.
                    Err(error) if is_collection_lock_held(&error) => {
                        let read_only = open_read_only(collection_path)?;
                        let embedding_dimension = accepted_embedding_dimension(&read_only)?;
                        (read_only, embedding_dimension, true)
                    }
                    Err(error) => return Err(error.into()),
                }
            };
        let hygiene = MemoryHygiene::load(&collection)?;
        let semantic: Option<Arc<dyn SemanticEmbed>> = None;
        Ok(Self {
            collection,
            path: collection_path.into(),
            semantic,
            semantic_index: std::sync::Mutex::new(None),
            embedding_dimension,
            read_only,
            mutation: std::sync::Mutex::new(hygiene),
            _open_lease: open_lease,
        })
    }

    /// Insert or replace one memory record by id.
    pub fn put(&self, record: &MemoryRecord) -> Result<()> {
        let mut hygiene = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("memory mutation lock is poisoned"))?;
        self.ensure_writable()?;
        hygiene.ensure_current(&self.collection)?;
        // Raw SDK writes retain their exact-content successful no-op policy.
        if hygiene.exact_duplicate(&record.content).is_some() {
            tracing::debug!(id = %record.id, "memory write skipped: exact text duplicate");
            let existing =
                self.collection
                    .fetch_with_options(&[&record.id], Some(&[CONTENT_FIELD]), false)?;
            if let Some(document) = existing.first() {
                if let Some(content) = document.get_string(CONTENT_FIELD)? {
                    self.update_semantic(&record.id, Some(&content));
                }
            }
            return Ok(());
        }
        self.commit_record(record, &mut hygiene)
    }

    fn ensure_writable(&self) -> Result<()> {
        if self.read_only {
            return Err(aster_vec::Error::permission_denied(
                "memory store is read-only: another Aster session holds the collection lock; \
                 the write is skipped and can be retried in an exclusive session",
            )
            .into());
        }
        Ok(())
    }

    fn commit_record(&self, record: &MemoryRecord, hygiene: &mut MemoryHygiene) -> Result<()> {
        let existing =
            self.collection
                .fetch_with_options(&[&record.id], Some(&[CONTENT_FIELD]), false)?;
        let old_content = existing
            .first()
            .map(|document| document.get_string(CONTENT_FIELD))
            .transpose()?
            .flatten();
        let mut doc = record_to_doc(record)?;
        if let Some(dimension) = self.embedding_dimension {
            let zeros = vec![0.0_f32; dimension];
            doc.add_vector_f32("embedding", &zeros)?;
        }
        let references = [&doc];
        hygiene.invalidate();
        let result = self.collection.upsert(&references)?;
        if let Some(rejected) = result.results.into_iter().find(|result| !result.success) {
            return Err(aster_vec::Error::new(rejected.code, rejected.message).into());
        }
        if let Some(content) = old_content {
            hygiene.forget(&record.id, &content);
        }
        hygiene.remember(&record.id, &record.content);
        hygiene.finish();
        self.update_semantic(&record.id, Some(&record.content));
        Ok(())
    }

    fn store_and_return_blocking(&self, mut item: MemoryItem) -> Result<MemoryItem> {
        let mut hygiene = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("memory mutation lock is poisoned"))?;
        self.ensure_writable()?;
        hygiene.ensure_current(&self.collection)?;
        item.content_lower = item.content.to_lowercase();
        if self
            .collection
            .fetch_with_options(&[&item.id], Some(&[]), false)?
            .is_empty()
        {
            if let Some(id) = hygiene.durable_duplicate(&item.content) {
                let canonical = self.retrieve_blocking(id)?.ok_or_else(|| {
                    aster_vec::Error::internal("memory duplicate lookup lost its primary record")
                })?;
                item = canonical.merge_duplicate(item);
            }
        }
        self.commit_record(&item_to_record(&item)?, &mut hygiene)?;
        Ok(item)
    }

    /// Full-text recall: rank memory records by FTS relevance to `query`.
    /// Returns `(id, score)` pairs, best first.
    pub fn recall(&self, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut fts = aster_vec::Fts::new().map_err(display_error)?;
        fts.set_match_string(query).map_err(display_error)?;
        let topk = i32::try_from(limit).map_err(|_| anyhow!("memory recall limit exceeds i32"))?;
        let mut search = SearchQuery::fts(CONTENT_FIELD, &fts, topk).map_err(display_error)?;
        search.set_output_fields(&[]).map_err(display_error)?;
        let documents = self.collection.query(&search).map_err(display_error)?;
        let fts_hits: Vec<(String, f32)> = documents
            .iter()
            .map(memory_id)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .zip(documents.iter().map(Doc::get_score))
            .collect();

        // Semantic recall (specs/013 P3+P5): cosine against the embedded
        // index via the light tier. When the provider is absent or the
        // query yields no embedding, FTS alone serves (fail-open to lexical).
        let semantic_hits = self.semantic_recall(query, limit * 2);

        // Merge by RRF: 1/(k+rank) with k=60 (the same constant the
        // retrieval pipeline uses), preserving the best of both channels.
        Ok(rrf_merge(fts_hits, semantic_hits, limit))
    }

    /// Cosine recall through the shared, bounded exact index.
    fn semantic_recall(&self, query: &str, limit: usize) -> Vec<(String, f32)> {
        let Some(provider) = &self.semantic else {
            return Vec::new();
        };
        let index = match self.semantic_index.lock() {
            Ok(slot) => slot.clone(),
            Err(_) => {
                tracing::warn!("Legacy memory semantic index lock is poisoned");
                return Vec::new();
            }
        };
        let Some(index) = index else {
            return Vec::new();
        };
        if index.status().record_count == 0 {
            return Vec::new();
        }
        let Some(query_vector) = provider.embed(query) else {
            return Vec::new();
        };
        match index.search_blocking(VectorSearchRequest::new(query_vector, limit)) {
            Ok(result) => result
                .hits
                .into_iter()
                .filter_map(|hit| {
                    hit.labels
                        .get(SEMANTIC_SOURCE_ID)
                        .cloned()
                        .map(|id| (id, hit.score))
                })
                .collect(),
            Err(_) => {
                tracing::warn!("Legacy memory semantic query was rejected by the shared index");
                Vec::new()
            }
        }
    }

    fn update_semantic(&self, id: &str, content: Option<&str>) {
        let Some(provider) = &self.semantic else {
            return;
        };
        let embedding = content.and_then(|text| provider.embed(text));
        // Fixed delimiters preserve valid legacy IDs whose edges contain whitespace.
        let partition = format!("legacy:{id}:");
        let result: Result<()> = (|| {
            let mut slot = self
                .semantic_index
                .lock()
                .map_err(|_| anyhow!("memory semantic index lock is poisoned"))?;
            let index = match slot.as_ref() {
                Some(index) => index.clone(),
                None => {
                    let Some(vector) = embedding.as_ref() else {
                        return Ok(());
                    };
                    InMemoryVectorIndex::new(VectorIndexDescriptor::new(vector.len()))?
                }
            };
            let mutation = match embedding {
                Some(vector) => index.replace_partition_blocking(
                    &partition,
                    vec![VectorRecord::new(&partition, vector).with_label(SEMANTIC_SOURCE_ID, id)],
                ),
                None => index.remove_partition_blocking(&partition),
            };
            if let Err(error) = mutation {
                // A rejected replacement must not retain a vector for the previous body.
                index.remove_partition_blocking(&partition)?;
                return Err(error.into());
            }
            *slot = Some(index);
            Ok(())
        })();
        if result.is_err() {
            tracing::warn!(
                "Legacy memory semantic update was rejected; lexical recall remains available"
            );
        }
    }

    /// Inject a semantic embedding provider. Call once after `open`;
    /// The previous derived generation is discarded; existing entries are
    /// embedded on their next successful `put` (including a content no-op).
    pub fn set_semantic_provider(&mut self, provider: Arc<dyn SemanticEmbed>) {
        self.semantic_index = std::sync::Mutex::new(None);
        self.semantic = Some(provider);
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One-time import of `items/*.json` written by `FileMemoryStore`.
    /// An id already in the collection is skipped. A file that cannot be read
    /// or parsed is an error after the readable files are stored, so the
    /// caller can leave the directory in place.
    pub fn import_legacy_json_items(&self, legacy_items_dir: &Path) -> Result<usize> {
        let entries = match std::fs::read_dir(legacy_items_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => {
                return Err(anyhow!(
                    "read legacy memory items {}: {error}",
                    legacy_items_dir.display()
                ));
            }
        };
        let mut existing = std::collections::HashSet::new();
        for document in self.collection.iter().map_err(display_error)? {
            existing.insert(memory_id(&document.map_err(display_error)?)?);
        }
        let mut imported = 0usize;
        let mut complete = true;
        for entry in entries {
            let Ok(entry) = entry else {
                complete = false;
                continue;
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&path) else {
                complete = false;
                continue;
            };
            let Ok(item) = serde_json::from_str::<MemoryItem>(&raw) else {
                complete = false;
                continue;
            };
            if existing.contains(item.id.as_str()) {
                continue;
            }
            let id = item.id.clone();
            self.store_and_return_blocking(item)?;
            existing.insert(id);
            imported += 1;
        }
        if !complete {
            return Err(anyhow!(
                "skipped unreadable legacy memory files in {}; imported {imported}",
                legacy_items_dir.display()
            ));
        }
        Ok(imported)
    }
}

/// Opens the durable aster-vec collection on the first memory operation.
///
/// Operations require a Tokio runtime and run synchronous engine work on its
/// blocking pool. Cancelling an await does not abort an already running write;
/// that worker retains the collection until it finishes.
pub struct LazyFileMemoryStore {
    directory: PathBuf,
    store: tokio::sync::OnceCell<SharedMemoryStore>,
}

#[derive(Clone)]
struct SharedMemoryStore(Option<Arc<VecMemoryStore>>);

impl SharedMemoryStore {
    fn get(&self) -> &VecMemoryStore {
        // Constructed with Some; only exclusive Drop takes it out.
        self.0.as_deref().expect("live shared memory handle")
    }
}

impl Drop for SharedMemoryStore {
    fn drop(&mut self) {
        let store = self.0.take();
        // A dead Weak must not race the previous engine/lease destructor.
        // Workers retain this same release protocol after an await is cancelled.
        let _registry = SHARED_STORES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(store);
    }
}

// Lazy session adapters share one engine handle in this process. Weak entries
// do not keep a collection open after the last session releases it. Direct
// VecMemoryStore::open callers retain their exclusive-open contract.
static SHARED_STORES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, Weak<VecMemoryStore>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

impl LazyFileMemoryStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            store: tokio::sync::OnceCell::new(),
        }
    }

    async fn inner(&self, runtime: &tokio::runtime::Handle) -> Result<&SharedMemoryStore> {
        self.store
            .get_or_try_init(|| async {
                let directory = self.directory.clone();
                let span = tracing::Span::current();
                runtime
                    .spawn_blocking(move || {
                        let _entered = span.enter();
                        Self::shared(&directory)
                    })
                    .await
                    .context("memory initialization worker failed")?
                    .map_err(|error| {
                        let message = format!(
                            "failed to initialize aster-vec Memory store at {}: {error:#}",
                            self.directory.display(),
                        );
                        error.context(message)
                    })
            })
            .await
    }

    async fn with_store<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&VecMemoryStore) -> T + Send + 'static,
    ) -> Result<T> {
        let runtime = tokio::runtime::Handle::try_current()
            .context("lazy memory operations require a Tokio runtime")?;
        let store = self.inner(&runtime).await?.clone();
        let span = tracing::Span::current();
        runtime
            .spawn_blocking(move || {
                let _entered = span.enter();
                operation(store.get())
            })
            .await
            .context("memory operation worker failed")
    }

    fn shared(directory: &Path) -> Result<SharedMemoryStore> {
        fs_err_create_dir_all(directory)?;
        let canonical = directory
            .canonicalize()
            .with_context(|| format!("canonicalize memory root {}", directory.display()))?;
        let mut stores = SHARED_STORES
            .lock()
            .map_err(|_| anyhow!("shared memory registry is poisoned"))?;
        stores.retain(|_, store| store.strong_count() > 0);
        if let Some(store) = stores.get(&canonical).and_then(Weak::upgrade) {
            return Ok(SharedMemoryStore(Some(store)));
        }
        // Open and import are synchronous. Publish only after both finish so
        // simultaneous first use cannot open or import the same collection twice.
        // No synchronous registry guard is held across an await.
        let store = Arc::new(Self::open_and_import(&canonical)?);
        stores.insert(canonical, Arc::downgrade(&store));
        Ok(SharedMemoryStore(Some(store)))
    }

    fn open_and_import(directory: &Path) -> Result<VecMemoryStore> {
        let store = VecMemoryStore::open(directory)?;
        let legacy_items = directory.join("items");
        let legacy_index = directory.join("index.json");
        if legacy_items.is_dir() {
            match store.import_legacy_json_items(&legacy_items) {
                Ok(imported) if imported > 0 => {
                    tracing::info!(
                        imported,
                        dir = %directory.display(),
                        "imported legacy JSON memories into aster-vec"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "legacy memory import failed; legacy files kept"
                    );
                    return Ok(store);
                }
            }
            let _ = std::fs::rename(&legacy_items, directory.join("items.imported"));
            if legacy_index.is_file() {
                let _ = std::fs::rename(&legacy_index, directory.join("index.json.imported"));
            }
        }
        Ok(store)
    }

    #[cfg(test)]
    fn is_initialized(&self) -> bool {
        self.store.get().is_some()
    }
}

#[async_trait::async_trait]
impl MemoryStore for LazyFileMemoryStore {
    async fn store(&self, item: MemoryItem) -> Result<()> {
        self.store_and_return(item).await.map(|_| ())
    }

    async fn store_and_return(&self, item: MemoryItem) -> Result<MemoryItem> {
        self.with_store(move |store| store.store_and_return_blocking(item))
            .await?
    }

    async fn retrieve(&self, id: &str) -> Result<Option<MemoryItem>> {
        let id = id.to_owned();
        self.with_store(move |store| store.retrieve_blocking(&id))
            .await?
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>> {
        let query = query.to_owned();
        self.with_store(move |store| store.search_blocking(&query, limit))
            .await?
    }

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<MemoryItem>> {
        let tags = tags.to_owned();
        self.with_store(move |store| store.search_by_tags_blocking(&tags, limit))
            .await?
    }

    async fn get_recent(&self, limit: usize) -> Result<Vec<MemoryItem>> {
        self.with_store(move |store| store.get_recent_blocking(limit))
            .await?
    }

    async fn get_important(&self, threshold: f32, limit: usize) -> Result<Vec<MemoryItem>> {
        self.with_store(move |store| store.get_important_blocking(threshold, limit))
            .await?
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.to_owned();
        self.with_store(move |store| store.delete_blocking(&id))
            .await?
    }

    async fn clear(&self) -> Result<()> {
        self.with_store(VecMemoryStore::clear_blocking).await?
    }

    async fn count(&self) -> Result<usize> {
        self.with_store(VecMemoryStore::record_count).await?
    }

    async fn prune(&self, policy: &aster_durable_memory::PrunePolicy) -> Result<usize> {
        let policy = ValidatedPrunePolicy::try_new(policy, Utc::now())?;
        self.with_store(move |store| store.prune_blocking(&policy))
            .await?
    }
}

fn fs_err_create_dir_all(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("create memory root {}", root.display()))?;
    crate::secure_state::restrict_state_dir(root)
        .with_context(|| format!("restrict memory root {}", root.display()))
}

#[cfg(windows)]
fn collection_path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

#[cfg(not(windows))]
fn collection_path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod async_tests;

#[cfg(test)]
mod wal_tests;

#[cfg(test)]
mod canonical_tests;

#[cfg(test)]
mod canonical_lifecycle_tests;

#[cfg(test)]
#[path = "durable_memory_vec/semantic_tests.rs"]
mod semantic_tests;

/// Metadata that rides with a record but does not need to be queryable.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct RecordMeta {
    #[serde(default)]
    metadata: std::collections::HashMap<String, String>,
    #[serde(default)]
    access_count: u32,
    #[serde(default)]
    last_accessed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

fn memory_type_to_string(memory_type: &MemoryType) -> Result<String> {
    serde_json::to_string(memory_type).map_err(display_error)
}

fn memory_type_from_string(raw: &str) -> Result<MemoryType> {
    serde_json::from_str(raw).map_err(display_error)
}

fn doc_to_item(document: &Doc) -> Result<MemoryItem> {
    let id = memory_id(document)?;
    let content = document
        .get_string(CONTENT_FIELD)
        .map_err(display_error)?
        .unwrap_or_default();
    let tags = document
        .get_string("tags")
        .map_err(display_error)?
        .unwrap_or_default();
    let importance = document
        .get_f32("importance")
        .map_err(display_error)?
        .unwrap_or_default();
    let memory_type = document
        .get_string("memory_type")
        .map_err(display_error)?
        .unwrap_or_default();
    let timestamp = document
        .get_string("timestamp")
        .map_err(display_error)?
        .unwrap_or_default();
    let meta_raw = document
        .get_string("meta")
        .map_err(display_error)?
        .unwrap_or_default();
    let meta: RecordMeta = serde_json::from_str(&meta_raw).unwrap_or_default();
    let timestamp = DateTime::parse_from_rfc3339(&timestamp)
        .map(|parsed| parsed.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let memory_type = memory_type_from_string(&memory_type).unwrap_or(MemoryType::Semantic);
    Ok(MemoryItem {
        id,
        content_lower: content.to_lowercase(),
        content,
        timestamp,
        importance,
        tags: decode_stored_tags(meta.tags, &tags),
        memory_type,
        metadata: meta.metadata,
        access_count: meta.access_count,
        last_accessed: meta.last_accessed.and_then(|raw| {
            DateTime::parse_from_rfc3339(&raw)
                .map(|parsed| parsed.with_timezone(&Utc))
                .ok()
        }),
    })
}

fn item_to_record(item: &MemoryItem) -> Result<MemoryRecord> {
    let meta = RecordMeta {
        metadata: item.metadata.clone(),
        access_count: item.access_count,
        last_accessed: item.last_accessed.map(|stamp| stamp.to_rfc3339()),
        tags: item
            .tags
            .iter()
            .any(|tag| tag.is_empty() || tag.chars().any(char::is_whitespace))
            .then(|| item.tags.clone()),
    };
    Ok(MemoryRecord {
        id: item.id.clone(),
        content: item.content.clone(),
        tags: item.tags.clone(),
        importance: item.importance,
        memory_type: memory_type_to_string(&item.memory_type)?,
        timestamp: item.timestamp.to_rfc3339(),
        meta: serde_json::to_string(&meta).map_err(display_error)?,
    })
}

fn decode_stored_tags(exact: Option<Vec<String>>, legacy: &str) -> Vec<String> {
    exact.unwrap_or_else(|| legacy.split_whitespace().map(str::to_owned).collect())
}

impl VecMemoryStore {
    fn prune_blocking(&self, policy: &ValidatedPrunePolicy) -> Result<usize> {
        let mut hygiene = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("memory mutation lock is poisoned"))?;
        if self.read_only {
            return Err(aster_vec::Error::permission_denied("memory owner is read-only").into());
        }
        let started = std::time::Instant::now();
        let mut items = Vec::new();
        for document in self.collection.iter()? {
            items.push(doc_to_item(&document?)?);
        }
        let read_elapsed = started.elapsed();
        let started = std::time::Instant::now();
        let delete_ids = policy.select(&items)?;
        let selection_elapsed = started.elapsed();
        let started = std::time::Instant::now();
        let ids: Vec<_> = delete_ids.iter().map(String::as_str).collect();
        self.delete_records(&ids, &mut hygiene)?;
        tracing::debug!(
            target: "aster_core::memory_retention",
            records = items.len(), deleted = delete_ids.len(), delete_batches = usize::from(!delete_ids.is_empty()),
            read_ms = read_elapsed.as_secs_f64() * 1_000.0,
            selection_ms = selection_elapsed.as_secs_f64() * 1_000.0,
            deletion_ms = started.elapsed().as_secs_f64() * 1_000.0,
            "native memory retention completed"
        );
        Ok(delete_ids.len())
    }

    fn retrieve_blocking(&self, id: &str) -> Result<Option<MemoryItem>> {
        self.collection
            .fetch_with_options(&[id], None, false)?
            .first()
            .map(doc_to_item)
            .transpose()
    }

    fn search_blocking(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>> {
        let hits = self.recall(query, limit)?;
        if hits.is_empty() {
            return Ok(Vec::new());
        }
        // Primary-key fetch, not a collection scan: this runs on EVERY
        // model prompt (recall_similar), and the previous full `iter()`
        // made per-turn recall O(collection) to map ≤limit ids back to
        // documents. `fetch` keeps the FTS ranking via the re-sort below.
        // Vectors are skipped — recall needs fields only.
        let pks: Vec<&str> = hits.iter().map(|(id, _)| id.as_str()).collect();
        let documents = self
            .collection
            .fetch_with_options(&pks, None, false)
            .map_err(display_error)?;
        let mut items = Vec::with_capacity(documents.len());
        for document in &documents {
            items.push(doc_to_item(document)?);
        }
        // Preserve FTS ranking order (fetch returns primary-key order).
        items.sort_by_cached_key(|item| {
            hits.iter()
                .position(|(id, _)| id == &item.id)
                .unwrap_or(usize::MAX)
        });
        Ok(items)
    }

    fn search_by_tags_blocking(&self, tags: &[String], limit: usize) -> Result<Vec<MemoryItem>> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }
        let wanted: std::collections::HashSet<&str> = tags.iter().map(|tag| tag.as_str()).collect();
        let mut items = Vec::new();
        for document in self.collection.iter().map_err(display_error)? {
            let document = document.map_err(display_error)?;
            let item = doc_to_item(&document)?;
            if item.tags.iter().any(|tag| wanted.contains(tag.as_str())) {
                items.push(item);
                if items.len() == limit {
                    break;
                }
            }
        }
        Ok(items)
    }

    fn get_recent_blocking(&self, limit: usize) -> Result<Vec<MemoryItem>> {
        let mut items = Vec::new();
        for document in self.collection.iter().map_err(display_error)? {
            items.push(doc_to_item(&document.map_err(display_error)?)?);
        }
        items.sort_by_key(|item| std::cmp::Reverse(item.timestamp));
        items.truncate(limit);
        Ok(items)
    }

    fn get_important_blocking(&self, threshold: f32, limit: usize) -> Result<Vec<MemoryItem>> {
        let mut items = Vec::new();
        for document in self.collection.iter().map_err(display_error)? {
            let item = doc_to_item(&document.map_err(display_error)?)?;
            if item.importance >= threshold {
                items.push(item);
            }
        }
        items.sort_by(|left, right| {
            right
                .importance
                .partial_cmp(&left.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        items.truncate(limit);
        Ok(items)
    }

    fn delete_blocking(&self, id: &str) -> Result<()> {
        let mut hygiene = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("memory mutation lock is poisoned"))?;
        self.delete_record(id, &mut hygiene)
    }

    fn clear_blocking(&self) -> Result<()> {
        let mut hygiene = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("memory mutation lock is poisoned"))?;
        let mut ids = Vec::new();
        for document in self.collection.iter().map_err(display_error)? {
            ids.push(memory_id(&document.map_err(display_error)?)?);
        }
        if ids.is_empty() {
            hygiene.clear();
            return Ok(());
        }
        let refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        hygiene.invalidate();
        self.commit_delete(&refs)?;
        hygiene.clear();
        *self
            .semantic_index
            .lock()
            .map_err(|_| anyhow!("memory semantic index lock is poisoned"))? = None;
        Ok(())
    }
}

#[async_trait::async_trait]
impl MemoryStore for VecMemoryStore {
    async fn store(&self, item: MemoryItem) -> Result<()> {
        self.store_and_return_blocking(item).map(|_| ())
    }

    async fn store_and_return(&self, item: MemoryItem) -> Result<MemoryItem> {
        self.store_and_return_blocking(item)
    }

    async fn retrieve(&self, id: &str) -> Result<Option<MemoryItem>> {
        self.retrieve_blocking(id)
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>> {
        self.search_blocking(query, limit)
    }

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<MemoryItem>> {
        self.search_by_tags_blocking(tags, limit)
    }

    async fn get_recent(&self, limit: usize) -> Result<Vec<MemoryItem>> {
        self.get_recent_blocking(limit)
    }

    async fn get_important(&self, threshold: f32, limit: usize) -> Result<Vec<MemoryItem>> {
        self.get_important_blocking(threshold, limit)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.delete_blocking(id)
    }

    async fn clear(&self) -> Result<()> {
        self.clear_blocking()
    }

    async fn count(&self) -> Result<usize> {
        self.record_count()
    }

    async fn prune(&self, policy: &aster_durable_memory::PrunePolicy) -> Result<usize> {
        let policy = ValidatedPrunePolicy::try_new(policy, Utc::now())?;
        self.prune_blocking(&policy)
    }
}

#[cfg(test)]
mod multi_session_lock_tests {
    use super::*;

    #[test]
    fn a_locked_collection_refuses_the_open_fast() {
        let root = tempfile::tempdir().unwrap();
        let memory_root = root.path().join("memory");
        let first = VecMemoryStore::open(&memory_root).expect("first session opens exclusive");
        assert!(!first.read_only);

        // The exclusive lock is held (this process, via `first`): the probe
        // must refuse before any engine work, naming the peer session.
        let refused =
            probe_collection_lock(&memory_root).expect_err("a locked collection refuses the probe");
        assert!(
            refused.to_string().contains("another Aster session"),
            "the refusal names the peer: {refused}"
        );
        assert!(
            refused.to_string().contains("#17"),
            "the refusal cites its origin: {refused}"
        );

        // The exclusive owner still writes and recalls normally.
        first
            .put(&MemoryRecord {
                id: "lock-probe".into(),
                content: "probe".into(),
                tags: Vec::new(),
                importance: 0.5,
                memory_type: "note".into(),
                timestamp: "2026-10-23T00:00:00Z".into(),
                meta: "{}".into(),
            })
            .expect("the exclusive owner still writes");
        drop(first);
        // After the owner releases, the probe succeeds again.
        probe_collection_lock(&memory_root).expect("released lock probes clean");
    }

    #[test]
    fn is_collection_lock_held_matches_the_engine_text() {
        let error = aster_vec::Error::new(
            aster_vec::ErrorCode::AlreadyExists,
            "collection is locked by another process (recorded exclusive owner pid=1): x",
        );
        assert!(is_collection_lock_held(&error));
        let other = aster_vec::Error::new(
            aster_vec::ErrorCode::InternalError,
            "unrelated engine failure",
        );
        assert!(!is_collection_lock_held(&other));
    }
}

/// Reciprocal Rank Fusion: 1/(k+rank) per channel, k=60. The merged score
/// is the sum; ties break by first-channel rank (FTS first). The limit
/// truncates after sort.
fn rrf_merge(
    fts: Vec<(String, f32)>,
    semantic: Vec<(String, f32)>,
    limit: usize,
) -> Vec<(String, f32)> {
    let mut fusion = crate::rank_fusion::RankFusion::<_, _, f64>::new();
    for (rank, (id, _)) in fts.into_iter().enumerate() {
        fusion.add(id, 0_u8, rank + 1, (), |_, _| {});
    }
    for (rank, (id, _)) in semantic.into_iter().enumerate() {
        fusion.add(id, 1_u8, rank + 1, (), |_, _| {});
    }
    let mut merged = fusion
        .into_entries()
        .map(|(id, entry)| {
            let first = entry.rank(0).map_or(usize::MAX, |rank| rank.get());
            let second = entry.rank(1).map_or(usize::MAX, |rank| rank.get());
            (id, entry.score() as f32, first, second)
        })
        .collect::<Vec<_>>();
    merged.sort_by(
        |(id, score, first, second), (other_id, other_score, other_first, other_second)| {
            other_score
                .total_cmp(score)
                .then_with(|| first.cmp(other_first))
                .then_with(|| second.cmp(other_second))
                .then_with(|| id.cmp(other_id))
        },
    );
    merged.truncate(limit);
    merged
        .into_iter()
        .map(|(id, score, _, _)| (id, score))
        .collect()
}
