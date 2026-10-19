#![cfg(test)]

use super::*;
use a3s_use_reconcile::{Payload, Publication, ReconcileStore};

fn dependency_publication(
    store: &ReconcileStore,
    package_id: &str,
    version: &str,
    dependencies: &[(&str, &str)],
) -> Publication {
    let dependency_blocks = dependencies
        .iter()
        .map(|(id, requirement)| format!("dependency \"{id}\" {{ version = \"{requirement}\" }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let (_, local_id) = package_id.rsplit_once('/').unwrap();
    let manifest = format!(
        "extension \"{package_id}\" {{ schema_version = 3 version = \"{version}\" requires_use = \">=0.3.0, <0.4.0\" route = \"{local_id}\" actions = [\"read\"] repository {{ url = \"https://example.org/proof/{local_id}\" }} {dependency_blocks} ui \"home\" {{ entry = \"ui/index.html\" point = \"studio.editor\" }} }}"
    );
    let files = [
        (MANIFEST.to_owned(), manifest.into_bytes()),
        ("README.md".to_owned(), b"# Dependency fixture".to_vec()),
        ("ui/index.html".to_owned(), b"<html/>".to_vec()),
    ];
    let spec = super::package_spec(&files, &format!("use/{package_id}")).unwrap();
    store.apply_sync(&spec).unwrap()
}

#[test]
fn selected_aix_dependencies_require_pinned_versions_and_an_acyclic_closure() {
    let root = std::env::temp_dir().join(format!("studio-aix-deps-{}", uuid::Uuid::now_v7()));
    let store = ReconcileStore::open(&root).unwrap();
    let base = dependency_publication(&store, "proof/base", "1.2.0", &[]);
    let app = dependency_publication(&store, "proof/app", "2.0.0", &[("proof/base", "^1.0.0")]);

    super::validate_selection([&base, &app]).unwrap();
    assert!(
        super::validate_selection([&app]).is_err(),
        "missing dependency pin must fail"
    );

    let incompatible_store = ReconcileStore::open(root.join("incompatible")).unwrap();
    let old_base = dependency_publication(&incompatible_store, "proof/base", "0.9.0", &[]);
    let dependent = dependency_publication(
        &incompatible_store,
        "proof/app",
        "2.0.0",
        &[("proof/base", "^1.0.0")],
    );
    assert!(super::validate_selection([&old_base, &dependent]).is_err());

    let cyclic_store = ReconcileStore::open(root.join("cyclic")).unwrap();
    let left = dependency_publication(
        &cyclic_store,
        "proof/left",
        "1.0.0",
        &[("proof/right", "^1.0.0")],
    );
    let right = dependency_publication(
        &cyclic_store,
        "proof/right",
        "1.0.0",
        &[("proof/left", "^1.0.0")],
    );
    let error = super::validate_selection([&left, &right]).unwrap_err();
    assert!(error.to_string().contains("cycle"));
    std::fs::remove_dir_all(root).unwrap();
}

// Catch command/menu metadata or exact executable bytes being lost across native archive admission.
#[test]
#[ignore = "requires view commands archive and explicit native catalogue output"]
fn aix_view_commands_built_artifact_projects_commands_and_assets() {
    use sha2::{Digest, Sha256};
    use std::time::{SystemTime, UNIX_EPOCH};
    let archive = std::fs::read(
        std::env::var("A3S_CLOUD_TEST_AIX_COMMANDS_PACKAGE").expect("commands archive"),
    )
    .unwrap();
    let output =
        std::env::var("A3S_CLOUD_TEST_AIX_COMMANDS_CATALOGUE_OUTPUT").expect("catalogue output");
    let root = std::env::temp_dir().join(format!("a3s-aix-commands-{}", uuid::Uuid::now_v7()));
    let local = super::super::LocalCapabilities::open_at(Some(root.join("capabilities")));
    let install = serde_json::json!({"slug":"proof-view-commands","kind":"plugin","installed":true,
        "install":{"type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/view-commands",
        "digest":format!("sha256:{:x}",Sha256::digest(&archive)),"size":archive.len()}});
    local
        .reconcile(&[install], |_| Ok(archive.clone()))
        .unwrap();
    let bytes = local.plugin_views().unwrap();
    let catalogue: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let package = &catalogue["packages"][0];
    let view = package["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["id"] == "left")
        .unwrap();
    assert_eq!(view["commands"][0]["id"], "refresh");
    assert_eq!(view["commands"][0]["enablement"]["theme"], "light");
    assert_eq!(view["menus"][0]["location"], "view.title");
    assert_eq!(view["menus"][2]["when"]["locale"], "en-US");
    let mut tokens = Vec::new();
    for (instance, surface) in ["left", "left", "right", "tasks", "app"].iter().enumerate() {
        let request = serde_json::json!({"protocolVersion":3,"requestId":format!("{:064x}",instance),
            "activationId":"b".repeat(64),"instanceId":format!("{:064x}",instance + 1),
            "expiresAt":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64+30_000,
            "slug":"proof-view-commands","surface":surface,"revision":package["revision"]});
        let admitted: serde_json::Value = serde_json::from_slice(
            &local
                .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        let token = admitted["token"].as_str().unwrap().to_owned();
        let served = local
            .plugin_view_asset("member", &token, &format!("ui/{surface}/app.js"))
            .unwrap()
            .1;
        if std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_CASE").as_deref()
            == Ok("unsupported-port-version")
        {
            assert!(
                std::str::from_utf8(&served)
                    .unwrap()
                    .contains("protocolVersion: 2, type: 'aix.command.ready'")
            );
        } else {
            assert_eq!(
                served,
                include_bytes!("../../tests/fixtures/aix/view-commands/app.js")
            );
        }
        assert!(!tokens.iter().any(|(previous, _)| previous == &token));
        tokens.push((token, *surface));
    }
    std::fs::write(output, bytes).unwrap();
    local
        .reconcile(&[], |_| panic!("uninstall cannot fetch"))
        .unwrap();
    for (token, surface) in tokens {
        assert!(
            local
                .plugin_view_asset("member", &token, &format!("ui/{surface}/app.js"))
                .is_err()
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

// A package rejected by native reconciliation must leave no installed generation or view contribution.
#[test]
#[ignore = "requires explicit malformed AIX archive and catalogue output"]
fn aix_invalid_built_artifact_is_rejected_without_catalogue_contributions() {
    use sha2::{Digest, Sha256};
    let archive = std::fs::read(
        std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_PACKAGE").expect("negative AIX archive"),
    )
    .unwrap();
    let output = std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT")
        .expect("empty native catalogue output");
    let root = std::env::temp_dir().join(format!("studio-aix-negative-{}", uuid::Uuid::now_v7()));
    let local = super::super::LocalCapabilities::open_at(Some(root.clone()));
    let case = std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_CASE").unwrap_or_default();
    let digest = if case == "digest-substitution" {
        format!("sha256:{}", "0".repeat(64))
    } else {
        format!("sha256:{:x}", Sha256::digest(&archive))
    };
    let install = serde_json::json!({"slug":"proof-view-contract","kind":"plugin","installed":true,
        "install":{"type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/view-commands",
        "digest":digest,"size":archive.len()}});
    let error = local
        .reconcile(&[install], |_| Ok(archive.clone()))
        .unwrap_err();
    let message = error.to_string().to_lowercase();
    let expected = std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION")
        .expect("negative AIX expectation")
        .to_lowercase();
    assert!(
        message.contains(&expected),
        "expected {expected}, got {message}"
    );
    assert!(!root.join("plugins/proof-view-contract").exists());
    let catalogue = local.plugin_views().unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&catalogue).unwrap();
    assert_eq!(parsed["packages"].as_array().unwrap().len(), 0);
    std::fs::write(output, catalogue).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

// A valid workspace contribution reaches the native catalogue for renderer-side service admission.
#[test]
#[ignore = "requires explicit valid workspace AIX archive and catalogue output"]
fn aix_missing_service_built_artifact_projects_workspace_view() {
    use sha2::{Digest, Sha256};
    let archive = std::fs::read(
        std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_PACKAGE").expect("workspace AIX archive"),
    )
    .unwrap();
    let output = std::env::var("A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT")
        .expect("workspace native catalogue output");
    let root = std::env::temp_dir().join(format!("studio-aix-service-{}", uuid::Uuid::now_v7()));
    let local = super::super::LocalCapabilities::open_at(Some(root.clone()));
    let install = serde_json::json!({"slug":"proof-view-contract","kind":"plugin","installed":true,
        "install":{"type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/view-commands",
        "digest":format!("sha256:{:x}",Sha256::digest(&archive)),"size":archive.len()}});
    local
        .reconcile(&[install], |_| Ok(archive.clone()))
        .unwrap();
    let catalogue = local.plugin_views().unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&catalogue).unwrap();
    assert!(
        parsed["packages"][0]["views"]
            .as_array()
            .unwrap()
            .iter()
            .any(|view| view["id"] == "workspace" && view["point"] == "studio.workspace")
    );
    std::fs::write(output, catalogue).unwrap();
    local
        .reconcile(&[], |_| panic!("catalogue cleanup must not fetch"))
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

// Catch condition metadata disappearing between archive admission and native catalogue discovery.
#[test]
#[ignore = "requires context visibility archive and explicit native catalogue output"]
fn aix_context_visibility_built_artifact_projects_conditions_and_assets() {
    use sha2::{Digest, Sha256};
    use std::time::{SystemTime, UNIX_EPOCH};
    let archive = std::fs::read(
        std::env::var("A3S_CLOUD_TEST_AIX_CONTEXT_PACKAGE").expect("context archive"),
    )
    .unwrap();
    let output =
        std::env::var("A3S_CLOUD_TEST_AIX_CONTEXT_CATALOGUE_OUTPUT").expect("catalogue output");
    let root = std::env::temp_dir().join(format!("a3s-aix-context-{}", uuid::Uuid::now_v7()));
    let local = super::super::LocalCapabilities::open_at(Some(root.join("capabilities")));
    let install = serde_json::json!({"slug":"proof-context-visibility","kind":"plugin","installed":true,
        "install":{"type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/context-visibility",
        "digest":format!("sha256:{:x}",Sha256::digest(&archive)),"size":archive.len()}});
    local
        .reconcile(&[install], |_| Ok(archive.clone()))
        .unwrap();
    let bytes = local.plugin_views().unwrap();
    let catalogue: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let package = &catalogue["packages"][0];
    assert_eq!(package["containers"][0]["when"]["theme"], "dark");
    assert_eq!(package["containers"][1]["when"]["theme"], "light");
    let view = |id: &str| {
        package["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|view| view["id"] == id)
            .unwrap()
    };
    assert_eq!(view("tasks")["when"]["locale"], "en-US");
    assert_eq!(view("app")["when"]["theme"], "light");
    assert!(view("editor").get("when").is_none());
    let request = serde_json::json!({"protocolVersion":3,"requestId":"a".repeat(64),
        "activationId":"b".repeat(64),"instanceId":"c".repeat(64),
        "expiresAt":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64+30_000,
        "slug":"proof-context-visibility","surface":"app","revision":package["revision"]});
    let admitted: serde_json::Value = serde_json::from_slice(
        &local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    let token = admitted["token"].as_str().unwrap();
    assert_eq!(
        local
            .plugin_view_asset("member", token, "ui/app/app.js")
            .unwrap()
            .1,
        include_bytes!("../../tests/fixtures/aix/miniapp-environment/app.js")
    );
    std::fs::write(output, bytes).unwrap();
    local
        .reconcile(&[], |_| panic!("uninstall cannot fetch"))
        .unwrap();
    assert!(
        local
            .plugin_view_asset("member", token, "ui/app/app.js")
            .is_err()
    );
    let withdrawn: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    assert_eq!(withdrawn["packages"], serde_json::json!([]));
    std::fs::remove_dir_all(root).unwrap();
}

fn package() -> Vec<(String, Vec<u8>)> {
    let acl = r#"extension "proof/views" {
        schema_version = 3 version = "1.0.0" requires_use = ">=0.3.0, <0.4.0" route = "proof-views" actions = ["read"] repository { url = "https://example.org/proof/views" }
        view_container "resources" { title = "Resources" icon = "database" location = "activitybar" }
        ui "connections" { entry = "assets/connections/page.html" root = "assets/connections" container = "resources" }
        ui "preview" { entry = "assets/preview/page.html" container = "resources" }
    }"#;
    [
        (MANIFEST, acl),
        ("README.md", "# Views"),
        ("assets/connections/page.html", "<html>Connections</html>"),
        ("assets/connections/icon.svg", "<svg/>"),
        ("assets/preview/page.html", "<html>Preview</html>"),
        ("ui/undeclared/index.html", "<html>Undeclared</html>"),
        ("tools/hidden", "hidden executable"),
        ("skills/hidden/SKILL.md", "# Hidden"),
    ]
    .into_iter()
    .map(|(path, body)| (path.to_owned(), body.as_bytes().to_vec()))
    .collect()
}

// Catch Composer assets being omitted from AIX admission or retaining executable access after uninstall.
#[test]
fn aix_composer_archive_admits_only_declared_provider_assets_and_revokes_on_uninstall() {
    use std::io::{Cursor, Write};

    const SCRIPT: &[u8] = include_bytes!("../../tests/fixtures/aix/composer-normalize/provider.js");
    const ENTRY: &[u8] = include_bytes!("../../tests/fixtures/aix/composer-normalize/index.html");
    let files = [
        (
            MANIFEST,
            include_bytes!("../../tests/fixtures/aix/composer-normalize/a3s-use-extension.acl")
                .as_slice(),
        ),
        ("index.html", ENTRY),
        ("provider.js", SCRIPT),
        (
            "README.md",
            include_bytes!("../../tests/fixtures/aix/composer-normalize/README.md").as_slice(),
        ),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in files {
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    let archive = zip.finish().unwrap().into_inner();
    assert_composer_archive(archive);
}

#[test]
#[ignore = "requires explicit A3S_CLOUD_TEST_AIX_COMPOSER_PACKAGE archive"]
fn aix_composer_built_artifact_is_admitted() {
    let path =
        std::env::var("A3S_CLOUD_TEST_AIX_COMPOSER_PACKAGE").expect("Composer artifact path");
    assert_composer_archive(std::fs::read(path).unwrap());
}

// Catch Run/Turn metadata or executable assets changing through native installation and catalogue publication.
#[test]
#[ignore = "requires execution Composer archive and an explicit catalogue output"]
fn aix_execution_composer_built_artifact_projects_scopes_and_verified_assets() {
    use sha2::{Digest, Sha256};
    use std::time::{SystemTime, UNIX_EPOCH};
    let archive = std::fs::read(
        std::env::var("A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_PACKAGE")
            .expect("execution Composer archive"),
    )
    .unwrap();
    let output = std::env::var("A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_CATALOGUE_OUTPUT")
        .expect("catalogue output");
    let root = std::env::temp_dir().join(format!(
        "a3s-aix-execution-composer-{}",
        uuid::Uuid::now_v7()
    ));
    let local = super::super::LocalCapabilities::open_at(Some(root.join("capabilities")));
    let install = serde_json::json!({"slug":"proof-composer-execution","kind":"plugin","installed":true,
        "install":{"type":"plugin","packageType":"aix","available":true,
        "packageId":"use/proof/composer-execution","digest":format!("sha256:{:x}",Sha256::digest(&archive)),"size":archive.len()}});
    local
        .reconcile(&[install], |_| Ok(archive.clone()))
        .unwrap();
    let catalogue_bytes = local.plugin_views().unwrap();
    let catalogue: serde_json::Value = serde_json::from_slice(&catalogue_bytes).unwrap();
    let package = &catalogue["packages"][0];
    assert_eq!(package["views"].as_array().unwrap().len(), 4);
    for (index, scope) in ["run", "turn"].iter().enumerate() {
        let surface = format!("{scope}-normalize");
        let view = package["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|view| view["id"] == surface)
            .unwrap();
        assert_eq!(view["scope"], *scope);
        assert_eq!(view["point"], "studio.composer.actions");
        let request = serde_json::json!({"protocolVersion":3,"requestId":format!("{:064x}",index+1),
            "activationId":"b".repeat(64),"instanceId":format!("{:064x}",index+100),
            "expiresAt":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 + 30_000,
            "slug":"proof-composer-execution","surface":surface,"revision":package["revision"]});
        let admitted: serde_json::Value = serde_json::from_slice(
            &local
                .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        let token = admitted["token"].as_str().unwrap();
        assert_eq!(
            local
                .plugin_view_asset("member", token, "provider.js")
                .unwrap()
                .1,
            include_bytes!("../../tests/fixtures/aix/composer-normalize/provider.js")
        );
        local.close_plugin_view("member", token).unwrap();
        assert!(
            local
                .plugin_view_asset("member", token, "provider.js")
                .is_err()
        );
    }
    std::fs::write(output, catalogue_bytes).unwrap();
    local
        .reconcile(&[], |_| panic!("uninstall cannot fetch"))
        .unwrap();
    let withdrawn: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    assert_eq!(withdrawn["packages"], serde_json::json!([]));
    std::fs::remove_dir_all(root).unwrap();
}

fn assert_composer_archive(archive: Vec<u8>) {
    use sha2::{Digest, Sha256};
    use std::time::{SystemTime, UNIX_EPOCH};
    const SCRIPT: &[u8] = include_bytes!("../../tests/fixtures/aix/composer-normalize/provider.js");
    const ENTRY: &[u8] = include_bytes!("../../tests/fixtures/aix/composer-normalize/index.html");
    let root = std::env::temp_dir().join(format!("a3s-aix-composer-{}", uuid::Uuid::now_v7()));
    let local = super::super::LocalCapabilities::open_at(Some(root.join("capabilities")));
    let install = serde_json::json!({"slug":"proof-composer","kind":"plugin","installed":true,"install":{
        "type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/composer",
        "digest":format!("sha256:{:x}", Sha256::digest(&archive)),"size":archive.len()
    }});
    local
        .reconcile(&[install], |_| Ok(archive.clone()))
        .unwrap();
    let catalogue: serde_json::Value =
        serde_json::from_slice(&local.plugin_views().unwrap()).unwrap();
    let package = &catalogue["packages"][0];
    let view = package["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["id"] == "normalize")
        .unwrap();
    assert_eq!(view["point"], "studio.composer.actions");
    assert_eq!(view["container"], serde_json::Value::Null);
    assert!(view.get("scope").is_none());
    let scoped = package["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["id"] == "session-normalize")
        .unwrap();
    assert_eq!(scoped["point"], "studio.composer.actions");
    assert_eq!(scoped["scope"], "session");
    assert_eq!(scoped["container"], serde_json::Value::Null);
    let request = serde_json::json!({"protocolVersion":3,"requestId":"a".repeat(64),
        "activationId":"b".repeat(64),"instanceId":"c".repeat(64),
        "expiresAt":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 + 30_000,
        "slug":"proof-composer","surface":"normalize","revision":package["revision"]});
    let session: serde_json::Value = serde_json::from_slice(
        &local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    let token = session["token"].as_str().unwrap();
    assert_eq!(
        local
            .plugin_view_asset("member", token, "index.html")
            .unwrap()
            .1,
        ENTRY
    );
    assert_eq!(
        local
            .plugin_view_asset("member", token, "provider.js")
            .unwrap()
            .1,
        SCRIPT
    );
    assert!(
        local
            .plugin_view_asset("member", token, "tools/hidden")
            .is_err()
    );
    local
        .reconcile(&[], |_| panic!("uninstall must not fetch"))
        .unwrap();
    assert!(
        local
            .plugin_view_asset("member", token, "provider.js")
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn aix_account_install_uses_declared_snapshots_and_refuses_legacy_archive_fallback() {
    use sha2::{Digest, Sha256};
    use std::io::{Cursor, Write};
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, body) in package() {
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&body).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    let root = std::env::temp_dir().join(format!("a3s-aix-account-{}", uuid::Uuid::now_v7()));
    let caps = root.join("capabilities");
    let local = super::super::LocalCapabilities::open_at(Some(caps.clone()));
    let entry = serde_json::json!({"slug":"proof-views","kind":"plugin","installed":true,"install":{
        "type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/views",
        "digest":format!("sha256:{:x}", Sha256::digest(&bytes)), "size":bytes.len()
    }});
    local
        .reconcile(&[entry.clone()], |_| Ok(bytes.clone()))
        .unwrap();
    let calls: serde_json::Value =
        serde_json::from_slice(&local.callable_plugin("proof-views").unwrap()).unwrap();
    assert!(
        calls["surfaces"].as_array().unwrap().is_empty(),
        "unlisted executables and skills cannot be called"
    );
    let tools: serde_json::Value =
        serde_json::from_slice(&std::fs::read(caps.join("tools.json")).unwrap()).unwrap();
    assert!(tools["tools"].as_array().unwrap().is_empty());
    assert!(!root.join("agents/hidden").exists());
    local
        .reconcile(&[entry], |_| {
            panic!("restart should use a verified generation")
        })
        .unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "ui/home/index.html",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"<html>Legacy</html>").unwrap();
    let legacy = zip.finish().unwrap().into_inner();
    let forged = serde_json::json!({"slug":"legacy-view","kind":"plugin","installed":true,"install":{
        "type":"plugin","packageType":"aix","available":true,"packageId":"use/proof/legacy",
        "digest":format!("sha256:{:x}", Sha256::digest(&legacy)), "size":legacy.len()
    }});
    assert!(local.reconcile(&[forged], |_| Ok(legacy.clone())).is_err());
    assert!(!caps.join("plugins/legacy-view").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn aix_declared_views_publish_custom_entries_with_owned_manifest_and_assets() {
    let files = package();
    let spec = package_spec(&files, "use/proof/views").unwrap();
    assert_eq!(
        spec.surfaces.len(),
        2,
        "unlisted directory is not a contribution"
    );
    let root = std::env::temp_dir().join(format!("a3s-aix-views-{}", uuid::Uuid::now_v7()));
    let store = ReconcileStore::open(&root).unwrap();
    let publication = store.apply_sync(&spec).unwrap();
    let cursor = publication.cursor();
    let lease = store.acquire_sync(&cursor).unwrap();
    let view = lease
        .publication()
        .surfaces
        .iter()
        .find(|view| view.id == "connections")
        .unwrap();
    assert_eq!(
        std::fs::read(view.entry.as_ref().unwrap()).unwrap(),
        b"<html>Connections</html>"
    );
    let acl = std::fs::read_to_string(view.directory.join(MANIFEST)).unwrap();
    let parsed = ExtensionManifest::parse_acl(&acl).unwrap();
    assert_eq!(parsed.view_containers[0].location.as_str(), "activitybar");
    assert!(view.directory.join("assets/connections/icon.svg").is_file());
    assert!(!view.directory.join("ui/undeclared/index.html").exists());
    // Tampering must invalidate the whole leased UI generation.
    std::fs::write(view.directory.join(MANIFEST), b"altered").unwrap();
    assert!(store.acquire_sync(&cursor).is_err());
    drop(lease);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn aix_projection_refuses_identity_missing_entry_and_unimplemented_runtime() {
    assert!(package_spec(&package(), "use/foreign/views").is_err());
    let mut files = package();
    files.retain(|(path, _)| path != "assets/preview/page.html");
    assert!(package_spec(&files, "use/proof/views").is_err());
    let mut files = package();
    let (_, acl) = files.iter_mut().find(|(path, _)| path == MANIFEST).unwrap();
    let text = String::from_utf8(acl.clone()).unwrap();
    let closing = text.rfind('}').unwrap();
    *acl = format!(
        "{} mcp \"server\" {{ transport = \"stdio\" executable = \"bin/server\" }} }}",
        &text[..closing]
    )
    .into_bytes();
    files.push(("bin/server".to_owned(), b"server".to_vec()));
    assert!(
        package_spec(&files, "use/proof/views")
            .unwrap_err()
            .to_string()
            .contains("catalog MCP")
    );
    let spec = package_spec(&package(), "use/proof/views").unwrap();
    assert!(
        spec.surfaces
            .iter()
            .all(|surface| matches!(surface.payload, Payload::Files(_)))
    );
}

#[test]
fn catalog_mcp_and_agent_packages_project_their_surfaces() {
    let mcp = r#"extension "a3s/github" {
  schema_version = 3
  version = "1.0.0"
  route = "github"
  requires_use = ">=0.3.0, <0.4.0"
  actions = ["read"]
  repository {
    url = "https://a3s.dev/packages/connectors/github"
    revision = "0000000000000000000000000000000000000000"
  }
  mcp "github" {
    transport = "catalog"
    server = "server.json"
    optional = false
  }
}
"#;
    let mcp_files = [
        (MANIFEST.to_owned(), mcp.as_bytes().to_vec()),
        ("README.md".to_owned(), b"# github\n".to_vec()),
        (
            "server.json".to_owned(),
            br#"{"type":"http","url":"https://example.com/mcp"}"#.to_vec(),
        ),
    ];
    let spec = package_spec(&mcp_files, "use/a3s/github").unwrap();
    assert_eq!(spec.surfaces[0].kind, SurfaceKind::Mcp);
    assert_eq!(spec.surfaces[0].id, "github");

    let agent = r#"extension "a3s/writer" {
  schema_version = 3
  version = "1.0.0"
  route = "writer"
  requires_use = ">=0.3.0, <0.4.0"
  actions = ["read"]
  repository {
    url = "https://a3s.dev/packages/agents/writer"
    revision = "0000000000000000000000000000000000000000"
  }
  agent "writer" {
    harness = "builtin"
    instructions = "agents/writer/INSTRUCTIONS.md"
    runtime = "edge"
    optional = false
    resources {
      cpu_millis = 500
      memory_bytes = 268435456
    }
  }
}
"#;
    let agent_files = [
        (MANIFEST.to_owned(), agent.as_bytes().to_vec()),
        ("README.md".to_owned(), b"# writer\n".to_vec()),
        (
            "agents/writer/INSTRUCTIONS.md".to_owned(),
            "用简洁中文写".as_bytes().to_vec(),
        ),
    ];
    let spec = package_spec(&agent_files, "use/a3s/writer").unwrap();
    assert_eq!(spec.surfaces[0].kind, SurfaceKind::Agent);
    assert_eq!(spec.surfaces[0].id, "writer");

    let schema = br#"{"type":"object","properties":{"days":{"type":"integer","title":"Days","minimum":1,"maximum":14}},"required":["days"]}"#;
    let declared = agent.replace(
        "    optional = false\n",
        "    parameters = \"agents/writer/parameters.schema.json\"\n    optional = false\n",
    );
    let declared_files = [
        (MANIFEST.to_owned(), declared.as_bytes().to_vec()),
        ("README.md".to_owned(), b"# writer\n".to_vec()),
        (
            "agents/writer/INSTRUCTIONS.md".to_owned(),
            "用简洁中文写".as_bytes().to_vec(),
        ),
        (
            "agents/writer/parameters.schema.json".to_owned(),
            schema.to_vec(),
        ),
    ];
    let spec = package_spec(&declared_files, "use/a3s/writer").unwrap();
    assert!(
        matches!(&spec.surfaces[0].payload, a3s_use_reconcile::Payload::Files(files) if files.iter().any(|file| file.path == "agents/writer/parameters.schema.json"))
    );
    let nested = declared_files
        .iter()
        .map(|(path, bytes)| {
            if path.ends_with("parameters.schema.json") {
                (
                    path.clone(),
                    br#"{"type":"object","properties":{"body":{"type":"object"}}}"#.to_vec(),
                )
            } else {
                (path.clone(), bytes.clone())
            }
        })
        .collect::<Vec<_>>();
    let refused = package_spec(&nested, "use/a3s/writer").unwrap_err();
    assert!(refused.to_string().contains("parameters are invalid"));

    let with_okf = r#"extension "a3s/writer" {
  schema_version = 3
  version = "1.0.0"
  route = "writer"
  requires_use = ">=0.3.0, <0.4.0"
  actions = ["read"]
  repository {
    url = "https://a3s.dev/packages/agents/writer"
    revision = "0000000000000000000000000000000000000000"
  }
  okf "domain" {
    format_version         = "0.2"
    root                   = "okf/domain"
    content_digest         = "sha256:bd85b0b63adb32bdf616384a619286af4c32401542655dd09e00450902ab478d"
    concept_count          = 1
    file_count             = 1
    expanded_bytes         = 32
    max_files              = 256
    max_concepts           = 64
    max_expanded_bytes     = 67108864
    max_document_bytes     = 1048576
    max_links_per_document = 2048
    optional               = false
  }
  agent "writer" {
    harness = "builtin"
    instructions = "agents/writer/INSTRUCTIONS.md"
    runtime = "edge"
    requires_okf = ["domain"]
    optional = false
    resources {
      cpu_millis = 500
      memory_bytes = 268435456
    }
  }
}
"#;
    let knowledge_files = [
        (MANIFEST.to_owned(), with_okf.as_bytes().to_vec()),
        ("README.md".to_owned(), b"# writer\n".to_vec()),
        (
            "agents/writer/INSTRUCTIONS.md".to_owned(),
            "用简洁中文写".as_bytes().to_vec(),
        ),
        (
            "okf/domain/index.md".to_owned(),
            "---\ntitle: 领域\n---\n默认条款\n".as_bytes().to_vec(),
        ),
    ];
    let spec = package_spec(&knowledge_files, "use/a3s/writer").unwrap();
    assert!(spec.surfaces.iter().any(|surface| {
        surface.kind == SurfaceKind::Okf
            && surface.id == "domain"
            && matches!(&surface.payload, Payload::Files(files) if files.iter().any(|file| file.path == "okf/domain/index.md"))
    }));
    assert!(
        spec.surfaces
            .iter()
            .any(|surface| surface.kind == SurfaceKind::Agent && surface.id == "writer")
    );
}

#[test]
fn a_package_harness_keeps_the_files_beside_its_entry() {
    let manifest = r#"extension "a3s/cobot-arm" {
  schema_version = 3
  version = "1.0.0"
  route = "cobot-arm"
  requires_use = ">=0.3.0, <0.4.0"
  actions = ["read"]
  repository {
    url = "https://a3s.dev/packages/cobot-arm"
    revision = "0000000000000000000000000000000000000000"
  }
  skill "cobot-arm" {
    path = "skills/cobot-arm/SKILL.md"
    optional = false
  }
  agent "cobot-arm" {
    harness = "package"
    runtime = "edge"
    entry = "agent/run.mjs"
    instructions = "agents/cobot-arm/INSTRUCTIONS.md"
    requires_skill = ["cobot-arm"]
    optional = false
    resources {
      cpu_millis = 500
      memory_bytes = 268435456
    }
  }
}
"#;
    let files = [
        (MANIFEST.to_owned(), manifest.as_bytes().to_vec()),
        ("README.md".to_owned(), b"# lab\n".to_vec()),
        (
            "skills/cobot-arm/SKILL.md".to_owned(),
            b"# skill\n".to_vec(),
        ),
        (
            "agents/cobot-arm/INSTRUCTIONS.md".to_owned(),
            "选一个动作".as_bytes().to_vec(),
        ),
        (
            "agent/run.mjs".to_owned(),
            b"import './choose.mjs'\n".to_vec(),
        ),
        (
            "agent/choose.mjs".to_owned(),
            b"export function choose() {}\n".to_vec(),
        ),
        (
            "agent/lib/helper.mjs".to_owned(),
            b"export const n = 1\n".to_vec(),
        ),
        (
            "notes.txt".to_owned(),
            b"not part of the program\n".to_vec(),
        ),
    ];
    let spec = package_spec(&files, "use/a3s/cobot-arm").unwrap();
    let agent = spec
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Agent)
        .unwrap();
    let Payload::Files(payload) = &agent.payload else {
        panic!("agent payload");
    };
    let paths = payload
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();
    for required in [
        "agent/run.mjs",
        "agent/choose.mjs",
        "agent/lib/helper.mjs",
        "agents/cobot-arm/INSTRUCTIONS.md",
        MANIFEST,
        "README.md",
    ] {
        assert!(
            paths.contains(&required),
            "{required} missing from {paths:?}"
        );
    }
    assert!(!paths.contains(&"notes.txt"));
    assert!(!paths.contains(&"skills/cobot-arm/SKILL.md"));
    let skill = spec
        .surfaces
        .iter()
        .find(|surface| surface.kind == SurfaceKind::Skill)
        .unwrap();
    let Payload::Files(skill_files) = &skill.payload else {
        panic!("skill payload");
    };
    assert!(skill_files.iter().any(|file| file.path == "SKILL.md"));
    assert!(
        !skill_files
            .iter()
            .any(|file| file.path == "agent/choose.mjs")
    );
}

#[test]
fn an_installed_acp_cloud_agent_streams_two_turns_on_one_session() {
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    let archive = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../apps/web/public/cases/acp-cloud-agent.aix"
    ));
    let files = aix_entries(archive);
    let spec = package_spec(&files, "use/a3s/acp-cloud").expect("AIX install admission");
    assert!(
        spec.surfaces
            .iter()
            .any(|surface| surface.kind == SurfaceKind::Skill && surface.id == "acp-cloud")
    );
    let root = std::env::temp_dir().join(format!("studio-acp-cloud-{}", uuid::Uuid::now_v7()));
    let store = ReconcileStore::open(root.join("package-generations")).unwrap();
    store.apply_sync(&spec).unwrap();

    let agent = files
        .iter()
        .find(|(path, _)| path == "agent/acp-agent.mjs")
        .expect("packaged agent");
    let agent_path = root.join("acp-agent.mjs");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&agent_path, &agent.1).unwrap();

    let mut child = Command::new("node")
        .arg(&agent_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("node is required to call the ACP cloud agent");
    let stdout = child.stdout.take().expect("stdout");
    let mut stdin = child.stdin.take().expect("stdin");
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });

    let (initialized, _) = acp_request(
        &mut stdin,
        &receiver,
        1,
        "initialize",
        json!({"protocolVersion": 1}),
    );
    assert_eq!(initialized["result"]["protocolVersion"], 1);
    let (opened, _) = acp_request(
        &mut stdin,
        &receiver,
        2,
        "session/new",
        json!({"cwd": "/workspace", "mcpServers": []}),
    );
    let session_id = opened["result"]["sessionId"]
        .as_str()
        .expect("sessionId")
        .to_owned();
    let (first, first_updates) = acp_request(
        &mut stdin,
        &receiver,
        3,
        "session/prompt",
        json!({"sessionId": session_id, "prompt": [{"type": "text", "text": "第一句"}]}),
    );
    assert_eq!(first["result"]["stopReason"], "end_turn");
    assert_eq!(streamed_text(&first_updates), "收到：第一句");
    assert!(
        first_updates
            .iter()
            .all(|update| update.get("id").is_none())
    );
    let (second, second_updates) = acp_request(
        &mut stdin,
        &receiver,
        4,
        "session/prompt",
        json!({"sessionId": session_id, "prompt": [{"type": "text", "text": "第二句"}]}),
    );
    assert_eq!(second["result"]["stopReason"], "end_turn");
    assert_eq!(
        streamed_text(&second_updates),
        "上一轮是第一句。这一轮是第二句"
    );

    let _ = child.kill();
    let _ = std::fs::remove_dir_all(&root);

    fn aix_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("aix zip");
        let mut files = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).expect("zip entry");
            if entry.is_dir() {
                continue;
            }
            let mut contents = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut contents).expect("zip bytes");
            files.push((entry.name().to_owned(), contents));
        }
        files
    }

    fn acp_request(
        stdin: &mut impl Write,
        receiver: &mpsc::Receiver<String>,
        id: i64,
        method: &str,
        params: serde_json::Value,
    ) -> (serde_json::Value, Vec<serde_json::Value>) {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{line}").expect("write ACP request");
        let _ = stdin.flush();
        let mut updates = Vec::new();
        loop {
            let line = receiver
                .recv_timeout(Duration::from_secs(5))
                .expect("ACP reply");
            let message: serde_json::Value = serde_json::from_str(&line).expect("ACP json");
            if message.get("id").and_then(serde_json::Value::as_i64) == Some(id) {
                return (message, updates);
            }
            if message.get("method").and_then(serde_json::Value::as_str) == Some("session/update") {
                updates.push(message);
            }
        }
    }

    fn streamed_text(updates: &[serde_json::Value]) -> String {
        updates
            .iter()
            .filter_map(|update| {
                update
                    .pointer("/params/update/content/text")
                    .and_then(serde_json::Value::as_str)
            })
            .collect()
    }
}
