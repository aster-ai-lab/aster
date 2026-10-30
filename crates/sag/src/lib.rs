//! Native SAG over a3s-vec collections with one source-publication fence. The host supplies model-extracted
//! events and entities; indexed association records form query-local hyperedges.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use a3s_vec::{
    Collection, CollectionSchema, DataType, Doc, FieldSchema, Fts, IndexParams, MetricType,
    SearchQuery,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod catalog;
use catalog::SourceRecord;
pub use catalog::{
    CatalogEntry, CatalogPage, CatalogQuery, CatalogStats, FileKind, IndexState, Order, Sort, View,
};

#[cfg(test)]
mod tests;

pub const MAX_CHUNKS: usize = 256;
pub const MAX_ENTITIES: usize = 32;
pub const MAX_RECORDS: usize = 100_000;
const SOURCE_RECORDS: usize = 1 + MAX_CHUNKS * (2 + 2 * MAX_ENTITIES);
const SEEDS: i32 = 24;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid SAG input: {0}")]
    Invalid(&'static str),
    #[error("The source changed; review it again")]
    Conflict,
    #[error("SAG index model or schema differs; use a separate index")]
    Identity,
    #[error("SAG index exceeds its declared bound")]
    Capacity,
    #[error(transparent)]
    Vector(#[from] a3s_vec::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

macro_rules! wire_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $wire)] $variant),+ }
        impl $name {
            pub fn parse(value: &str) -> Result<Self> {
                match value { $($wire => Ok(Self::$variant),)+ _ => Err(Error::Invalid(concat!(stringify!($name), " refuses an unknown value"))) }
            }
            pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $wire),+ } }
        }
    };
}
pub(crate) use wire_enum;
wire_enum!(SearchMode { Fast => "fast", Precise => "precise" });
wire_enum!(SourceArea { Personal => "personal", Workspace => "workspace" });

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub area: SourceArea,
    pub path: String,
    pub name: String,
    pub parent: String,
    pub kind: FileKind,
    pub size_bytes: u64,
    pub modified_ms: u64,
    pub revision: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub event: String,
    pub entities: Vec<String>,
}
impl Event {
    pub fn validate(&self, body: &str) -> Result<()> {
        if self.event.trim().is_empty() || self.event.len() > 4096 || self.event.contains('\0') {
            return Err(Error::Invalid("event must contain 1-4096 bytes"));
        }
        if self.entities.len() > MAX_ENTITIES {
            return Err(Error::Invalid("too many entities"));
        }
        let mut seen = BTreeSet::new();
        for name in &self.entities {
            if name.trim().is_empty()
                || name.len() > 256
                || !body.contains(name.as_str())
                || !seen.insert(name)
            {
                return Err(Error::Invalid(
                    "entities must be distinct exact source spans",
                ));
            }
        }
        Ok(())
    }
}
pub struct Entity {
    pub name: String,
    pub embedding: Vec<f32>,
}
pub struct Chunk {
    pub heading: String,
    pub body: String,
    pub event: String,
    pub embedding: Vec<f32>,
    pub event_embedding: Vec<f32>,
    pub entities: Vec<Entity>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchHit {
    pub id: String,
    pub path: String,
    pub name: String,
    pub generation: String,
    pub ordinal: usize,
    pub heading: String,
    pub body: String,
    pub entities: Vec<String>,
    pub score: f64,
    pub related: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchResult {
    pub mode: SearchMode,
    pub hits: Vec<SearchHit>,
    pub candidates: usize,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    generation: String,
    ordinal: usize,
    heading: String,
    body: String,
    entities: Vec<String>,
}

pub struct Index {
    collection: Collection,
    chunks: Collection,
    entities: Collection,
    sources: BTreeMap<String, SourceRecord>,
    dimension: usize,
}

impl Index {
    pub fn open(path: &Path, model: &str, dimension: usize) -> Result<Self> {
        if model.is_empty() || dimension == 0 || dimension > 8192 {
            return Err(Error::Invalid("embedding identity or dimension"));
        }
        std::fs::create_dir_all(path)?;
        let collection = open_collection(&path.join("records"), &schema(dimension, &[])?)?;
        let chunks = open_collection(
            &path.join("events"),
            &schema(dimension, &["embedding", "event_embedding"])?,
        )?;
        let entities =
            open_collection(&path.join("entities"), &schema(dimension, &["embedding"])?)?;
        let identity = format!("sag-vec-v2:{model}:{dimension}");
        let saved = collection.fetch(&["sag:meta"])?;
        if let Some(meta) = saved.first() {
            if meta.get_string("payload")?.as_deref() != Some(identity.as_str()) {
                return Err(Error::Identity);
            }
        } else {
            let mut meta = node("sag:meta", "meta", "", false)?;
            meta.add_string("payload", &identity)?;
            write(&collection, &[meta])?;
        }
        let rows = collection.query_by_filter("record_type='source'", 20_001, None)?;
        if rows.len() > 20_000 {
            return Err(Error::Capacity);
        }
        let mut sources = BTreeMap::new();
        for row in rows {
            let source: SourceRecord = serde_json::from_str(&required(&row, "payload")?)?;
            if sources
                .insert(source.entry.source.path.clone(), source)
                .is_some()
            {
                return Err(Error::Invalid("duplicate source identity"));
            }
        }
        Ok(Self {
            collection,
            chunks,
            entities,
            sources,
            dimension,
        })
    }

    pub fn current(&self, path: &str, revision: &str, extractor: &str) -> Result<bool> {
        Ok(self.sources.get(path).is_some_and(|s| {
            s.entry.source.revision == revision
                && s.entry.state == IndexState::Ready
                && s.extractor.as_deref() == Some(extractor)
        }))
    }

    pub fn replace(
        &mut self,
        path: &str,
        revision: &str,
        generation: &str,
        extractor: &str,
        chunks: &[Chunk],
    ) -> Result<()> {
        if chunks.is_empty()
            || chunks.len() > MAX_CHUNKS
            || generation.len() != 64
            || extractor.is_empty()
        {
            return Err(Error::Invalid(
                "source generation, extractor or chunk count",
            ));
        }
        let source = self.sources.get(path).ok_or(Error::Conflict)?;
        if source.entry.source.revision != revision {
            return Err(Error::Conflict);
        }
        let mut total = 0;
        for chunk in chunks {
            total += chunk.body.len();
            if chunk.body.trim().is_empty()
                || chunk.body.len() > 16 * 1024
                || chunk.heading.len() > 1024
                || total > 256 * 1024
            {
                return Err(Error::Invalid("chunk text exceeds its bound"));
            }
            Event {
                event: chunk.event.clone(),
                entities: chunk.entities.iter().map(|e| e.name.clone()).collect(),
            }
            .validate(&chunk.body)?;
            self.validate_vector(&chunk.embedding)?;
            self.validate_vector(&chunk.event_embedding)?;
            for entity in &chunk.entities {
                self.validate_vector(&entity.embedding)?;
            }
        }
        let old = self.source_nodes(path)?;
        let publication = publication_key(path, generation, extractor);
        let mut rows = BTreeMap::new();
        let mut counts = BTreeMap::<String, usize>::new();
        for (ordinal, chunk) in chunks.iter().enumerate() {
            let id = digest(&format!("chunk\0{publication}\0{ordinal}"));
            let mut event = node(&id, "chunk", path, true)?;
            event.add_string("publication_key", &publication)?;
            event.add_string("generation", generation)?;
            event.add_string("extractor", extractor)?;
            event.add_string("text", &format!("{}\n{}", chunk.body, chunk.event))?;
            event.add_string(
                "payload",
                &serde_json::to_string(&Evidence {
                    generation: generation.into(),
                    ordinal,
                    heading: chunk.heading.clone(),
                    body: chunk.body.clone(),
                    entities: chunk.entities.iter().map(|e| e.name.clone()).collect(),
                })?,
            )?;
            event.add_vector_f32("embedding", &chunk.embedding)?;
            event.add_vector_f32("event_embedding", &chunk.event_embedding)?;
            rows.insert(id.clone(), event);
            for entity in &chunk.entities {
                let key = digest(&entity.name);
                let entity_id = digest(&format!("entity\0{publication}\0{key}"));
                if !rows.contains_key(&entity_id) {
                    let mut row = node(&entity_id, "entity", path, true)?;
                    row.add_string("publication_key", &publication)?;
                    row.add_string("generation", generation)?;
                    row.add_string("extractor", extractor)?;
                    row.add_string("text", &entity.name)?;
                    row.add_string("entity_key", &key)?;
                    row.add_vector_f32("embedding", &entity.embedding)?;
                    rows.insert(entity_id, row);
                }
                let edge_id = digest(&format!("association\0{id}\0{key}"));
                let mut edge = node(&edge_id, "association", path, true)?;
                edge.add_string("publication_key", &publication)?;
                edge.add_string("generation", generation)?;
                edge.add_string("extractor", extractor)?;
                edge.add_string("entity_key", &key)?;
                edge.add_string("event_id", &id)?;
                rows.insert(edge_id, edge);
                *counts.entry(entity.name.clone()).or_default() += 1;
            }
        }
        let mut next = source.clone();
        next.entry.state = IndexState::Ready;
        next.entry.error = None;
        next.generation = Some(generation.into());
        next.extractor = Some(extractor.into());
        next.chunks = chunks.len();
        next.entity_counts = counts;
        let source_doc = next.document()?;
        if (self.collection.count()? + self.chunks.count()? + self.entities.count()?)
            .saturating_sub(old.len())
            + rows.len()
            + 1
            > MAX_RECORDS
        {
            return Err(Error::Capacity);
        }
        let retired = old
            .into_iter()
            .filter(|row| {
                row.get_pk()
                    .is_some_and(|id| !rows.contains_key(id) && Some(id) != source_doc.get_pk())
            })
            .collect::<Vec<_>>();
        // Vector and association rows are staged first; the source pointer publishes their generation last.
        let rows = rows.into_values().collect::<Vec<_>>();
        for collection in [&self.chunks, &self.entities, &self.collection] {
            let docs = rows
                .iter()
                .filter(|row| std::ptr::eq(self.owner(row), collection))
                .cloned()
                .collect::<Vec<_>>();
            if !docs.is_empty() {
                write(collection, &docs)?;
            }
        }
        write(&self.collection, &[source_doc])?;
        self.sources.insert(path.into(), next);
        self.delete_nodes(&retired)?;
        Ok(())
    }

    pub fn fail(&mut self, path: &str, revision: &str, reason: &str) -> Result<()> {
        if let Some(source) = self.sources.get(path) {
            if source.entry.source.revision == revision {
                let mut next = source.clone();
                next.entry.state = IndexState::Failed;
                next.entry.error = Some(reason.into());
                self.retire(path)?;
                write(&self.collection, &[next.document()?])?;
                self.sources.insert(path.into(), next);
            }
        }
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        embedding: &[f32],
        mode: SearchMode,
        limit: usize,
        paths: &[String],
    ) -> Result<SearchResult> {
        let started = Instant::now();
        if query.trim().is_empty()
            || query.len() > 1024
            || !(1..=50).contains(&limit)
            || paths.len() > 100
        {
            return Err(Error::Invalid("query, result limit or source scope"));
        }
        self.validate_vector(embedding)?;
        let allowed = self
            .sources
            .iter()
            .filter(|(path, source)| {
                source.entry.state == IndexState::Ready
                    && (paths.is_empty() || paths.contains(path))
            })
            .filter_map(|(path, source)| {
                Some(publication_key(
                    path,
                    source.generation.as_deref()?,
                    source.extractor.as_deref()?,
                ))
            })
            .collect::<Vec<_>>();
        if allowed.is_empty() {
            return Ok(SearchResult {
                mode,
                hits: Vec::new(),
                candidates: 0,
                elapsed_ms: started.elapsed().as_millis() as u64,
            });
        }
        let scope = format!(
            " AND publication_key IN [{}]",
            allowed
                .iter()
                .map(|key| format!("'{key}'"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let chunk_filter = format!("record_type='chunk' AND active=true{scope}");
        let mut scores = BTreeMap::<String, (f64, bool)>::new();
        for field in ["embedding", "event_embedding"] {
            let mut request = SearchQuery::new(field, embedding, SEEDS)?;
            request.set_filter(&chunk_filter)?;
            request.set_radius(0.2)?;
            request.set_output_fields(&[])?;
            for (rank, row) in self.chunks.query(&request)?.iter().enumerate() {
                scores.entry(pk(row)?).or_default().0 += 1.0 / (60.0 + rank as f64);
            }
        }
        for (rank, row) in self
            .lexical(&self.chunks, query, &chunk_filter, SEEDS)?
            .iter()
            .enumerate()
        {
            scores.entry(pk(row)?).or_default().0 += 1.0 / (60.0 + rank as f64);
        }
        let seeds: BTreeSet<_> = scores.keys().cloned().collect();
        if mode == SearchMode::Precise {
            let mut entity_keys = BTreeSet::new();
            let seed_docs = self
                .chunks
                .fetch(&seeds.iter().map(String::as_str).collect::<Vec<_>>())?;
            for doc in seed_docs {
                let evidence: Evidence = serde_json::from_str(&required(&doc, "payload")?)?;
                entity_keys.extend(evidence.entities.iter().map(|name| digest(name)));
            }
            let entity_filter = format!("record_type='entity' AND active=true{scope}");
            let mut request = SearchQuery::new("embedding", embedding, 16)?;
            request.set_filter(&entity_filter)?;
            request.set_radius(0.35)?;
            request.set_output_fields(&["entity_key"])?;
            for doc in self
                .entities
                .query(&request)?
                .into_iter()
                .chain(self.lexical(&self.entities, query, &entity_filter, 16)?)
            {
                entity_keys.insert(required(&doc, "entity_key")?);
            }
            if !entity_keys.is_empty() {
                let keys = entity_keys
                    .iter()
                    .map(|key| format!("'{key}'"))
                    .collect::<Vec<_>>()
                    .join(",");
                let filter = format!(
                    "record_type='association' AND active=true AND entity_key IN [{keys}]{scope}"
                );
                let edges = self.collection.query_by_filter(
                    &filter,
                    4096,
                    Some(&["entity_key", "event_id"]),
                )?;
                let mut degrees = BTreeMap::<String, usize>::new();
                for (path, source) in &self.sources {
                    if source.entry.state != IndexState::Ready
                        || (!paths.is_empty() && !paths.contains(path))
                    {
                        continue;
                    }
                    for (name, count) in &source.entity_counts {
                        *degrees.entry(digest(name)).or_default() += count;
                    }
                }
                let mut related = BTreeMap::<String, f64>::new();
                for edge in edges {
                    let key = required(&edge, "entity_key")?;
                    let degree = degrees
                        .get(&key)
                        .copied()
                        .ok_or(Error::Invalid("association degree is missing"))?;
                    *related.entry(required(&edge, "event_id")?).or_default() +=
                        1.0 / degree.max(1) as f64;
                }
                let mut related: Vec<_> = related.into_iter().collect();
                related.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                related.truncate(128);
                for (rank, (id, weight)) in related.into_iter().enumerate() {
                    let score = scores.entry(id.clone()).or_default();
                    score.0 += weight.min(2.0) / (60.0 + rank as f64);
                    score.1 = !seeds.contains(&id);
                }
            }
        }
        let candidates = scores.len();
        let mut ranked: Vec<_> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0).then_with(|| a.0.cmp(&b.0)));
        ranked.truncate(limit);
        let docs = self
            .chunks
            .fetch(&ranked.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>())?;
        let mut by_id: BTreeMap<_, _> = docs
            .into_iter()
            .map(|doc| Ok((pk(&doc)?, doc)))
            .collect::<Result<_>>()?;
        let mut hits = Vec::new();
        for (id, (score, related)) in ranked {
            let doc = by_id.remove(&id).ok_or(Error::Conflict)?;
            let path = required(&doc, "path")?;
            let source = self.sources.get(&path).ok_or(Error::Conflict)?;
            let evidence: Evidence = serde_json::from_str(&required(&doc, "payload")?)?;
            if source.entry.state != IndexState::Ready
                || source.generation.as_deref() != Some(&evidence.generation)
            {
                return Err(Error::Conflict);
            }
            hits.push(SearchHit {
                id,
                path,
                name: source.entry.source.name.clone(),
                generation: evidence.generation,
                ordinal: evidence.ordinal,
                heading: evidence.heading,
                body: evidence.body,
                entities: evidence.entities,
                score,
                related,
            });
        }
        Ok(SearchResult {
            mode,
            hits,
            candidates,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn lexical(
        &self,
        collection: &Collection,
        query: &str,
        filter: &str,
        limit: i32,
    ) -> Result<Vec<Doc>> {
        let mut fts = Fts::new()?;
        let literal = query
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace(['\n', '\r'], " ");
        fts.set_query_string(&format!("\"{literal}\""))?;
        let mut request = SearchQuery::fts("text", &fts, limit)?;
        request.set_filter(filter)?;
        request.set_output_fields(&["entity_key"])?;
        Ok(collection.query(&request)?)
    }

    fn source_nodes(&self, path: &str) -> Result<Vec<Doc>> {
        let mut rows = Vec::new();
        for collection in [&self.collection, &self.chunks, &self.entities] {
            rows.extend(collection.query_by_filter(
                &format!("path_key='{}'", digest(path)),
                SOURCE_RECORDS + 1,
                None,
            )?);
        }
        if rows.len() > SOURCE_RECORDS {
            return Err(Error::Capacity);
        }
        Ok(rows)
    }
    fn owner(&self, doc: &Doc) -> &Collection {
        match doc.field("record_type") {
            Some(a3s_vec::FieldValue::String(kind)) if kind == "chunk" => &self.chunks,
            Some(a3s_vec::FieldValue::String(kind)) if kind == "entity" => &self.entities,
            _ => &self.collection,
        }
    }
    fn delete_nodes(&self, docs: &[Doc]) -> Result<()> {
        for collection in [&self.chunks, &self.entities, &self.collection] {
            let ids = docs
                .iter()
                .filter(|doc| std::ptr::eq(self.owner(doc), collection))
                .map(pk)
                .collect::<Result<Vec<_>>>()?;
            if !ids.is_empty() {
                let result =
                    collection.delete(&ids.iter().map(String::as_str).collect::<Vec<_>>())?;
                if result.error_count > 0 {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(())
    }
    fn update_nodes(&self, docs: &[Doc]) -> Result<()> {
        for collection in [&self.chunks, &self.entities, &self.collection] {
            let rows = docs
                .iter()
                .filter(|doc| std::ptr::eq(self.owner(doc), collection))
                .collect::<Vec<_>>();
            if !rows.is_empty() {
                let result = collection.update(&rows)?;
                if result.error_count > 0 {
                    return Err(Error::Conflict);
                }
            }
        }
        Ok(())
    }
    fn retire(&self, path: &str) -> Result<()> {
        let mut patches = Vec::new();
        for doc in self.source_nodes(path)? {
            let kind = required(&doc, "record_type")?;
            if kind == "source" {
                continue;
            }
            let mut patch = Doc::with_pk(&pk(&doc)?)?;
            patch.add_string("record_type", &kind)?;
            patch.add_bool("active", false)?;
            patches.push(patch);
        }
        self.update_nodes(&patches)
    }

    fn validate_vector(&self, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dimension
            || vector.iter().any(|v| !v.is_finite())
            || vector.iter().map(|v| v * v).sum::<f32>() < 1e-12
        {
            return Err(Error::Invalid(
                "embedding dimension, finite values or zero norm",
            ));
        }
        Ok(())
    }
}

fn schema(dimension: usize, vector_fields: &[&str]) -> Result<CollectionSchema> {
    let mut schema = CollectionSchema::new("aster_sag_v1")?;
    for name in [
        "record_type",
        "path_key",
        "entity_key",
        "event_id",
        "publication_key",
    ] {
        let mut field = FieldSchema::new(name, DataType::String, name != "record_type", 0)?;
        field.set_index_params(&IndexParams::invert(false, false)?)?;
        schema.add_field(&field)?;
    }
    let mut active = FieldSchema::new("active", DataType::Bool, false, 0)?;
    active.set_index_params(&IndexParams::invert(false, false)?)?;
    schema.add_field(&active)?;
    for name in ["path", "payload", "generation", "extractor"] {
        schema.add_field(&FieldSchema::new(name, DataType::String, true, 0)?)?;
    }
    let mut text = FieldSchema::new("text", DataType::String, true, 0)?;
    text.set_index_params(&IndexParams::fts(
        Some("ngram"),
        Some(&["lowercase"]),
        Some(r#"{"ngram_min":2,"ngram_max":3}"#),
    )?)?;
    schema.add_field(&text)?;
    for name in vector_fields {
        let mut vector = FieldSchema::new(name, DataType::VectorFp32, true, dimension as u32)?;
        vector.set_index_params(&IndexParams::hnsw(MetricType::Cosine, 16, 64)?)?;
        schema.add_field(&vector)?;
    }
    Ok(schema)
}
fn node(id: &str, kind: &str, path: &str, active: bool) -> Result<Doc> {
    let mut doc = Doc::with_pk(id)?;
    doc.add_string("record_type", kind)?;
    doc.add_string("path", path)?;
    doc.add_string("path_key", &digest(path))?;
    doc.add_bool("active", active)?;
    Ok(doc)
}
fn write(collection: &Collection, docs: &[Doc]) -> Result<()> {
    let result = collection.upsert(&docs.iter().collect::<Vec<_>>())?;
    if result.error_count > 0 || result.success_count != docs.len() as u64 {
        return Err(Error::Invalid("owner rejected an index document"));
    }
    Ok(())
}
fn required(doc: &Doc, name: &str) -> Result<String> {
    doc.get_string(name)?
        .ok_or(Error::Invalid("stored index field is missing"))
}
fn pk(doc: &Doc) -> Result<String> {
    doc.get_pk()
        .map(str::to_owned)
        .ok_or(Error::Invalid("stored document identity is missing"))
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn publication_key(path: &str, generation: &str, extractor: &str) -> String {
    digest(&format!("{path}\0{generation}\0{extractor}"))
}
fn open_collection(path: &Path, schema: &CollectionSchema) -> Result<Collection> {
    let name = path
        .to_str()
        .ok_or(Error::Invalid("index path is not UTF-8"))?;
    Ok(if path.exists() {
        Collection::open(name, None)?
    } else {
        Collection::create_and_open(name, schema, None)?
    })
}
