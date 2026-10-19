#![cfg(test)]

use super::super::LocalCapabilities;
use super::*;

// Catch a source-only scaffold passing while the built archive cannot expose both entry points.
#[test]
fn digital_twin_archive_exposes_activitybar_and_composer_editor_with_verified_assets() {
    let root = std::env::temp_dir().join(format!("a3s-digital-twin-{}", uuid::Uuid::now_v7()));
    let local = LocalCapabilities::open_at(Some(root.join("capabilities")));
    let bytes = super::tests::workbench_bundle("digital-twin");
    let digest = super::super::digest_of(&bytes);
    let entry = json!({"slug":"a3s-digital-twin", "kind":"plugin", "installed":true, "install":{
        "type":"plugin", "packageType":"aix", "available":true, "packageId":"use/a3s/digital-twin",
        "digest":digest, "size":bytes.len() }});
    local.reconcile(&[entry], |_| Ok(bytes.clone())).unwrap();
    let catalogue: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    assert_eq!(catalogue["diagnostics"], json!([]), "{catalogue}");
    let package = &catalogue["packages"][0];
    assert_eq!(package["slug"], "a3s-digital-twin");
    assert_eq!(package["containers"][0]["id"], "twin");
    assert_eq!(package["containers"][0]["location"], "activitybar");
    assert_eq!(package["composerModes"][0]["agent"], "builder");
    assert_eq!(package["composerModes"][0]["editor"], "editor");
    let views = package["views"].as_array().unwrap();
    assert_eq!(views.len(), 2);
    for view in views {
        let request = json!({"protocolVersion":VIEW_PROTOCOL, "slug":"a3s-digital-twin", "surface":view["id"], "revision":package["revision"],
            "requestId": if view["id"] == "workbench" { "b".repeat(64) } else { "c".repeat(64) }, "activationId":"d".repeat(64), "instanceId":if view["id"] == "workbench" { "e".repeat(64) } else { "f".repeat(64) }, "expiresAt":now_ms().unwrap() + 30_000});
        let opened: serde_json::Value = serde_json::from_slice(
            &local
                .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        let token = opened["token"].as_str().unwrap();
        for (path, mime) in [
            ("ui/editor/index.html", "text/html; charset=utf-8"),
            ("ui/editor/app.js", "text/javascript; charset=utf-8"),
            ("ui/editor/app.css", "text/css; charset=utf-8"),
            (
                "ui/editor/fonts/ubuntu-latin-400-normal.woff2",
                "font/woff2",
            ),
        ] {
            let (actual_mime, actual) = local.plugin_view_asset("member", token, path).unwrap();
            assert_eq!(actual_mime, mime);
            let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../use-registry/packages/digital-twin")
                .join(path);
            assert_eq!(actual, fs::read(source).unwrap());
        }
        assert!(local.close_plugin_view("other-member", token).is_err());
        assert!(
            local
                .plugin_view_asset("member", token, "src/app.jsx")
                .is_err()
        );
        local.close_plugin_view("member", token).unwrap();
        assert!(
            local
                .plugin_view_asset("member", token, "ui/editor/app.js")
                .is_err()
        );
        assert!(
            local
                .plugin_view_asset("other-member", token, "ui/editor/app.js")
                .is_err()
        );
    }
    if let Ok(output) = std::env::var("A3S_CLOUD_TEST_TWIN_ARCHIVE_OUTPUT") {
        fs::write(output, &bytes).unwrap();
    }
    local
        .reconcile(&[], |_| unreachable!("uninstall does not download"))
        .unwrap();
    let removed: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    assert_eq!(removed["packages"], json!([]));
    drop(local);
    fs::remove_dir_all(root).unwrap();
}
