use super::*;
use std::io::Write;

const MANIFEST: &str = r#"extension "aster/sample" {
  schema_version = 3
  version = "1.2.3"
  route = "sample"
  requires_use = ">=0.3.0, <0.4.0"
  actions = ["read"]
  repository {
    url = "https://github.com/aster-ai-lab/plugins"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }
  skill "guide" {
    path = "skills/guide/SKILL.md"
    requires_tool = []
    requires_mcp = []
    optional = false
  }
}"#;

#[tokio::test]
async fn registry_package_build_produces_a_checkable_archive_and_preserves_existing_output() {
    let temporary = tempfile::tempdir().unwrap();
    let repository = temporary.path().join("源码 repository");
    let source = repository.join("plugins/knowledge/aster/sample");
    std::fs::create_dir_all(source.join("skills/guide")).unwrap();
    std::fs::write(
        repository.join("domains.acl"),
        r#"plugin_domains {
      schema_version = 1
      domain "knowledge" { owners = ["fixture-owner"] publishers = ["aster"] }
    }"#,
    )
    .unwrap();
    for (name, bytes) in [
        ("aster-use-extension.acl", MANIFEST),
        ("README.md", "# Build fixture\nNative archive.\n"),
        ("LICENSE", "MIT\nSynthetic fixture license.\n"),
        ("skills/guide/SKILL.md", "# Guide\nUse this fixture.\n"),
    ] {
        std::fs::write(source.join(name), bytes).unwrap();
    }
    let output = temporary.path().join("插件 artifact.aix");
    let args = vec![
        "package".into(),
        "build".into(),
        source.to_str().unwrap().into(),
        "--output".into(),
        output.to_str().unwrap().into(),
        "--json".into(),
    ];
    let result = run(&args).await.unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        result.json["data"]["aixPackage"]["packageId"],
        "aster/sample"
    );
    let check = run(&[
        "package".into(),
        "check".into(),
        output.to_str().unwrap().into(),
        "--json".into(),
    ])
    .await
    .unwrap();
    assert_eq!(
        result.json["data"]["aixPackage"],
        check.json["data"]["aixPackage"]
    );
    let manifest = temporary.path().join("manifest.json");
    let publication = run(&[
        "package".into(),
        "prepare".into(),
        output.to_str().unwrap().into(),
        "--repository".into(),
        repository.to_str().unwrap().into(),
        "--domain".into(),
        "knowledge".into(),
        "--output".into(),
        manifest.to_str().unwrap().into(),
        "--json".into(),
    ])
    .await
    .unwrap();
    assert_eq!(
        publication.json["data"]["aixPublication"]["package"],
        result.json["data"]["aixPackage"]
    );
    assert!(manifest.is_file());
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(run(&args).await.err().unwrap().code, "use.extension.io");
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    assert_eq!(
        run(&[
            "package".into(),
            "build".into(),
            source.to_str().unwrap().into()
        ])
        .await
        .err()
        .unwrap()
        .code,
        "use.cli.invalid_usage"
    );
}

#[tokio::test]
async fn registry_package_check_validates_a_native_archive_without_installing_it() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("插件 sample.aix");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (name, content) in [
        ("aster-use-extension.acl", MANIFEST),
        (
            "README.md",
            "# Sample plugin\nA native inspection fixture.\n",
        ),
        (
            "skills/guide/SKILL.md",
            "# Guide\nInspect the native package.\n",
        ),
    ] {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(content.as_bytes()).unwrap();
    }
    archive.finish().unwrap();
    let args = vec![
        "package".into(),
        "check".into(),
        path.to_str().unwrap().into(),
        "--json".into(),
    ];
    let result = run(&args).await.unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        result.json.pointer("/data/aixPackage/packageId"),
        Some(&serde_json::json!("aster/sample"))
    );
    assert_eq!(
        result.json.pointer("/data/aixPackage/version"),
        Some(&serde_json::json!("1.2.3"))
    );
    assert_eq!(
        result.json.pointer("/data/aixPackage/fileCount"),
        Some(&serde_json::json!(3))
    );
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 1);

    let mut unknown = args.clone();
    unknown.push("--trust".into());
    assert_eq!(
        run(&unknown).await.err().unwrap().code,
        "use.cli.invalid_usage"
    );
    let missing = vec!["package".into(), "check".into()];
    assert_eq!(
        run(&missing).await.err().unwrap().code,
        "use.cli.invalid_usage"
    );
}

#[tokio::test]
async fn registry_package_prepare_writes_exact_oci_bytes_and_preserves_existing_outputs() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("中文 repository");
    let source = root.join("plugins/knowledge/aster/sample");
    std::fs::create_dir_all(source.join("skills/guide")).unwrap();
    std::fs::write(
        root.join("domains.acl"),
        r#"plugin_domains {
 schema_version = 1
 domain "knowledge" { owners = ["fixture-owner"] publishers = ["aster"] }
}"#,
    )
    .unwrap();
    let archive_path = temporary.path().join("sample.aix");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive_path).unwrap());
    for (name, content) in [
        ("aster-use-extension.acl", MANIFEST),
        ("README.md", "# Sample\nNative source.\n"),
        ("LICENSE", "MIT\nSynthetic fixture.\n"),
        ("skills/guide/SKILL.md", "# Guide\nUse this fixture.\n"),
    ] {
        std::fs::write(source.join(name), content).unwrap();
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    let output = temporary.path().join("OCI 清单.json");
    let mut args = vec![
        "package".into(),
        "prepare".into(),
        archive_path.to_str().unwrap().into(),
        "--repository".into(),
        root.to_str().unwrap().into(),
        "--domain".into(),
        "knowledge".into(),
        "--output".into(),
        output.to_str().unwrap().into(),
        "--json".into(),
    ];
    let result = run(&args).await.unwrap();
    assert_eq!(result.exit_code, 0);
    let bytes = std::fs::read(&output).unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(
        result.json["data"]["aixPublication"]["manifestSha256"],
        format!("sha256:{:x}", Sha256::digest(&bytes))
    );
    assert_eq!(
        result.json["data"]["aixPublication"]["ociRepository"],
        "ghcr.io/aster-ai-lab/plugins/knowledge/aster/sample"
    );
    let manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        manifest["layers"][0]["annotations"]["org.opencontainers.image.title"],
        "sample.aix"
    );
    assert_eq!(run(&args).await.err().unwrap().code, "use.cli.io");
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    args[8] = source.join("manifest.json").to_str().unwrap().into();
    assert_eq!(
        run(&args).await.err().unwrap().code,
        "use.cli.invalid_usage"
    );
    assert!(!source.join("manifest.json").exists());
    args.push("--trust".into());
    assert_eq!(
        run(&args).await.err().unwrap().code,
        "use.cli.invalid_usage"
    );
}
