use super::*;

#[tokio::test]
async fn ui_projection_keeps_non_web_package_files_out_of_the_asset_descriptor() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("界面 files");
    std::fs::create_dir_all(root.join("ui")).unwrap();
    for (name, bytes) in [
        ("index.html", b"<html>native UI</html>".as_slice()),
        ("app.js", b"document.title = 'native'".as_slice()),
        ("cover.jpeg", b"\xff\xd8\xff\xd9".as_slice()),
        ("LICENSE", b"Original license bytes\n".as_slice()),
        ("THIRD_PARTY_NOTICES.md", b"# Notices\n".as_slice()),
    ] {
        std::fs::write(root.join("ui").join(name), bytes).unwrap();
    }
    let manifest = aster_use_extension::ExtensionManifest::parse_acl(
        r#"
extension "example/ui-inventory" {
 schema_version = 3 version = "1.0.0" route = "ui-inventory"
 requires_use = ">=0.3.0, <0.4.0" actions = ["read"]
 repository { url = "https://github.com/example/ui-inventory" }
 ui "panel" { entry = "ui/index.html" root = "ui" }
}"#,
    )
    .unwrap();
    let (entry, _, scripts, assets) = project_ui_root(&manifest.ui[0], &root).await.unwrap();
    assert_eq!(entry.path, root.join("ui/index.html"));
    assert_eq!(scripts.len(), 1);
    assert_eq!(assets.len(), 3);
    assert!(assets
        .iter()
        .any(|asset| asset.path.ends_with("cover.jpeg") && asset.media_type == "image/jpeg"));
    assert!(!assets
        .iter()
        .any(|asset| asset.path.ends_with("LICENSE")
            || asset.path.ends_with("THIRD_PARTY_NOTICES.md")));
    assert_eq!(
        std::fs::read(root.join("ui/LICENSE")).unwrap(),
        b"Original license bytes\n"
    );
}
