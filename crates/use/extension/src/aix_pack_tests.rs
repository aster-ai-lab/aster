use super::*;
use std::path::PathBuf;

const MANIFEST: &str = r#"extension "aster/pack-test" {
  schema_version = 3
  version = "1.0.0"
  route = "pack-test"
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

#[test]
fn zip_filesystem_failures_retain_the_recoverable_io_error_code() {
    let error = archive_error(zip::result::ZipError::Io(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "synthetic output failure",
    )));
    assert_eq!(error.code, "use.extension.io");
}

fn source(root: &Path) -> PathBuf {
    let source = root.join("插件 source");
    std::fs::create_dir_all(source.join("skills/guide")).unwrap();
    for (path, bytes) in [
        ("aster-use-extension.acl", MANIFEST),
        (
            "README.md",
            "# Pack fixture\nNative archive construction.\n",
        ),
        ("LICENSE", "MIT\nSynthetic fixture.\n"),
        ("skills/guide/SKILL.md", "# Guide\nRead this fixture.\n"),
    ] {
        std::fs::write(source.join(path), bytes).unwrap();
    }
    source
}

#[tokio::test]
async fn builds_reproducible_archives_that_match_the_native_source_inventory() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    let first = temporary.path().join("第一份 archive.aix");
    let second = temporary.path().join("第二份 archive.aix");
    let expected = inspect_release_bundle(&source).await.unwrap();
    let result = build_aix_package(&source, &first).await.unwrap();
    assert_eq!(result, expected);
    assert_eq!(result.file_count, 4);
    assert_eq!(inspect_aix_package(&first).await.unwrap(), expected);

    let readme = std::fs::read(source.join("README.md")).unwrap();
    std::fs::remove_file(source.join("README.md")).unwrap();
    std::fs::write(source.join("README.md"), readme).unwrap();
    assert_eq!(build_aix_package(&source, &second).await.unwrap(), expected);
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );

    let mut archive = zip::ZipArchive::new(File::open(&first).unwrap()).unwrap();
    let names: Vec<_> = (0..archive.len())
        .map(|index| {
            let entry = archive.by_index(index).unwrap();
            assert_eq!(entry.last_modified(), Some(zip::DateTime::default()));
            entry.name().to_owned()
        })
        .collect();
    assert_eq!(
        names,
        [
            "LICENSE",
            "README.md",
            "aster-use-extension.acl",
            "skills/guide/SKILL.md"
        ]
    );
}

#[tokio::test]
async fn refuses_existing_outputs_inside_source_and_non_native_suffixes() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    let output = temporary.path().join("existing.aix");
    std::fs::write(&output, b"keep original archive").unwrap();
    assert_eq!(
        build_aix_package(&source, &output).await.unwrap_err().code,
        "use.extension.io"
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"keep original archive");
    for invalid in [
        source.join("inside.aix"),
        temporary.path().join("wrong.AIX"),
        temporary.path().join("wrong.zip"),
    ] {
        assert_eq!(
            build_aix_package(&source, &invalid).await.unwrap_err().code,
            "use.extension.package_output_invalid"
        );
        assert!(!invalid.exists());
    }
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 2);
}

#[tokio::test]
async fn refuses_invalid_packages_without_leaving_a_partial_archive() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    std::fs::remove_file(source.join("skills/guide/SKILL.md")).unwrap();
    let output = temporary.path().join("invalid.aix");
    assert!(build_aix_package(&source, &output).await.is_err());
    assert!(!output.exists());
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 1);
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn refuses_linked_source_directories_without_reading_outside_the_package() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    let link = temporary.path().join("linked source");
    crate::test_filesystem::create_directory_link(&source, &link);
    let output = temporary.path().join("linked.aix");
    assert!(build_aix_package(&link, &output).await.is_err());
    assert!(!output.exists());
    let outside = temporary.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("private.txt"), b"not a package member").unwrap();
    crate::test_filesystem::create_directory_link(&outside, &source.join("linked"));
    assert_eq!(
        build_aix_package(&source, &output).await.unwrap_err().code,
        "use.extension.package_symlink"
    );
    assert!(!output.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn preserves_declared_native_executable_modes() {
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    let manifest = MANIFEST.replace("actions = [\"read\"]", "actions = [\"execute\"]").replace(
        "  skill \"guide\" {\n    path = \"skills/guide/SKILL.md\"\n    requires_tool = []\n    requires_mcp = []\n    optional = false\n  }",
        "  tool \"probe\" { workload = \"task\" interface = \"cli\" executable = \"tools/probe\" command = \"pack-test-probe\" json_output = false interactive = false timeout_ms = 1000 activation = \"lazy\" optional = false }"
    );
    std::fs::write(source.join("aster-use-extension.acl"), manifest).unwrap();
    std::fs::create_dir(source.join("tools")).unwrap();
    std::fs::write(source.join("tools/probe"), b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(
        source.join("tools/probe"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let output = temporary.path().join("executable.aix");
    let package = build_aix_package(&source, &output).await.unwrap();
    assert_eq!(package.surfaces, ["tool"]);
    let mut archive = zip::ZipArchive::new(File::open(output).unwrap()).unwrap();
    assert_eq!(
        archive.by_name("tools/probe").unwrap().unix_mode().unwrap() & 0o777,
        0o755
    );
}

#[cfg(unix)]
#[tokio::test]
async fn bounds_expanded_source_before_reading_a_sparse_oversized_asset() {
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    File::create(source.join("oversized.bin"))
        .unwrap()
        .set_len(crate::package::MAX_PACKAGE_BYTES + 1)
        .unwrap();
    let output = temporary.path().join("oversized.aix");
    assert_eq!(
        build_aix_package(&source, &output).await.unwrap_err().code,
        "use.extension.package_too_large"
    );
    assert!(!output.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn preserves_native_output_filename_semantics() {
    use std::os::unix::ffi::OsStringExt;
    let temporary = tempfile::tempdir().unwrap();
    let source = source(temporary.path());
    let output = temporary
        .path()
        .join(std::ffi::OsString::from_vec(b"native-\xff.aix".to_vec()));
    match File::create(&output) {
        Ok(file) => {
            drop(file);
            std::fs::remove_file(&output).unwrap();
            let package = build_aix_package(&source, &output).await.unwrap();
            assert_eq!(inspect_aix_package(&output).await.unwrap(), package);
            assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 2);
        }
        Err(_) => {
            assert_eq!(
                build_aix_package(&source, &output).await.unwrap_err().code,
                "use.extension.io"
            );
            assert!(!output.exists());
            assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 1);
        }
    }
}
