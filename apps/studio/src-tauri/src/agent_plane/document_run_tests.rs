#![cfg(test)]

use super::*;
use crate::workspace_snapshot::WorkspaceSnapshots;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("a3s-run-pin-{}", uuid::Uuid::now_v7()));
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
        json!({"sessionId":"owned-session","path":self.0.join("note.txt")})
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pin(workspace: &Workspace, nonce: &str) -> Value {
    let mut params = workspace.params();
    params["runId"] = json!(nonce);
    params
}

fn release(nonce: &str) -> Value {
    json!({"sessionId":"owned-session","runId":nonce})
}

#[test]
fn pinned_clean_model_survives_view_and_temporary_reader_until_run_release() {
    let workspace = Workspace::new();
    let documents = workspace.documents();
    let reader = DocumentReader::new(documents.clone(), &workspace.0).unwrap();
    let nonce = uuid::Uuid::now_v7().to_string();
    let editor = documents
        .open(
            workspace.0.to_str().unwrap(),
            workspace.0.join("note.txt").to_str().unwrap(),
        )
        .unwrap();
    let pinned = reader
        .request("owned-session", pin(&workspace, &nonce))
        .unwrap();
    assert_eq!(
        pinned["snapshot"]["documentId"],
        editor.snapshot.document_id
    );
    documents.close(&editor.lease).unwrap();
    for _ in 0..1100 {
        let read = reader.request("owned-session", workspace.params()).unwrap();
        assert_eq!(read["snapshot"]["documentId"], editor.snapshot.document_id);
        let read = reader
            .request("owned-session", pin(&workspace, &nonce))
            .unwrap();
        assert_eq!(read["snapshot"]["documentId"], editor.snapshot.document_id);
        assert_eq!(read["released"], false);
    }
    assert_eq!(
        reader
            .release_run("owned-session", release(&nonce))
            .unwrap()["released"],
        true
    );
    assert_eq!(
        reader
            .release_run("owned-session", release(&nonce))
            .unwrap()["released"],
        true
    );
    assert_ne!(
        reader.request("owned-session", workspace.params()).unwrap()["snapshot"]["documentId"],
        editor.snapshot.document_id
    );
    assert_eq!(
        reader.request("owned-session", pin(&workspace, &nonce)),
        Err(DocumentError::NotFound)
    );
}

#[test]
fn run_release_does_not_revoke_an_independent_owner_or_allow_foreign_cleanup() {
    let workspace = Workspace::new();
    let reader = DocumentReader::new(workspace.documents(), &workspace.0).unwrap();
    let left = uuid::Uuid::now_v7().to_string();
    let right = uuid::Uuid::now_v7().to_string();
    let snapshot = reader
        .request("owned-session", pin(&workspace, &left))
        .unwrap();
    reader
        .request("owned-session", pin(&workspace, &right))
        .unwrap();
    assert_eq!(
        reader.release_run("foreign", release(&left)),
        Err(DocumentError::NotFound)
    );
    reader.release_run("owned-session", release(&left)).unwrap();
    assert_eq!(
        reader
            .request("owned-session", pin(&workspace, &right))
            .unwrap()["snapshot"]["documentId"],
        snapshot["snapshot"]["documentId"]
    );
    reader
        .release_run("owned-session", release(&right))
        .unwrap();
    assert_ne!(
        reader.request("owned-session", workspace.params()).unwrap()["snapshot"]["documentId"],
        snapshot["snapshot"]["documentId"]
    );
}

#[test]
fn release_still_acknowledges_when_bound_workspace_is_replaced() {
    let workspace = Workspace::new();
    let reader = DocumentReader::new(workspace.documents(), &workspace.0).unwrap();
    let nonce = uuid::Uuid::now_v7().to_string();
    reader
        .request("owned-session", pin(&workspace, &nonce))
        .unwrap();
    let retired = workspace.0.with_extension("retired");
    std::fs::rename(&workspace.0, &retired).unwrap();
    std::fs::create_dir(&workspace.0).unwrap();
    std::fs::write(workspace.0.join("note.txt"), "replacement").unwrap();
    assert_eq!(
        reader.request("owned-session", pin(&workspace, &nonce)),
        Err(DocumentError::NotFound)
    );
    assert_eq!(
        reader
            .release_run("owned-session", release(&nonce))
            .unwrap()["released"],
        true
    );
    assert_eq!(
        std::fs::read_to_string(workspace.0.join("note.txt")).unwrap(),
        "replacement"
    );
    std::fs::remove_dir_all(retired).unwrap();
}

#[test]
fn reader_drop_releases_pins_and_oversize_refusal_does_not_leak() {
    let workspace = Workspace::new();
    let documents = workspace.documents();
    let nonce = uuid::Uuid::now_v7().to_string();
    let identity = {
        let reader = DocumentReader::new(documents.clone(), &workspace.0).unwrap();
        reader
            .request("owned-session", pin(&workspace, &nonce))
            .unwrap()["snapshot"]["documentId"]
            .clone()
    };
    let reader = DocumentReader::new(documents, &workspace.0).unwrap();
    assert_ne!(
        reader.request("owned-session", workspace.params()).unwrap()["snapshot"]["documentId"],
        identity
    );
    std::fs::write(workspace.0.join("note.txt"), "x".repeat(TEXT_LIMIT + 1)).unwrap();
    for _ in 0..1100 {
        assert_eq!(
            reader.request("owned-session", pin(&workspace, &nonce)),
            Err(DocumentError::TooLarge)
        );
    }
    assert!(reader.runs.lock().unwrap().is_empty());
    assert_eq!(
        reader
            .release_run("owned-session", release(&nonce))
            .unwrap()["released"],
        true
    );
}

#[test]
fn malformed_or_injected_pin_and_release_requests_cannot_admit_resources() {
    let workspace = Workspace::new();
    let reader = DocumentReader::new(workspace.documents(), &workspace.0).unwrap();
    assert_eq!(
        reader.request("owned-session", pin(&workspace, "fake-run")),
        Err(DocumentError::Invalid)
    );
    let nonce = uuid::Uuid::now_v7().to_string();
    let mut injected = pin(&workspace, &nonce);
    injected["grant"] = json!({"write":true});
    assert_eq!(
        reader.request("owned-session", injected),
        Err(DocumentError::Invalid)
    );
    let mut injected = release(&nonce);
    injected["root"] = json!(workspace.0);
    assert_eq!(
        reader.release_run("owned-session", injected),
        Err(DocumentError::Invalid)
    );
    assert!(reader.runs.lock().unwrap().is_empty());
}

#[test]
fn owner_document_and_closed_nonce_limits_refuse_without_eviction_or_leaking() {
    let workspace = Workspace::new();
    let reader = DocumentReader::new(workspace.documents(), &workspace.0).unwrap();
    let nonces: Vec<_> = (0..65).map(|_| uuid::Uuid::now_v7().to_string()).collect();
    for nonce in nonces.iter().take(64) {
        reader
            .request("owned-session", pin(&workspace, nonce))
            .unwrap();
    }
    assert_eq!(
        reader.request("owned-session", pin(&workspace, &nonces[64])),
        Err(DocumentError::Capacity)
    );
    reader
        .release_run("owned-session", release(&nonces[0]))
        .unwrap();
    reader
        .request("owned-session", pin(&workspace, &nonces[64]))
        .unwrap();
    for nonce in nonces.iter().skip(1) {
        reader.release_run("owned-session", release(nonce)).unwrap();
    }
    assert_eq!(
        reader.request("owned-session", pin(&workspace, &nonces[0])),
        Err(DocumentError::NotFound)
    );
    let nonce = uuid::Uuid::now_v7().to_string();
    for index in 0..33 {
        let path = workspace.0.join(format!("{index}.txt"));
        std::fs::write(&path, "small text").unwrap();
        let params = json!({"sessionId":"owned-session","runId":nonce,"path":path});
        if index < 32 {
            reader.request("owned-session", params).unwrap();
        } else {
            assert_eq!(
                reader.request("owned-session", params),
                Err(DocumentError::Capacity)
            );
        }
    }
    reader
        .release_run("owned-session", release(&nonce))
        .unwrap();
    while reader.runs.lock().unwrap().len() < 1024 {
        reader
            .release_run("owned-session", release(&uuid::Uuid::now_v7().to_string()))
            .unwrap();
    }
    assert_eq!(
        reader.request(
            "owned-session",
            pin(&workspace, &uuid::Uuid::now_v7().to_string())
        ),
        Err(DocumentError::Capacity)
    );
    assert_eq!(
        reader.release_run("owned-session", release(&uuid::Uuid::now_v7().to_string())),
        Err(DocumentError::Capacity)
    );
    assert_eq!(
        reader
            .release_run("owned-session", release(&nonce))
            .unwrap()["released"],
        true
    );
    assert_eq!(
        reader.request("owned-session", pin(&workspace, &nonces[0])),
        Err(DocumentError::NotFound)
    );
}
