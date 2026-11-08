#![cfg(test)]

use super::*;

fn archive(path: &Path, entry: &str, bytes: &[u8]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    writer
        .start_file(entry, zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(bytes).unwrap();
    writer.finish().unwrap();
}

#[tokio::test]
async fn aix_extracts_the_existing_acl_manifest_and_preserves_package_identity() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("acme-research-2.0.0.aix");
    let bytes = include_bytes!("../fixtures/packages/plugin-v3/package/aster-use-extension.acl");
    let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/packages/plugin-v3/package");
    let mut stack = vec![package.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(&package)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let options = zip::write::SimpleFileOptions::default();
            #[cfg(unix)]
            let options = {
                use std::os::unix::fs::PermissionsExt;
                options.unix_permissions(std::fs::metadata(&path).unwrap().permissions().mode())
            };
            writer.start_file(relative, options).unwrap();
            writer.write_all(&std::fs::read(path).unwrap()).unwrap();
        }
    }
    writer.finish().unwrap();
    let source = prepare_package_source(&path).await.unwrap();
    assert_eq!(
        fs::read(source.root().join(MANIFEST_NAME)).await.unwrap(),
        bytes
    );
    let (manifest, _) = super::super::package::read_manifest(source.root())
        .await
        .unwrap();
    assert_eq!(manifest.package_id, "acme/research");
    assert_eq!(manifest.version.to_string(), "2.0.0");
    let inspected = super::super::inspect_aix_package(&path).await.unwrap();
    assert_eq!(inspected.package_id, "acme/research");
    assert_eq!(inspected.version, "2.0.0");
    assert_eq!(inspected.surfaces, ["tool", "mcp", "skill", "ui"]);
    assert_eq!(inspected.package_sha256.len(), 64);
}

#[tokio::test]
async fn aix_refuses_traversal_and_a_renamed_vsix_without_the_native_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let traversal = temp.path().join("traversal.aix");
    archive(&traversal, "../aster-use-extension.acl", b"escape");
    assert_eq!(
        super::super::inspect_aix_package(&traversal)
            .await
            .unwrap_err()
            .code,
        "use.extension.path_escape"
    );
    let foreign = temp.path().join("foreign.aix");
    archive(
        &foreign,
        "extension/package.json",
        br#"{"publisher":"sample","name":"foreign"}"#,
    );
    assert_eq!(
        super::super::inspect_aix_package(&foreign)
            .await
            .unwrap_err()
            .code,
        "use.extension.package_layout_invalid"
    );
    let invalid = temp.path().join("invalid.aix");
    fs::write(&invalid, b"not an archive").await.unwrap();
    assert_eq!(
        super::super::inspect_aix_package(&invalid)
            .await
            .unwrap_err()
            .code,
        "use.extension.package_archive_invalid"
    );
}

#[tokio::test]
async fn aix_refuses_incomplete_native_packages_and_nested_manifest_roots() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = include_bytes!("../fixtures/packages/plugin-v3/package/aster-use-extension.acl");
    let incomplete = temp.path().join("incomplete.aix");
    archive(&incomplete, MANIFEST_NAME, manifest);
    assert_eq!(
        super::super::inspect_aix_package(&incomplete)
            .await
            .unwrap_err()
            .code,
        "use.extension.package_file_missing"
    );
    let nested = temp.path().join("nested.aix");
    archive(&nested, "package/aster-use-extension.acl", manifest);
    assert_eq!(
        super::super::inspect_aix_package(&nested)
            .await
            .unwrap_err()
            .code,
        "use.extension.package_layout_invalid"
    );
}

#[tokio::test]
async fn published_aix_inspection_refuses_directories_and_other_extensions() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["legacy.zip", "uppercase.AIX"] {
        let path = temp.path().join(name);
        fs::write(&path, b"not a native archive").await.unwrap();
        assert_eq!(
            super::super::inspect_aix_package(&path)
                .await
                .unwrap_err()
                .code,
            "use.extension.package_unsupported"
        );
    }
    let directory = temp.path().join("native.aix");
    fs::create_dir(&directory).await.unwrap();
    assert_eq!(
        super::super::inspect_aix_package(&directory)
            .await
            .unwrap_err()
            .code,
        "use.extension.package_unsupported"
    );
}
