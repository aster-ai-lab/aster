use super::*;

const PUBLICATION_LIMIT: usize = 16;
const ITEM_LIMIT: usize = 128;
const PUBLICATION_BYTES: usize = 16 * 1024;
const DOCUMENT_BYTES: usize = 64 * 1024;
const STORE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            "information" => Ok(Self::Information),
            "hint" => Ok(Self::Hint),
            _ => Err("invalid diagnostic severity"),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "information",
            Self::Hint => "hint",
        }
    }
}
impl Serialize for DiagnosticSeverity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for DiagnosticSeverity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiagnosticPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentDiagnostic {
    pub start: DiagnosticPosition,
    pub end: DiagnosticPosition,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentDiagnosticPublication {
    pub publisher_id: String,
    pub sequence: u64,
    pub version: u64,
    pub items: Arc<Vec<DocumentDiagnostic>>,
    #[serde(skip)]
    bytes: usize,
}

#[derive(Serialize)]
pub(crate) struct DocumentDiagnosticObservation {
    pub snapshot: DocumentSnapshot,
    pub diagnostics: Vec<DocumentDiagnosticPublication>,
}

impl WorkspaceDocuments {
    pub fn publish_diagnostics(
        &self,
        lease_id: &str,
        sequence: u64,
        version: u64,
        items: Vec<DocumentDiagnostic>,
    ) -> Result<DocumentDiagnosticPublication, DocumentError> {
        if sequence == 0 || sequence > MAX_VERSION || items.len() > ITEM_LIMIT {
            return Err(DocumentError::Invalid);
        }
        let encoded = serde_json::to_vec(&items).map_err(|_| DocumentError::Invalid)?;
        if encoded.len() > PUBLICATION_BYTES {
            return Err(DocumentError::TooLarge);
        }
        let bytes = encoded.len()
            + items.len() * std::mem::size_of::<DocumentDiagnostic>()
            + std::mem::size_of::<DocumentDiagnosticPublication>();
        let mut state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let lease = state.leases.get(lease_id).ok_or(DocumentError::NotFound)?;
        let publisher_id = lease
            .diagnostic_owner
            .clone()
            .ok_or(DocumentError::NotFound)?;
        let id = lease.document.clone();
        let doc = state.documents.get(&id).ok_or(DocumentError::NotFound)?;
        lease.current(doc)?;
        if version != doc.version {
            return Err(DocumentError::Conflict);
        }
        if doc.text.len() > DOCUMENT_BYTES {
            return Err(DocumentError::TooLarge);
        }
        for item in &items {
            if item.message.is_empty()
                || item.message.len() > 2048
                || item.message.contains('\0')
                || item.start > item.end
                || !position_valid(&doc.text, item.start)
                || !position_valid(&doc.text, item.end)
            {
                return Err(DocumentError::Invalid);
            }
        }
        if let Some(previous) = &lease.diagnostic_publication {
            if sequence == previous.sequence {
                return if previous.version == version && *previous.items == items {
                    Ok(previous.clone())
                } else {
                    Err(DocumentError::Conflict)
                };
            }
            if sequence != previous.sequence + 1 {
                return Err(DocumentError::Conflict);
            }
        } else if sequence != 1 {
            return Err(DocumentError::Conflict);
        }
        let mut document_count = 0;
        let mut document_bytes = bytes;
        let mut store_bytes = bytes;
        for (other_id, other) in &state.leases {
            if other_id == lease_id {
                continue;
            }
            if let Some(publication) = &other.diagnostic_publication {
                store_bytes += publication.bytes;
                if other.document == id {
                    document_count += 1;
                    document_bytes += publication.bytes;
                }
            }
        }
        if document_count >= PUBLICATION_LIMIT
            || document_bytes > DOCUMENT_BYTES
            || store_bytes > STORE_BYTES
        {
            return Err(DocumentError::Capacity);
        }
        let publication = DocumentDiagnosticPublication {
            publisher_id,
            sequence,
            version,
            items: Arc::new(items),
            bytes,
        };
        state
            .leases
            .get_mut(lease_id)
            .ok_or(DocumentError::NotFound)?
            .diagnostic_publication = Some(publication.clone());
        Ok(publication)
    }

    pub fn diagnostics(
        &self,
        lease_id: &str,
    ) -> Result<DocumentDiagnosticObservation, DocumentError> {
        let state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let lease = state.leases.get(lease_id).ok_or(DocumentError::NotFound)?;
        let doc = state
            .documents
            .get(&lease.document)
            .ok_or(DocumentError::NotFound)?;
        lease.current(doc)?;
        let mut diagnostics: Vec<_> = state
            .leases
            .values()
            .filter(|other| other.document == lease.document)
            .filter_map(|other| {
                let publication = other.diagnostic_publication.as_ref()?;
                (publication.version == doc.version && other.current(doc).is_ok())
                    .then_some(publication)
            })
            .cloned()
            .collect();
        diagnostics.sort_by(|a, b| a.publisher_id.cmp(&b.publisher_id));
        Ok(DocumentDiagnosticObservation {
            snapshot: doc.snapshot(&lease.document),
            diagnostics,
        })
    }
}

fn position_valid(text: &str, position: DiagnosticPosition) -> bool {
    let Some(line) = text.split('\n').nth(position.line as usize) else {
        return false;
    };
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut offset = 0;
    for character in line.chars() {
        if offset == position.character as usize {
            return true;
        }
        offset += character.len_utf16();
    }
    offset == position.character as usize
}

#[cfg(test)]
#[path = "workspace_diagnostics_tests.rs"]
mod tests;
