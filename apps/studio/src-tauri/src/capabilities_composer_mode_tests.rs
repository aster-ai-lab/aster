#![cfg(test)]

use super::super::LocalCapabilities;
use super::*;

// Catch native discovery losing mode metadata or retaining modes after their package is removed.
#[test]
fn composer_modes_follow_the_verified_installed_package_lifetime() {
    let root = std::env::temp_dir().join(format!("a3s-composer-mode-{}", uuid::Uuid::now_v7()));
    let local = LocalCapabilities::open_at(Some(root.join("capabilities")));
    let bytes = super::tests::workbench_bundle("probe-composer-mode");
    let digest = super::super::digest_of(&bytes);
    let entry = json!({"slug":"probe-composer-mode", "kind":"plugin", "installed":true, "install":{
        "type":"plugin", "packageType":"aix", "available":true, "packageId":"use/a3s/probe-composer-mode",
        "digest":digest, "size":bytes.len() }});
    local.reconcile(&[entry], |_| Ok(bytes.clone())).unwrap();
    let catalogue: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    let package = &catalogue["packages"][0];
    assert_eq!(package["composerModes"][0]["id"], "build");
    assert_eq!(package["composerModes"][0]["agent"], "builder");
    assert_eq!(package["composerModes"][0]["editor"], "editor");
    assert_eq!(
        package["composerModes"][0]["context"],
        json!(["workspace", "document"])
    );
    assert_eq!(package["composerModes"][1]["scope"], "session");
    assert_eq!(
        catalogue["extensionPoints"]["composerMode"]["declaration"],
        "composer_mode"
    );
    assert_eq!(
        catalogue["extensionPoints"]["composerMode"]["presentations"],
        json!([])
    );
    if let Ok(path) = std::env::var("A3S_CLOUD_TEST_AIX_MODE_CATALOGUE_OUTPUT") {
        fs::write(path, serde_json::to_vec_pretty(&catalogue).unwrap()).unwrap();
    }
    let store = ReconcileStore::open(root.join("package-generations")).unwrap();
    let publication = store
        .current_sync("use/a3s/probe-composer-mode")
        .unwrap()
        .unwrap();
    let view = publication
        .surfaces
        .iter()
        .find(|surface| surface.id == "editor")
        .unwrap();
    fs::write(view.entry.as_ref().unwrap(), "<script>tampered</script>").unwrap();
    let damaged: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    assert_eq!(damaged["packages"], json!([]));
    assert_eq!(
        damaged["diagnostics"],
        json!([{"slug":"probe-composer-mode", "reason":"package-content-refused"}])
    );
    local
        .reconcile(&[], |_| unreachable!("no download on uninstall"))
        .unwrap();
    let removed: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    assert_eq!(removed["packages"], json!([]));
    drop(local);
    fs::remove_dir_all(root).unwrap();
}
