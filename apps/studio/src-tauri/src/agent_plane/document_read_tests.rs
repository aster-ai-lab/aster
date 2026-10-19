#![cfg(test)]

use super::*;
use crate::workspace_snapshot::WorkspaceSnapshots;

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("a3s-harness-read-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("note.txt"), "disk text").unwrap();
        Self(root)
    }

    fn documents(&self) -> Arc<WorkspaceDocuments> {
        Arc::new(WorkspaceDocuments::new(Arc::new(
            WorkspaceSnapshots::default(),
        )))
    }

    fn params(&self) -> Value {
        json!({"sessionId":"owned-session", "path":self.0.join("note.txt")})
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn harness_reads_unsaved_model_after_view_closes_without_leaking_leases() {
    let workspace = Workspace::new();
    let documents = workspace.documents();
    let editor = documents
        .open(
            workspace.0.to_str().unwrap(),
            workspace.0.join("note.txt").to_str().unwrap(),
        )
        .unwrap();
    let changed = documents
        .edit(
            &editor.lease,
            1,
            editor.snapshot.version,
            "unsaved editor text",
        )
        .unwrap();
    let reader = DocumentReader::new(documents.clone(), &workspace.0).unwrap();
    documents.close(&editor.lease).unwrap();
    // More calls than the host lease ceiling expose forgotten read leases.
    for _ in 0..1100 {
        let value = reader.request("owned-session", workspace.params()).unwrap();
        assert_eq!(value["snapshot"]["text"], "unsaved editor text");
        assert_eq!(value["snapshot"]["version"], changed.version);
        assert_eq!(value["snapshot"]["documentId"], editor.snapshot.document_id);
        assert_eq!(value["released"], true);
        assert_eq!(value["snapshot"]["dirty"], true);
    }
    assert_eq!(
        std::fs::read_to_string(workspace.0.join("note.txt")).unwrap(),
        "disk text"
    );
}

#[test]
fn requests_cannot_change_session_or_workspace_authority() {
    let workspace = Workspace::new();
    let other = Workspace::new();
    let reader = DocumentReader::new(workspace.documents(), &workspace.0).unwrap();
    assert_eq!(
        reader.request("another-session", workspace.params()),
        Err(DocumentError::NotFound)
    );
    let mut injected = workspace.params();
    injected["root"] = json!(other.0);
    assert_eq!(
        reader.request("owned-session", injected),
        Err(DocumentError::Invalid)
    );
    let mut foreign = workspace.params();
    foreign["path"] = json!(other.0.join("note.txt"));
    assert!(reader.request("owned-session", foreign).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(other.0.join("note.txt"), workspace.0.join("escape.txt"))
            .unwrap();
        let mut escape = workspace.params();
        escape["path"] = json!(workspace.0.join("escape.txt"));
        assert!(reader.request("owned-session", escape).is_err());
    }
    let retired = workspace.0.with_extension("retired");
    std::fs::rename(&workspace.0, &retired).unwrap();
    std::fs::create_dir(&workspace.0).unwrap();
    std::fs::write(workspace.0.join("note.txt"), "replacement").unwrap();
    assert_eq!(
        reader.request("owned-session", workspace.params()),
        Err(DocumentError::NotFound)
    );
    std::fs::remove_dir_all(retired).unwrap();
}

#[test]
fn oversize_reads_release_before_refusing_observation() {
    let workspace = Workspace::new();
    std::fs::write(workspace.0.join("note.txt"), "x".repeat(TEXT_LIMIT + 1)).unwrap();
    let reader = DocumentReader::new(workspace.documents(), &workspace.0).unwrap();
    for _ in 0..1100 {
        assert_eq!(
            reader.request("owned-session", workspace.params()),
            Err(DocumentError::TooLarge)
        );
    }
    std::fs::write(workspace.0.join("note.txt"), "small again").unwrap();
    assert_eq!(
        reader.request("owned-session", workspace.params()).unwrap()["snapshot"]["text"],
        "small again"
    );
}
