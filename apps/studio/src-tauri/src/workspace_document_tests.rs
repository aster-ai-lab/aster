#![cfg(test)]

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

use super::{DocumentError, DocumentRequest, WorkspaceDocuments};
use crate::workspace_snapshot::WorkspaceSnapshots;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("a3s-document-{}", uuid::Uuid::now_v7()));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("note.txt"), b"disk text").unwrap();
        Self(path)
    }
    fn root(&self) -> &str {
        self.0.to_str().unwrap()
    }
    fn path(&self) -> String {
        self.0.join("note.txt").to_str().unwrap().to_owned()
    }
    fn store() -> WorkspaceDocuments {
        WorkspaceDocuments::new(Arc::new(WorkspaceSnapshots::default()))
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn two_views_observe_one_unsaved_model_and_only_explicit_save_changes_disk() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let first = store.open(workspace.root(), &workspace.path()).unwrap();
    let second = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_ne!(first.lease, second.lease);
    assert_eq!(first.snapshot.document_id, second.snapshot.document_id);
    assert!(first.snapshot.uri.starts_with("file://"));
    let changed = store
        .edit(&first.lease, 1, first.snapshot.version, "unsaved text")
        .unwrap();
    let observed = store.read(&second.lease).unwrap();
    assert_eq!(&*observed.text, "unsaved text");
    assert_eq!(observed.version, changed.version);
    assert!(observed.dirty);
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
    let saved = store.save(&second.lease, 1, observed.version).unwrap();
    assert!(!saved.dirty);
    assert_eq!(fs::read(workspace.path()).unwrap(), b"unsaved text");
    assert!(!store.read(&first.lease).unwrap().dirty);
}

#[test]
fn concurrent_views_cannot_both_edit_one_observed_version() {
    let workspace = Workspace::new();
    let store = Arc::new(Workspace::store());
    let barrier = Arc::new(Barrier::new(2));
    let first = store.open(workspace.root(), &workspace.path()).unwrap();
    let second = store.open(workspace.root(), &workspace.path()).unwrap();
    let handles: Vec<_> = [
        (first.lease.clone(), "first"),
        (second.lease.clone(), "second"),
    ]
    .into_iter()
    .map(|(lease, text)| {
        let store = store.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.edit(&lease, 1, 1, text)
        })
    })
    .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(DocumentError::Conflict))
            .count(),
        1
    );
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
    assert!(matches!(
        &*store.read(&first.lease).unwrap().text,
        "first" | "second"
    ));
}

#[test]
fn restoring_text_does_not_restore_the_old_document_version() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let view = store.open(workspace.root(), &workspace.path()).unwrap();
    let edited = store.edit(&view.lease, 1, 1, "intermediate").unwrap();
    let restored = store
        .edit(&view.lease, 2, edited.version, "disk text")
        .unwrap();
    assert!(!restored.dirty);
    assert_eq!(
        store.edit(&view.lease, 3, 1, "stale"),
        Err(DocumentError::Conflict)
    );
    assert_eq!(&*store.read(&view.lease).unwrap().text, "disk text");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
}

#[test]
fn repeated_command_returns_its_receipt_without_reapplying_after_newer_edits() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let view = store.open(workspace.root(), &workspace.path()).unwrap();
    let first = store.edit(&view.lease, 1, 1, "first").unwrap();
    let saved = store.save(&view.lease, 2, first.version).unwrap();
    let later = store.edit(&view.lease, 3, saved.version, "later").unwrap();
    assert_eq!(store.save(&view.lease, 2, first.version), Ok(saved));
    assert_eq!(store.edit(&view.lease, 1, 1, "first"), Ok(first));
    assert_eq!(
        store.edit(&view.lease, 1, 1, "altered retry"),
        Err(DocumentError::Conflict)
    );
    let current = store.read(&view.lease).unwrap();
    assert_eq!(current.version, later.version);
    assert_eq!(&*current.text, "later");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"first");
}

#[test]
fn expired_receipts_and_sequence_gaps_cannot_be_reexecuted() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let view = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_eq!(
        store.edit(&view.lease, 2, 1, "gap"),
        Err(DocumentError::Conflict)
    );
    for sequence in 1..=40 {
        store
            .edit(&view.lease, sequence, sequence, &sequence.to_string())
            .unwrap();
    }
    assert_eq!(
        store.edit(&view.lease, 1, 1, "1"),
        Err(DocumentError::NotFound)
    );
    let current = store.read(&view.lease).unwrap();
    assert_eq!(current.version, 41);
    assert_eq!(&*current.text, "40");
}

#[test]
fn closing_a_view_revokes_its_access_and_preserves_owned_unsaved_content() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let first = store.open(workspace.root(), &workspace.path()).unwrap();
    let second = store.open(workspace.root(), &workspace.path()).unwrap();
    let edited = store.edit(&first.lease, 1, 1, "retained draft").unwrap();
    store.close(&first.lease).unwrap();
    assert_eq!(
        store.read(&first.lease).err(),
        Some(DocumentError::NotFound)
    );
    assert_eq!(
        store.edit(&first.lease, 2, edited.version, "late"),
        Err(DocumentError::NotFound)
    );
    assert_eq!(&*store.read(&second.lease).unwrap().text, "retained draft");
    store.close(&second.lease).unwrap();
    let reopened = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_eq!(reopened.snapshot.document_id, first.snapshot.document_id);
    assert_eq!(&*reopened.snapshot.text, "retained draft");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
    store.discard(&reopened.lease, 1, edited.version).unwrap();
    store.close(&reopened.lease).unwrap();
    let fresh = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_ne!(fresh.snapshot.document_id, first.snapshot.document_id);
    assert_eq!(&*fresh.snapshot.text, "disk text");
}

#[test]
fn external_changes_refuse_save_and_explicit_discard_reads_the_actual_disk() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let view = store.open(workspace.root(), &workspace.path()).unwrap();
    let edited = store.edit(&view.lease, 1, 1, "local draft").unwrap();
    fs::write(workspace.path(), b"external change").unwrap();
    assert_eq!(
        store.save(&view.lease, 2, edited.version),
        Err(DocumentError::Conflict)
    );
    let current = store.read(&view.lease).unwrap();
    assert!(current.disk_conflict);
    assert_eq!(&*current.text, "local draft");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"external change");
    let discarded = store.discard(&view.lease, 3, current.version).unwrap();
    assert!(!discarded.dirty && !discarded.disk_conflict);
    assert_eq!(&*store.read(&view.lease).unwrap().text, "external change");
    assert_eq!(
        store.save(&view.lease, 2, edited.version),
        Err(DocumentError::Conflict)
    );
}

#[test]
fn workspace_and_file_addresses_are_isolated_even_when_text_matches() {
    let first = Workspace::new();
    let second = Workspace::new();
    let store = Workspace::store();
    let a = store.open(first.root(), &first.path()).unwrap();
    let b = store.open(second.root(), &second.path()).unwrap();
    assert_ne!(a.snapshot.document_id, b.snapshot.document_id);
    store.edit(&a.lease, 1, 1, "private draft").unwrap();
    assert_eq!(&*store.read(&b.lease).unwrap().text, "disk text");
    assert_eq!(
        store.open(first.root(), &second.path()).err(),
        Some(DocumentError::NotFound)
    );
    assert_eq!(
        store.read("missing-lease").err(),
        Some(DocumentError::NotFound)
    );
}

#[cfg(unix)]
#[test]
fn root_replacement_and_alias_retarget_refuse_old_leases() {
    use std::os::unix::fs::symlink;
    let first = Workspace::new();
    let second = Workspace::new();
    let store = Workspace::store();
    let alias = first.0.join("alias");
    symlink(&first.0, &alias).unwrap();
    let view = store
        .open(
            alias.to_str().unwrap(),
            alias.join("note.txt").to_str().unwrap(),
        )
        .unwrap();
    let canonical = store.open(first.root(), &first.path()).unwrap();
    fs::remove_file(&alias).unwrap();
    symlink(&second.0, &alias).unwrap();
    assert_eq!(store.read(&view.lease).err(), Some(DocumentError::NotFound));
    assert_eq!(&*store.read(&canonical.lease).unwrap().text, "disk text");
    let displaced = first.0.with_extension("old");
    fs::rename(&first.0, &displaced).unwrap();
    fs::create_dir(&first.0).unwrap();
    fs::write(first.path(), b"disk text").unwrap();
    assert_eq!(
        store.read(&canonical.lease).err(),
        Some(DocumentError::NotFound)
    );
    assert_eq!(
        store.save(&canonical.lease, 1, 1),
        Err(DocumentError::NotFound)
    );
    assert_eq!(fs::read(first.path()).unwrap(), b"disk text");
    fs::remove_dir_all(displaced).unwrap();
}

#[test]
fn invalid_text_and_oversized_edits_preserve_current_content_and_version() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let view = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_eq!(
        store.edit(&view.lease, 1, 1, "invalid\0text"),
        Err(DocumentError::Unsupported)
    );
    assert_eq!(
        store.edit(&view.lease, 2, 1, &"x".repeat(super::TEXT_LIMIT + 1)),
        Err(DocumentError::TooLarge)
    );
    assert_eq!(store.read(&view.lease).unwrap().version, 1);
    assert_eq!(&*store.read(&view.lease).unwrap().text, "disk text");
    assert_eq!(
        store.edit(&view.lease, 0, 1, "bad"),
        Err(DocumentError::Invalid)
    );
}

#[test]
fn native_request_schema_refuses_unknown_fields_and_drives_the_real_model() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let open =
        serde_json::json!({"operation":"open","root":workspace.root(),"path":workspace.path()});
    let response: serde_json::Value = serde_json::from_str(
        &store
            .request(serde_json::from_value(open).unwrap())
            .unwrap(),
    )
    .unwrap();
    let lease = response["lease"].as_str().unwrap();
    let edit = serde_json::json!({"operation":"edit","lease":lease,"sequence":1,"version":1,"text":"native draft"});
    store
        .request(serde_json::from_value(edit.clone()).unwrap())
        .unwrap();
    let invalid = serde_json::json!({"operation":"edit","lease":lease,"sequence":2,"version":2,"text":"bad","root":workspace.root()});
    assert!(serde_json::from_value::<DocumentRequest>(invalid).is_err());
    let current: serde_json::Value = serde_json::from_str(
        &store
            .request(DocumentRequest::Read {
                lease: lease.to_owned(),
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(current["text"], "native draft");
    assert_eq!(current["version"], 2);
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
}

#[test]
fn saving_a_clean_model_does_not_replace_disk_or_accept_unobserved_external_bytes() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let view = store.open(workspace.root(), &workspace.path()).unwrap();
    let before = WorkspaceSnapshots::default()
        .read(workspace.root(), &workspace.path())
        .unwrap()
        .revision;
    let saved = store.save(&view.lease, 1, view.snapshot.version).unwrap();
    let after = WorkspaceSnapshots::default()
        .read(workspace.root(), &workspace.path())
        .unwrap()
        .revision;
    assert_eq!(before, after);
    fs::write(workspace.path(), b"external").unwrap();
    let again = store.save(&view.lease, 2, saved.version).unwrap();
    assert_eq!(fs::read(workspace.path()).unwrap(), b"external");
    assert_eq!(&*store.read(&view.lease).unwrap().text, "disk text");
    let edited = store
        .edit(&view.lease, 3, again.version, "new draft")
        .unwrap();
    assert_eq!(
        store.save(&view.lease, 4, edited.version),
        Err(DocumentError::Conflict)
    );
    assert_eq!(fs::read(workspace.path()).unwrap(), b"external");
}

#[test]
fn refused_user_input_survives_close_without_replacing_accepted_peer_content() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let owner = store.open(workspace.root(), &workspace.path()).unwrap();
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    let changed = store.edit(&peer.lease, 1, 1, "peer text").unwrap();
    let saved = store.save(&peer.lease, 2, changed.version).unwrap();
    let retained = store
        .input(&owner.lease, 1, 1, "refused local text")
        .unwrap();
    assert!(retained.retained);
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "peer text");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"peer text");
    store.close(&owner.lease).unwrap();
    store.close(&peer.lease).unwrap();
    let reopened = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_eq!(reopened.snapshot.document_id, owner.snapshot.document_id);
    assert_eq!(&*reopened.snapshot.text, "peer text");
    let drafts = store.recoveries(&reopened.lease).unwrap();
    assert_eq!(drafts.len(), 1);
    let review = store
        .review_recovery(&reopened.lease, &drafts[0].id)
        .unwrap();
    assert_eq!(&*review.text, "refused local text");
    let restored = store
        .resolve_recovery(
            &reopened.lease,
            1,
            saved.version,
            &drafts[0].id,
            drafts[0].revision,
            true,
        )
        .unwrap();
    assert!(!restored.retained);
    assert!(store.recoveries(&reopened.lease).unwrap().is_empty());
    assert_eq!(
        &*store.read(&reopened.lease).unwrap().text,
        "refused local text"
    );
    assert_eq!(fs::read(workspace.path()).unwrap(), b"peer text");
}

#[test]
fn input_replay_does_not_rewind_a_newer_recovery_and_old_reviews_cannot_consume_it() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let owner = store.open(workspace.root(), &workspace.path()).unwrap();
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    let changed = store.edit(&peer.lease, 1, 1, "peer").unwrap();
    let first = store.input(&owner.lease, 1, 1, "first refused").unwrap();
    let old = store.review_recovery(&peer.lease, &owner.lease).unwrap();
    store.input(&owner.lease, 2, 1, "latest refused").unwrap();
    assert_eq!(store.input(&owner.lease, 1, 1, "first refused"), Ok(first));
    assert_eq!(
        &*store
            .review_recovery(&peer.lease, &owner.lease)
            .unwrap()
            .text,
        "latest refused"
    );
    store.close(&owner.lease).unwrap();
    assert_eq!(
        store.resolve_recovery(
            &peer.lease,
            2,
            changed.version,
            &owner.lease,
            old.recovery.revision,
            true
        ),
        Err(DocumentError::Conflict)
    );
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "peer");
    let current = store.review_recovery(&peer.lease, &owner.lease).unwrap();
    let dropped = store
        .resolve_recovery(
            &peer.lease,
            3,
            current.snapshot.version,
            &owner.lease,
            current.recovery.revision,
            false,
        )
        .unwrap();
    assert!(!dropped.retained);
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "peer");
    assert!(store.recoveries(&peer.lease).unwrap().is_empty());
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
}

#[test]
fn changed_model_and_foreign_document_refuse_recovery_resolution() {
    let workspace = Workspace::new();
    let other = Workspace::new();
    let store = Workspace::store();
    let owner = store.open(workspace.root(), &workspace.path()).unwrap();
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    let foreign = store.open(other.root(), &other.path()).unwrap();
    let changed = store.edit(&peer.lease, 1, 1, "peer").unwrap();
    store.input(&owner.lease, 1, 1, "refused").unwrap();
    store.close(&owner.lease).unwrap();
    let review = store.review_recovery(&peer.lease, &owner.lease).unwrap();
    assert!(matches!(
        store.review_recovery(&foreign.lease, &owner.lease),
        Err(DocumentError::NotFound)
    ));
    store
        .edit(&peer.lease, 2, changed.version, "later peer")
        .unwrap();
    assert_eq!(
        store.resolve_recovery(
            &peer.lease,
            3,
            review.snapshot.version,
            &owner.lease,
            review.recovery.revision,
            true
        ),
        Err(DocumentError::Conflict)
    );
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "later peer");
    assert_eq!(
        &*store
            .review_recovery(&peer.lease, &owner.lease)
            .unwrap()
            .text,
        "refused"
    );
}

#[test]
fn oversized_recovery_input_and_capacity_refusal_do_not_replace_retained_work() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let owner = store.open(workspace.root(), &workspace.path()).unwrap();
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    store.edit(&peer.lease, 1, 1, "peer").unwrap();
    store.input(&owner.lease, 1, 1, "retained").unwrap();
    assert_eq!(
        store.input(&owner.lease, 2, 1, &"x".repeat(super::TEXT_LIMIT + 1)),
        Err(DocumentError::TooLarge)
    );
    assert_eq!(
        store.input(&owner.lease, 3, 1, "bad\0text"),
        Err(DocumentError::Unsupported)
    );
    assert_eq!(
        store.input(&owner.lease, 4, 100, "future observation"),
        Err(DocumentError::Conflict)
    );
    assert_eq!(
        &*store
            .review_recovery(&peer.lease, &owner.lease)
            .unwrap()
            .text,
        "retained"
    );
    for _ in 1..super::RECOVERY_LIMIT {
        let another = store.open(workspace.root(), &workspace.path()).unwrap();
        store
            .input(&another.lease, 1, 1, "retained sibling")
            .unwrap();
        store.close(&another.lease).unwrap();
    }
    let extra = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_eq!(
        store.input(&extra.lease, 1, 1, "over capacity"),
        Err(DocumentError::Capacity)
    );
    assert_eq!(
        store.recoveries(&peer.lease).unwrap().len(),
        super::RECOVERY_LIMIT
    );
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "peer");
}

#[test]
fn a_live_foreign_owner_fences_recovery_until_release_but_can_resolve_its_own_input() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let owner = store.open(workspace.root(), &workspace.path()).unwrap();
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    let changed = store.edit(&peer.lease, 1, 1, "peer").unwrap();
    store.input(&owner.lease, 1, 1, "refused").unwrap();
    let review = store.review_recovery(&peer.lease, &owner.lease).unwrap();
    assert!(!review.recovery.can_resolve);
    assert_eq!(
        store.resolve_recovery(
            &peer.lease,
            2,
            changed.version,
            &owner.lease,
            review.recovery.revision,
            false
        ),
        Err(DocumentError::Conflict)
    );
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "peer");
    let own = store.review_recovery(&owner.lease, &owner.lease).unwrap();
    assert!(own.recovery.can_resolve);
    let restored = store
        .resolve_recovery(
            &owner.lease,
            2,
            own.snapshot.version,
            &owner.lease,
            own.recovery.revision,
            true,
        )
        .unwrap();
    let newer = store
        .edit(&peer.lease, 3, restored.version, "newer peer")
        .unwrap();
    store.input(&owner.lease, 3, 1, "latest refused").unwrap();
    store.close(&owner.lease).unwrap();
    let released = store.review_recovery(&peer.lease, &owner.lease).unwrap();
    assert!(released.recovery.can_resolve);
    store
        .resolve_recovery(
            &peer.lease,
            4,
            newer.version,
            &owner.lease,
            released.recovery.revision,
            false,
        )
        .unwrap();
    assert!(store.recoveries(&peer.lease).unwrap().is_empty());
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "newer peer");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
}

#[test]
fn retained_input_shares_the_native_memory_budget_and_explicit_drop_releases_capacity() {
    let workspace = Workspace::new();
    let store = Workspace::store();
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    let changed = store.edit(&peer.lease, 1, 1, "peer").unwrap();
    let payload = "x".repeat(super::TEXT_LIMIT);
    for _ in 0..6 {
        let owner = store.open(workspace.root(), &workspace.path()).unwrap();
        assert!(store.input(&owner.lease, 1, 1, &payload).unwrap().retained);
        store.close(&owner.lease).unwrap();
    }
    let extra = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_eq!(
        store.input(&extra.lease, 1, 1, &payload),
        Err(DocumentError::Capacity)
    );
    let drafts = store.recoveries(&peer.lease).unwrap();
    assert_eq!(drafts.len(), 6);
    store
        .resolve_recovery(
            &peer.lease,
            2,
            changed.version,
            &drafts[0].id,
            drafts[0].revision,
            false,
        )
        .unwrap();
    assert!(store.input(&extra.lease, 2, 1, &payload).unwrap().retained);
    assert_eq!(store.recoveries(&peer.lease).unwrap().len(), 6);
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "peer");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
}

#[test]
fn delegated_read_access_cannot_mutate_recover_or_address_another_lease() {
    use super::DocumentPermissions;
    let workspace = Workspace::new();
    let store = Workspace::store();
    let parent = store.open(workspace.root(), &workspace.path()).unwrap();
    store.input(&parent.lease, 1, 1, "unsaved").unwrap();
    let child = store
        .delegate(
            &parent.lease,
            DocumentPermissions {
                edit: false,
                save: false,
            },
        )
        .unwrap();
    assert_eq!(child.lease.len(), 64);
    assert_eq!(&*child.snapshot.text, "unsaved");
    assert!(store.allows_bearer(&child.lease));
    assert!(!store.allows_bearer(&parent.lease));
    assert_eq!(
        store.edit(&child.lease, 1, 2, "denied"),
        Err(DocumentError::NotFound)
    );
    assert_eq!(store.save(&child.lease, 1, 2), Err(DocumentError::NotFound));
    assert_eq!(
        store.input(&child.lease, 1, 2, "denied"),
        Err(DocumentError::NotFound)
    );
    assert_eq!(
        store.discard(&child.lease, 1, 2),
        Err(DocumentError::NotFound)
    );
    assert!(matches!(
        store.recoveries(&child.lease),
        Err(DocumentError::NotFound)
    ));
    assert!(matches!(
        store.request_scoped(
            &child.lease,
            DocumentRequest::Read {
                lease: parent.lease.clone()
            }
        ),
        Err(DocumentError::NotFound)
    ));
    assert!(matches!(
        store.request_scoped(
            &child.lease,
            DocumentRequest::Open {
                root: workspace.root().into(),
                path: workspace.path()
            }
        ),
        Err(DocumentError::NotFound)
    ));
    assert_eq!(&*store.read(&parent.lease).unwrap().text, "unsaved");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
}

#[test]
fn delegated_edit_and_save_are_separate_and_children_cannot_expand_access() {
    use super::DocumentPermissions;
    let workspace = Workspace::new();
    let store = Workspace::store();
    let parent = store.open(workspace.root(), &workspace.path()).unwrap();
    assert!(matches!(
        store.delegate(
            &parent.lease,
            DocumentPermissions {
                edit: false,
                save: true
            }
        ),
        Err(DocumentError::Invalid)
    ));
    let editor = store
        .delegate(
            &parent.lease,
            DocumentPermissions {
                edit: true,
                save: false,
            },
        )
        .unwrap();
    let receipt = store.edit(&editor.lease, 1, 1, "edited").unwrap();
    assert_eq!(
        store.save(&editor.lease, 2, receipt.version),
        Err(DocumentError::NotFound)
    );
    assert!(matches!(
        store.delegate(
            &editor.lease,
            DocumentPermissions {
                edit: true,
                save: true
            }
        ),
        Err(DocumentError::NotFound)
    ));
    let reader = store
        .delegate(
            &editor.lease,
            DocumentPermissions {
                edit: false,
                save: false,
            },
        )
        .unwrap();
    assert!(matches!(
        store.delegate(
            &reader.lease,
            DocumentPermissions {
                edit: true,
                save: false
            }
        ),
        Err(DocumentError::NotFound)
    ));
    assert_eq!(&*store.read(&reader.lease).unwrap().text, "edited");
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
    let writer = store
        .delegate(
            &parent.lease,
            DocumentPermissions {
                edit: true,
                save: true,
            },
        )
        .unwrap();
    store.save(&writer.lease, 1, receipt.version).unwrap();
    assert_eq!(fs::read(workspace.path()).unwrap(), b"edited");
}

#[test]
fn closing_an_owner_revokes_its_descendants_without_closing_independent_peers() {
    use super::DocumentPermissions;
    let workspace = Workspace::new();
    let store = Workspace::store();
    let parent = store.open(workspace.root(), &workspace.path()).unwrap();
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    let rights = DocumentPermissions {
        edit: true,
        save: false,
    };
    let first = store.delegate(&parent.lease, rights).unwrap();
    let sibling = store.delegate(&parent.lease, rights).unwrap();
    let child = store.delegate(&first.lease, rights).unwrap();
    store.edit(&child.lease, 1, 1, "kept unsaved").unwrap();
    store.close(&first.lease).unwrap();
    assert!(matches!(
        store.read(&child.lease),
        Err(DocumentError::NotFound)
    ));
    assert_eq!(
        store.edit(&child.lease, 1, 1, "kept unsaved"),
        Err(DocumentError::NotFound)
    );
    assert_eq!(&*store.read(&sibling.lease).unwrap().text, "kept unsaved");
    store.close(&parent.lease).unwrap();
    assert!(!store.allows_bearer(&sibling.lease));
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "kept unsaved");
    let replacement = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_ne!(parent.lease, replacement.lease);
    assert!(matches!(
        store.read(&sibling.lease),
        Err(DocumentError::NotFound)
    ));
    assert_eq!(fs::read(workspace.path()).unwrap(), b"disk text");
}

#[test]
fn delegated_lease_depth_is_bounded_and_missing_permission_fields_fail_closed() {
    use super::DocumentPermissions;
    let workspace = Workspace::new();
    let store = Workspace::store();
    let parent = store.open(workspace.root(), &workspace.path()).unwrap();
    let mut current = parent.lease.clone();
    for _ in 0..super::DELEGATION_DEPTH {
        current = store
            .delegate(
                &current,
                DocumentPermissions {
                    edit: false,
                    save: false,
                },
            )
            .unwrap()
            .lease;
    }
    assert!(matches!(
        store.delegate(
            &current,
            DocumentPermissions {
                edit: false,
                save: false
            }
        ),
        Err(DocumentError::Capacity)
    ));
    assert!(serde_json::from_value::<DocumentRequest>(serde_json::json!({"operation":"delegate","lease":parent.lease,"permissions":{"edit":true}})).is_err());
    assert!(serde_json::from_value::<DocumentRequest>(serde_json::json!({"operation":"delegate","lease":parent.lease,"permissions":{"edit":true,"save":false,"execute":true}})).is_err());
    store.close(&parent.lease).unwrap();
    assert!(matches!(store.read(&current), Err(DocumentError::NotFound)));
}

#[test]
fn delegated_capacity_cannot_exhaust_all_shell_slots_and_revoke_releases_it() {
    use super::DocumentPermissions;
    let workspace = Workspace::new();
    let store = Workspace::store();
    let parent = store.open(workspace.root(), &workspace.path()).unwrap();
    let rights = DocumentPermissions {
        edit: false,
        save: false,
    };
    let mut grants = Vec::new();
    for _ in 0..super::DELEGATED_LEASE_LIMIT {
        grants.push(store.delegate(&parent.lease, rights).unwrap().lease);
    }
    assert!(matches!(
        store.delegate(&parent.lease, rights),
        Err(DocumentError::Capacity)
    ));
    let peer = store.open(workspace.root(), &workspace.path()).unwrap();
    assert_eq!(&*store.read(&peer.lease).unwrap().text, "disk text");
    store.close(&grants[0]).unwrap();
    assert!(store.delegate(&parent.lease, rights).is_ok());
    store.close(&parent.lease).unwrap();
    assert!(store.delegate(&peer.lease, rights).is_ok());
}

#[test]
fn delegated_close_can_release_resources_after_the_document_binding_is_missing() {
    use super::DocumentPermissions;
    let workspace = Workspace::new();
    let store = Workspace::store();
    let parent = store.open(workspace.root(), &workspace.path()).unwrap();
    let grant = store
        .delegate(
            &parent.lease,
            DocumentPermissions {
                edit: false,
                save: false,
            },
        )
        .unwrap();
    fs::remove_file(workspace.path()).unwrap();
    assert!(matches!(
        store.read(&grant.lease),
        Err(DocumentError::NotFound)
    ));
    store
        .request_scoped(
            &grant.lease,
            DocumentRequest::Close {
                lease: grant.lease.clone(),
            },
        )
        .unwrap();
    assert!(!store.allows_bearer(&grant.lease));
    store.close(&parent.lease).unwrap();
}
