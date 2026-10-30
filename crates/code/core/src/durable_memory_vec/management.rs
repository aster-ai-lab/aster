//! Bounded inspection and conditional deletion through the live memory owner.

use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const PAGE_SIZE: usize = 50;
const MAX_RECORDS: usize = 10_000;
const MAX_RECORD_BYTES: usize = 64 * 1024;
const MAX_SCAN_BYTES: usize = 16 * 1024 * 1024;

macro_rules! closed_set {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $wire)] $variant),+ }
        impl $name {
            pub fn parse(value: &str) -> Option<Self> { match value { $($wire => Some(Self::$variant),)+ _ => None } }
            pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $wire),+ } }
        }
    };
}
closed_set!(StoredMemoryType { Episodic => "episodic", Semantic => "semantic", Procedural => "procedural", Working => "working" });
closed_set!(MemoryInspectionLayer { Working => "working", Session => "session", Durable => "durable" });
closed_set!(MemoryManagementCode { Invalid => "invalid", Missing => "missing", Busy => "busy", Conflict => "conflict", Limit => "limit", Failed => "failed" });

impl Default for MemoryInspectionLayer {
    fn default() -> Self {
        Self::Durable
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryManagementFailure {
    pub code: MemoryManagementCode,
    pub message: String,
}
impl MemoryManagementFailure {
    pub fn new(code: MemoryManagementCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    fn failed(error: impl std::fmt::Display) -> Self {
        Self::new(MemoryManagementCode::Failed, error.to_string())
    }
}
impl std::fmt::Display for MemoryManagementFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}
impl std::error::Error for MemoryManagementFailure {}
type ManagementResult<T> = std::result::Result<T, MemoryManagementFailure>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryManagementRequest {
    Search {
        query: String,
        limit: usize,
    },
    List {
        query: String,
        memory_type: Option<StoredMemoryType>,
        cursor: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        created_after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        created_before: Option<String>,
    },
    Read {
        id: String,
    },
    Delete {
        id: String,
        revision: String,
    },
    Update {
        id: String,
        revision: String,
        content: String,
    },
}
impl MemoryManagementRequest {
    pub fn parse(raw: &str) -> ManagementResult<Self> {
        let request: Self = serde_json::from_str(raw).map_err(|_| {
            MemoryManagementFailure::new(
                MemoryManagementCode::Invalid,
                "memory request refuses unknown fields, operations or types",
            )
        })?;
        request.validate()?;
        Ok(request)
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Search { .. } => "search",
            Self::List { .. } => "list",
            Self::Read { .. } => "read",
            Self::Delete { .. } => "delete",
            Self::Update { .. } => "update",
        }
    }
    pub fn validate(&self) -> ManagementResult<()> {
        let invalid = || {
            MemoryManagementFailure::new(
                MemoryManagementCode::Invalid,
                "memory request exceeds its bounds or has an invalid revision",
            )
        };
        match self {
            Self::Search { query, limit }
                if query.trim().is_empty()
                    || query.contains('\0')
                    || query.chars().count() > 2000
                    || !(1..=12).contains(limit) =>
            {
                Err(invalid())
            }
            Self::List { query, cursor, .. }
                if query.len() > 1024 || cursor.as_ref().is_some_and(|value| value.len() > 512) =>
            {
                Err(invalid())
            }
            Self::List {
                created_after,
                created_before,
                ..
            } => {
                memory_time_bounds(created_after.as_deref(), created_before.as_deref())?;
                Ok(())
            }
            Self::Read { id } | Self::Delete { id, .. } | Self::Update { id, .. }
                if id.is_empty() || id.len() > 256 || id.contains('\0') =>
            {
                Err(invalid())
            }
            Self::Delete { revision, .. } | Self::Update { revision, .. }
                if revision.len() != 64
                    || !revision
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) =>
            {
                Err(invalid())
            }
            Self::Update { content, .. } if content.len() > MAX_RECORD_BYTES => Err(invalid()),
            _ => Ok(()),
        }
    }
}

fn memory_time_bounds(
    after: Option<&str>,
    before: Option<&str>,
) -> ManagementResult<std::ops::RangeInclusive<DateTime<Utc>>> {
    let parse = |raw: Option<&str>, default| match raw {
        None => Ok(default),
        Some(raw) if raw.len() <= 64 => DateTime::parse_from_rfc3339(raw)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_| {
                MemoryManagementFailure::new(
                    MemoryManagementCode::Invalid,
                    "memory time bound must be RFC3339",
                )
            }),
        _ => Err(MemoryManagementFailure::new(
            MemoryManagementCode::Invalid,
            "memory time bound exceeds its limit",
        )),
    };
    let start = parse(after, DateTime::<Utc>::MIN_UTC)?;
    let end = parse(before, DateTime::<Utc>::MAX_UTC)?;
    if start > end {
        return Err(MemoryManagementFailure::new(
            MemoryManagementCode::Invalid,
            "memory time range is reversed",
        ));
    }
    Ok(start..=end)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMemory {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub importance: f32,
    pub memory_type: StoredMemoryType,
    pub timestamp: String,
    pub metadata: BTreeMap<String, String>,
    pub access_count: u32,
    pub last_accessed: Option<String>,
    pub revision: String,
}

impl ManagedMemory {
    /// Preserve the live session owner and layer even when a durable copy has the same item ID.
    pub fn from_session_item(
        workspace: &Path,
        session_id: &str,
        layer: MemoryInspectionLayer,
        item: MemoryItem,
    ) -> ManagementResult<Self> {
        if layer == MemoryInspectionLayer::Durable
            || session_id.is_empty()
            || session_id.len() > 256
            || item.id.is_empty()
            || item.id.len() > 256
        {
            return Err(MemoryManagementFailure::new(
                MemoryManagementCode::Invalid,
                "live memory requires a session and a transient layer",
            ));
        }
        let mut metadata: BTreeMap<_, _> = item.metadata.into_iter().collect();
        metadata.insert("a3s.session_id".into(), session_id.into());
        metadata.insert("a3s.memory_id".into(), item.id.clone());
        let memory_type = match item.memory_type {
            MemoryType::Working => StoredMemoryType::Working,
            MemoryType::Episodic => StoredMemoryType::Episodic,
            MemoryType::Semantic => StoredMemoryType::Semantic,
            MemoryType::Procedural => StoredMemoryType::Procedural,
        };
        let mut managed = Self {
            id: digest(
                &serde_json::to_vec(&(workspace, session_id, layer, &item.id))
                    .map_err(MemoryManagementFailure::failed)?,
            ),
            content: item.content,
            tags: item.tags,
            importance: item.importance,
            memory_type,
            timestamp: item.timestamp.to_rfc3339(),
            metadata,
            access_count: item.access_count,
            last_accessed: item.last_accessed.map(|value| value.to_rfc3339()),
            revision: String::new(),
        };
        let bytes = serde_json::to_vec(&managed).map_err(MemoryManagementFailure::failed)?;
        if bytes.len() + 64 > MAX_RECORD_BYTES
            || !managed.importance.is_finite()
            || !(0.0..=1.0).contains(&managed.importance)
        {
            return Err(MemoryManagementFailure::new(
                MemoryManagementCode::Limit,
                "live memory record exceeds inspection bounds",
            ));
        }
        managed.revision = digest(&bytes);
        Ok(managed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryManagementResponse {
    Search {
        items: Vec<ManagedMemory>,
    },
    List {
        items: Vec<ManagedMemory>,
        total: usize,
        next_cursor: Option<String>,
    },
    Read {
        item: ManagedMemory,
    },
    Delete {
        id: String,
    },
    Update {
        item: ManagedMemory,
    },
}
impl MemoryManagementResponse {
    pub fn parse(raw: &str) -> ManagementResult<Self> {
        serde_json::from_str(raw).map_err(MemoryManagementFailure::failed)
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Search { .. } => "search",
            Self::List { .. } => "list",
            Self::Read { .. } => "read",
            Self::Delete { .. } => "delete",
            Self::Update { .. } => "update",
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    generation: u64,
    filter: String,
    offset: usize,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct InspectedMeta {
    metadata: BTreeMap<String, String>,
    access_count: u32,
    last_accessed: Option<String>,
}

fn digest(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn inspect(document: &Doc, path: &Path) -> ManagementResult<ManagedMemory> {
    let decode = |error: anyhow::Error| MemoryManagementFailure::failed(error);
    let string = |field| {
        document
            .get_string(field)
            .map_err(MemoryManagementFailure::failed)?
            .ok_or_else(|| {
                MemoryManagementFailure::failed(format!("memory record omitted {field}"))
            })
    };
    let content = string("content")?;
    let tags = string("tags")?;
    let timestamp = string("timestamp")?;
    let memory_type = string("memory_type")?;
    let meta_raw = string("meta")?;
    let id = memory_id(document).map_err(decode)?;
    let importance = document
        .get_f32("importance")
        .map_err(MemoryManagementFailure::failed)?
        .ok_or_else(|| MemoryManagementFailure::failed("memory record omitted importance"))?;
    if id.is_empty()
        || id.len() > 256
        || !importance.is_finite()
        || !(0.0..=1.0).contains(&importance)
        || content.len() + tags.len() + meta_raw.len() > MAX_RECORD_BYTES
    {
        return Err(MemoryManagementFailure::new(
            MemoryManagementCode::Limit,
            "memory record exceeds inspection limits",
        ));
    }
    DateTime::parse_from_rfc3339(&timestamp).map_err(MemoryManagementFailure::failed)?;
    let kind: String =
        serde_json::from_str(&memory_type).map_err(MemoryManagementFailure::failed)?;
    let kind = StoredMemoryType::parse(&kind).ok_or_else(|| {
        MemoryManagementFailure::failed("memory record has an unknown stored type")
    })?;
    let meta: InspectedMeta =
        serde_json::from_str(&meta_raw).map_err(MemoryManagementFailure::failed)?;
    if let Some(stamp) = &meta.last_accessed {
        DateTime::parse_from_rfc3339(stamp).map_err(MemoryManagementFailure::failed)?;
    }
    let revision = digest(
        &serde_json::to_vec(&(
            path.to_string_lossy(),
            &id,
            &content,
            &tags,
            importance.to_bits(),
            &memory_type,
            &timestamp,
            &meta_raw,
        ))
        .map_err(MemoryManagementFailure::failed)?,
    );
    let item = ManagedMemory {
        id,
        content,
        tags: tags.split_whitespace().map(str::to_owned).collect(),
        importance,
        memory_type: kind,
        timestamp,
        metadata: meta.metadata,
        access_count: meta.access_count,
        last_accessed: meta.last_accessed,
        revision,
    };
    if serde_json::to_vec(&item)
        .map_err(MemoryManagementFailure::failed)?
        .len()
        > MAX_RECORD_BYTES
    {
        return Err(MemoryManagementFailure::new(
            MemoryManagementCode::Limit,
            "memory detail exceeds inspection limits",
        ));
    }
    Ok(item)
}

impl VecMemoryStore {
    pub fn manage(
        &self,
        request: MemoryManagementRequest,
    ) -> ManagementResult<MemoryManagementResponse> {
        request.validate()?;
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| MemoryManagementFailure::failed("memory mutation lock is poisoned"))?;
        let stats = self
            .collection
            .stats()
            .map_err(MemoryManagementFailure::failed)?;
        if !matches!(&request, MemoryManagementRequest::Search { .. })
            && (stats.accounted_document_bytes > MAX_SCAN_BYTES as u64
                || stats.doc_count > MAX_RECORDS as u64)
        {
            return Err(MemoryManagementFailure::new(
                MemoryManagementCode::Limit,
                "memory catalog exceeds the inspection budget",
            ));
        }
        match request {
            MemoryManagementRequest::Search { query, limit } => {
                let hits = self
                    .recall(query.trim(), limit)
                    .map_err(MemoryManagementFailure::failed)?;
                // Existing whitespace collections need indexed substring recall for unsegmented text.
                let expression = query
                    .split_whitespace()
                    .map(|term| {
                        let escaped: String = term.chars().flat_map(|ch| ['\\', ch]).collect();
                        format!("*{escaped}*")
                    })
                    .collect::<Vec<_>>()
                    .join(" AND ");
                let mut fts = a3s_vec::Fts::new().map_err(MemoryManagementFailure::failed)?;
                fts.set_query_string(&expression)
                    .map_err(MemoryManagementFailure::failed)?;
                let mut substring = SearchQuery::fts(CONTENT_FIELD, &fts, limit as i32)
                    .map_err(MemoryManagementFailure::failed)?;
                substring
                    .set_output_fields(&[])
                    .map_err(MemoryManagementFailure::failed)?;
                let matches = self
                    .collection
                    .query(&substring)
                    .map_err(MemoryManagementFailure::failed)?;
                let literal = matches
                    .iter()
                    .map(|doc| memory_id(doc).map(|id| (id, doc.get_score())))
                    .collect::<Result<Vec<_>>>()
                    .map_err(MemoryManagementFailure::failed)?;
                let hits = rrf_merge(hits, literal, limit);
                if hits.is_empty() {
                    return Ok(MemoryManagementResponse::Search { items: Vec::new() });
                }
                let keys: Vec<_> = hits.iter().map(|(id, _)| id.as_str()).collect();
                let documents = self
                    .collection
                    .fetch_with_options(&keys, None, false)
                    .map_err(MemoryManagementFailure::failed)?;
                let mut items = documents
                    .iter()
                    .map(|document| inspect(document, &self.path))
                    .collect::<ManagementResult<Vec<_>>>()?;
                items.sort_by_cached_key(|item| {
                    hits.iter()
                        .position(|(id, _)| id == &item.id)
                        .unwrap_or(usize::MAX)
                });
                Ok(MemoryManagementResponse::Search { items })
            }
            MemoryManagementRequest::List {
                query,
                memory_type,
                cursor,
                created_after,
                created_before,
            } => {
                let time = memory_time_bounds(created_after.as_deref(), created_before.as_deref())?;
                let documents = self
                    .collection
                    .iter_with_options(None, false)
                    .map_err(MemoryManagementFailure::failed)?;
                let generation = documents.revision();
                let query = query.trim().to_lowercase();
                let filter = digest(
                    &serde_json::to_vec(&(
                        self.path.to_string_lossy(),
                        &query,
                        memory_type,
                        &created_after,
                        &created_before,
                    ))
                    .map_err(MemoryManagementFailure::failed)?,
                );
                let offset = if let Some(raw) = cursor {
                    let cursor: Cursor = serde_json::from_str(&raw).map_err(|_| {
                        MemoryManagementFailure::new(
                            MemoryManagementCode::Invalid,
                            "memory cursor is invalid",
                        )
                    })?;
                    if cursor.generation != generation || cursor.filter != filter {
                        return Err(MemoryManagementFailure::new(
                            MemoryManagementCode::Conflict,
                            "memory catalog changed; refresh the list",
                        ));
                    }
                    cursor.offset
                } else {
                    0
                };
                let mut items = Vec::new();
                let mut bytes = 0usize;
                for document in documents {
                    let item = inspect(
                        &document.map_err(MemoryManagementFailure::failed)?,
                        &self.path,
                    )?;
                    bytes += serde_json::to_vec(&item)
                        .map_err(MemoryManagementFailure::failed)?
                        .len();
                    if bytes > MAX_SCAN_BYTES {
                        return Err(MemoryManagementFailure::new(
                            MemoryManagementCode::Limit,
                            "memory catalog exceeds 16 MiB inspection budget",
                        ));
                    }
                    if memory_type.is_none_or(|kind| kind == item.memory_type)
                        && (query.is_empty() || item.content.to_lowercase().contains(&query))
                        && time.contains(
                            &DateTime::parse_from_rfc3339(&item.timestamp)
                                .map_err(MemoryManagementFailure::failed)?
                                .with_timezone(&Utc),
                        )
                    {
                        items.push(item);
                    }
                }
                items.sort_by(|a, b| a.id.cmp(&b.id));
                let total = items.len();
                if offset > total {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Invalid,
                        "memory cursor is outside the catalog",
                    ));
                }
                let mut end = offset;
                let mut page_bytes = 0usize;
                for item in items.iter().skip(offset).take(PAGE_SIZE) {
                    let next_bytes = serde_json::to_vec(item)
                        .map_err(MemoryManagementFailure::failed)?
                        .len();
                    if page_bytes + next_bytes > 256 * 1024 {
                        break;
                    }
                    page_bytes += next_bytes;
                    end += 1;
                }
                let next_cursor = if end < total {
                    Some(
                        serde_json::to_string(&Cursor {
                            generation,
                            filter,
                            offset: end,
                        })
                        .map_err(MemoryManagementFailure::failed)?,
                    )
                } else {
                    None
                };
                let items = items.into_iter().skip(offset).take(end - offset).collect();
                Ok(MemoryManagementResponse::List {
                    items,
                    total,
                    next_cursor,
                })
            }
            MemoryManagementRequest::Read { id } => Ok(MemoryManagementResponse::Read {
                item: self.inspect_id(&id)?,
            }),
            MemoryManagementRequest::Update {
                id,
                revision,
                content,
            } => {
                if self.read_only {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Busy,
                        "memory owner is read-only",
                    ));
                }
                let mut item = self.inspect_id(&id)?;
                if item.revision != revision {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Conflict,
                        "memory record changed; inspect it again before saving",
                    ));
                }
                let old_hash = <[u8; 32]>::from(Sha256::digest(item.content.trim().as_bytes()));
                let growth = content.len().saturating_sub(item.content.len());
                if stats.accounted_document_bytes.saturating_add(growth as u64)
                    > MAX_SCAN_BYTES as u64
                {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Limit,
                        "memory catalog exceeds its byte budget",
                    ));
                }
                item.content = content;
                if serde_json::to_vec(&item)
                    .map_err(MemoryManagementFailure::failed)?
                    .len()
                    > MAX_RECORD_BYTES
                {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Limit,
                        "memory detail exceeds inspection limits",
                    ));
                }
                let new_hash = <[u8; 32]>::from(Sha256::digest(item.content.trim().as_bytes()));
                let embedded = self
                    .semantic
                    .as_ref()
                    .and_then(|provider| provider.embed(&item.content));
                let mut hygiene = self.hygiene.lock().map_err(|_| {
                    MemoryManagementFailure::failed("memory hygiene lock is poisoned")
                })?;
                let mut vectors = self.vectors.lock().map_err(|_| {
                    MemoryManagementFailure::failed("memory vectors lock is poisoned")
                })?;
                if new_hash != old_hash && hygiene.contains(&new_hash) {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Conflict,
                        "another memory has this exact content",
                    ));
                }
                let mut patch = Doc::new().map_err(MemoryManagementFailure::failed)?;
                patch.set_pk(&id);
                patch
                    .add_string(CONTENT_FIELD, &item.content)
                    .map_err(MemoryManagementFailure::failed)?;
                if let Some(dimension) = self.embedding_dimension {
                    patch
                        .add_vector_f32("embedding", &vec![0.0; dimension])
                        .map_err(MemoryManagementFailure::failed)?;
                }
                let result = self
                    .collection
                    .update(&[&patch])
                    .map_err(MemoryManagementFailure::failed)?;
                if result.error_count != 0 || result.success_count != 1 {
                    return Err(MemoryManagementFailure::failed(
                        "memory update was rejected",
                    ));
                }
                hygiene.retain(|known| *known != old_hash && *known != new_hash);
                hygiene.push(new_hash);
                vectors.retain(|(key, _)| key != &id);
                if let Some(vector) = embedded {
                    vectors.push((id.clone(), vector));
                }
                self.collection
                    .flush()
                    .map_err(MemoryManagementFailure::failed)?;
                Ok(MemoryManagementResponse::Update {
                    item: self.inspect_id(&id)?,
                })
            }
            MemoryManagementRequest::Delete { id, revision } => {
                if self.read_only {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Busy,
                        "memory owner is read-only",
                    ));
                }
                if self.inspect_id(&id)?.revision != revision {
                    return Err(MemoryManagementFailure::new(
                        MemoryManagementCode::Conflict,
                        "memory record changed; inspect it again before deleting",
                    ));
                }
                self.delete_record(&id)
                    .map_err(MemoryManagementFailure::failed)?;
                Ok(MemoryManagementResponse::Delete { id })
            }
        }
    }

    fn inspect_id(&self, id: &str) -> ManagementResult<ManagedMemory> {
        let documents = self
            .collection
            .fetch_with_options(&[id], None, false)
            .map_err(MemoryManagementFailure::failed)?;
        let document = documents.first().ok_or_else(|| {
            MemoryManagementFailure::new(
                MemoryManagementCode::Missing,
                "memory record is unavailable",
            )
        })?;
        inspect(document, &self.path)
    }

    pub(super) fn delete_record(&self, id: &str) -> Result<()> {
        let documents = self
            .collection
            .fetch_with_options(&[id], Some(&[CONTENT_FIELD]), false)
            .map_err(display_error)?;
        let content = documents
            .first()
            .map(|document| document.get_string(CONTENT_FIELD))
            .transpose()
            .map_err(display_error)?
            .flatten();
        let mut hygiene = self
            .hygiene
            .lock()
            .map_err(|_| anyhow!("memory hygiene lock is poisoned"))?;
        let mut vectors = self
            .vectors
            .lock()
            .map_err(|_| anyhow!("memory vectors lock is poisoned"))?;
        let result = self.collection.delete(&[id]).map_err(display_error)?;
        if result.error_count != 0 {
            return Err(anyhow!("a3s-vec memory delete rejected {id}"));
        }
        if let Some(content) = content {
            let hash = <[u8; 32]>::from(Sha256::digest(content.trim().as_bytes()));
            hygiene.retain(|known| *known != hash);
        }
        vectors.retain(|(key, _)| key != id);
        self.collection.flush().map_err(display_error)?;
        Ok(())
    }
}

impl LazyFileMemoryStore {
    pub async fn manage(
        &self,
        request: MemoryManagementRequest,
    ) -> ManagementResult<MemoryManagementResponse> {
        request.validate()?;
        self.inner()
            .await
            .map_err(|error| {
                let message = error.to_string();
                let code = if message.contains("collection is locked") {
                    MemoryManagementCode::Busy
                } else {
                    MemoryManagementCode::Failed
                };
                MemoryManagementFailure::new(code, message)
            })?
            .manage(request)
    }
}

#[cfg(test)]
#[path = "management_tests.rs"]
mod tests;
