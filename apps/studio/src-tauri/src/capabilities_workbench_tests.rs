#![cfg(test)]

use super::super::LocalCapabilities;
use super::*;
use serde_json::Value;
use std::io::{Cursor, Write};

struct Fixture {
    root: std::path::PathBuf,
    local: LocalCapabilities,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("a3s-workbench-{}", uuid::Uuid::now_v7()));
        let local = LocalCapabilities::open_at(Some(root.join("capabilities")));
        Self { root, local }
    }
    fn install(&self, text: &str) {
        self.install_set(&[("proof-views", "proof/views", text)]);
    }
    fn install_set(&self, packages: &[(&str, &str, &str)]) {
        let mut entries = Vec::new();
        let mut bundles = BTreeMap::new();
        for (slug, package_id, text) in packages {
            let acl = r#"extension "proof/views" {
            schema_version = 3 version = "1.0.0" requires_use = ">=0.3.0, <0.4.0" route = "proof-views" actions = ["read"]
            repository { url = "https://example.org/proof/views" }
            view_container "resources" { title = "Resources" icon = "database" location = "activitybar" }
            view_container "output" { title = "Output" icon = "terminal" location = "panel" }
            ui "connections" { title = "Connections" entry = "ui/connections/page.html" root = "ui/connections" container = "resources" }
            ui "preview" { title = "Preview" entry = "ui/preview.html" container = "resources" }
            ui "output" { entry = "ui/output.html" container = "output" }
            ui "app" { title = "Workbench App" entry = "ui/app/index.html" root = "ui/app" point = "studio.applets" }
            ui "workspace" { entry = "ui/workspace.html" point = "studio.workspace" }
            ui "tree" { title = "Tasks" entry = "ui/tree/index.html" root = "ui/tree" container = "resources" presentation = "tree" }
        }"#.replace("proof/views", package_id).replace("proof-views", slug);
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for (name, data) in [
                ("a3s-use-extension.acl", acl.as_str()),
                ("README.md", "# Proof"),
                ("ui/connections/page.html", *text),
                (
                    "ui/connections/view.js",
                    "document.body.textContent = 'loaded'",
                ),
                ("ui/preview.html", "<html>preview</html>"),
                ("ui/output.html", "<html>output</html>"),
                (
                    "ui/app/index.html",
                    include_str!("../../tests/fixtures/aix/miniapp-environment/index.html"),
                ),
                (
                    "ui/app/app.js",
                    include_str!("../../tests/fixtures/aix/miniapp-environment/app.js"),
                ),
                ("ui/workspace.html", "<html>workspace</html>"),
                (
                    "ui/tree/index.html",
                    include_str!("../../tests/fixtures/aix/tree-provider/index.html"),
                ),
                (
                    "ui/tree/provider.js",
                    include_str!("../../tests/fixtures/aix/tree-provider/provider.js"),
                ),
            ] {
                zip.start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(data.as_bytes()).unwrap();
            }
            let bytes = zip.finish().unwrap().into_inner();
            let entry = json!({"slug":slug, "kind":"plugin", "installed":true, "install":{
            "type":"plugin", "packageType":"aix", "available":true, "packageId":format!("use/{package_id}"),
            "digest":super::super::digest_of(&bytes), "size":bytes.len() }});
            entries.push(entry);
            bundles.insert((*slug).to_owned(), bytes);
        }
        self.local
            .reconcile(&entries, |slug| {
                bundles.get(slug).cloned().ok_or_else(missing)
            })
            .unwrap();
    }
    fn catalogue(&self) -> Value {
        serde_json::from_slice(&self.local.plugin_views().unwrap()).unwrap()
    }
    fn request(&self) -> Value {
        let catalogue = self.catalogue();
        json!({"protocolVersion":VIEW_PROTOCOL,"requestId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),"activationId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),"instanceId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),"expiresAt":now_ms().unwrap()+ADMISSION_LIFETIME_MS,"slug":"proof-views", "surface":"connections", "revision":catalogue["packages"][0]["revision"]})
    }
    fn open(&self, owner: &str) -> Value {
        let request = self.request();
        serde_json::from_slice(
            &self
                .local
                .open_plugin_view(owner, &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn agent_only_packages_publish_the_active_archive_identity_without_a_window() {
    let f = Fixture::new();
    let acl = r#"extension "proof/agent-only" {
        schema_version = 3 version = "1.0.1" requires_use = ">=0.3.0, <0.4.0" route = "agent-only" actions = ["read"]
        repository { url = "https://example.org/proof/agent-only" }
        skill "proof" { path = "skills/proof/SKILL.md" }
        agent "proof" { harness = "builtin" runtime = "edge" instructions = "agents/proof/INSTRUCTIONS.md"
            requires_skill = ["proof"] resources { cpu_millis = 500 memory_bytes = 268435456 } }
    }"#;
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents) in [
        ("a3s-use-extension.acl", acl),
        ("README.md", "# Agent-only identity proof"),
        ("skills/proof/SKILL.md", "# Proof"),
        ("agents/proof/INSTRUCTIONS.md", "Use the selected source."),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    let digest = super::super::digest_of(&bytes);
    let entry = json!({"slug":"agent-only", "kind":"plugin", "installed":true, "install":{
        "type":"plugin", "packageType":"aix", "available":true, "packageId":"use/proof/agent-only",
        "digest":digest, "size":bytes.len() }});
    f.local.reconcile(&[entry], |_| Ok(bytes.clone())).unwrap();
    let catalog = f.catalogue();
    assert_eq!(catalog["packages"].as_array().unwrap().len(), 1);
    let package = &catalog["packages"][0];
    assert_eq!(package["packageDigest"], digest);
    assert_eq!(package["views"], json!([]));
    assert_eq!(package["containers"], json!([]));
    let mut request = f.request();
    request["slug"] = json!("agent-only");
    assert!(
        f.local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
            .is_err()
    );
}

// Catch another package's installation or admission revoking this unchanged package's executing view.
#[test]
fn unchanged_view_sessions_survive_other_package_install_upgrade_open_and_uninstall() {
    let f = Fixture::new();
    f.install("<html>original</html>");
    let first = f.open("member");
    let token = first["token"].as_str().unwrap();
    let before = f.catalogue();
    f.install_set(&[
        ("proof-views", "proof/views", "<html>original</html>"),
        ("other-views", "proof/other", "<html>other</html>"),
    ]);
    assert_ne!(f.catalogue()["revision"], before["revision"]);
    assert_eq!(
        f.local
            .plugin_view_asset("member", token, "ui/connections/page.html")
            .unwrap()
            .1,
        b"<html>original</html>"
    );
    let mut request = f.request();
    request["slug"] = json!("other-views");
    request["revision"] = f.catalogue()["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["slug"] == "other-views")
        .unwrap()["revision"]
        .clone();
    let other: Value = serde_json::from_slice(
        &f.local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    let other_token = other["token"].as_str().unwrap();
    assert!(
        f.local
            .plugin_view_asset("member", token, "ui/connections/page.html")
            .is_ok()
    );
    f.install_set(&[
        ("proof-views", "proof/views", "<html>original</html>"),
        ("other-views", "proof/other", "<html>changed</html>"),
    ]);
    assert!(
        f.local
            .plugin_view_asset("member", token, "ui/connections/page.html")
            .is_ok()
    );
    assert!(
        f.local
            .plugin_view_asset("member", other_token, "ui/connections/page.html")
            .is_err()
    );
    f.install("<html>original</html>");
    assert!(
        f.local
            .plugin_view_asset("member", token, "ui/connections/page.html")
            .is_ok()
    );
    f.local.close_plugin_view("member", token).unwrap();
    assert!(
        f.local
            .plugin_view_asset("member", token, "ui/connections/page.html")
            .is_err()
    );
}

// Catch unrelated installation resetting the admission budget or old tokens surviving an own-package reinstall.
#[test]
fn session_capacity_and_generation_revocation_follow_exact_package_ownership() {
    let f = Fixture::new();
    f.install("<html>original</html>");
    let mut tokens = Vec::new();
    for _ in 0..MAX_SESSIONS {
        tokens.push(f.open("member")["token"].as_str().unwrap().to_owned());
    }
    f.install_set(&[
        ("proof-views", "proof/views", "<html>original</html>"),
        ("other-views", "proof/other", "<html>other</html>"),
    ]);
    let mut request = f.request();
    request["slug"] = json!("other-views");
    request["revision"] = f.catalogue()["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["slug"] == "other-views")
        .unwrap()["revision"]
        .clone();
    assert_eq!(
        f.local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    for token in &tokens {
        assert!(
            f.local
                .plugin_view_asset("member", token, "ui/connections/page.html")
                .is_ok()
        );
    }
    f.install("<html>upgraded</html>");
    let current = f.open("member");
    assert_eq!(
        f.local
            .plugin_view_asset(
                "member",
                current["token"].as_str().unwrap(),
                "ui/connections/page.html"
            )
            .unwrap()
            .1,
        b"<html>upgraded</html>"
    );
    for token in tokens {
        assert!(
            f.local
                .plugin_view_asset("member", &token, "ui/connections/page.html")
                .is_err()
        );
    }
    f.local
        .reconcile(&[], |_| panic!("uninstall must not fetch"))
        .unwrap();
    f.install("<html>upgraded</html>");
    assert!(
        f.local
            .plugin_view_asset(
                "member",
                current["token"].as_str().unwrap(),
                "ui/connections/page.html"
            )
            .is_err()
    );
    assert!(
        f.local
            .plugin_view_asset(
                "member",
                f.open("member")["token"].as_str().unwrap(),
                "ui/connections/page.html"
            )
            .is_ok()
    );
}

// Catch the actual built AIX artifact losing its declared UI/Skill or retaining admission after uninstall.
#[test]
#[ignore = "build the real AIX workbench package and set A3S_CLOUD_TEST_AIX_WORKBENCH_PACKAGE"]
fn built_aix_workbench_artifact_installs_declared_views_and_retires_assets() {
    let bytes =
        fs::read(std::env::var("A3S_CLOUD_TEST_AIX_WORKBENCH_PACKAGE").expect("AIX artifact path"))
            .unwrap();
    let f = Fixture::new();
    let entry = json!({"slug":"proof-workbench", "kind":"plugin", "installed":true, "install":{
        "type":"plugin", "packageType":"aix", "available":true, "packageId":"use/proof/workbench",
        "digest":super::super::digest_of(&bytes), "size":bytes.len()
    }});
    f.local.reconcile(&[entry], |_| Ok(bytes.clone())).unwrap();
    let catalogue = f.catalogue();
    let package = &catalogue["packages"][0];
    assert_eq!(package["slug"], "proof-workbench");
    assert_eq!(package["containers"].as_array().unwrap().len(), 2);
    assert_eq!(package["views"].as_array().unwrap().len(), 5);
    let views = package["views"].as_array().unwrap();
    assert_eq!(
        views.iter().find(|view| view["id"] == "tasks").unwrap()["presentation"],
        "tree"
    );
    assert_eq!(
        views.iter().find(|view| view["id"] == "app").unwrap()["point"],
        "studio.applets"
    );
    let callable: Value =
        serde_json::from_slice(&f.local.callable_plugin("proof-workbench").unwrap()).unwrap();
    assert_eq!(callable["surfaces"].as_array().unwrap().len(), 1);
    assert_eq!(callable["surfaces"][0]["kind"], "skill");
    let mut admitted = Vec::new();
    for (surface, path, expected) in [
        (
            "tasks",
            "ui/tasks/provider.js",
            include_bytes!("../../tests/fixtures/aix/tree-provider/provider.js").as_slice(),
        ),
        (
            "app",
            "ui/app/app.js",
            include_bytes!("../../tests/fixtures/aix/miniapp-environment/app.js").as_slice(),
        ),
    ] {
        let mut request = f.request();
        request["slug"] = json!("proof-workbench");
        request["surface"] = json!(surface);
        let session: Value = serde_json::from_slice(
            &f.local
                .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        let token = session["token"].as_str().unwrap();
        let asset = f.local.plugin_view_asset("member", token, path).unwrap();
        assert_eq!(asset.1, expected);
        let sibling = if surface == "app" {
            "ui/tasks/provider.js"
        } else {
            "ui/app/app.js"
        };
        assert!(f.local.plugin_view_asset("member", token, sibling).is_err());
        admitted.push((token.to_owned(), path));
    }
    f.local
        .reconcile(&[], |_| panic!("uninstall must not fetch"))
        .unwrap();
    assert_eq!(f.catalogue()["packages"], json!([]));
    assert!(f.local.callable_plugin("proof-workbench").is_err());
    for (token, path) in admitted {
        assert!(f.local.plugin_view_asset("member", &token, path).is_err());
    }
}

// Catch directory inference and container/view declarations read from unverified raw installs.
#[test]
fn verified_catalogue_preserves_owned_multi_view_placements_and_removes_uninstalled_packages() {
    let f = Fixture::new();
    assert_eq!(f.catalogue()["packages"], json!([]));
    f.install("<script src='view.js'></script>");
    let catalogue = f.catalogue();
    let package = &catalogue["packages"][0];
    assert_eq!(package["packageId"], "use/proof/views");
    assert_eq!(package["containers"][0]["location"], "activitybar");
    assert_eq!(package["containers"][1]["location"], "panel");
    assert_eq!(catalogue["protocolVersion"], 1);
    assert_eq!(package["views"].as_array().unwrap().len(), 6);
    let views = package["views"].as_array().unwrap();
    assert_eq!(
        views.iter().find(|v| v["id"] == "tree").unwrap()["presentation"],
        "tree"
    );
    assert_eq!(
        views.iter().find(|v| v["id"] == "app").unwrap()["point"],
        "studio.applets"
    );
    assert_eq!(
        views.iter().find(|v| v["id"] == "workspace").unwrap()["point"],
        "studio.workspace"
    );
    assert_eq!(
        views.iter().find(|v| v["id"] == "connections").unwrap()["container"],
        "resources"
    );
    f.local
        .reconcile(&[], |_| panic!("uninstall must not fetch"))
        .unwrap();
    assert_eq!(f.catalogue()["packages"], json!([]));
}

// Catch one damaged package blocking unrelated catalogues, fresh sessions or already-owned assets.
#[test]
fn damaged_package_is_quarantined_without_revoking_a_verified_sibling() {
    let f = Fixture::new();
    f.install_set(&[
        ("proof-views", "proof/views", "<html>healthy</html>"),
        ("other-views", "proof/other", "<html>other</html>"),
    ]);
    let before = f.catalogue();
    let healthy = before["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["slug"] == "proof-views")
        .unwrap();
    let request = json!({"protocolVersion":VIEW_PROTOCOL,
        "requestId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
        "activationId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
        "instanceId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
        "expiresAt":now_ms().unwrap()+ADMISSION_LIFETIME_MS,
        "slug":"proof-views", "surface":"connections", "revision":healthy["revision"]});
    let opened: Value = serde_json::from_slice(
        &f.local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    let store = ReconcileStore::open(f.root.join("package-generations")).unwrap();
    let other = store.current_sync("use/proof/other").unwrap().unwrap();
    let surface = other
        .surfaces
        .iter()
        .find(|s| s.id == "connections")
        .unwrap();
    fs::write(surface.entry.as_ref().unwrap(), "<script>tampered</script>").unwrap();
    let after = f.catalogue();
    assert_eq!(after["packages"], json!([healthy]));
    assert_ne!(after["revision"], before["revision"]);
    assert_eq!(after["revision"], f.catalogue()["revision"]);
    assert_eq!(
        after["diagnostics"],
        json!([{"slug":"other-views", "reason":"package-content-refused"}])
    );
    assert_eq!(
        f.local
            .plugin_view_asset(
                "member",
                opened["token"].as_str().unwrap(),
                "ui/connections/page.html"
            )
            .unwrap()
            .1,
        b"<html>healthy</html>"
    );
    let mut fresh = request.clone();
    fresh["requestId"] = json!(format!("{:064x}", uuid::Uuid::now_v7().as_u128()));
    fresh["activationId"] = json!(format!("{:064x}", uuid::Uuid::now_v7().as_u128()));
    fresh["instanceId"] = json!(format!("{:064x}", uuid::Uuid::now_v7().as_u128()));
    assert!(
        f.local
            .open_plugin_view("member", &serde_json::to_vec(&fresh).unwrap())
            .is_ok()
    );
    fresh["slug"] = json!("other-views");
    fresh["revision"] = json!(other.cursor().revision);
    fresh["requestId"] = json!(format!("{:064x}", uuid::Uuid::now_v7().as_u128()));
    fresh["activationId"] = json!(format!("{:064x}", uuid::Uuid::now_v7().as_u128()));
    fresh["instanceId"] = json!(format!("{:064x}", uuid::Uuid::now_v7().as_u128()));
    assert!(
        f.local
            .open_plugin_view("member", &serde_json::to_vec(&fresh).unwrap())
            .is_err()
    );
    let selection = store
        .selection_sync(super::super::selection::INSTALLED_SELECTION)
        .unwrap()
        .unwrap();
    assert!(store.acquire_selection_sync(&selection.cursor()).is_err());
    fs::write(surface.entry.as_ref().unwrap(), "<html>other</html>").unwrap();
    assert_eq!(f.catalogue(), before);
    assert!(
        f.local
            .open_plugin_view("member", &serde_json::to_vec(&fresh).unwrap())
            .is_ok()
    );
}

// Catch a damaged installation marker taking a healthy sibling out of the workbench.
#[test]
fn invalid_install_marker_is_quarantined_and_changes_catalogue_revision() {
    let f = Fixture::new();
    f.install_set(&[
        ("proof-views", "proof/views", "<html>healthy</html>"),
        ("other-views", "proof/other", "<html>other</html>"),
    ]);
    let before = f.catalogue();
    fs::write(
        f.root
            .join("capabilities/plugins/other-views")
            .join(PACKAGE_ID_MARKER),
        "invalid",
    )
    .unwrap();
    let after = f.catalogue();
    assert_eq!(after["packages"].as_array().unwrap().len(), 1);
    assert_eq!(after["packages"][0]["slug"], "proof-views");
    assert_eq!(
        after["diagnostics"],
        json!([{"slug":"other-views", "reason":"package-unavailable"}])
    );
    assert_ne!(after["revision"], before["revision"]);
}

// Catch a corrupted marker relabelling another package and breaking every unrelated contribution.
#[test]
fn ambiguous_install_identity_refuses_both_claims_and_preserves_an_independent_package() {
    let f = Fixture::new();
    f.install_set(&[
        ("proof-views", "proof/views", "<html>first</html>"),
        ("other-views", "proof/other", "<html>second</html>"),
        ("third-views", "proof/third", "<html>independent</html>"),
    ]);
    let before = f.catalogue();
    fs::write(
        f.root
            .join("capabilities/plugins/other-views")
            .join(PACKAGE_ID_MARKER),
        "use/proof/views",
    )
    .unwrap();
    let after = f.catalogue();
    assert_eq!(after["packages"].as_array().unwrap().len(), 1);
    assert_eq!(after["packages"][0]["slug"], "third-views");
    assert_eq!(
        after["diagnostics"],
        json!([
            {"slug":"other-views", "reason":"package-unavailable"},
            {"slug":"proof-views", "reason":"package-unavailable"}
        ])
    );
    for slug in ["proof-views", "other-views", "third-views"] {
        let package = before["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| {
                p["slug"]
                    == if slug == "other-views" {
                        "proof-views"
                    } else {
                        slug
                    }
            })
            .unwrap();
        let request = json!({"protocolVersion":VIEW_PROTOCOL,
            "requestId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
            "activationId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
            "instanceId":format!("{:064x}", uuid::Uuid::now_v7().as_u128()),
            "expiresAt":now_ms().unwrap()+ADMISSION_LIFETIME_MS,
            "slug":slug, "surface":"connections", "revision":package["revision"]});
        let result = f
            .local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap());
        assert_eq!(result.is_ok(), slug == "third-views");
    }
}

// Catch declared executable surfaces escaping their asset roots or surviving session retirement.
#[test]
fn installed_provider_and_miniapp_use_verified_owned_assets_and_retire_with_their_sessions() {
    let f = Fixture::new();
    f.install("<html>view</html>");
    for (surface, path, expected) in [
        (
            "tree",
            "ui/tree/provider.js",
            include_bytes!("../../tests/fixtures/aix/tree-provider/provider.js").as_slice(),
        ),
        (
            "app",
            "ui/app/app.js",
            include_bytes!("../../tests/fixtures/aix/miniapp-environment/app.js").as_slice(),
        ),
    ] {
        let mut request = f.request();
        request["surface"] = json!(surface);
        let session: Value = serde_json::from_slice(
            &f.local
                .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        let token = session["token"].as_str().unwrap();
        let asset = f.local.plugin_view_asset("member", token, path).unwrap();
        assert_eq!(asset.0, "text/javascript; charset=utf-8");
        assert_eq!(asset.1, expected);
        for foreign in [
            "ui/connections/view.js",
            if surface == "app" {
                "ui/tree/provider.js"
            } else {
                "ui/app/app.js"
            },
        ] {
            assert!(f.local.plugin_view_asset("member", token, foreign).is_err());
        }
        f.local
            .cancel_plugin_view_request(
                "member",
                request["requestId"].as_str().unwrap(),
                &cancel_body(&request),
            )
            .unwrap();
        assert!(f.local.plugin_view_asset("member", token, path).is_err());
    }
}

// Catch full-package file exposure, traversal, stale version reads and leaked bearer authority.
#[test]
fn asset_sessions_only_read_the_owned_surface_and_revoke_on_upgrade_close_and_owner_change() {
    let f = Fixture::new();
    f.install("<script src='view.js'></script>");
    let first = f.open("member-one");
    let token = first["token"].as_str().unwrap();
    assert_eq!(token.len(), 64);
    let asset = f
        .local
        .plugin_view_asset("member-one", token, "ui/connections/view.js")
        .unwrap();
    assert_eq!(asset.0, "text/javascript; charset=utf-8");
    assert!(String::from_utf8(asset.1).unwrap().contains("loaded"));
    for path in [
        "a3s-use-extension.acl",
        "README.md",
        "ui/preview.html",
        "ui/connections/../../../README.md",
        "/ui/connections/page.html",
        "ui\\connections\\page.html",
    ] {
        assert!(
            f.local
                .plugin_view_asset("member-one", token, path)
                .is_err(),
            "{path}"
        );
    }
    assert!(f.local.close_plugin_view("member-two", token).is_err());
    f.install("<html>new</html>");
    assert!(
        f.local
            .plugin_view_asset("member-one", token, "ui/connections/page.html")
            .is_err()
    );
    let second = f.open("member-one");
    let second = second["token"].as_str().unwrap();
    f.local.close_plugin_view("member-one", second).unwrap();
    assert!(
        f.local
            .plugin_view_asset("member-one", second, "ui/connections/page.html")
            .is_err()
    );
    let third = f.open("member-one");
    let third = third["token"].as_str().unwrap();
    assert!(
        f.local
            .plugin_view_asset("member-two", third, "ui/connections/page.html")
            .is_err()
    );
    assert!(
        f.local
            .plugin_view_asset("member-one", third, "ui/connections/page.html")
            .is_err()
    );
    let fourth = f.open("member-one");
    let fourth = fourth["token"].as_str().unwrap();
    f.local
        .reconcile(&[], |_| panic!("uninstall must not fetch"))
        .unwrap();
    assert!(
        f.local
            .plugin_view_asset("member-one", fourth, "ui/connections/page.html")
            .is_err()
    );
}

// Catch tampered or expired UI code being served after a valid session is minted.
#[test]
fn sessions_expire_are_bounded_and_recheck_content_before_returning_assets() {
    let f = Fixture::new();
    f.install("<html>safe</html>");
    let caps = f.root.join("capabilities");
    let mut sessions = UiSessions::default();
    let request = f.request();
    let body = serde_json::to_vec(&request).unwrap();
    for _ in 0..MAX_SESSIONS {
        sessions
            .open(&caps, "member", &serde_json::to_vec(&f.request()).unwrap())
            .unwrap();
    }
    assert_eq!(
        sessions.open(&caps, "member", &body).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    for session in sessions.sessions.values_mut() {
        session.expires = Instant::now();
    }
    let opened: Value =
        serde_json::from_slice(&sessions.open(&caps, "member", &body).unwrap()).unwrap();
    assert_eq!(sessions.sessions.len(), 1);
    let token = opened["token"].as_str().unwrap();
    let store = ReconcileStore::open(f.root.join("package-generations")).unwrap();
    let publication = store.current_sync("use/proof/views").unwrap().unwrap();
    let surface = publication
        .surfaces
        .iter()
        .find(|s| s.id == "connections")
        .unwrap();
    assert_eq!(
        sessions
            .asset(&caps, "member", token, "ui/connections/page.html")
            .unwrap()
            .1,
        b"<html>safe</html>"
    );
    fs::write(surface.entry.as_ref().unwrap(), "<script>tampered</script>").unwrap();
    assert!(
        sessions
            .asset(&caps, "member", token, "ui/connections/page.html")
            .is_err()
    );
    fs::write(surface.entry.as_ref().unwrap(), "<html>safe</html>").unwrap();
    let sibling = surface.directory.join("ui/connections/page.html.tmp");
    fs::write(&sibling, "injected").unwrap();
    assert!(
        sessions
            .asset(&caps, "member", token, "ui/connections/page.html")
            .is_err(),
        "a refused read must drop the cached view and re-verify the whole surface"
    );
    fs::remove_file(sibling).unwrap();
    fs::write(surface.entry.as_ref().unwrap(), "<script>tampered</script>").unwrap();
    let refused = f.catalogue();
    assert_eq!(refused["packages"], json!([]));
    assert_eq!(
        refused["diagnostics"],
        json!([{"slug":"proof-views", "reason":"package-content-refused"}])
    );
}

fn cancel_body(request: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"protocolVersion":VIEW_PROTOCOL,"activationId":request["activationId"],"instanceId":request["instanceId"],"expiresAt":request["expiresAt"]}))
        .unwrap()
}

// Catch a lost create response or a cancel arriving first leaving an executing native resource.
#[test]
fn cancellation_fences_late_creation_and_retires_an_unacknowledged_ticket() {
    let f = Fixture::new();
    f.install("<html>safe</html>");
    let caps = f.root.join("capabilities");
    let mut sessions = UiSessions::default();
    for create_first in [false, true] {
        let request = f.request();
        let id = request["requestId"].as_str().unwrap();
        if create_first {
            sessions
                .open(&caps, "member", &serde_json::to_vec(&request).unwrap())
                .unwrap();
        }
        let ack: Value = serde_json::from_slice(
            &sessions
                .cancel_request("member", id, &cancel_body(&request))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            ack,
            json!({"protocolVersion":VIEW_PROTOCOL,"requestId":id,"activationId":request["activationId"],"instanceId":request["instanceId"],"expiresAt":request["expiresAt"],"closed":true})
        );
        assert!(sessions.sessions.is_empty());
        assert!(
            sessions
                .open(&caps, "member", &serde_json::to_vec(&request).unwrap())
                .is_err()
        );
        assert!(
            sessions
                .cancel_request("member", id, &cancel_body(&request))
                .is_ok()
        );
    }
}

// Catch replayed creation, cross-owner cancel, deadline changes and old/unbounded protocol admission.
#[test]
fn request_identity_owner_deadline_and_protocol_are_checked_before_resource_mutation() {
    let f = Fixture::new();
    f.install("<html>safe</html>");
    let caps = f.root.join("capabilities");
    let mut sessions = UiSessions::default();
    let request = f.request();
    let body = serde_json::to_vec(&request).unwrap();
    let opened: Value =
        serde_json::from_slice(&sessions.open(&caps, "member", &body).unwrap()).unwrap();
    let id = request["requestId"].as_str().unwrap();
    let token = opened["token"].as_str().unwrap();
    assert!(sessions.open(&caps, "member", &body).is_err());
    let mut duplicate_instance = request.clone();
    duplicate_instance["requestId"] = json!(format!("{:064x}", uuid::Uuid::now_v7().as_u128()));
    assert!(
        sessions
            .open(
                &caps,
                "member",
                &serde_json::to_vec(&duplicate_instance).unwrap()
            )
            .is_err()
    );
    assert!(
        sessions
            .cancel_request("other-member", id, &cancel_body(&request))
            .is_err()
    );
    let mut changed = request.clone();
    changed["activationId"] = json!("f".repeat(64));
    assert!(
        sessions
            .cancel_request("member", id, &cancel_body(&changed))
            .is_err()
    );
    assert!(
        sessions
            .asset(&caps, "member", token, "ui/connections/page.html")
            .is_ok()
    );
    for (key, value) in [
        ("protocolVersion", json!(1)),
        ("requestId", json!("bad")),
        ("activationId", json!("bad")),
        ("instanceId", json!("bad")),
        ("expiresAt", json!(0)),
        ("expiresAt", json!(now_ms().unwrap() - 1)),
        (
            "expiresAt",
            json!(now_ms().unwrap() + ADMISSION_LIFETIME_MS * 2),
        ),
        ("unknown", json!(true)),
    ] {
        let mut invalid = f.request();
        invalid[key] = value;
        assert!(
            sessions
                .open(&caps, "member", &serde_json::to_vec(&invalid).unwrap())
                .is_err()
        );
    }
    assert_eq!(sessions.sessions.len(), 1);
}

// Catch expired cancellation fences being deleted and then reopened after a clock rollback.
#[test]
fn expired_requests_stay_expired_when_wall_clock_moves_backwards() {
    let f = Fixture::new();
    f.install("<html>safe</html>");
    let caps = f.root.join("capabilities");
    let mut sessions = UiSessions::default();
    let request = f.request();
    let id = request["requestId"].as_str().unwrap();
    sessions.observed_time = request["expiresAt"].as_u64().unwrap() + 1;
    sessions
        .cancel_request("member", id, &cancel_body(&request))
        .unwrap();
    assert!(sessions.requests.is_empty());
    assert!(
        sessions
            .open(&caps, "member", &serde_json::to_vec(&request).unwrap())
            .is_err()
    );
    assert!(sessions.sessions.is_empty());
}

// Catch admission expiry discarding the identity needed to close a longer-lived view session.
#[test]
fn expired_admission_still_retires_its_live_session_with_the_original_owner_and_deadline() {
    let f = Fixture::new();
    f.install("<html>safe</html>");
    let caps = f.root.join("capabilities");
    let mut sessions = UiSessions::default();
    let request = f.request();
    let id = request["requestId"].as_str().unwrap();
    let opened: Value = serde_json::from_slice(
        &sessions
            .open(&caps, "member", &serde_json::to_vec(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    let token = opened["token"].as_str().unwrap();
    sessions.observed_time = request["expiresAt"].as_u64().unwrap() + 1;
    assert!(
        sessions
            .cancel_request("other-member", id, &cancel_body(&request))
            .is_err()
    );
    assert!(sessions.requests.is_empty());
    let mut changed = request.clone();
    changed["expiresAt"] = json!(request["expiresAt"].as_u64().unwrap() - 1);
    assert!(
        sessions
            .cancel_request("member", id, &cancel_body(&changed))
            .is_err()
    );
    assert!(
        sessions
            .asset(&caps, "member", token, "ui/connections/page.html")
            .is_ok()
    );
    sessions
        .cancel_request("member", id, &cancel_body(&request))
        .unwrap();
    assert!(
        sessions
            .asset(&caps, "member", token, "ui/connections/page.html")
            .is_err()
    );
    assert!(sessions.sessions.is_empty());
}

// Catch an unbounded cancellation history or a false ACK when no late-create fence can be stored.
#[test]
fn cancellation_fences_are_bounded_and_expiration_reclaims_capacity() {
    let mut sessions = UiSessions::default();
    let deadline = now_ms().unwrap() + ADMISSION_LIFETIME_MS;
    for index in 0..MAX_REQUEST_FENCES {
        let identity = format!("{index:064x}");
        let body = serde_json::to_vec(&json!({"protocolVersion":VIEW_PROTOCOL,"activationId":identity,"instanceId":identity,"expiresAt":deadline})).unwrap();
        sessions.cancel_request("member", &identity, &body).unwrap();
    }
    let identity = format!("{:064x}", MAX_REQUEST_FENCES);
    let body = serde_json::to_vec(&json!({"protocolVersion":VIEW_PROTOCOL,"activationId":identity,"instanceId":identity,"expiresAt":deadline})).unwrap();
    assert_eq!(
        sessions
            .cancel_request("member", &identity, &body)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    sessions.observed_time = deadline + 1;
    sessions
        .cancel_request("member", &format!("{:064x}", MAX_REQUEST_FENCES), &body)
        .unwrap();
    assert!(sessions.requests.is_empty());
}

/// A use-registry package as the catalog publishes it: everything but tests, tools and brand.
pub(super) fn workbench_bundle(name: &str) -> Vec<u8> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../use-registry/packages")
        .join(name);
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if relative.starts_with('.')
                || ["tests", "tools", "brand", "src"].contains(&relative.as_str())
            {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push((relative, fs::read(&path).unwrap()));
            }
        }
    }
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn workbench_view_load_timing(name: &str) {
    // A3S_STUDIO_PROBE_TENANT=<a Studio tenant directory> times a copy of its
    // installed set (`capabilities` and `package-generations` only) instead of
    // a store holding only this package.
    let f = match std::env::var_os("A3S_STUDIO_PROBE_TENANT") {
        Some(source) => {
            let root = std::env::temp_dir().join(format!("a3s-workbench-{}", uuid::Uuid::now_v7()));
            fs::create_dir_all(&root).unwrap();
            for part in ["capabilities", "package-generations"] {
                let copied = std::process::Command::new("cp")
                    .arg("-R")
                    .arg(std::path::Path::new(&source).join(part))
                    .arg(root.join(part))
                    .status()
                    .unwrap();
                assert!(copied.success());
            }
            let local = LocalCapabilities::open_at(Some(root.join("capabilities")));
            Fixture { root, local }
        }
        None => {
            let f = Fixture::new();
            let bytes = workbench_bundle(name);
            let entry = json!({"slug":format!("a3s-{name}"), "kind":"plugin", "installed":true, "install":{
                "type":"plugin", "packageType":"aix", "available":true, "packageId":format!("use/a3s/{name}"),
                "digest":super::super::digest_of(&bytes), "size":bytes.len() }});
            f.local.reconcile(&[entry], |_| Ok(bytes.clone())).unwrap();
            f
        }
    };
    let started = Instant::now();
    let catalogue = f.catalogue();
    eprintln!("catalogue {:?}", started.elapsed());
    let catalogue = json!({"packages": catalogue["packages"].as_array().unwrap().iter()
        .filter(|p| p["slug"] == format!("a3s-{name}")).cloned().collect::<Vec<_>>()});
    let started = Instant::now();
    let request = json!({"protocolVersion":VIEW_PROTOCOL,"requestId":format!("{:064x}", 1u128),"activationId":format!("{:064x}", 2u128),"instanceId":format!("{:064x}", 3u128),"expiresAt":now_ms().unwrap()+ADMISSION_LIFETIME_MS,"slug":format!("a3s-{name}"), "surface":"workbench", "revision":catalogue["packages"][0]["revision"]});
    let opened: Value = serde_json::from_slice(
        &f.local
            .open_plugin_view("member", &serde_json::to_vec(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    eprintln!("open {:?}", started.elapsed());
    let token = opened["token"].as_str().unwrap();
    let all = Instant::now();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../use-registry/packages")
        .join(name)
        .join("ui/workbench");
    let mut paths = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "js" || ext == "css" || ext == "html" || ext == "woff2")
            {
                paths.push(
                    path.strip_prefix(&root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    paths.sort();
    for path in paths {
        let one = Instant::now();
        let (_, body) = f
            .local
            .plugin_view_asset("member", token, &format!("ui/workbench/{path}"))
            .unwrap();
        eprintln!("{path} {} bytes {:?}", body.len(), one.elapsed());
    }
    eprintln!("all assets {:?}", all.elapsed());
}

#[test]
#[ignore = "timing probe"]
fn cobot_arm_view_load_timing() {
    workbench_view_load_timing("cobot-arm");
}

#[test]
#[ignore = "timing probe"]
fn g1_arm_view_load_timing() {
    workbench_view_load_timing("g1-arm");
}

#[test]
#[ignore = "timing probe"]
fn iris_view_load_timing() {
    workbench_view_load_timing("iris");
}
