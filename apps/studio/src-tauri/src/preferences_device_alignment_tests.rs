#![cfg(test)]

use super::*;
use serde_json::json;

const A: &str = "11111111-1111-4111-8111-111111111111";
const B: &str = "22222222-2222-4222-8222-222222222222";

#[test]
fn native_alignment_model_preferences_are_device_scoped_and_other_values_are_private() {
    let root = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(root.path().to_path_buf());
    let before_b = store.read(B).unwrap();
    store
        .commit(&Commit {
            scope: A.into(),
            expected_revision: 0,
            updates: BTreeMap::from([
                ("chat.default_model_id".into(), json!("a3s::synthetic")),
                (
                    "feature.quick_assistant.model_id".into(),
                    json!("a3s::quick"),
                ),
                ("feature.translate.model_id".into(), json!("a3s::translate")),
                ("ui.theme_mode".into(), json!("dark")),
            ]),
        })
        .unwrap();
    let b = store.read(B).unwrap();
    assert_eq!(b.values["chat.default_model_id"], "a3s::synthetic");
    assert_eq!(b.values["feature.quick_assistant.model_id"], "a3s::quick");
    assert_eq!(b.values["feature.translate.model_id"], "a3s::translate");
    assert!(!b.values.contains_key("ui.theme_mode"));
    assert!(matches!(
        store.commit(&Commit {
            scope: B.into(),
            expected_revision: before_b.revision,
            updates: BTreeMap::from([("chat.default_model_id".into(), json!("a3s::stale"))])
        }),
        Err(PreferenceError::Conflict { .. })
    ));
    assert_eq!(store.read(A).unwrap().values["ui.theme_mode"], "dark");
}

#[test]
fn native_alignment_reserved_device_scope_cannot_be_opened_as_a_member() {
    let root = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(root.path().to_path_buf());
    let reserved = "00000000-0000-0000-0000-000000000000";
    assert!(matches!(store.read(reserved), Err(PreferenceError::Denied)));
    assert!(matches!(
        store.commit(&Commit {
            scope: reserved.into(),
            expected_revision: 0,
            updates: BTreeMap::new()
        }),
        Err(PreferenceError::Denied)
    ));
}

#[test]
fn native_alignment_mixed_commit_overflow_is_refused_before_either_write() {
    let root = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(root.path().to_path_buf());
    let member = Snapshot {
        scope: A.into(),
        revision: u64::MAX - 1,
        values: BTreeMap::new(),
    };
    fs::write(
        root.path().join(format!("{A}.acl")),
        encode(&member).unwrap(),
    )
    .unwrap();
    let before = store.read(A).unwrap();
    assert!(matches!(
        store.commit(&Commit {
            scope: A.into(),
            expected_revision: before.revision,
            updates: BTreeMap::from([
                ("chat.default_model_id".into(), json!("a3s::fixture")),
                ("ui.theme_mode".into(), json!("dark"))
            ])
        }),
        Err(PreferenceError::Invalid)
    ));
    assert_eq!(
        store.read(A).unwrap(),
        before,
        "failed commit is entirely unchanged"
    );
}

#[test]
fn native_alignment_exhausted_member_revision_does_not_write_device_values() {
    let root = tempfile::tempdir().unwrap();
    let store = PreferenceStore::new(root.path().to_path_buf());
    let member = Snapshot {
        scope: A.into(),
        revision: u64::MAX,
        values: BTreeMap::new(),
    };
    fs::write(
        root.path().join(format!("{A}.acl")),
        encode(&member).unwrap(),
    )
    .unwrap();
    let before = store.read(A).unwrap();
    assert!(
        store
            .commit(&Commit {
                scope: A.into(),
                expected_revision: before.revision,
                updates: BTreeMap::from([
                    ("chat.default_model_id".into(), json!("a3s::fixture")),
                    ("ui.theme_mode".into(), json!("dark"))
                ])
            })
            .is_err()
    );
    assert_eq!(store.read(A).unwrap(), before);
}
