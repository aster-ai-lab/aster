#![cfg(test)]

use super::*;

// Catch a review worker dropping the selection that the real ACP requires.
#[test]
fn document_worker_keeps_the_published_selection_and_harness() {
    let root = std::env::temp_dir().join(format!("studio-review-binding-{}", uuid::Uuid::now_v7()));
    let store = a3s_use_reconcile::ReconcileStore::open(&root).unwrap();
    let selected = store
        .publish_selection_sync("studio-installed-plugins", None, &[])
        .unwrap();
    let binding = super::super::plugin_selection::Binding {
        store: root.clone(),
        cursor: selected.cursor(),
        mcp_names: vec![],
    };
    let params = session_new_params(
        "/workspace",
        &[],
        &[],
        None,
        &["system".into()],
        Some(&binding),
    );
    let admitted: a3s_use_reconcile::SelectionCursor =
        serde_json::from_value(params["_meta"]["a3sPluginSelection"].clone()).unwrap();
    assert_eq!(admitted, selected.cursor());
    assert_eq!(
        params["_meta"]["a3sHarness"]["components"],
        json!(["system"])
    );
    assert!(params["_meta"].get("a3sTools").is_none());
    std::fs::remove_dir_all(root).unwrap();
}
