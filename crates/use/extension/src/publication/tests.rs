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

#[tokio::test]
async fn catalog_generation_binds_native_package_and_distinct_manifest_digests() {
    let fixture = fixture();
    let prepared = prepare_aix_publication_with_catalog(
        &fixture.root,
        "knowledge",
        &fixture.archive,
        &catalog_metadata(false),
    )
    .await
    .unwrap();
    let record = prepared.catalog().unwrap();
    record.validate().unwrap();
    assert_eq!(record.package_id, prepared.package().package_id);
    assert_eq!(record.archive.sha256, prepared.archive_sha256);
    assert_eq!(record.archive.length, prepared.archive_length);
    assert_eq!(
        record.package.sha256,
        Some(format!("sha256:{}", prepared.package().package_sha256))
    );
    let manifest = std::fs::read(fixture.source.join("aster-use-extension.acl")).unwrap();
    assert_eq!(
        record.package.manifest_sha256,
        Some(format!("sha256:{:x}", Sha256::digest(manifest)))
    );
    assert!(record
        .archive
        .target_name
        .contains(prepared.manifest_sha256.trim_start_matches("sha256:")));
    assert!(prepared.planning_bytes().is_empty());
    assert!(record.planning.is_none());
}

#[tokio::test]
async fn mcp_publication_generates_native_planning_without_executing_the_package() {
    let fixture = mcp_fixture().await;
    let prepared = prepare_aix_publication_with_catalog(
        &fixture.root,
        "knowledge",
        &fixture.archive,
        &catalog_metadata(true),
    )
    .await
    .unwrap();
    let record = prepared.catalog().unwrap();
    let planning =
        aster_use_core::PluginPlanningBundle::from_json(prepared.planning_bytes()).unwrap();
    assert_eq!(
        record.planning.as_ref().unwrap().length,
        prepared.planning_bytes().len() as u64
    );
    assert_eq!(
        record.planning.as_ref().unwrap().sha256,
        format!("sha256:{:x}", Sha256::digest(prepared.planning_bytes()))
    );
    assert!(record
        .planning
        .as_ref()
        .unwrap()
        .target_name
        .ends_with("/planning-v1.json"));
    let [aster_use_core::ExecutablePlanningSurface::McpStdio {
        id,
        executable,
        args,
        ..
    }] = planning.surfaces.as_slice()
    else {
        panic!("expected one native stdio planning surface");
    };
    assert_eq!(id, "ocr");
    assert_eq!(executable, "bin/ocr");
    assert_eq!(args, &["serve", "--mcp"]);
    let skill = record
        .surfaces
        .iter()
        .find(|surface| surface.id == "guide")
        .unwrap();
    assert_eq!(skill.requires[0].id, "ocr");
    let error = prepare_aix_publication_with_catalog(
        &fixture.root,
        "knowledge",
        &fixture.archive,
        &catalog_metadata(false),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "use.extension.catalog_package_mismatch");
}

#[tokio::test]
async fn publication_debug_redacts_payloads_inside_containers() {
    let fixture = fixture();
    let mut publication = prepare_aix_publication(&fixture.root, "knowledge", &fixture.archive)
        .await
        .unwrap();
    let secret = "synthetic-private-mcp-argument";
    publication.planning_bytes = secret.as_bytes().to_vec();
    publication.manifest_bytes = secret.as_bytes().to_vec();
    let debug = format!("{:?}", vec![publication]);
    assert!(debug.contains("acme/guide"));
    assert!(!debug.contains(secret));
    assert!(!debug.contains(&format!("{:?}", secret.as_bytes())));
}

fn catalog_metadata(mcp: bool) -> Vec<u8> {
    let mut metadata = serde_json::json!({
        "schemaVersion": 1, "displayName": "Guide", "description": "Native publication fixture.",
        "keywords": ["guide"], "categories": ["knowledge"], "target": "any",
        "channel": "stable", "license": "MIT"
    });
    if mcp {
        metadata["permissionCeiling"] = serde_json::json!({
            "schema": "aster.use.plugin-permissions.v1", "surfaces": [{
                "surface": {"kind": "mcp", "id": "ocr"}, "nativeExecution": true,
                "childProcess": false, "filesystem": [], "networkEgress": [],
                "privateService": false, "secrets": [], "uiHttp": [],
                "resources": {"cpuMillis": 1000, "memoryBytes": 1073741824, "pids": 32, "ephemeralStorageBytes": 67108864}
            }]
        });
    }
    serde_json::to_vec(&metadata).unwrap()
}

async fn mcp_fixture() -> Fixture {
    let mut fixture = fixture();
    let manifest = MANIFEST
        .trim_end()
        .strip_suffix('}')
        .unwrap()
        .replace("requires_mcp = []", "requires_mcp = [\"ocr\"]");
    std::fs::write(fixture.source.join("aster-use-extension.acl"), format!(
        "{manifest}\n mcp \"ocr\" {{ transport = \"stdio\"\n executable = \"bin/ocr\"\n args = [\"serve\", \"--mcp\"]\n activation = \"lazy\"\n optional = false\n }}\n}}\n"
    )).unwrap();
    std::fs::create_dir(fixture.source.join("bin")).unwrap();
    let executable = fixture.source.join("bin/ocr");
    std::fs::write(&executable, b"#!/bin/sh\nexit 23\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    fixture.archive = fixture.root.join("mcp.aix");
    crate::build_aix_package(&fixture.source, &fixture.archive)
        .await
        .unwrap();
    fixture
}
