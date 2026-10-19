#![cfg(test)]

use super::*;
use std::path::PathBuf;

struct Fixture {
    root: PathBuf,
    store: WorkspaceDocuments,
    owner: DocumentLease,
}
impl Fixture {
    fn new(text: &str) -> Self {
        let root = std::env::temp_dir().join(format!("a3s-diagnostics-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("note.txt"), text).unwrap();
        let store = WorkspaceDocuments::new(Arc::new(WorkspaceSnapshots::default()));
        let owner = store
            .open(
                root.to_str().unwrap(),
                root.join("note.txt").to_str().unwrap(),
            )
            .unwrap();
        Self { root, store, owner }
    }
    fn diagnostic(&self) -> DocumentLease {
        self.store.delegate_diagnostics(&self.owner.lease).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn item(message: &str) -> DocumentDiagnostic {
    DocumentDiagnostic {
        start: DiagnosticPosition {
            line: 0,
            character: 0,
        },
        end: DiagnosticPosition {
            line: 0,
            character: 1,
        },
        severity: DiagnosticSeverity::Error,
        message: message.to_owned(),
    }
}

#[cfg(unix)]
#[test]
fn retargeted_publisher_alias_withdraws_observation_from_an_independent_reader() {
    use std::os::unix::fs::symlink;

    let f = Fixture::new("original");
    let alias = f.root.join("alias.txt");
    symlink(f.root.join("note.txt"), &alias).unwrap();
    let aliased_owner = f
        .store
        .open(f.root.to_str().unwrap(), alias.to_str().unwrap())
        .unwrap();
    let grant = f.store.delegate_diagnostics(&aliased_owner.lease).unwrap();
    f.store
        .publish_diagnostics(&grant.lease, 1, 1, vec![item("alias finding")])
        .unwrap();
    assert_eq!(
        f.store
            .diagnostics(&f.owner.lease)
            .unwrap()
            .diagnostics
            .len(),
        1
    );
    std::fs::write(f.root.join("other.txt"), "replacement").unwrap();
    std::fs::remove_file(&alias).unwrap();
    symlink(f.root.join("other.txt"), &alias).unwrap();
    let observation = f.store.diagnostics(&f.owner.lease).unwrap();
    assert_eq!(&*observation.snapshot.text, "original");
    assert!(observation.diagnostics.is_empty());
    assert!(matches!(
        f.store.publish_diagnostics(&grant.lease, 2, 1, vec![]),
        Err(DocumentError::NotFound)
    ));
    f.store.close(&aliased_owner.lease).unwrap();
    assert_eq!(&*f.store.read(&f.owner.lease).unwrap().text, "original");
}

#[test]
fn publication_requires_explicit_authority_and_does_not_grant_document_writes() {
    let f = Fixture::new("text");
    let reader = f
        .store
        .delegate(
            &f.owner.lease,
            DocumentPermissions {
                edit: false,
                save: false,
            },
        )
        .unwrap();
    assert!(matches!(
        f.store
            .publish_diagnostics(&reader.lease, 1, 1, vec![item("refused")]),
        Err(DocumentError::NotFound)
    ));
    assert!(matches!(
        f.store.delegate_diagnostics(&reader.lease),
        Err(DocumentError::NotFound)
    ));
    assert!(matches!(
        f.store.publish_diagnostics(&f.owner.lease, 1, 1, vec![]),
        Err(DocumentError::NotFound)
    ));
    let grant = f.diagnostic();
    let publication = f
        .store
        .publish_diagnostics(&grant.lease, 1, 1, vec![item("accepted")])
        .unwrap();
    assert_ne!(publication.publisher_id, grant.lease);
    assert!(matches!(
        f.store.edit(&grant.lease, 1, 1, "denied"),
        Err(DocumentError::NotFound)
    ));
    assert!(matches!(
        f.store.save(&grant.lease, 1, 1),
        Err(DocumentError::NotFound)
    ));
    let observer = f.store.diagnostics(&reader.lease).unwrap();
    assert_eq!(observer.snapshot.version, 1);
    assert_eq!(observer.diagnostics[0].items[0].message, "accepted");
    assert_eq!(
        std::fs::read_to_string(f.root.join("note.txt")).unwrap(),
        "text"
    );
}

#[test]
fn diagnostics_bind_unsaved_version_and_never_reappear_after_content_is_restored() {
    let f = Fixture::new("disk");
    let grant = f.diagnostic();
    f.store.edit(&f.owner.lease, 1, 1, "unsaved").unwrap();
    f.store
        .publish_diagnostics(&grant.lease, 1, 2, vec![item("unsaved error")])
        .unwrap();
    let observation = f.store.diagnostics(&f.owner.lease).unwrap();
    assert_eq!(&*observation.snapshot.text, "unsaved");
    assert_eq!(
        observation.diagnostics[0].version,
        observation.snapshot.version
    );
    f.store.edit(&f.owner.lease, 2, 2, "new input").unwrap();
    assert!(
        f.store
            .diagnostics(&f.owner.lease)
            .unwrap()
            .diagnostics
            .is_empty()
    );
    assert!(matches!(
        f.store.publish_diagnostics(&grant.lease, 2, 2, vec![]),
        Err(DocumentError::Conflict)
    ));
    f.store.edit(&f.owner.lease, 3, 3, "unsaved").unwrap();
    assert!(
        f.store
            .diagnostics(&f.owner.lease)
            .unwrap()
            .diagnostics
            .is_empty()
    );
    f.store
        .publish_diagnostics(&grant.lease, 2, 4, vec![])
        .unwrap();
    assert_eq!(
        f.store.diagnostics(&f.owner.lease).unwrap().diagnostics[0].sequence,
        2
    );
    assert_eq!(
        std::fs::read_to_string(f.root.join("note.txt")).unwrap(),
        "disk"
    );
}

#[test]
fn exact_retry_is_idempotent_and_changed_or_out_of_order_publications_are_refused() {
    let f = Fixture::new("text");
    let grant = f.diagnostic();
    let first = f
        .store
        .publish_diagnostics(&grant.lease, 1, 1, vec![item("first")])
        .unwrap();
    let retry = f
        .store
        .publish_diagnostics(&grant.lease, 1, 1, vec![item("first")])
        .unwrap();
    assert_eq!(first.publisher_id, retry.publisher_id);
    for sequence in [0, 3, MAX_VERSION + 1] {
        assert!(
            f.store
                .publish_diagnostics(&grant.lease, sequence, 1, vec![])
                .is_err()
        );
    }
    assert!(matches!(
        f.store
            .publish_diagnostics(&grant.lease, 1, 1, vec![item("changed")]),
        Err(DocumentError::Conflict)
    ));
    f.store
        .publish_diagnostics(&grant.lease, 2, 1, vec![item("latest")])
        .unwrap();
    assert_eq!(
        f.store.diagnostics(&grant.lease).unwrap().diagnostics[0].items[0].message,
        "latest"
    );
}

#[test]
fn publisher_close_recursively_withdraws_only_its_observations_and_fences_reuse() {
    let f = Fixture::new("text");
    let first = f.diagnostic();
    let child = f.store.delegate_diagnostics(&first.lease).unwrap();
    let independent = f.diagnostic();
    f.store
        .publish_diagnostics(&first.lease, 1, 1, vec![item("first")])
        .unwrap();
    f.store
        .publish_diagnostics(&child.lease, 1, 1, vec![item("child")])
        .unwrap();
    let peer = f
        .store
        .publish_diagnostics(&independent.lease, 1, 1, vec![item("peer")])
        .unwrap();
    f.store.close(&first.lease).unwrap();
    let observation = f.store.diagnostics(&f.owner.lease).unwrap();
    assert_eq!(observation.diagnostics.len(), 1);
    assert_eq!(observation.diagnostics[0].publisher_id, peer.publisher_id);
    assert!(matches!(
        f.store.publish_diagnostics(&child.lease, 2, 1, vec![]),
        Err(DocumentError::NotFound)
    ));
    f.store.close(&f.owner.lease).unwrap();
    assert!(matches!(
        f.store.diagnostics(&independent.lease),
        Err(DocumentError::NotFound)
    ));
}

#[test]
fn ranges_use_zero_based_utf16_and_refuse_surrogate_splits_and_crlf_overrun() {
    let f = Fixture::new("🙂x\r\nz\n");
    let grant = f.diagnostic();
    let mut diagnostic = item("unicode");
    diagnostic.end.character = 2;
    f.store
        .publish_diagnostics(&grant.lease, 1, 1, vec![diagnostic.clone()])
        .unwrap();
    for end in [
        DiagnosticPosition {
            line: 0,
            character: 1,
        },
        DiagnosticPosition {
            line: 0,
            character: 4,
        },
        DiagnosticPosition {
            line: 3,
            character: 0,
        },
    ] {
        diagnostic.end = end;
        assert!(matches!(
            f.store
                .publish_diagnostics(&grant.lease, 2, 1, vec![diagnostic.clone()]),
            Err(DocumentError::Invalid)
        ));
    }
    diagnostic.end = DiagnosticPosition {
        line: 2,
        character: 0,
    };
    f.store
        .publish_diagnostics(&grant.lease, 2, 1, vec![diagnostic])
        .unwrap();
}

#[test]
fn publication_limits_refuse_without_erasing_previous_result_and_reclaim_on_close() {
    let f = Fixture::new("text");
    let mut grants = Vec::new();
    for _ in 0..PUBLICATION_LIMIT {
        let grant = f.diagnostic();
        f.store
            .publish_diagnostics(&grant.lease, 1, 1, vec![])
            .unwrap();
        grants.push(grant);
    }
    let extra = f.diagnostic();
    assert!(matches!(
        f.store.publish_diagnostics(&extra.lease, 1, 1, vec![]),
        Err(DocumentError::Capacity)
    ));
    assert_eq!(
        f.store
            .diagnostics(&f.owner.lease)
            .unwrap()
            .diagnostics
            .len(),
        PUBLICATION_LIMIT
    );
    f.store.close(&grants[0].lease).unwrap();
    f.store
        .publish_diagnostics(&extra.lease, 1, 1, vec![item("kept")])
        .unwrap();
    for items in [
        vec![item("large"); ITEM_LIMIT + 1],
        vec![item(&"x".repeat(2049))],
        vec![item("nul\0")],
        vec![item(&"x".repeat(2048)); 8],
    ] {
        assert!(
            f.store
                .publish_diagnostics(&extra.lease, 2, 1, items)
                .is_err()
        );
    }
    assert_eq!(
        f.store
            .diagnostics(&extra.lease)
            .unwrap()
            .diagnostics
            .iter()
            .find(|p| p.publisher_id
                == f.store.state.lock().unwrap().leases[&extra.lease]
                    .diagnostic_owner
                    .clone()
                    .unwrap())
            .unwrap()
            .items[0]
            .message,
        "kept"
    );
}

#[test]
fn typed_scoped_protocol_refuses_foreign_bearers_injected_fields_and_unknown_severity() {
    let f = Fixture::new("text");
    let grant = f.diagnostic();
    let input = serde_json::json!({"operation":"publish_diagnostics","lease":grant.lease,"sequence":1,"version":1,"items":[item("wire")]});
    let command = || serde_json::from_value::<DocumentRequest>(input.clone()).unwrap();
    assert!(matches!(
        f.store.request_scoped("foreign", command()),
        Err(DocumentError::NotFound)
    ));
    f.store.request_scoped(&grant.lease, command()).unwrap();
    let mut forged = input.clone();
    forged["root"] = serde_json::json!("/other");
    assert!(serde_json::from_value::<DocumentRequest>(forged).is_err());
    let mut forged = input;
    forged["items"][0]["severity"] = serde_json::json!("fatal");
    assert!(
        serde_json::from_value::<DocumentRequest>(forged)
            .err()
            .unwrap()
            .to_string()
            .contains("diagnostic severity")
    );
}

#[test]
fn two_workspaces_with_matching_text_do_not_share_diagnostics() {
    let f = Fixture::new("same");
    let other = Fixture::new("same");
    let owner = f
        .store
        .open(
            other.root.to_str().unwrap(),
            other.root.join("note.txt").to_str().unwrap(),
        )
        .unwrap();
    let grant = f.diagnostic();
    f.store
        .publish_diagnostics(&grant.lease, 1, 1, vec![item("first workspace")])
        .unwrap();
    assert_ne!(f.owner.snapshot.document_id, owner.snapshot.document_id);
    assert!(
        f.store
            .diagnostics(&owner.lease)
            .unwrap()
            .diagnostics
            .is_empty()
    );
}

#[test]
fn global_publication_budget_refuses_and_recovers_without_evicting_live_publishers() {
    let f = Fixture::new("text");
    let mut published = Vec::new();
    let mut refused = None;
    'documents: for index in 0..125 {
        let path = f.root.join(format!("budget-{index}.txt"));
        std::fs::write(&path, "text").unwrap();
        let owner = f
            .store
            .open(f.root.to_str().unwrap(), path.to_str().unwrap())
            .unwrap();
        for _ in 0..2 {
            let grant = f.store.delegate_diagnostics(&owner.lease).unwrap();
            let items = vec![item("x"); 128];
            match f.store.publish_diagnostics(&grant.lease, 1, 1, items) {
                Ok(publication) => published.push((grant.lease, publication.publisher_id)),
                Err(DocumentError::Capacity) => {
                    refused = Some(grant.lease);
                    break 'documents;
                }
                Err(error) => panic!("unexpected budget refusal: {error}"),
            }
        }
    }
    let refused =
        refused.expect("the global budget must be exercised before lease or document limits");
    assert!(published.len() > 100);
    for (lease, publisher_id) in &published {
        assert_eq!(
            f.store
                .diagnostics(lease)
                .unwrap()
                .diagnostics
                .iter()
                .find(|p| &p.publisher_id == publisher_id)
                .unwrap()
                .items
                .len(),
            128
        );
    }
    f.store.close(&published[0].0).unwrap();
    f.store
        .publish_diagnostics(&refused, 1, 1, vec![item("x"); 128])
        .unwrap();
}

#[test]
fn replaced_workspace_refuses_old_publications_and_oversize_models_cannot_publish() {
    let f = Fixture::new("text");
    let grant = f.diagnostic();
    f.store
        .publish_diagnostics(&grant.lease, 1, 1, vec![item("old binding")])
        .unwrap();
    let retired = f.root.with_extension("retired");
    std::fs::rename(&f.root, &retired).unwrap();
    std::fs::create_dir(&f.root).unwrap();
    std::fs::write(f.root.join("note.txt"), "text").unwrap();
    assert!(matches!(
        f.store.diagnostics(&grant.lease),
        Err(DocumentError::NotFound)
    ));
    assert!(matches!(
        f.store.publish_diagnostics(&grant.lease, 2, 1, vec![]),
        Err(DocumentError::NotFound)
    ));
    f.store.close(&grant.lease).unwrap();
    std::fs::remove_dir_all(&f.root).unwrap();
    std::fs::rename(retired, &f.root).unwrap();
    let large = Fixture::new(&"x".repeat(DOCUMENT_BYTES + 1));
    let grant = large.diagnostic();
    assert!(matches!(
        large.store.publish_diagnostics(&grant.lease, 1, 1, vec![]),
        Err(DocumentError::TooLarge)
    ));
}
