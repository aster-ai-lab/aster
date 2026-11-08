use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    digest, node, pk, publication_key, wire_enum, write, Error, Index, Result, Source, SourceArea,
};

wire_enum!(FileKind { Folder => "folder", Document => "document", Sheet => "sheet", Slides => "slides", Pdf => "pdf", Image => "image", Other => "other" });
wire_enum!(View { All => "all", Folder => "folder", Recent => "recent", Favorites => "favorites", Artifacts => "artifacts" });
wire_enum!(Sort { Name => "name", Modified => "modified", Visited => "visited" });
wire_enum!(Order { Asc => "asc", Desc => "desc" });
wire_enum!(IndexState { Stored => "stored", Ready => "ready", Failed => "failed" });

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogQuery {
    pub view: View,
    pub folder: String,
    pub search: String,
    pub kind: Option<FileKind>,
    pub sort_by: Sort,
    pub sort_order: Order,
    pub page: usize,
    pub limit: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogEntry {
    #[serde(flatten)]
    pub source: Source,
    pub visited_ms: u64,
    pub favorite: bool,
    pub state: IndexState,
    pub error: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogStats {
    pub files: usize,
    pub indexed: usize,
    pub pending: usize,
    pub failed: usize,
    pub chunks: usize,
    pub entities: usize,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogPage {
    pub entries: Vec<CatalogEntry>,
    pub total: usize,
    pub page: usize,
    pub limit: usize,
    pub folders: Vec<Source>,
    pub folders_capped: bool,
    pub stats: CatalogStats,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceRecord {
    pub entry: CatalogEntry,
    pub generation: Option<String>,
    pub extractor: Option<String>,
    pub chunks: usize,
    pub entity_counts: BTreeMap<String, usize>,
}
impl SourceRecord {
    pub(super) fn document(&self) -> Result<aster_vec::Doc> {
        let mut doc = node(
            &digest(&format!("source\0{}", self.entry.source.path)),
            "source",
            &self.entry.source.path,
            true,
        )?;
        doc.add_string("payload", &serde_json::to_string(self)?)?;
        Ok(doc)
    }
}

impl Index {
    pub fn sync(&mut self, sources: &[Source]) -> Result<()> {
        if sources.len() > 20_000 {
            return Err(Error::Capacity);
        }
        let mut seen = BTreeSet::new();
        for source in sources {
            if source.path.is_empty()
                || source.path.len() > 4096
                || source.name.is_empty()
                || source.revision.is_empty()
                || !seen.insert(source.path.clone())
            {
                return Err(Error::Invalid("catalog source identity"));
            }
        }
        let removed: Vec<_> = self
            .sources
            .keys()
            .filter(|path| !seen.contains(*path))
            .cloned()
            .collect();
        let mut deleted_nodes = Vec::new();
        for path in &removed {
            for doc in self.source_nodes(path)? {
                deleted_nodes.push(doc);
            }
        }
        self.delete_nodes(&deleted_nodes)?;
        for path in removed {
            self.sources.remove(&path);
        }
        let mut changed = Vec::new();
        for source in sources {
            let next = if let Some(old) = self.sources.get(&source.path) {
                if old.entry.source == *source {
                    continue;
                }
                let mut next = old.clone();
                if next.entry.source.revision != source.revision {
                    next.entry.state = IndexState::Stored;
                    next.entry.error = None;
                    next.generation = None;
                    next.extractor = None;
                    next.chunks = 0;
                    next.entity_counts.clear();
                    self.retire(&source.path)?;
                }
                next.entry.source = source.clone();
                next
            } else {
                SourceRecord {
                    entry: CatalogEntry {
                        source: source.clone(),
                        visited_ms: 0,
                        favorite: false,
                        state: IndexState::Stored,
                        error: None,
                    },
                    generation: None,
                    extractor: None,
                    chunks: 0,
                    entity_counts: BTreeMap::new(),
                }
            };
            changed.push(next);
        }
        if !changed.is_empty() {
            let docs = changed
                .iter()
                .map(SourceRecord::document)
                .collect::<Result<Vec<_>>>()?;
            write(&self.collection, &docs)?;
            for record in changed {
                self.sources
                    .insert(record.entry.source.path.clone(), record);
            }
        }
        Ok(())
    }

    pub fn relocate(&mut self, from: &str, destination: &str) -> Result<()> {
        let affected: Vec<_> = self
            .sources
            .keys()
            .filter(|path| path.as_str() == from || path.starts_with(&format!("{from}/")))
            .cloned()
            .collect();
        let mut changes = Vec::new();
        let mut patches = Vec::new();
        let mut old_sources = Vec::new();
        for path in &affected {
            let mut source = self.sources.get(path).ok_or(Error::Conflict)?.clone();
            let next = format!("{destination}{}", &path[from.len()..]);
            for doc in self.source_nodes(path)? {
                if doc.get_string("record_type")?.as_deref() == Some("source") {
                    old_sources.push(pk(&doc)?);
                    continue;
                }
                let mut patch = aster_vec::Doc::with_pk(&pk(&doc)?)?;
                patch.add_string("record_type", &super::required(&doc, "record_type")?)?;
                if let (Some(generation), Some(extractor)) =
                    (doc.get_string("generation")?, doc.get_string("extractor")?)
                {
                    patch.add_string(
                        "publication_key",
                        &publication_key(&next, &generation, &extractor),
                    )?;
                }
                patch.add_string("path", &next)?;
                patch.add_string("path_key", &digest(&next))?;
                patches.push(patch);
            }
            source.entry.source.path = next.clone();
            source.entry.source.name = next
                .rsplit('/')
                .next()
                .ok_or(Error::Invalid("destination name"))?
                .into();
            source.entry.source.parent = next.rsplit_once('/').map(|p| p.0).unwrap_or("").into();
            changes.push(source);
        }
        self.update_nodes(&patches)?;
        write(
            &self.collection,
            &changes
                .iter()
                .map(SourceRecord::document)
                .collect::<Result<Vec<_>>>()?,
        )?;
        if !old_sources.is_empty() {
            let deleted = self
                .collection
                .delete(&old_sources.iter().map(String::as_str).collect::<Vec<_>>())?;
            if deleted.error_count > 0 {
                return Err(Error::Conflict);
            }
        }
        for path in affected {
            self.sources.remove(&path);
        }
        for source in changes {
            self.sources
                .insert(source.entry.source.path.clone(), source);
        }
        Ok(())
    }

    pub fn visit(&mut self, path: &str, now_ms: u64) -> Result<()> {
        let mut source = self
            .sources
            .get(path)
            .filter(|s| s.entry.source.kind != FileKind::Folder)
            .ok_or(Error::Conflict)?
            .clone();
        source.entry.visited_ms = now_ms;
        write(&self.collection, &[source.document()?])?;
        self.sources.insert(path.into(), source);
        Ok(())
    }
    pub fn favorite(&mut self, path: &str, favorite: bool) -> Result<()> {
        let mut source = self.sources.get(path).ok_or(Error::Conflict)?.clone();
        source.entry.favorite = favorite;
        write(&self.collection, &[source.document()?])?;
        self.sources.insert(path.into(), source);
        Ok(())
    }

    pub fn catalog(&self, query: &CatalogQuery) -> Result<CatalogPage> {
        if query.page == 0
            || ![10, 20, 50, 100].contains(&query.limit)
            || query.search.len() > 1024
            || query.folder.len() > 4096
        {
            return Err(Error::Invalid("catalog query or paging"));
        }
        let search = query.search.trim().to_lowercase();
        let mut matching: Vec<_> = self
            .sources
            .values()
            .map(|s| &s.entry)
            .filter(|entry| {
                let source = &entry.source;
                (if query.view == View::Folder {
                    source.parent == query.folder
                } else {
                    source.kind != FileKind::Folder
                }) && (query.view != View::Recent || entry.visited_ms > 0)
                    && (query.view != View::Favorites || entry.favorite)
                    && (query.view != View::All || source.area == SourceArea::Personal)
                    && (query.view != View::Artifacts || source.area == SourceArea::Workspace)
                    && query.kind.is_none_or(|kind| source.kind == kind)
                    && (search.is_empty()
                        || source.name.to_lowercase().contains(&search)
                        || source.path.to_lowercase().contains(&search))
            })
            .collect();
        matching.sort_by(|left, right| {
            let order = match query.sort_by {
                Sort::Name => left
                    .source
                    .name
                    .to_lowercase()
                    .cmp(&right.source.name.to_lowercase()),
                Sort::Modified => left.source.modified_ms.cmp(&right.source.modified_ms),
                Sort::Visited => left.visited_ms.cmp(&right.visited_ms),
            };
            (if query.sort_order == Order::Desc {
                order.reverse()
            } else {
                order
            })
            .then_with(|| left.source.path.cmp(&right.source.path))
        });
        let total = matching.len();
        let page = query.page.min(total.saturating_sub(1) / query.limit + 1);
        let entries = matching
            .into_iter()
            .skip((page - 1) * query.limit)
            .take(query.limit)
            .cloned()
            .collect();
        let mut folders: Vec<_> = self
            .sources
            .values()
            .filter(|s| {
                s.entry.source.kind == FileKind::Folder
                    && s.entry.source.area == SourceArea::Personal
            })
            .map(|s| s.entry.source.clone())
            .take(129)
            .collect();
        let folders_capped = folders.len() > 128;
        folders.truncate(128);
        Ok(CatalogPage {
            entries,
            total,
            page,
            limit: query.limit,
            folders,
            folders_capped,
            stats: self.stats()?,
        })
    }

    pub fn stats(&self) -> Result<CatalogStats> {
        let mut stats = CatalogStats {
            files: 0,
            indexed: 0,
            pending: 0,
            failed: 0,
            chunks: 0,
            entities: 0,
        };
        let mut entities = BTreeSet::new();
        for source in self
            .sources
            .values()
            .filter(|s| s.entry.source.kind != FileKind::Folder)
        {
            stats.files += 1;
            match source.entry.state {
                IndexState::Ready => {
                    stats.indexed += 1;
                    stats.chunks += source.chunks;
                    entities.extend(source.entity_counts.keys());
                }
                IndexState::Failed => stats.failed += 1,
                IndexState::Stored => stats.pending += 1,
            }
        }
        stats.entities = entities.len();
        Ok(stats)
    }
}
