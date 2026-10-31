#![cfg(test)]

use super::ExtensionManifest;
use std::collections::BTreeSet;
use std::path::PathBuf;

const MANIFEST: &str = r#"
extension "proof/views" {
 schema_version = 3
 version = "1.0.0"
 requires_use = ">=0.3.0, <0.4.0"
 route = "proof-views"
 actions = ["read"]
 repository { url = "https://example.org/proof/views" }
 view_container "resources" { title = "Resources" icon = "database" location = "activitybar" }
 ui "connections" { container = "resources" entry = "ui/connections/index.html" scripts = ["ui/connections/app.js"] }
 ui "preview" { container = "resources" entry = "ui/preview/index.html" }
}
"#;

fn inventory() -> BTreeSet<PathBuf> {
    [
        "aster-use-extension.acl",
        "README.md",
        "ui/connections/index.html",
        "ui/connections/app.js",
        "ui/preview/index.html",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

#[test]
fn aix_inventory_requires_every_owned_view_entry_and_script() {
    let manifest = ExtensionManifest::parse_acl(MANIFEST).unwrap();
    let files = inventory();
    manifest.validate_file_inventory(&files).unwrap();
    for path in &files {
        let mut missing = files.clone();
        missing.remove(path);
        assert_eq!(
            manifest.validate_file_inventory(&missing).unwrap_err().code,
            "use.extension.package_file_missing"
        );
    }
}

#[test]
fn aix_inventory_rejects_escape_and_file_directory_aliases() {
    let manifest = ExtensionManifest::parse_acl(MANIFEST).unwrap();
    for path in ["../outside", "/outside", "ui/connections"] {
        let mut files = inventory();
        files.insert(PathBuf::from(path));
        assert!(manifest.validate_file_inventory(&files).is_err());
    }
    let mut files = inventory();
    files.remove(&PathBuf::from("ui/preview/index.html"));
    files.insert(PathBuf::from("ui/preview/index.html/child"));
    assert!(
        manifest.validate_file_inventory(&files).is_err(),
        "a directory prefix is not an HTML entry file"
    );
}
