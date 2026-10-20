#![cfg(test)]

use super::*;
use serde_json::json;

#[test]
fn default_aster_anonymous_preferences_follow_the_compiled_capability() {
    let root = tempfile::tempdir().unwrap();
    let member = Arc::new(crate::member_session::MemberSessionStore::open_at(None));
    assert!(!member.has_session());
    let host = PreferenceHost::for_tenant(
        root.path().into(),
        member.clone(),
        "synthetic-native-host".into(),
        "aster",
    );
    let read = host.scoped(|store, scope| store.read(scope));
    if cfg!(feature = "tenant-toyou") {
        assert!(matches!(read, Err(PreferenceError::Denied)));
        return;
    }
    let initial = read.unwrap();
    assert_eq!(initial.scope, "00000000-0000-4000-8000-000000000001");
    let input = Commit {
        scope: initial.scope.clone(),
        expected_revision: initial.revision,
        updates: [("app.harness.assemble".into(), json!("lean"))].into(),
    };
    let foreign = Commit {
        scope: "11111111-1111-4111-8111-111111111111".into(),
        expected_revision: 0,
        updates: input.updates.clone(),
    };
    assert!(matches!(
        host.commit_scoped(&foreign),
        Err(PreferenceError::Denied)
    ));
    host.commit_scoped(&input).unwrap();
    let device = PreferenceStore::new(root.path().into());
    device
        .set_device(
            [
                ("chat.default_model_id".into(), json!("openai::gpt-test")),
                (
                    "feature.quick_assistant.model_id".into(),
                    json!("openai::gpt-test"),
                ),
                (
                    "feature.translate.model_id".into(),
                    json!("openai::gpt-test"),
                ),
            ]
            .into(),
        )
        .unwrap();
    let again = PreferenceHost::for_tenant(
        root.path().into(),
        member.clone(),
        "synthetic-native-host".into(),
        "aster",
    );
    let snapshot = again.scoped(|store, scope| store.read(scope)).unwrap();
    assert_eq!(snapshot.values["app.harness.assemble"], "lean");
    assert_eq!(snapshot.values["chat.default_model_id"], "openai::gpt-test");
    assert_eq!(
        device
            .read(&initial.scope)
            .unwrap()
            .values
            .get("app.harness.assemble"),
        None
    );
    assert!(!member.has_session());
    assert!(member.token().is_none());
    assert!(member.member_id().is_none());
    assert!(member.with_resource_session(|_, _| Ok(())).is_err());
}

#[test]
fn default_aster_toyou_anonymous_preferences_remain_denied() {
    let root = tempfile::tempdir().unwrap();
    let member = Arc::new(crate::member_session::MemberSessionStore::open_at(None));
    let host = PreferenceHost::for_tenant(
        root.path().into(),
        member,
        "synthetic-native-host".into(),
        "toyou",
    );
    assert!(matches!(
        host.scoped(|store, scope| store.read(scope)),
        Err(PreferenceError::Denied)
    ));
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
