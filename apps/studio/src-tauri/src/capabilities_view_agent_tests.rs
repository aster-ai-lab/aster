#![cfg(test)]

use super::super::LocalCapabilities;
use super::*;
use serde_json::Value;
use std::io::{Cursor, Write};

const AGENT: &str = r#"agent "lab" {
    harness = "package" runtime = "edge" entry = "agent/run.mjs" optional = false
    resources { cpu_millis = 500 memory_bytes = 268435456 }
  }"#;

struct Fixture {
    root: std::path::PathBuf,
    local: LocalCapabilities,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("a3s-view-agent-{}", uuid::Uuid::now_v7()));
        let local = LocalCapabilities::open_at(Some(root.join("capabilities")));
        Self { root, local }
    }

    /// Each package is `(slug, package id, agent declaration, agent program)`.
    fn install(&self, packages: &[(&str, &str, &str, &str)]) {
        let mut entries = Vec::new();
        let mut bundles = BTreeMap::new();
        for (slug, package_id, agent, program) in packages {
            let acl = format!(
                r#"extension "{package_id}" {{
  schema_version = 3 version = "1.0.0" requires_use = ">=0.3.0, <0.4.0" route = "{slug}" actions = ["read"]
  repository {{ url = "https://example.org/{package_id}" }}
  view_container "lab" {{ title = "Lab" icon = "bot" location = "activitybar" }}
  ui "workbench" {{ title = "Lab" entry = "ui/index.html" container = "lab" }}
  {agent}
}}"#
            );
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for (name, data) in [
                ("a3s-use-extension.acl", acl.as_str()),
                ("README.md", "# Lab"),
                ("ui/index.html", "<html>lab</html>"),
                ("agent/run.mjs", *program),
                ("agents/lab/INSTRUCTIONS.md", "Answer briefly."),
            ] {
                zip.start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(data.as_bytes()).unwrap();
            }
            let bytes = zip.finish().unwrap().into_inner();
            entries.push(json!({"slug":slug, "kind":"plugin", "installed":true, "install":{
                "type":"plugin", "packageType":"aix", "available":true, "packageId":format!("use/{package_id}"),
                "digest":super::super::digest_of(&bytes), "size":bytes.len() }}));
            bundles.insert((*slug).to_owned(), bytes);
        }
        self.local
            .reconcile(&entries, |slug| {
                bundles.get(slug).cloned().ok_or_else(missing)
            })
            .unwrap();
    }

    fn open(&self, owner: &str, slug: &str) -> String {
        let catalogue: Value = serde_json::from_slice(&self.local.plugin_views().unwrap()).unwrap();
        let revision = catalogue["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["slug"] == slug)
            .unwrap()["revision"]
            .clone();
        let identity = || format!("{:064x}", uuid::Uuid::now_v7().as_u128());
        let request = json!({"protocolVersion":VIEW_PROTOCOL,"requestId":identity(),"activationId":identity(),"instanceId":identity(),"expiresAt":now_ms().unwrap()+ADMISSION_LIFETIME_MS,"slug":slug, "surface":"workbench", "revision":revision});
        let session: Value = serde_json::from_slice(
            &self
                .local
                .open_plugin_view(owner, &serde_json::to_vec(&request).unwrap())
                .unwrap(),
        )
        .unwrap();
        session["token"].as_str().unwrap().to_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// Catch a view reaching an agent its own package publication does not declare as an edge package.
#[test]
fn a_view_binds_only_its_own_declared_edge_package_agent() {
    let f = Fixture::new();
    f.install(&[
        ("lab-one", "proof/lab-one", AGENT, "// one"),
        ("lab-two", "proof/lab-two", AGENT, "// two"),
    ]);
    let token = f.open("member", "lab-one");
    let agent = f.local.plugin_view_agent("member", &token, "lab").unwrap();
    assert_eq!(fs::read_to_string(&agent.entry).unwrap(), "// one");
    assert_eq!(agent.declared_entry, "agent/run.mjs");
    assert!(agent.release_identity.starts_with("sha256:"));
    assert_eq!((agent.cpu_millis, agent.memory_bytes), (500, 268435456));
    assert!(
        f.local
            .plugin_view_agent("member", &token, "other")
            .is_err()
    );
    assert!(
        f.local
            .plugin_view_agent("member", &"0".repeat(64), "lab")
            .is_err()
    );
    f.local.close_plugin_view("member", &token).unwrap();
    assert!(f.local.plugin_view_agent("member", &token, "lab").is_err());
}

// Catch a stale view, a signed-out owner, or a non-package agent still reaching a box.
#[test]
fn upgrade_owner_change_and_undeclared_harness_refuse_the_binding() {
    let f = Fixture::new();
    f.install(&[("lab-one", "proof/lab-one", AGENT, "// one")]);
    let token = f.open("member", "lab-one");
    assert!(
        f.local
            .plugin_view_agent("member-two", &token, "lab")
            .is_err()
    );
    assert!(f.local.plugin_view_agent("member", &token, "lab").is_err());

    let token = f.open("member", "lab-one");
    f.install(&[("lab-one", "proof/lab-one", AGENT, "// upgraded")]);
    assert!(f.local.plugin_view_agent("member", &token, "lab").is_err());
    let token = f.open("member", "lab-one");
    let agent = f.local.plugin_view_agent("member", &token, "lab").unwrap();
    assert_eq!(fs::read_to_string(&agent.entry).unwrap(), "// upgraded");

    let builtin = r#"agent "lab" {
    harness = "builtin" runtime = "edge" instructions = "agents/lab/INSTRUCTIONS.md" optional = false
    resources { cpu_millis = 500 memory_bytes = 268435456 }
  }"#;
    f.install(&[("lab-one", "proof/lab-one", builtin, "// builtin")]);
    let token = f.open("member", "lab-one");
    assert!(f.local.plugin_view_agent("member", &token, "lab").is_err());
}
