//! a3s-vec backed durable memory store — the convergence target for A3S
//! memory.
//!
//! One system, A3S's own engine: memory items live in an in-process a3s-vec
//! collection with a full-text index over their content, so recall is a real
//! relevance query instead of loading every item. Importance and type ride
//! alongside as typed fields; vector (semantic) retrieval slots into the same
//! collection once an embedding source is wired.
//!
//! This module intentionally depends only on `a3s-vec` (already optional in
//! core behind `a3s-vec-fts`) so the memory layer and the workspace lexical
//! index share one engine and one operational model.

use a3s_memory::{MemoryItem, MemoryStore, MemoryType};
use a3s_vec::{Collection, CollectionSchema, DataType, Doc, FieldSchema, IndexParams, SearchQuery};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};

static ENGINE_INIT: OnceLock<Result<(), String>> = OnceLock::new();

fn ensure_initialized() -> Result<(), String> {
    // `a3s_vec::initialize` is idempotent; always call it so the cold path
    // stays reachable under the process-wide OnceLock.
    ENGINE_INIT
        .get_or_init(|| a3s_vec::initialize(None).map_err(|error| error.to_string()))
        .clone()
}

fn display_error(error: impl std::fmt::Display) -> anyhow::Error {
    anyhow!(error.to_string())
}

/// One durable memory record as stored in the a3s-vec collection.
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

/// An a3s-vec collection holding durable memory items, opened at a fixed path.
pub struct VecMemoryStore {
    collection: Collection,
    path: PathBuf,
    /// Set when an existing collection already has one non-null fp32
    /// `embedding` column. Inserts fill it with zeros and recall stays FTS.
    embedding_dimension: Option<usize>,
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
    let mut doc = Doc::new().map_err(display_error)?;
    doc.set_pk(&record.id);
    doc.add_string(CONTENT_FIELD, &record.content)
        .map_err(display_error)?;
    doc.add_string("tags", &record.tags.join(" "))
        .map_err(display_error)?;
    doc.add_f32("importance", record.importance)
        .map_err(display_error)?;
    doc.add_string("memory_type", &record.memory_type)
        .map_err(display_error)?;
    doc.add_string("timestamp", &record.timestamp)
        .map_err(display_error)?;
    doc.add_string("meta", &record.meta)
        .map_err(display_error)?;
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
        .ok_or_else(|| anyhow!("a3s-vec memory result omitted its primary key"))
}

impl VecMemoryStore {
    /// Open the memory collection at `root`, creating it on first use.
    pub fn open(root: &Path) -> Result<Self> {
        ensure_initialized().map_err(|error| anyhow!("a3s-vec engine init failed: {error}"))?;
        fs_err_create_dir_all(root)?;
        let canonical = root
            .canonicalize()
            .map_err(|error| anyhow!("canonicalize memory root {}: {error}", root.display()))?;
        // Register before opening. A refused second open must not close a file
        // descriptor that would release the first store's engine lock.
        let open_lease = register_open_directory(canonical)?;
        let collection_path = root.join(COLLECTION_NAME);
        let collection_path = collection_path
            .to_str()
            .ok_or_else(|| anyhow!("memory collection path is not UTF-8"))?;
        let (collection, embedding_dimension) = if collection_path_exists(collection_path) {
            let collection = Collection::open(collection_path, None).map_err(display_error)?;
            let embedding_dimension = accepted_embedding_dimension(&collection)?;
            (collection, embedding_dimension)
        } else {
            let schema = memory_schema()?;
            let collection = Collection::create_and_open(collection_path, &schema, None)
                .map_err(display_error)?;
            (collection, None)
        };
        Ok(Self {
            collection,
            path: collection_path.into(),
            embedding_dimension,
            _open_lease: open_lease,
        })
    }

    /// Insert or replace one memory record by id.
    pub fn put(&self, record: &MemoryRecord) -> Result<()> {
        let mut doc = record_to_doc(record)?;
        if let Some(dimension) = self.embedding_dimension {
            let zeros = vec![0.0_f32; dimension];
            doc.add_vector_f32("embedding", &zeros)
                .map_err(display_error)?;
        }
        let references = [&doc];
        let result = self.collection.insert(&references).map_err(display_error)?;
        if result.error_count != 0 {
            return Err(anyhow!(
                "a3s-vec memory insert rejected {} document(s)",
                result.error_count
            ));
        }
        self.collection.flush().map_err(display_error)?;
        Ok(())
    }

    /// Full-text recall: rank memory records by FTS relevance to `query`.
    /// Returns `(id, score)` pairs, best first.
    pub fn recall(&self, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut fts = a3s_vec::Fts::new().map_err(display_error)?;
        fts.set_match_string(query).map_err(display_error)?;
        let topk = i32::try_from(limit).map_err(|_| anyhow!("memory recall limit exceeds i32"))?;
        let mut search = SearchQuery::fts(CONTENT_FIELD, &fts, topk).map_err(display_error)?;
        search.set_output_fields(&[]).map_err(display_error)?;
        let documents = self.collection.query(&search).map_err(display_error)?;
        Ok(documents
            .iter()
            .map(memory_id)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .zip(documents.iter().map(Doc::get_score))
            .collect())
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
                ))
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
            self.put(&item_to_record(&item)?)?;
            existing.insert(item.id);
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

/// Opens the durable a3s-vec collection on the first memory operation.
pub struct LazyFileMemoryStore {
    directory: PathBuf,
    store: tokio::sync::OnceCell<Arc<VecMemoryStore>>,
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

    async fn inner(&self) -> Result<&VecMemoryStore> {
        let directory = self.directory.clone();
        self.store
            .get_or_try_init(|| async move { Self::shared(&directory) })
            .await
            .map(Arc::as_ref)
            .map_err(|error| {
                anyhow!(
                    "failed to initialize a3s-vec Memory store at {}: {error:#}",
                    self.directory.display()
                )
            })
    }

    fn shared(directory: &Path) -> Result<Arc<VecMemoryStore>> {
        fs_err_create_dir_all(directory)?;
        let canonical = directory.canonicalize().map_err(|error| {
            anyhow!("canonicalize memory root {}: {error}", directory.display())
        })?;
        let mut stores = SHARED_STORES
            .lock()
            .map_err(|_| anyhow!("shared memory registry is poisoned"))?;
        stores.retain(|_, store| store.strong_count() > 0);
        if let Some(store) = stores.get(&canonical).and_then(Weak::upgrade) {
            return Ok(store);
        }
        // Open and import are synchronous. Publish only after both finish so
        // simultaneous first use cannot open or import the same collection twice.
        // No synchronous registry guard is held across an await.
        let store = Arc::new(Self::open_and_import(&canonical)?);
        stores.insert(canonical, Arc::downgrade(&store));
        Ok(store)
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
                        "imported legacy JSON memories into a3s-vec"
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

impl Drop for LazyFileMemoryStore {
    fn drop(&mut self) {
        let Some(store) = self.store.take() else {
            return;
        };
        // All owned shared handles stay in lazy cells. Serialize the last
        // release with upgrade/open, including the engine and lease destructor;
        // a dead Weak must never race an otherwise still-open collection.
        let _registry = SHARED_STORES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(store);
    }
}

#[async_trait::async_trait]
impl MemoryStore for LazyFileMemoryStore {
    async fn store(&self, item: MemoryItem) -> Result<()> {
        MemoryStore::store(self.inner().await?, item).await
    }

    async fn store_and_return(&self, item: MemoryItem) -> Result<MemoryItem> {
        MemoryStore::store_and_return(self.inner().await?, item).await
    }

    async fn retrieve(&self, id: &str) -> Result<Option<MemoryItem>> {
        MemoryStore::retrieve(self.inner().await?, id).await
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>> {
        MemoryStore::search(self.inner().await?, query, limit).await
    }

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<MemoryItem>> {
        MemoryStore::search_by_tags(self.inner().await?, tags, limit).await
    }

    async fn get_recent(&self, limit: usize) -> Result<Vec<MemoryItem>> {
        MemoryStore::get_recent(self.inner().await?, limit).await
    }

    async fn get_important(&self, threshold: f32, limit: usize) -> Result<Vec<MemoryItem>> {
        MemoryStore::get_important(self.inner().await?, threshold, limit).await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        MemoryStore::delete(self.inner().await?, id).await
    }

    async fn clear(&self) -> Result<()> {
        MemoryStore::clear(self.inner().await?).await
    }

    async fn count(&self) -> Result<usize> {
        MemoryStore::count(self.inner().await?).await
    }

    async fn prune(&self, policy: &a3s_memory::PrunePolicy) -> Result<usize> {
        MemoryStore::prune(self.inner().await?, policy).await
    }
}

fn fs_err_create_dir_all(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)
        .map_err(|error| anyhow!("create memory root {}: {error}", root.display()))?;
    crate::secure_state::restrict_state_dir(root)
        .map_err(|error| anyhow!("restrict memory root {}: {error}", root.display()))
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

/// Metadata that rides with a record but does not need to be queryable.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct RecordMeta {
    #[serde(default)]
    metadata: std::collections::HashMap<String, String>,
    #[serde(default)]
    access_count: u32,
    #[serde(default)]
    last_accessed: Option<String>,
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
        content,
        timestamp,
        importance,
        tags: tags.split_whitespace().map(str::to_owned).collect(),
        memory_type,
        metadata: meta.metadata,
        access_count: meta.access_count,
        last_accessed: meta.last_accessed.and_then(|raw| {
            DateTime::parse_from_rfc3339(&raw)
                .map(|parsed| parsed.with_timezone(&Utc))
                .ok()
        }),
        content_lower: String::new(),
    })
}

fn item_to_record(item: &MemoryItem) -> Result<MemoryRecord> {
    let meta = RecordMeta {
        metadata: item.metadata.clone(),
        access_count: item.access_count,
        last_accessed: item.last_accessed.map(|stamp| stamp.to_rfc3339()),
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

#[async_trait::async_trait]
impl MemoryStore for VecMemoryStore {
    async fn store(&self, item: MemoryItem) -> Result<()> {
        self.put(&item_to_record(&item)?)
    }

    async fn retrieve(&self, id: &str) -> Result<Option<MemoryItem>> {
        for document in self.collection.iter().map_err(display_error)? {
            let document = document.map_err(display_error)?;
            if memory_id(&document)?.as_str() == id {
                return Ok(Some(doc_to_item(&document)?));
            }
        }
        Ok(None)
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>> {
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

    async fn search_by_tags(&self, tags: &[String], limit: usize) -> Result<Vec<MemoryItem>> {
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

    async fn get_recent(&self, limit: usize) -> Result<Vec<MemoryItem>> {
        let mut items = Vec::new();
        for document in self.collection.iter().map_err(display_error)? {
            items.push(doc_to_item(&document.map_err(display_error)?)?);
        }
        items.sort_by_key(|item| std::cmp::Reverse(item.timestamp));
        items.truncate(limit);
        Ok(items)
    }

    async fn get_important(&self, threshold: f32, limit: usize) -> Result<Vec<MemoryItem>> {
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

    async fn delete(&self, id: &str) -> Result<()> {
        let result = self.collection.delete(&[id]).map_err(display_error)?;
        if result.error_count != 0 {
            return Err(anyhow!("a3s-vec memory delete rejected {id}"));
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut ids = Vec::new();
        for document in self.collection.iter().map_err(display_error)? {
            ids.push(memory_id(&document.map_err(display_error)?)?);
        }
        if ids.is_empty() {
            return Ok(());
        }
        let refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        let result = self.collection.delete(&refs).map_err(display_error)?;
        if result.error_count != 0 {
            return Err(anyhow!(
                "a3s-vec memory clear rejected {} document(s)",
                result.error_count
            ));
        }
        Ok(())
    }

    async fn count(&self) -> Result<usize> {
        self.collection.count().map_err(display_error)
    }
}
