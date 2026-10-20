#![cfg(test)]

use super::*;
use crate::preferences::{Commit, PreferenceStore};
use std::collections::BTreeMap;

const SCOPE: &str = "11111111-1111-4111-8111-111111111111";

fn choices(model: &str) -> Value {
    let model = json!({"providerId":"openai","modelId":model});
    json!({"defaultModel":model,"quickModel":model,"translateModel":model})
}

fn fixture(path: PathBuf) -> ProviderStore {
    let mut store = ProviderStore::open_at(Some(path));
    store.update("openai", br#"{"enabled":true}"#).unwrap();
    for id in ["old-model", "new-model", "quick-model", "translate-model"] {
        store
            .upsert_model("openai", json!({"id":id}).to_string().as_bytes())
            .unwrap();
    }
    store
        .set_defaults(choices("old-model").to_string().as_bytes())
        .unwrap();
    let preferences = PreferenceStore::new(
        store
            .path
            .as_ref()
            .unwrap()
            .parent()
            .unwrap()
            .join("preferences"),
    );
    preferences
        .commit(&Commit {
            scope: SCOPE.into(),
            expected_revision: preferences.read(SCOPE).unwrap().revision,
            updates: BTreeMap::from([
                ("chat.default_model_id".into(), json!("openai::old-model")),
                (
                    "feature.quick_assistant.model_id".into(),
                    json!("openai::old-model"),
                ),
                (
                    "feature.translate.model_id".into(),
                    json!("openai::old-model"),
                ),
                ("ui.theme_mode".into(), json!("dark")),
            ]),
        })
        .unwrap();
    store
}

#[test]
fn native_alignment_defaults_update_all_three_preferences_after_reopen() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("providers.json");
    let mut store = fixture(path.clone());
    let next = json!({
        "defaultModel":{"providerId":"openai","modelId":"new-model"},
        "quickModel":{"providerId":"openai","modelId":"quick-model"},
        "translateModel":{"providerId":"openai","modelId":"translate-model"},
    });
    store.set_defaults(next.to_string().as_bytes()).unwrap();
    assert_eq!(json!(ProviderStore::open_at(Some(path)).defaults), next);
    let preferences = PreferenceStore::new(root.path().join("preferences"))
        .read(SCOPE)
        .unwrap();
    for (key, model) in [
        ("chat.default_model_id", "new-model"),
        ("feature.quick_assistant.model_id", "quick-model"),
        ("feature.translate.model_id", "translate-model"),
    ] {
        assert_eq!(preferences.values[key], format!("openai::{model}"), "{key}");
    }
    assert_eq!(preferences.values["ui.theme_mode"], "dark");
}

#[test]
fn native_alignment_unavailable_preferences_roll_back_defaults() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("providers.json");
    let mut store = fixture(path.clone());
    fs::rename(
        root.path().join("preferences"),
        root.path().join("preferences-backup"),
    )
    .unwrap();
    fs::write(root.path().join("preferences"), "blocked").unwrap();
    assert!(
        store
            .set_defaults(choices("new-model").to_string().as_bytes())
            .is_err()
    );
    assert_eq!(json!(store.defaults), choices("old-model"));
    assert_eq!(
        json!(ProviderStore::open_at(Some(path)).defaults),
        choices("old-model")
    );
}

#[test]
fn native_alignment_provider_write_failure_rolls_back_in_memory_defaults() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("providers.json");
    let mut store = fixture(path.clone());
    let preferences = PreferenceStore::new(root.path().join("preferences"));
    let before = preferences.read(SCOPE).unwrap();
    fs::create_dir(path.with_extension("json.tmp")).unwrap();
    assert!(
        store
            .set_defaults(choices("new-model").to_string().as_bytes())
            .is_err()
    );
    assert_eq!(json!(store.defaults), choices("old-model"));
    assert_eq!(
        json!(ProviderStore::open_at(Some(path)).defaults),
        choices("old-model")
    );
    assert_eq!(preferences.read(SCOPE).unwrap(), before);
}

#[test]
fn native_alignment_session_identity_is_explicit_and_invalid_instance_has_no_fallback() {
    assert_eq!(
        session_scope_from(Ok("towork".into()), Ok(Some("proof".into()))),
        Some("towork-proof".into())
    );
    assert_eq!(
        session_scope_from(Ok("towork".into()), Ok(None)),
        Some("towork".into())
    );
    assert!(session_scope_from(Err(std::io::Error::other("missing tenant")), Ok(None)).is_none());
    assert!(
        session_scope_from(
            Ok("towork".into()),
            Err(std::io::Error::other("invalid instance"))
        )
        .is_none()
    );
    assert!(session_slug_for("towork", Some("../primary")).is_none());
}

#[test]
fn native_alignment_defaults_do_not_admit_managed_or_disabled_models() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("providers.json");
    let mut store = fixture(path);
    let before = serde_json::to_value(&store.defaults).unwrap();
    let preferences = PreferenceStore::new(root.path().join("preferences"));
    let before_preferences = preferences.read(SCOPE).unwrap();
    for model in [
        json!({"providerId":"a3s","modelId":"gpt-studio-fixture"}),
        json!({"providerId":"openai","modelId":"not-admitted"}),
    ] {
        let request = json!({"defaultModel":model,"quickModel":model,"translateModel":model});
        assert!(store.set_defaults(request.to_string().as_bytes()).is_err());
        assert_eq!(serde_json::to_value(&store.defaults).unwrap(), before);
        assert_eq!(preferences.read(SCOPE).unwrap(), before_preferences);
    }
    assert!(signed_in_gateway_credentials("openai", Some("synthetic")).is_none());
}
