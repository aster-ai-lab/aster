#![cfg(test)]

use std::sync::Arc;

use super::plugin_document_response;
use crate::workspace_document::{DocumentPermissions, WorkspaceDocuments};
use crate::workspace_snapshot::WorkspaceSnapshots;

#[test]
fn diagnostic_publication_route_requires_its_own_scoped_native_grant() {
    let root =
        std::env::temp_dir().join(format!("a3s-diagnostic-authority-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("note.txt");
    std::fs::write(&path, "unsaved source").unwrap();
    let documents = WorkspaceDocuments::new(Arc::new(WorkspaceSnapshots::default()));
    let owner = documents
        .open(root.to_str().unwrap(), path.to_str().unwrap())
        .unwrap();
    let readonly = documents
        .delegate(
            &owner.lease,
            DocumentPermissions {
                edit: false,
                save: false,
            },
        )
        .unwrap();
    let publisher = documents.delegate_diagnostics(&owner.lease).unwrap();
    let body = |lease: &str| {
        serde_json::json!({"operation":"publish_diagnostics","lease":lease,"sequence":1,"version":1,"items":[{"start":{"line":0,"character":0},"end":{"line":0,"character":7},"severity":"warning","message":"finding"}]}).to_string()
    };
    assert_eq!(
        plugin_document_response(
            "POST",
            body(&readonly.lease).as_bytes(),
            &readonly.lease,
            &documents
        )
        .0,
        404
    );
    assert_eq!(
        plugin_document_response(
            "POST",
            body(&publisher.lease).as_bytes(),
            &readonly.lease,
            &documents
        )
        .0,
        404
    );
    let accepted = plugin_document_response(
        "POST",
        body(&publisher.lease).as_bytes(),
        &publisher.lease,
        &documents,
    );
    assert_eq!(accepted.0, 200);
    let publication: serde_json::Value = serde_json::from_slice(&accepted.3).unwrap();
    assert!(publication["publisherId"].as_str().unwrap().len() < publisher.lease.len());
    assert!(
        !String::from_utf8(accepted.3)
            .unwrap()
            .contains(&publisher.lease)
    );
    documents.close(&publisher.lease).unwrap();
    assert_eq!(
        plugin_document_response(
            "POST",
            body(&publisher.lease).as_bytes(),
            &publisher.lease,
            &documents
        )
        .0,
        404
    );
    assert!(
        documents
            .diagnostics(&owner.lease)
            .unwrap()
            .diagnostics
            .is_empty()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn plugin_document_handler_uses_only_scoped_bearers_and_typed_owned_commands() {
    let root = std::env::temp_dir().join(format!("a3s-document-auth-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("note.txt");
    std::fs::write(&path, "disk").unwrap();
    let documents = WorkspaceDocuments::new(Arc::new(WorkspaceSnapshots::default()));
    let owner = documents
        .open(root.to_str().unwrap(), path.to_str().unwrap())
        .unwrap();
    let grant = documents
        .delegate(
            &owner.lease,
            DocumentPermissions {
                edit: false,
                save: false,
            },
        )
        .unwrap();
    let scoped_head = format!(
        "POST /api/workspace-documents HTTP/1.1\r\nAuthorization: Bearer {}\r\n",
        grant.lease
    );
    assert!(!super::authorized(&scoped_head, &"ab".repeat(32)));
    let read = serde_json::json!({"operation":"read", "lease":grant.lease}).to_string();
    assert_eq!(
        plugin_document_response("POST", read.as_bytes(), "missing", &documents).0,
        404
    );
    assert_eq!(
        plugin_document_response("POST", read.as_bytes(), &owner.lease, &documents).0,
        404
    );
    let accepted = plugin_document_response("POST", read.as_bytes(), &grant.lease, &documents);
    assert_eq!(accepted.0, 200);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&accepted.3).unwrap()["text"],
        "disk"
    );
    let foreign = serde_json::json!({"operation":"read", "lease":owner.lease}).to_string();
    assert_eq!(
        plugin_document_response("POST", foreign.as_bytes(), &grant.lease, &documents).0,
        404
    );
    let unknown =
        serde_json::json!({"operation":"read", "lease":grant.lease,"root":root}).to_string();
    assert_eq!(
        plugin_document_response("POST", unknown.as_bytes(), &grant.lease, &documents).0,
        400
    );
    let edit = serde_json::json!({"operation":"edit","lease":grant.lease,"sequence":1,"version":1,"text":"denied"}).to_string();
    assert_eq!(
        plugin_document_response("POST", edit.as_bytes(), &grant.lease, &documents).0,
        404
    );
    assert_eq!(
        plugin_document_response("GET", read.as_bytes(), &grant.lease, &documents).0,
        404
    );
    documents.close(&owner.lease).unwrap();
    assert_eq!(
        plugin_document_response("POST", read.as_bytes(), &grant.lease, &documents).0,
        404
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"disk");
    std::fs::remove_dir_all(root).unwrap();
}
