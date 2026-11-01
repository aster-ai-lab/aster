use super::*;
use std::io::Write;

pub(super) const POLICY: &str = r#"plugin_domains {
  schema_version = 1
  domain "knowledge" {
    owners = ["fixture-maintainer"]
    publishers = ["acme"]
  }
  domain "development" {
    owners = ["fixture-other"]
    publishers = ["other"]
  }
}"#;

const MANIFEST: &str = r#"extension "acme/guide" {
  schema_version = 3
  version = "1.2.3"
  route = "guide"
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

struct Fixture {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    source: PathBuf,
    archive: PathBuf,
}

fn fixture() -> Fixture {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("插件 source with spaces");
    let source = root.join("plugins/acme/guide");
    std::fs::create_dir_all(source.join("skills/guide")).unwrap();
    std::fs::write(root.join("domains.acl"), POLICY).unwrap();
    let archive = root.join("guide.aix");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    for (path, bytes) in [
        ("aster-use-extension.acl", MANIFEST),
        ("README.md", "# Guide\nA native package.\n"),
        ("LICENSE", "MIT\nSynthetic fixture license.\n"),
        ("skills/guide/SKILL.md", "# Guide\nUse the native source.\n"),
    ] {
        std::fs::write(source.join(path), bytes).unwrap();
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    Fixture {
        _temporary: temporary,
        root,
        source,
        archive,
    }
}

#[tokio::test]
async fn publication_binds_explicit_domain_source_archive_and_exact_oci_bytes() {
    let fixture = fixture();
    let prepared = prepare_aix_publication(&fixture.root, "knowledge", &fixture.archive)
        .await
        .unwrap();
    assert_eq!(prepared.package().package_id, "acme/guide");
    assert_eq!(prepared.package().file_count, 4);
    assert_eq!(prepared.owners, ["fixture-maintainer"]);
    assert_eq!(
        prepared.oci_repository,
        "ghcr.io/aster-ai-lab/plugins/acme/guide"
    );
    let archive = std::fs::read(&fixture.archive).unwrap();
    assert_eq!(
        prepared.archive_sha256,
        format!("sha256:{:x}", Sha256::digest(&archive))
    );
    assert_eq!(prepared.archive_length, archive.len() as u64);
    assert_eq!(
        prepared.manifest_sha256,
        format!("sha256:{:x}", Sha256::digest(prepared.manifest_bytes()))
    );
    let manifest: serde_json::Value = serde_json::from_slice(prepared.manifest_bytes()).unwrap();
    assert_eq!(manifest["layers"][0]["digest"], prepared.archive_sha256);
    assert_eq!(
        manifest["layers"][0]["annotations"]["org.opencontainers.image.title"],
        "guide.aix"
    );
    assert_eq!(
        manifest["annotations"]["org.opencontainers.image.source"],
        crate::remote::oci::REPOSITORY
    );
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 3);
    assert_eq!(
        std::fs::read(fixture.root.join("domains.acl")).unwrap(),
        POLICY.as_bytes()
    );
}

#[tokio::test]
async fn admitted_domain_changes_preserve_package_source_and_oci_identity() {
    let fixture = fixture();
    std::fs::write(
        fixture.root.join("domains.acl"),
        POLICY.replace("publishers = [\"other\"]", "publishers = [\"acme\"]"),
    )
    .unwrap();
    let knowledge = prepare_aix_publication(&fixture.root, "knowledge", &fixture.archive)
        .await
        .unwrap();
    let development = prepare_aix_publication(&fixture.root, "development", &fixture.archive)
        .await
        .unwrap();
    assert_eq!(knowledge.source_path(), development.source_path());
    assert_eq!(knowledge.oci_repository, development.oci_repository);
    assert_eq!(knowledge.manifest_bytes(), development.manifest_bytes());
    assert_ne!(knowledge.owners, development.owners);
}

#[tokio::test]
async fn publication_refuses_unadmitted_domain_publishers_and_source_drift() {
    let fixture = fixture();
    for domain in ["development", "unknown", "../knowledge"] {
        assert_eq!(
            prepare_aix_publication(&fixture.root, domain, &fixture.archive)
                .await
                .unwrap_err()
                .code,
            ERROR
        );
    }
    std::fs::write(
        fixture.source.join("skills/guide/SKILL.md"),
        "Changed source\n",
    )
    .unwrap();
    assert_eq!(
        prepare_aix_publication(&fixture.root, "knowledge", &fixture.archive)
            .await
            .unwrap_err()
            .code,
        ERROR
    );
}

#[tokio::test]
async fn publication_requires_an_included_nonempty_utf8_license() {
    let fixture = fixture();
    for bytes in [b"".as_slice(), b"\xff".as_slice()] {
        std::fs::write(fixture.source.join("LICENSE"), bytes).unwrap();
        assert_eq!(
            prepare_aix_publication(&fixture.root, "knowledge", &fixture.archive)
                .await
                .unwrap_err()
                .code,
            ERROR
        );
    }
    std::fs::remove_file(fixture.source.join("LICENSE")).unwrap();
    assert_eq!(
        prepare_aix_publication(&fixture.root, "knowledge", &fixture.archive)
            .await
            .unwrap_err()
            .code,
        "use.extension.io"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn publication_refuses_a_link_redirecting_the_explicit_domain_directory() {
    let fixture = fixture();
    let moved = fixture.root.join("other-source");
    std::fs::rename(&fixture.source, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &fixture.source).unwrap();
    assert_eq!(
        prepare_aix_publication(&fixture.root, "knowledge", &fixture.archive)
            .await
            .unwrap_err()
            .code,
        ERROR
    );
}
