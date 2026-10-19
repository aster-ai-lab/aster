//! Shell-owned unsaved text models. Leases address access; mutations carry version and sequence.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, PathBuf};
use std::sync::{Arc, Mutex};

use same_file::Handle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::workspace_snapshot::WorkspaceSnapshots;

#[path = "workspace_diagnostics.rs"]
mod diagnostics;
pub(crate) use diagnostics::DocumentDiagnostic;

pub(crate) const TEXT_LIMIT: usize = 20 * 1024 * 1024;
const MEMORY_LIMIT: usize = 128 * 1024 * 1024;
const DOCUMENT_LIMIT: usize = 128;
const LEASE_LIMIT: usize = 1024;
const RECEIPT_LIMIT: usize = 32;
const RECOVERY_LIMIT: usize = 128;
const DELEGATION_DEPTH: usize = 64;
const DELEGATED_LEASE_LIMIT: usize = 256;
const MAX_VERSION: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum DocumentError {
    #[error("document not found")]
    NotFound,
    #[error("invalid document command")]
    Invalid,
    #[error("document conflict")]
    Conflict,
    #[error("document too large")]
    TooLarge,
    #[error("document capacity exhausted")]
    Capacity,
    #[error("document unavailable")]
    Unavailable,
    #[error("document is not UTF-8 text")]
    Unsupported,
}

impl DocumentError {
    pub fn status(self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Invalid => 400,
            Self::Conflict => 409,
            Self::TooLarge => 413,
            Self::Capacity | Self::Unavailable => 503,
            Self::Unsupported => 415,
        }
    }

    fn disk(status: u16) -> Self {
        match status {
            400 => Self::Invalid,
            403 | 404 => Self::NotFound,
            409 => Self::Conflict,
            413 => Self::TooLarge,
            _ => Self::Unavailable,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentSnapshot {
    pub document_id: String,
    pub uri: String,
    pub version: u64,
    pub text: Arc<str>,
    pub dirty: bool,
    pub disk_conflict: bool,
}

#[derive(Serialize)]
pub(crate) struct DocumentLease {
    pub lease: String,
    pub snapshot: DocumentSnapshot,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DocumentPermissions {
    pub edit: bool,
    pub save: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationReceipt {
    pub version: u64,
    pub dirty: bool,
    pub disk_conflict: bool,
    pub retained: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryMetadata {
    pub id: String,
    pub revision: u64,
    pub base_version: u64,
    pub bytes: usize,
    pub can_resolve: bool,
}

#[derive(Serialize)]
pub(crate) struct RecoveryReview {
    pub snapshot: DocumentSnapshot,
    pub recovery: RecoveryMetadata,
    pub text: Arc<str>,
}

struct Recovery {
    revision: u64,
    base_version: u64,
    text: Arc<str>,
}
impl Recovery {
    fn metadata(&self, id: &str, can_resolve: bool) -> RecoveryMetadata {
        RecoveryMetadata {
            id: id.to_owned(),
            revision: self.revision,
            base_version: self.base_version,
            bytes: self.text.len(),
            can_resolve,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DocumentRequest {
    Open {
        root: String,
        path: String,
    },
    Read {
        lease: String,
    },
    Delegate {
        lease: String,
        permissions: DocumentPermissions,
    },
    DelegateDiagnostics {
        lease: String,
    },
    PublishDiagnostics {
        lease: String,
        sequence: u64,
        version: u64,
        items: Vec<DocumentDiagnostic>,
    },
    Diagnostics {
        lease: String,
    },
    Watch {
        lease: String,
        version: u64,
        #[serde(rename = "diskConflict")]
        disk_conflict: bool,
    },
    Edit {
        lease: String,
        sequence: u64,
        version: u64,
        text: String,
    },
    Input {
        lease: String,
        sequence: u64,
        version: u64,
        text: String,
    },
    Recoveries {
        lease: String,
    },
    ReviewRecovery {
        lease: String,
        id: String,
    },
    ResolveRecovery {
        lease: String,
        sequence: u64,
        version: u64,
        id: String,
        revision: u64,
        restore: bool,
    },
    Save {
        lease: String,
        sequence: u64,
        version: u64,
    },
    Discard {
        lease: String,
        sequence: u64,
        version: u64,
    },
    Close {
        lease: String,
    },
}

struct Document {
    root: PathBuf,
    path: PathBuf,
    binding: Handle,
    uri: String,
    text: Arc<str>,
    saved: Arc<str>,
    disk_revision: String,
    version: u64,
    disk_conflict: bool,
    recoveries: HashMap<String, Recovery>,
}

impl Document {
    fn current(&self) -> Result<(), DocumentError> {
        let root = std::fs::canonicalize(&self.root).map_err(|_| DocumentError::NotFound)?;
        let handle = Handle::from_path(&root).map_err(|_| DocumentError::NotFound)?;
        let path = std::fs::canonicalize(&self.path).map_err(|_| DocumentError::NotFound)?;
        if root != self.root || handle != self.binding || path != self.path {
            return Err(DocumentError::NotFound);
        }
        Ok(())
    }
    fn dirty(&self) -> bool {
        self.text != self.saved
    }
    fn snapshot(&self, id: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            document_id: id.to_owned(),
            uri: self.uri.clone(),
            version: self.version,
            text: self.text.clone(),
            dirty: self.dirty(),
            disk_conflict: self.disk_conflict,
        }
    }
    fn receipt(&self) -> MutationReceipt {
        MutationReceipt {
            version: self.version,
            dirty: self.dirty(),
            disk_conflict: self.disk_conflict,
            retained: false,
        }
    }
    fn memory(&self) -> usize {
        self.text.len()
            + self.saved.len()
            + self
                .recoveries
                .values()
                .map(|draft| draft.text.len())
                .sum::<usize>()
    }
}

struct Recorded {
    sequence: u64,
    digest: [u8; 32],
    result: Result<MutationReceipt, DocumentError>,
}
struct Lease {
    document: String,
    root: String,
    path: String,
    sequence: u64,
    receipts: VecDeque<Recorded>,
    parent: Option<String>,
    permissions: Option<DocumentPermissions>,
    depth: usize,
    diagnostic_owner: Option<String>,
    diagnostic_publication: Option<diagnostics::DocumentDiagnosticPublication>,
}
impl Lease {
    fn current(&self, doc: &Document) -> Result<(), DocumentError> {
        doc.current()?;
        if std::fs::canonicalize(&self.root).map_err(|_| DocumentError::NotFound)? != doc.root
            || std::fs::canonicalize(&self.path).map_err(|_| DocumentError::NotFound)? != doc.path
        {
            return Err(DocumentError::NotFound);
        }
        Ok(())
    }
}
#[derive(Default)]
struct State {
    documents: HashMap<String, Document>,
    leases: HashMap<String, Lease>,
}

pub(crate) struct WorkspaceDocuments {
    state: Mutex<State>,
    disk: Arc<WorkspaceSnapshots>,
}

#[derive(Clone, Copy)]
enum Mutation<'a> {
    Edit(&'a str),
    Input(&'a str),
    Resolve {
        id: &'a str,
        revision: u64,
        restore: bool,
    },
    Save,
    Discard,
}

impl WorkspaceDocuments {
    pub fn new(disk: Arc<WorkspaceSnapshots>) -> Self {
        Self {
            state: Mutex::new(State::default()),
            disk,
        }
    }

    pub fn open(&self, raw_root: &str, raw_path: &str) -> Result<DocumentLease, DocumentError> {
        let root =
            crate::local_documents::canonical_workspace(raw_root).map_err(DocumentError::disk)?;
        let path = PathBuf::from(raw_path);
        if raw_path.len() > 4096
            || raw_path.contains('\0')
            || !path.is_absolute()
            || path.components().any(|part| part == Component::ParentDir)
        {
            return Err(DocumentError::Invalid);
        }
        let path = std::fs::canonicalize(path).map_err(|_| DocumentError::NotFound)?;
        if !path.starts_with(&root) {
            return Err(DocumentError::NotFound);
        }
        let binding = Handle::from_path(&root).map_err(|_| DocumentError::NotFound)?;
        let mut state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        if state.leases.len() >= LEASE_LIMIT {
            return Err(DocumentError::Capacity);
        }
        let existing = state
            .documents
            .iter()
            .find(|(_, doc)| doc.root == root && doc.path == path && doc.binding == binding)
            .map(|(id, _)| id.clone());
        let id = if let Some(id) = existing {
            id
        } else {
            if state.documents.len() >= DOCUMENT_LIMIT {
                return Err(DocumentError::Capacity);
            }
            let disk = self
                .disk
                .read(
                    root.to_str().ok_or(DocumentError::Invalid)?,
                    path.to_str().ok_or(DocumentError::Invalid)?,
                )
                .map_err(DocumentError::disk)?;
            if disk.bytes.len() > TEXT_LIMIT {
                return Err(DocumentError::TooLarge);
            }
            if disk.bytes.contains(&0) {
                return Err(DocumentError::Unsupported);
            }
            let text: Arc<str> = String::from_utf8(disk.bytes)
                .map_err(|_| DocumentError::Unsupported)?
                .into();
            if state
                .documents
                .values()
                .map(Document::memory)
                .sum::<usize>()
                + text.len() * 2
                > MEMORY_LIMIT
            {
                return Err(DocumentError::Capacity);
            }
            let uri = url::Url::from_file_path(&path)
                .map_err(|_| DocumentError::Invalid)?
                .to_string();
            let document = Document {
                root,
                path,
                binding,
                uri,
                text: text.clone(),
                saved: text,
                disk_revision: disk.revision,
                version: 1,
                disk_conflict: false,
                recoveries: HashMap::new(),
            };
            document.current()?;
            let id = uuid::Uuid::now_v7().to_string();
            state.documents.insert(id.clone(), document);
            id
        };
        let doc = state.documents.get(&id).ok_or(DocumentError::NotFound)?;
        doc.current()?;
        let snapshot = doc.snapshot(&id);
        let lease = uuid::Uuid::now_v7().to_string();
        state.leases.insert(
            lease.clone(),
            Lease {
                document: id,
                root: raw_root.to_owned(),
                path: raw_path.to_owned(),
                sequence: 0,
                receipts: VecDeque::new(),
                parent: None,
                permissions: None,
                depth: 0,
                diagnostic_owner: None,
                diagnostic_publication: None,
            },
        );
        Ok(DocumentLease { lease, snapshot })
    }

    pub fn delegate(
        &self,
        parent_id: &str,
        permissions: DocumentPermissions,
    ) -> Result<DocumentLease, DocumentError> {
        self.delegate_document(parent_id, permissions, false)
    }

    pub fn delegate_diagnostics(&self, parent_id: &str) -> Result<DocumentLease, DocumentError> {
        self.delegate_document(
            parent_id,
            DocumentPermissions {
                edit: false,
                save: false,
            },
            true,
        )
    }

    fn delegate_document(
        &self,
        parent_id: &str,
        permissions: DocumentPermissions,
        diagnostics: bool,
    ) -> Result<DocumentLease, DocumentError> {
        let mut state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let parent = state.leases.get(parent_id).ok_or(DocumentError::NotFound)?;
        if diagnostics && parent.permissions.is_some() && parent.diagnostic_owner.is_none() {
            return Err(DocumentError::NotFound);
        }
        let doc = state
            .documents
            .get(&parent.document)
            .ok_or(DocumentError::NotFound)?;
        parent.current(doc)?;
        if parent.permissions.is_some_and(|ceiling| {
            (permissions.edit && !ceiling.edit) || (permissions.save && !ceiling.save)
        }) {
            return Err(DocumentError::NotFound);
        }
        if permissions.save && !permissions.edit {
            return Err(DocumentError::Invalid);
        }
        if state.leases.len() >= LEASE_LIMIT
            || parent.depth >= DELEGATION_DEPTH
            || state
                .leases
                .values()
                .filter(|lease| lease.permissions.is_some())
                .count()
                >= DELEGATED_LEASE_LIMIT
        {
            return Err(DocumentError::Capacity);
        }
        let snapshot = doc.snapshot(&parent.document);
        let child = Lease {
            document: parent.document.clone(),
            root: parent.root.clone(),
            path: parent.path.clone(),
            sequence: 0,
            receipts: VecDeque::new(),
            parent: Some(parent_id.to_owned()),
            permissions: Some(permissions),
            depth: parent.depth + 1,
            diagnostic_owner: diagnostics.then(|| uuid::Uuid::now_v7().to_string()),
            diagnostic_publication: None,
        };
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|_| DocumentError::Unavailable)?;
        let lease: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        if state.leases.contains_key(&lease) {
            return Err(DocumentError::Unavailable);
        }
        state.leases.insert(lease.clone(), child);
        Ok(DocumentLease { lease, snapshot })
    }

    pub fn read(&self, lease: &str) -> Result<DocumentSnapshot, DocumentError> {
        let state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let lease = state.leases.get(lease).ok_or(DocumentError::NotFound)?;
        let doc = state
            .documents
            .get(&lease.document)
            .ok_or(DocumentError::NotFound)?;
        lease.current(doc)?;
        Ok(doc.snapshot(&lease.document))
    }

    pub fn recoveries(&self, lease: &str) -> Result<Vec<RecoveryMetadata>, DocumentError> {
        let caller = lease;
        let state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let lease = state.leases.get(lease).ok_or(DocumentError::NotFound)?;
        if lease.permissions.is_some() {
            return Err(DocumentError::NotFound);
        }
        let doc = state
            .documents
            .get(&lease.document)
            .ok_or(DocumentError::NotFound)?;
        lease.current(doc)?;
        let mut entries: Vec<_> = doc
            .recoveries
            .iter()
            .map(|(id, draft)| draft.metadata(id, id == caller || !state.leases.contains_key(id)))
            .collect();
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(entries)
    }

    pub fn review_recovery(&self, lease: &str, id: &str) -> Result<RecoveryReview, DocumentError> {
        let can_resolve = id == lease;
        let state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let lease = state.leases.get(lease).ok_or(DocumentError::NotFound)?;
        if lease.permissions.is_some() {
            return Err(DocumentError::NotFound);
        }
        let doc = state
            .documents
            .get(&lease.document)
            .ok_or(DocumentError::NotFound)?;
        lease.current(doc)?;
        let draft = doc.recoveries.get(id).ok_or(DocumentError::NotFound)?;
        Ok(RecoveryReview {
            snapshot: doc.snapshot(&lease.document),
            recovery: draft.metadata(id, can_resolve || !state.leases.contains_key(id)),
            text: draft.text.clone(),
        })
    }

    pub fn input(
        &self,
        lease: &str,
        sequence: u64,
        version: u64,
        text: &str,
    ) -> Result<MutationReceipt, DocumentError> {
        self.mutate(lease, sequence, version, Mutation::Input(text))
    }

    pub fn resolve_recovery(
        &self,
        lease: &str,
        sequence: u64,
        version: u64,
        id: &str,
        revision: u64,
        restore: bool,
    ) -> Result<MutationReceipt, DocumentError> {
        self.mutate(
            lease,
            sequence,
            version,
            Mutation::Resolve {
                id,
                revision,
                restore,
            },
        )
    }

    pub fn edit(
        &self,
        lease: &str,
        sequence: u64,
        version: u64,
        text: &str,
    ) -> Result<MutationReceipt, DocumentError> {
        self.mutate(lease, sequence, version, Mutation::Edit(text))
    }
    pub fn save(
        &self,
        lease: &str,
        sequence: u64,
        version: u64,
    ) -> Result<MutationReceipt, DocumentError> {
        self.mutate(lease, sequence, version, Mutation::Save)
    }
    pub fn discard(
        &self,
        lease: &str,
        sequence: u64,
        version: u64,
    ) -> Result<MutationReceipt, DocumentError> {
        self.mutate(lease, sequence, version, Mutation::Discard)
    }

    fn mutate(
        &self,
        lease_id: &str,
        sequence: u64,
        version: u64,
        mutation: Mutation<'_>,
    ) -> Result<MutationReceipt, DocumentError> {
        if sequence == 0 || sequence > MAX_VERSION || version == 0 || version > MAX_VERSION {
            return Err(DocumentError::Invalid);
        }
        let mut digest = Sha256::new();
        digest.update(version.to_le_bytes());
        match mutation {
            Mutation::Edit(text) => {
                digest.update(b"edit");
                digest.update(text.as_bytes());
            }
            Mutation::Input(text) => {
                digest.update(b"input");
                digest.update(text.as_bytes());
            }
            Mutation::Resolve {
                id,
                revision,
                restore,
            } => {
                digest.update(b"resolve");
                digest.update(id.as_bytes());
                digest.update(revision.to_le_bytes());
                digest.update([u8::from(restore)]);
            }
            Mutation::Save => digest.update(b"save"),
            Mutation::Discard => digest.update(b"discard"),
        }
        let digest: [u8; 32] = digest.finalize().into();
        let mut state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let lease = state.leases.get(lease_id).ok_or(DocumentError::NotFound)?;
        if lease.permissions.is_some_and(|permissions| match mutation {
            Mutation::Edit(_) => !permissions.edit,
            Mutation::Save => !permissions.save,
            Mutation::Input(_) | Mutation::Resolve { .. } | Mutation::Discard => true,
        }) {
            return Err(DocumentError::NotFound);
        }
        let id = lease.document.clone();
        let doc = state.documents.get(&id).ok_or(DocumentError::NotFound)?;
        lease.current(doc)?;
        if sequence <= lease.sequence {
            let receipt = lease
                .receipts
                .iter()
                .find(|receipt| receipt.sequence == sequence)
                .ok_or(DocumentError::NotFound)?;
            return if receipt.digest == digest {
                receipt.result
            } else {
                Err(DocumentError::Conflict)
            };
        }
        if sequence != lease.sequence + 1 {
            return Err(DocumentError::Conflict);
        }
        let memory = state
            .documents
            .values()
            .map(Document::memory)
            .sum::<usize>();
        let recovery_count = state
            .documents
            .values()
            .map(|doc| doc.recoveries.len())
            .sum::<usize>();
        let foreign_owner = matches!(mutation, Mutation::Resolve { id, .. } if id != lease_id && state.leases.contains_key(id));
        let result = if doc.version != version {
            let doc = state
                .documents
                .get_mut(&id)
                .ok_or(DocumentError::NotFound)?;
            if let Mutation::Input(text) = mutation {
                (|| {
                    if version > doc.version {
                        return Err(DocumentError::Conflict);
                    }
                    if text.len() > TEXT_LIMIT {
                        return Err(DocumentError::TooLarge);
                    }
                    if text.contains('\0') {
                        return Err(DocumentError::Unsupported);
                    }
                    let prior = doc
                        .recoveries
                        .get(lease_id)
                        .map_or(0, |draft| draft.text.len());
                    if (!doc.recoveries.contains_key(lease_id) && recovery_count >= RECOVERY_LIMIT)
                        || memory - prior + text.len() > MEMORY_LIMIT
                    {
                        return Err(DocumentError::Capacity);
                    }
                    doc.recoveries.insert(
                        lease_id.to_owned(),
                        Recovery {
                            revision: sequence,
                            base_version: version,
                            text: Arc::from(text),
                        },
                    );
                    let mut receipt = doc.receipt();
                    receipt.retained = true;
                    Ok(receipt)
                })()
            } else {
                Err(DocumentError::Conflict)
            }
        } else if foreign_owner {
            Err(DocumentError::Conflict)
        } else {
            let doc = state
                .documents
                .get_mut(&id)
                .ok_or(DocumentError::NotFound)?;
            let clears_input = matches!(mutation, Mutation::Input(_) | Mutation::Discard);
            let released = if clears_input {
                doc.recoveries
                    .get(lease_id)
                    .map_or(0, |draft| draft.text.len())
            } else {
                0
            };
            let result = self.apply(doc, memory - released, mutation);
            if result.is_ok() && clears_input {
                doc.recoveries.remove(lease_id);
            }
            result
        };
        let lease = state
            .leases
            .get_mut(lease_id)
            .ok_or(DocumentError::NotFound)?;
        lease.sequence = sequence;
        lease.receipts.push_back(Recorded {
            sequence,
            digest,
            result,
        });
        while lease.receipts.len() > RECEIPT_LIMIT {
            lease.receipts.pop_front();
        }
        result
    }

    fn apply(
        &self,
        doc: &mut Document,
        memory: usize,
        mutation: Mutation<'_>,
    ) -> Result<MutationReceipt, DocumentError> {
        if doc.version >= MAX_VERSION {
            return Err(DocumentError::Capacity);
        }
        match mutation {
            Mutation::Edit(text) | Mutation::Input(text) => {
                if text.len() > TEXT_LIMIT {
                    return Err(DocumentError::TooLarge);
                }
                if text.contains('\0') {
                    return Err(DocumentError::Unsupported);
                }
                if memory - doc.text.len() + text.len() > MEMORY_LIMIT {
                    return Err(DocumentError::Capacity);
                }
                doc.text = Arc::from(text);
            }
            Mutation::Resolve {
                id,
                revision,
                restore,
            } => {
                let draft = doc.recoveries.get(id).ok_or(DocumentError::NotFound)?;
                if draft.revision != revision {
                    return Err(DocumentError::Conflict);
                }
                if restore {
                    doc.text = draft.text.clone();
                }
                doc.recoveries.remove(id);
            }
            Mutation::Save => {
                if !doc.dirty() && !doc.disk_conflict {
                    doc.version += 1;
                    return Ok(doc.receipt());
                }
                if memory - doc.saved.len() + doc.text.len() > MEMORY_LIMIT {
                    return Err(DocumentError::Capacity);
                }
                let saved = self.disk.write(
                    doc.root.to_str().ok_or(DocumentError::Invalid)?,
                    doc.path.to_str().ok_or(DocumentError::Invalid)?,
                    &doc.disk_revision,
                    doc.text.as_bytes(),
                );
                match saved {
                    Ok(saved) => {
                        doc.disk_revision = saved.revision;
                        doc.saved = doc.text.clone();
                        doc.disk_conflict = false;
                    }
                    Err(status) => {
                        doc.disk_conflict = true;
                        return Err(DocumentError::disk(status));
                    }
                }
            }
            Mutation::Discard => {
                let disk = self
                    .disk
                    .read(
                        doc.root.to_str().ok_or(DocumentError::Invalid)?,
                        doc.path.to_str().ok_or(DocumentError::Invalid)?,
                    )
                    .map_err(DocumentError::disk)?;
                if disk.bytes.len() > TEXT_LIMIT {
                    return Err(DocumentError::TooLarge);
                }
                let text = String::from_utf8(disk.bytes).map_err(|_| DocumentError::Unsupported)?;
                if text.contains('\0') {
                    return Err(DocumentError::Unsupported);
                }
                if memory - doc.text.len() - doc.saved.len() + text.len() * 2 > MEMORY_LIMIT {
                    return Err(DocumentError::Capacity);
                }
                doc.text = Arc::from(text);
                doc.saved = doc.text.clone();
                doc.disk_revision = disk.revision;
                doc.disk_conflict = false;
            }
        }
        doc.version += 1;
        Ok(doc.receipt())
    }

    pub fn close(&self, lease: &str) -> Result<(), DocumentError> {
        let mut state = self.state.lock().map_err(|_| DocumentError::Unavailable)?;
        let mut revoked = HashSet::from([lease.to_owned()]);
        let lease = state.leases.remove(lease).ok_or(DocumentError::NotFound)?;
        loop {
            let children: Vec<_> = state
                .leases
                .iter()
                .filter_map(|(id, item)| {
                    item.parent
                        .as_ref()
                        .filter(|parent| revoked.contains(*parent))
                        .filter(|_| !revoked.contains(id))
                        .map(|_| id.clone())
                })
                .collect();
            if children.is_empty() {
                break;
            }
            revoked.extend(children);
        }
        state.leases.retain(|id, _| !revoked.contains(id));
        let retained = state
            .leases
            .values()
            .any(|other| other.document == lease.document);
        if !retained
            && state
                .documents
                .get(&lease.document)
                .is_some_and(|doc| !doc.dirty() && !doc.disk_conflict && doc.recoveries.is_empty())
        {
            state.documents.remove(&lease.document);
        }
        Ok(())
    }

    pub fn allows_bearer(&self, bearer: &str) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        state
            .leases
            .get(bearer)
            .is_some_and(|lease| lease.permissions.is_some())
    }

    pub fn request_scoped(
        &self,
        bearer: &str,
        request: DocumentRequest,
    ) -> Result<String, DocumentError> {
        let lease = match &request {
            DocumentRequest::Read { lease }
            | DocumentRequest::Watch { lease, .. }
            | DocumentRequest::Edit { lease, .. }
            | DocumentRequest::Save { lease, .. }
            | DocumentRequest::Delegate { lease, .. }
            | DocumentRequest::DelegateDiagnostics { lease }
            | DocumentRequest::PublishDiagnostics { lease, .. }
            | DocumentRequest::Diagnostics { lease }
            | DocumentRequest::Close { lease } => lease,
            _ => return Err(DocumentError::NotFound),
        };
        if lease != bearer || !self.allows_bearer(bearer) {
            return Err(DocumentError::NotFound);
        }
        self.request(request)
    }

    pub fn request(&self, request: DocumentRequest) -> Result<String, DocumentError> {
        let value = match request {
            DocumentRequest::Open { root, path } => serde_json::to_value(self.open(&root, &path)?),
            DocumentRequest::Read { lease } => serde_json::to_value(self.read(&lease)?),
            DocumentRequest::Delegate { lease, permissions } => {
                serde_json::to_value(self.delegate(&lease, permissions)?)
            }
            DocumentRequest::DelegateDiagnostics { lease } => {
                serde_json::to_value(self.delegate_diagnostics(&lease)?)
            }
            DocumentRequest::PublishDiagnostics {
                lease,
                sequence,
                version,
                items,
            } => serde_json::to_value(self.publish_diagnostics(&lease, sequence, version, items)?),
            DocumentRequest::Diagnostics { lease } => {
                serde_json::to_value(self.diagnostics(&lease)?)
            }
            DocumentRequest::Recoveries { lease } => serde_json::to_value(self.recoveries(&lease)?),
            DocumentRequest::ReviewRecovery { lease, id } => {
                serde_json::to_value(self.review_recovery(&lease, &id)?)
            }
            DocumentRequest::Input {
                lease,
                sequence,
                version,
                text,
            } => serde_json::to_value(self.input(&lease, sequence, version, &text)?),
            DocumentRequest::ResolveRecovery {
                lease,
                sequence,
                version,
                id,
                revision,
                restore,
            } => serde_json::to_value(
                self.resolve_recovery(&lease, sequence, version, &id, revision, restore)?,
            ),
            DocumentRequest::Watch {
                lease,
                version,
                disk_conflict,
            } => {
                let snapshot = self.read(&lease)?;
                if snapshot.version == version && snapshot.disk_conflict == disk_conflict {
                    Ok(serde_json::json!({ "changed": false }))
                } else {
                    Ok(serde_json::json!({ "changed": true, "snapshot": snapshot }))
                }
            }
            DocumentRequest::Edit {
                lease,
                sequence,
                version,
                text,
            } => serde_json::to_value(self.edit(&lease, sequence, version, &text)?),
            DocumentRequest::Save {
                lease,
                sequence,
                version,
            } => serde_json::to_value(self.save(&lease, sequence, version)?),
            DocumentRequest::Discard {
                lease,
                sequence,
                version,
            } => serde_json::to_value(self.discard(&lease, sequence, version)?),
            DocumentRequest::Close { lease } => {
                self.close(&lease)?;
                Ok(serde_json::json!({ "closed": true }))
            }
        }
        .map_err(|_| DocumentError::Unavailable)?;
        Ok(value.to_string())
    }
}

#[cfg(test)]
#[path = "workspace_document_tests.rs"]
mod tests;
