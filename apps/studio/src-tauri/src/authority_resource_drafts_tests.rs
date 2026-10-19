#![cfg(test)]

use super::*;

fn campaign(store: &DraftStore) {
    let org = Uuid::now_v7();
    let owner = DraftStore::scope("member-1", org, "use/a3s/digital-twin", "editor");
    assert_eq!(store.read(&owner, "session").unwrap().revision, 0);
    let operation = Uuid::now_v7();
    let commit = || Commit {
        expected_revision: 0,
        operation_id: operation,
        content: Some("场景 🦾\nscene \"kept\" { revision = 1 }\t\0\u{0008}\\".into()),
    };
    assert_eq!(
        store.commit(&owner, "session", commit()).unwrap().revision,
        1
    );
    assert_eq!(
        store.commit(&owner, "session", commit()).unwrap().revision,
        1
    );
    assert_eq!(
        store.read(&owner, "session").unwrap().content,
        commit().content
    );
    let mut changed = commit();
    changed.content = Some("different input".into());
    assert_eq!(
        store.commit(&owner, "session", changed).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    let mut stale = commit();
    stale.operation_id = Uuid::now_v7();
    assert_eq!(
        store.commit(&owner, "session", stale).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    for scope in [
        DraftStore::scope("member-2", org, "use/a3s/digital-twin", "editor"),
        DraftStore::scope("member-1", Uuid::now_v7(), "use/a3s/digital-twin", "editor"),
        DraftStore::scope("member-1", org, "use/other/package", "editor"),
        DraftStore::scope("member-1", org, "use/a3s/digital-twin", "workbench"),
    ] {
        assert!(store.read(&scope, "session").unwrap().content.is_none());
    }
    assert!(store.read(&owner, "../outside").is_err());
    assert!(store.read("../outside", "session").is_err());
}

#[test]
fn authoring_recovery_memory_and_disk_share_the_same_contract() {
    campaign(&DraftStore::new(None));
    let root = tempfile::tempdir().unwrap();
    campaign(&DraftStore::new(Some(root.path().into())));
}

#[test]
fn a_restarted_store_replays_the_intent_and_refuses_corrupt_recovery() {
    let root = tempfile::tempdir().unwrap();
    let scope = DraftStore::scope("member-1", Uuid::now_v7(), "use/a3s/digital-twin", "editor");
    let operation = Uuid::now_v7();
    let input = || Commit {
        expected_revision: 0,
        operation_id: operation,
        content: Some("{\"saveIntent\":\"immutable operation\"}".into()),
    };
    DraftStore::new(Some(root.path().into()))
        .commit(&scope, "session", input())
        .unwrap();
    let restarted = DraftStore::new(Some(root.path().into()));
    assert_eq!(
        restarted.read(&scope, "session").unwrap().content,
        input().content
    );
    assert_eq!(
        restarted
            .commit(&scope, "session", input())
            .unwrap()
            .revision,
        1
    );
    let file = root
        .path()
        .join(&scope)
        .join(format!("{:x}.acl", Sha256::digest(b"session")));
    fs::write(file, "broken ACL").unwrap();
    assert!(restarted.read(&scope, "session").is_err());
}
