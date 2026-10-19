#![cfg(test)]

use super::super::LocalCapabilities;
use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};

struct Fixture {
    root: std::path::PathBuf,
    local: LocalCapabilities,
}

impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("a3s-intelligent-ui-{}", uuid::Uuid::now_v7()));
        let local = LocalCapabilities::open_at(Some(root.join("capabilities")));
        Self { root, local }
    }
    fn install(&self, bytes: Vec<u8>) {
        let entry = json!({"slug":"intelligent-ui","kind":"plugin","installed":true,"install":{
            "type":"plugin","packageType":"aix","available":true,"packageId":"use/a3s/intelligent-ui",
            "digest":format!("sha256:{:x}", Sha256::digest(&bytes)), "size":bytes.len()
        }});
        self.local
            .reconcile(&[entry], |_| Ok(bytes.clone()))
            .unwrap();
    }
    fn read(&self) -> io::Result<Vec<u8>> {
        self.local.plugin_ui("intelligent-ui", "message")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn archive(html: &str) -> Vec<u8> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/intelligent-ui-aix");
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in [
        "a3s-use-extension.acl",
        "README.md",
        "skills/intelligent-ui/SKILL.md",
        "ui/message/index.html",
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        let bytes = if name.ends_with("index.html") {
            html.as_bytes().to_vec()
        } else {
            fs::read(if name.starts_with("skills/") {
                base.join("../intelligent-ui").join(name)
            } else {
                base.join(name)
            })
            .unwrap()
        };
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn installed_declared_ui_uses_the_current_verified_selection_and_uninstall_revokes_it() {
    let f = Fixture::new();
    assert!(f.read().is_err());
    f.install(archive("<html>first</html>"));
    let first: Value = serde_json::from_slice(&f.read().unwrap()).unwrap();
    assert_eq!(first["html"], "<html>first</html>");
    assert_eq!(first["packageId"], "use/a3s/intelligent-ui");
    assert!(f.local.plugin_ui("intelligent-ui", "undeclared").is_err());
    assert!(f.local.plugin_ui("../aix-ui", "message").is_err());
    f.install(archive("<html>second</html>"));
    let second: Value = serde_json::from_slice(&f.read().unwrap()).unwrap();
    assert_ne!(first["revision"], second["revision"]);
    assert_eq!(second["html"], "<html>second</html>");
    f.local
        .reconcile(&[], |_| panic!("uninstall must not download"))
        .unwrap();
    assert!(f.read().is_err());
}

#[test]
fn changed_generation_content_or_symlink_installation_cannot_supply_ui_code() {
    let f = Fixture::new();
    f.install(archive("<html>safe</html>"));
    let store = ReconcileStore::open(f.root.join("package-generations")).unwrap();
    let publication = store
        .current_sync("use/a3s/intelligent-ui")
        .unwrap()
        .unwrap();
    let surface = publication
        .surfaces
        .iter()
        .find(|s| s.kind == SurfaceKind::Ui)
        .unwrap();
    fs::write(
        surface.directory.join(surface.entry.as_ref().unwrap()),
        "<script>tampered</script>",
    )
    .unwrap();
    assert!(f.read().is_err());
}

#[cfg(unix)]
#[test]
fn an_installed_directory_link_is_not_an_admission() {
    let f = Fixture::new();
    f.install(archive("<html>safe</html>"));
    let plugin = f.root.join("capabilities/plugins/intelligent-ui");
    let moved = f.root.join("moved");
    fs::rename(&plugin, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &plugin).unwrap();
    assert!(f.read().is_err());
}

#[test]
#[ignore = "build packages/intelligent-ui-aix first; certifies the real AIX artifact"]
fn built_intelligent_ui_aix_installs_and_exports_a_verified_document() {
    let f = Fixture::new();
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/intelligent-ui-aix");
    f.install(fs::read(base.join("dist/a3s-intelligent-ui-0.1.0.aix")).unwrap());
    let bytes = f.read().unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let html = value["html"].as_str().unwrap();
    assert!(html.contains("script-src 'sha256-"));
    assert!(html.contains("connect-src 'none'"));
    if let Ok(target) = std::env::var("A3S_CLOUD_INTELLIGENT_UI_CERT_DOCUMENT") {
        fs::write(target, bytes).unwrap();
    }
}
