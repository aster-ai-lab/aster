use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use same_file::Handle;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::workspace_document::{DocumentError, WorkspaceDocuments};

const TEXT_LIMIT: usize = 64 * 1024;
type RunOwners = HashMap<(String, String), Option<HashMap<String, String>>>;

/// A launcher-owned workspace binding for reads from the shell's document models.
pub(super) struct DocumentReader {
    documents: Arc<WorkspaceDocuments>,
    requested: PathBuf,
    root: PathBuf,
    binding: Handle,
    runs: Mutex<RunOwners>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadRequest {
    session_id: String,
    path: String,
    #[serde(default)]
    run_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseRequest {
    session_id: String,
    run_id: String,
}

impl DocumentReader {
    pub fn new(documents: Arc<WorkspaceDocuments>, root: &Path) -> Result<Self, DocumentError> {
        let requested = root.to_path_buf();
        let root = crate::local_documents::canonical_workspace(
            root.to_str().ok_or(DocumentError::Invalid)?,
        )
        .map_err(|_| DocumentError::NotFound)?;
        let binding = Handle::from_path(&root).map_err(|_| DocumentError::NotFound)?;
        Ok(Self {
            documents,
            requested,
            root,
            binding,
            runs: Mutex::new(HashMap::new()),
        })
    }

    pub fn request(&self, session: &str, params: Value) -> Result<Value, DocumentError> {
        let request: ReadRequest =
            serde_json::from_value(params).map_err(|_| DocumentError::Invalid)?;
        if request.session_id != session || session.is_empty() {
            return Err(DocumentError::NotFound);
        }
        match request.run_id {
            Some(run) => {
                validate_run(&run)?;
                self.pin(session, &run, &request.path)
            }
            None => self.read(&request.path),
        }
    }

    pub fn release_run(&self, session: &str, params: Value) -> Result<Value, DocumentError> {
        let request: ReleaseRequest =
            serde_json::from_value(params).map_err(|_| DocumentError::Invalid)?;
        if session.is_empty() || request.session_id != session {
            return Err(DocumentError::NotFound);
        }
        validate_run(&request.run_id)?;
        let key = (session.to_owned(), request.run_id.clone());
        let mut runs = self.runs.lock().map_err(|_| DocumentError::Unavailable)?;
        if !runs.contains_key(&key) && runs.len() >= 1024 {
            return Err(DocumentError::Capacity);
        }
        if let Some(Some(documents)) = runs.get_mut(&key) {
            let paths: Vec<_> = documents.keys().cloned().collect();
            for path in paths {
                let lease = documents.get(&path).ok_or(DocumentError::Unavailable)?;
                match self.documents.close(lease) {
                    Ok(()) | Err(DocumentError::NotFound) => {
                        documents.remove(&path);
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        runs.insert(key, None);
        Ok(json!({"runId":request.run_id,"released":true}))
    }

    fn validate_workspace(&self) -> Result<(), DocumentError> {
        if std::fs::canonicalize(&self.requested).ok().as_deref() != Some(self.root.as_path())
            || Handle::from_path(&self.requested).ok().as_ref() != Some(&self.binding)
        {
            return Err(DocumentError::NotFound);
        }
        Ok(())
    }

    fn pin(&self, session: &str, run: &str, path: &str) -> Result<Value, DocumentError> {
        self.validate_workspace()?;
        let key = (session.to_owned(), run.to_owned());
        let mut runs = self.runs.lock().map_err(|_| DocumentError::Unavailable)?;
        if matches!(runs.get(&key), Some(None)) {
            return Err(DocumentError::NotFound);
        }
        if let Some(lease) = runs
            .get(&key)
            .and_then(Option::as_ref)
            .and_then(|documents| documents.get(path))
        {
            let observation = self.documents.diagnostics(lease)?;
            if observation.snapshot.text.len() > TEXT_LIMIT {
                return Err(DocumentError::TooLarge);
            }
            return Ok(
                json!({"path":path,"snapshot":observation.snapshot,"diagnostics":observation.diagnostics,"released":false,"runId":run}),
            );
        }
        if (!runs.contains_key(&key)
            && (runs.len() >= 1024 || runs.values().filter(|run| run.is_some()).count() >= 64))
            || runs
                .get(&key)
                .and_then(Option::as_ref)
                .is_some_and(|documents| documents.len() >= 32)
        {
            return Err(DocumentError::Capacity);
        }
        let lease = self
            .documents
            .open(self.requested.to_str().ok_or(DocumentError::Invalid)?, path)?;
        let observation = match self.documents.diagnostics(&lease.lease) {
            Ok(observation) => observation,
            Err(error) => {
                self.documents.close(&lease.lease)?;
                return Err(error);
            }
        };
        if observation.snapshot.text.len() > TEXT_LIMIT {
            self.documents.close(&lease.lease)?;
            return Err(DocumentError::TooLarge);
        }
        runs.entry(key)
            .or_insert_with(|| Some(HashMap::new()))
            .as_mut()
            .ok_or(DocumentError::NotFound)?
            .insert(path.to_owned(), lease.lease);
        crate::local_documents::note_agent_retrieval(&self.root, path);
        Ok(
            json!({"path":path,"snapshot":observation.snapshot,"diagnostics":observation.diagnostics,"released":false,"runId":run}),
        )
    }

    fn read(&self, path: &str) -> Result<Value, DocumentError> {
        self.validate_workspace()?;
        let lease = self
            .documents
            .open(self.requested.to_str().ok_or(DocumentError::Invalid)?, path)?;
        let observation = self.documents.diagnostics(&lease.lease);
        // This independent lease never inherits a view's lifetime or escapes the host.
        self.documents.close(&lease.lease)?;
        let observation = observation?;
        if observation.snapshot.text.len() > TEXT_LIMIT {
            return Err(DocumentError::TooLarge);
        }
        crate::local_documents::note_agent_retrieval(&self.root, path);
        Ok(
            json!({"path":path,"snapshot":observation.snapshot,"diagnostics":observation.diagnostics,"released":true}),
        )
    }
}

impl Drop for DocumentReader {
    fn drop(&mut self) {
        let runs = self
            .runs
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for lease in runs
            .values()
            .filter_map(Option::as_ref)
            .flat_map(|documents| documents.values())
        {
            // Drop cannot acknowledge cleanup; release_run is the checked path.
            let _ = self.documents.close(lease);
        }
    }
}

fn validate_run(run: &str) -> Result<(), DocumentError> {
    let id = uuid::Uuid::parse_str(run).map_err(|_| DocumentError::Invalid)?;
    if id.to_string() != run {
        return Err(DocumentError::Invalid);
    }
    Ok(())
}

#[cfg(test)]
#[path = "document_run_tests.rs"]
mod run_tests;

#[cfg(test)]
#[path = "document_read_tests.rs"]
mod tests;
