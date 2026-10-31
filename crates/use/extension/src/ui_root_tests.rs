use super::{inspect_ui_surface_files, list_ui_package_assets};
use crate::ExtensionManifest;

const SPA: &str = r#"
extension "acme/spa" {
  schema_version = 3
  version        = "1.0.0"
  route          = "spa"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url = "https://github.com/aster-ai-lab/Use-Registry"
  }

  ui "panel" {
    title    = "Panel"
    entry    = "ui/panel/index.html"
    root     = "ui/panel"
    optional = false
  }
}
"#;

fn write_spa(root: &std::path::Path) {
    let panel = root.join("ui/panel");
    std::fs::create_dir_all(&panel).unwrap();
    std::fs::write(root.join("README.md"), "# spa\n").unwrap();
    std::fs::write(panel.join("index.html"), "<html><body>panel</body></html>").unwrap();
    std::fs::write(panel.join("app.js"), "render()\n").unwrap();
    std::fs::write(panel.join("app.css"), "body{margin:0}\n").unwrap();
    std::fs::write(panel.join("mark.png"), [0x89, b'P', b'N', b'G', 0x0d, 0x0a]).unwrap();
}

#[tokio::test]
async fn ui_root_digests_the_whole_tree_and_refuses_a_foreign_file() {
    let temp = tempfile::tempdir().unwrap();
    write_spa(temp.path());
    let manifest = ExtensionManifest::parse_acl(SPA).unwrap();
    let surface = &manifest.ui[0];
    assert_eq!(
        surface.root.as_deref(),
        Some(std::path::Path::new("ui/panel"))
    );

    let assets = list_ui_package_assets(surface, temp.path()).await.unwrap();
    let names: Vec<_> = assets
        .iter()
        .map(|asset| asset.relative_path.display().to_string())
        .collect();
    assert_eq!(
        names,
        vec![
            "ui/panel/app.css",
            "ui/panel/app.js",
            "ui/panel/index.html",
            "ui/panel/mark.png",
        ]
    );
    assert!(
        !assets
            .iter()
            .find(|asset| asset.relative_path.ends_with("mark.png"))
            .unwrap()
            .text
    );

    let first = inspect_ui_surface_files(surface, temp.path())
        .await
        .unwrap();
    assert_eq!(first.file_count(), 4);
    std::fs::write(temp.path().join("ui/panel/app.js"), "render(2)\n").unwrap();
    let second = inspect_ui_surface_files(surface, temp.path())
        .await
        .unwrap();
    assert_ne!(first.digest(), second.digest());

    std::fs::write(temp.path().join("ui/panel/notes.txt"), "no").unwrap();
    let error = list_ui_package_assets(surface, temp.path())
        .await
        .unwrap_err();
    assert_eq!(error.code, "use.extension.ui_asset_invalid");
    assert!(error.message.contains("media set"));

    std::fs::remove_file(temp.path().join("ui/panel/notes.txt")).unwrap();
    #[cfg(unix)]
    {
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret"), "x").unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), temp.path().join("ui/panel/secret"))
            .unwrap();
        let error = list_ui_package_assets(surface, temp.path())
            .await
            .unwrap_err();
        assert_eq!(error.code, "use.extension.ui_asset_invalid");
        assert!(error.message.contains("must not be a link"));
    }
}

#[test]
fn ui_entry_must_stay_inside_the_declared_root() {
    let acl = SPA.replace(
        "entry    = \"ui/panel/index.html\"",
        "entry    = \"other/index.html\"",
    );
    let error = ExtensionManifest::parse_acl(&acl).unwrap_err();
    assert_eq!(error.code, "use.extension.manifest_invalid");
    assert!(error.message.contains("inside its root"));
}
