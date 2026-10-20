#![cfg(test)]

use super::*;

#[cfg(feature = "tenant-aster")]
#[test]
fn default_aster_custom_provider_create_rename_delete_is_owned_by_its_profile() {
    let mut store = ProviderStore::open("aster");
    let id = "12345678-1234-4234-8234-123456789abc";
    store
        .update(
            id,
            br#"{"name":"Synthetic local","enabled":true,"baseUrl":"http://127.0.0.1:18431/v1"}"#,
        )
        .unwrap();
    store
        .upsert_model(id, br#"{"id":"gpt-test","enabled":true}"#)
        .unwrap();
    assert!(store.execution_credentials(id, None).is_some());
    store.update(id, br#"{"name":"Renamed local"}"#).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&store.detail(id).unwrap()).unwrap()["name"],
        "Renamed local"
    );
    store.update(id, br#"{"deleted":true}"#).unwrap();
    assert!(matches!(store.detail(id), Err(Fail::Missing)));
    assert!(store.execution_credentials(id, None).is_none());
    assert!(
        ProviderStore::open("toyou")
            .update(id, br#"{"name":"Forbidden"}"#)
            .is_err()
    );
}

#[test]
#[cfg(feature = "tenant-aster")]
fn default_aster_local_provider_accepts_byok_without_a_member_session() {
    let mut store = ProviderStore::open("aster");
    assert!(store.update("openai", br#"{"enabled":true,"baseUrl":"http://127.0.0.1:18431/v1","apiKey":"synthetic-local-only"}"#).is_ok());
    assert!(
        store
            .upsert_model(
                "openai",
                br#"{"id":"synthetic-local-model","enabled":true}"#
            )
            .is_ok()
    );
    assert!(store.eligible(&ModelRef {
        provider_id: "openai".into(),
        model_id: "synthetic-local-model".into()
    }));
}

#[cfg(feature = "tenant-aster")]
pub(crate) fn local_fixture(base: &str) -> ProviderStore {
    let mut store = ProviderStore::open("aster");
    store
        .update(
            "openai",
            json!({"enabled":true,"baseUrl":base,"apiKey":"synthetic-local-only"})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    store
        .upsert_model("openai", br#"{"id":"gpt-test","enabled":true}"#)
        .unwrap();
    store
}

pub(crate) fn managed_fixture(base: &str) -> ProviderStore {
    let mut store = ProviderStore::open("toyou");
    store.saved.insert(
        "openai".into(),
        SavedProvider {
            enabled: true,
            api_key: "synthetic-local-only".into(),
            base_url: base.into(),
            models: vec![SavedModel {
                id: "gpt-test".into(),
                name: "Synthetic".into(),
                enabled: true,
                chat: true,
            }],
        },
    );
    assert!(store.execution_credentials("openai", None).is_none());
    store
}

#[test]
fn default_aster_local_capability_cannot_relax_toyou_or_gateway_credentials() {
    let body = br#"{"enabled":true,"baseUrl":"http://127.0.0.1:18431/v1","apiKey":"synthetic-local-only"}"#;
    for tenant in ["toyou", "towork-cua", "a3s"] {
        let mut store = ProviderStore::open(tenant);
        assert!(matches!(
            store.update("openai", body),
            Err(Fail::Bad("managed_gateway"))
        ));
    }
    let mut aster = ProviderStore::open("aster");
    assert!(matches!(
        aster.update("a3s", body),
        Err(Fail::Bad("managed_gateway"))
    ));
}
