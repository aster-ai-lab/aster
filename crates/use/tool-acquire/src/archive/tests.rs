use super::*;
use crate::{
    AcquireRequest, InstallPayload, ToolPayload, ToolQuery, ToolSource, ToolSpec, ToolStore,
};

#[derive(Clone)]
struct Source(ArchivePayload);

impl ToolSource for Source {
    fn fetch(&self, _: &ToolQuery) -> Result<ToolPayload, ToolAcquireError> {
        Err(ToolAcquireError::AssetMissing)
    }
    fn latest(&self, _: &ToolSpec) -> Result<String, ToolAcquireError> {
        Ok(self.0.version.clone())
    }
    fn fetch_installation(&self, _: &ToolQuery) -> Result<InstallPayload, ToolAcquireError> {
        Ok(InstallPayload::Archive(self.0.clone()))
    }
}

fn request(version: &str) -> AcquireRequest {
    AcquireRequest {
        name: "uv".into(),
        spec: "github:astral-sh/uv".into(),
        version: Some(version.into()),
        executable_name: "uv".into(),
    }
}

#[test]
fn runtime_archives_can_exceed_the_legacy_companion_file_limit() {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    for index in 0..=crate::installation::MAX_TREE_ENTRIES {
        let mut header = tar::Header::new_gnu();
        header.set_size(0);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, format!("data/{index}"), std::io::empty())
            .unwrap();
    }
    let mut header = tar::Header::new_gnu();
    header.set_size(5);
    header.set_mode(0o755);
    header.set_cksum();
    let command = if cfg!(windows) { "uv.exe" } else { "uv" };
    tar.append_data(&mut header, command, b"probe".as_slice())
        .unwrap();
    let bytes = tar.into_inner().unwrap().finish().unwrap();
    let payload = ArchivePayload::new(
        "1.0.0".into(),
        &hex_encode(&Sha256::digest(&bytes)),
        bytes,
        ArchiveFormat::TarGz,
        PathBuf::new(),
        BTreeMap::from([("uv".into(), PathBuf::from(command))]),
        vec![PathBuf::new()],
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = ToolStore::open(root.path()).unwrap();
    let receipt = store.acquire_archive(request("1.0.0"), payload).unwrap();
    let paths = store.installation_paths(&receipt).unwrap();
    assert!(paths.executable().is_file());
    let directory = store.install_dir("uv").unwrap().unwrap();
    assert!(
        std::fs::read_dir(directory.join("data")).unwrap().count()
            > crate::installation::MAX_TREE_ENTRIES
    );
}

fn tar_payload(version: &str, links: &[(&str, &str, bool)], value: &str) -> ArchivePayload {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    let bin = "bin 空格 '$literal";
    let script = b"#!/bin/sh\ncat \"$(dirname \"$0\")/../share/value\"\n";
    for (path, bytes, mode) in [
        (format!("runtime/{bin}/uv"), script.as_slice(), 0o755),
        ("runtime/share/value".into(), value.as_bytes(), 0o644),
        (
            "runtime/lib/required.dat".into(),
            b"required library".as_slice(),
            0o644,
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        tar.append_data(&mut header, path, bytes).unwrap();
    }
    for (path, target, hard) in links {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(if *hard {
            tar::EntryType::Link
        } else {
            tar::EntryType::Symlink
        });
        header.set_size(0);
        header.set_mode(0o755);
        header.set_link_name(target).unwrap();
        header.set_cksum();
        tar.append_data(&mut header, format!("runtime/{path}"), std::io::empty())
            .unwrap();
    }
    let bytes = tar.into_inner().unwrap().finish().unwrap();
    let hash = hex_encode(&Sha256::digest(&bytes));
    let mut commands = BTreeMap::from([("uv".into(), PathBuf::from(bin).join("uv"))]);
    if links.iter().any(|(path, _, _)| path.ends_with("/alias")) {
        commands.insert("alias".into(), PathBuf::from(bin).join("alias"));
    }
    ArchivePayload::new(
        version.into(),
        &hash,
        bytes,
        ArchiveFormat::TarGz,
        "runtime".into(),
        commands,
        vec![bin.into()],
    )
    .unwrap()
}

#[cfg(unix)]
#[test]
fn dispatch_shims_rebuild_all_versions_and_retire_only_owned_commands() {
    let root = tempfile::tempdir().unwrap();
    let store = ToolStore::open(root.path().join("shim 中文 '$literal")).unwrap();
    let bin = "bin 空格 '$literal";
    store
        .acquire_archive(
            request("1.0.0"),
            tar_payload("1.0.0", &[(&format!("{bin}/alias"), "uv", false)], "one\n"),
        )
        .unwrap();
    store
        .acquire_archive(request("2.0.0"), tar_payload("2.0.0", &[], "two\n"))
        .unwrap();
    let render = |tool: &str, command: &str| {
        Ok(format!("printf '%s\\n' '{tool}' '{command}' \"$@\"\n").into_bytes())
    };
    assert_eq!(store.reshim_with(render).unwrap(), 2);
    let output = std::process::Command::new(store.shim_dir().join("alias"))
        .arg("space $literal '中文")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        "uv\nalias\nspace $literal '中文\n".as_bytes()
    );
    assert_eq!(
        std::fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
    std::fs::remove_file(store.shim_dir().join("uv")).unwrap();
    std::fs::write(store.shim_dir().join("custom"), b"user-owned content").unwrap();
    let outside = root.path().join("outside");
    std::fs::write(
        &outside,
        std::fs::read(store.shim_dir().join("alias")).unwrap(),
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, store.shim_dir().join("linked")).unwrap();
    assert_eq!(store.reshim_with(render).unwrap(), 2);
    assert!(store.shim_dir().join("uv").is_file());
    store.remove_version_sync("uv", "1.0.0").unwrap();
    assert_eq!(store.reshim_with(render).unwrap(), 1);
    assert!(!store.shim_dir().join("alias").exists());
    assert_eq!(
        std::fs::read(store.shim_dir().join("custom")).unwrap(),
        b"user-owned content"
    );
    assert!(std::fs::symlink_metadata(store.shim_dir().join("linked"))
        .unwrap()
        .file_type()
        .is_symlink());
    let previous = std::fs::read(store.shim_dir().join("uv")).unwrap();
    assert!(store
        .reshim_with(|_, _| Err(ToolAcquireError::InvalidInstallation(
            "renderer refused".into()
        )))
        .is_err());
    assert_eq!(
        std::fs::read(store.shim_dir().join("uv")).unwrap(),
        previous
    );
}

#[cfg(unix)]
#[test]
fn exact_archive_uninstall_preserves_other_versions_and_removes_current_entrypoints() {
    let temp = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temp.path().join("archive store")).unwrap();
    let bin = "bin 空格 '$literal";
    for (version, value) in [("1.0.0", "one\n"), ("2.0.0", "two\n")] {
        let source = Source(tar_payload(
            version,
            &[(&format!("{bin}/alias"), "uv", false)],
            value,
        ));
        store.acquire_with(request(version), &source).unwrap();
    }
    store.remove_version_sync("uv", "1.0.0").unwrap();
    assert!(!store.root_path().join("installs/uv/1.0.0").exists());
    for suffix in [".json", ".archive.json"] {
        assert!(!store
            .root_path()
            .join("receipts/uv")
            .join(format!("1.0.0{suffix}"))
            .exists());
    }
    for name in ["uv", "alias"] {
        let result = std::process::Command::new(store.shim_dir().join(name))
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, b"two\n");
    }
    store.remove_version_sync("uv", "2.0.0").unwrap();
    for name in ["uv", "alias"] {
        assert!(!store.shim_dir().join(name).exists());
    }
    assert!(store.installed_versions("uv").unwrap().is_empty());
    assert!(store.list_sync().unwrap().is_empty());
    assert!(!store
        .root_path()
        .join("receipts/uv/2.0.0.archive.json")
        .exists());
}

#[cfg(unix)]
#[test]
fn complete_runtime_layout_links_modes_and_native_paths_survive_installation() {
    let temp = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temp.path().join("工具 store with spaces")).unwrap();
    let bin = "bin 空格 '$literal";
    let source = Source(tar_payload(
        "1.0.0",
        &[
            (&format!("{bin}/alias"), "uv", false),
            (&format!("{bin}/hard"), &format!("runtime/{bin}/uv"), true),
        ],
        "runtime one\n",
    ));
    let receipt = store.acquire_with(request("1.0.0"), &source).unwrap();
    let paths = store.installation_paths(&receipt).unwrap();
    let directory = store.root_path().join("installs/uv/1.0.0");
    assert_eq!(paths.executable(), directory.join(bin).join("uv"));
    assert_eq!(paths.bin_dirs(), &[directory.join(bin)]);
    assert_eq!(
        fs::read(directory.join("lib/required.dat")).unwrap(),
        b"required library"
    );
    assert_eq!(
        fs::read_link(directory.join(bin).join("alias")).unwrap(),
        Path::new("uv")
    );
    use std::os::unix::fs::MetadataExt;
    assert_eq!(
        fs::metadata(directory.join(bin).join("hard"))
            .unwrap()
            .ino(),
        fs::metadata(paths.executable()).unwrap().ino()
    );
    for command in [
        store.which("uv").unwrap().unwrap(),
        store.shim_dir().join("alias"),
        paths.executable().to_path_buf(),
    ] {
        let output = std::process::Command::new(command).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"runtime one\n");
    }
    assert_eq!(
        store.installed_receipt("uv", Some("1.0.0")).unwrap(),
        Some(receipt)
    );
    let config = crate::EnvConfig {
        tools: BTreeMap::from([("uv".into(), "1.0.0".into())]),
        vars: BTreeMap::new(),
    };
    let lock = crate::EnvLock::capture(&config, &store).unwrap();
    let lock = crate::EnvLock::parse(&lock.render()).unwrap();
    assert_eq!(lock.install_with(&store, None).unwrap()[0].version, "1.0.0");
    let destination = ToolStore::open(temp.path().join("locked 空格 destination")).unwrap();
    assert!(matches!(
        lock.install_with(&destination, None),
        Err(ToolAcquireError::ToolNotInstalled { .. })
    ));
    lock.install_with(&destination, Some(&source)).unwrap();
    let output = std::process::Command::new(destination.shim_dir().join("alias"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"runtime one\n");
    store.remove_sync("uv").unwrap();
    assert!(!directory.exists());
    assert!(!store.shim_dir().join("alias").exists());
}

#[test]
fn archive_locks_check_download_and_source_layout_before_publication() {
    let temp = tempfile::tempdir().unwrap();
    let origin = ToolStore::open(temp.path().join("origin")).unwrap();
    let source = Source(tar_payload("1.0.0", &[], "one"));
    origin.acquire_with(request("1.0.0"), &source).unwrap();
    let config = crate::EnvConfig {
        tools: BTreeMap::from([("uv".into(), "1.0.0".into())]),
        vars: BTreeMap::new(),
    };
    let lock = crate::EnvLock::capture(&config, &origin).unwrap();
    let text = lock.render();
    for invalid in [
        text.replace("record_sha256 =", "unknown ="),
        text.replace("archive {", "archive \"unexpected\" {"),
        text.replace("archive {", "archive {\nextra {}"),
        text.replace("aster.env-lock.v3", "aster.env-lock.v2"),
        text.replace("record_sha256 = \"", "record_sha256 = \"invalid"),
    ] {
        assert!(crate::EnvLock::parse(&invalid).is_err());
    }
    let destination = ToolStore::open(temp.path().join("destination")).unwrap();
    destination
        .acquire_with(
            request("0.9.0"),
            &Source(tar_payload("0.9.0", &[], "previous")),
        )
        .unwrap();
    let mut different_layout = source.clone();
    different_layout.0.bin_dirs.push("lib".into());
    for changed in [
        Source(tar_payload("1.0.0", &[], "changed publisher contents")),
        different_layout,
    ] {
        assert!(lock.install_with(&destination, Some(&changed)).is_err());
        assert!(!destination.root_path().join("installs/uv/1.0.0").exists());
        assert_eq!(
            fs::read_to_string(destination.root_path().join("current/uv")).unwrap(),
            "0.9.0\n"
        );
        assert!(!destination
            .root_path()
            .join("receipts/uv/1.0.0.archive.json")
            .exists());
    }
    lock.install_with(&destination, Some(&source)).unwrap();
}

#[cfg(unix)]
#[test]
fn archive_locks_refuse_local_file_link_mode_membership_and_record_drift() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let bin = "bin 空格 '$literal";
    for change in ["bytes", "missing", "extra", "symlink", "mode", "layout"] {
        let temp = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temp.path()).unwrap();
        let source = Source(tar_payload(
            "1.0.0",
            &[(&format!("{bin}/alias"), "uv", false)],
            "one",
        ));
        store.acquire_with(request("1.0.0"), &source).unwrap();
        let config = crate::EnvConfig {
            tools: BTreeMap::from([("uv".into(), "1.0.0".into())]),
            vars: BTreeMap::new(),
        };
        let lock = crate::EnvLock::capture(&config, &store).unwrap();
        let directory = store.root_path().join("installs/uv/1.0.0");
        let data = directory.join("share/value");
        match change {
            "bytes" => fs::write(data, "changed").unwrap(),
            "missing" => fs::remove_file(data).unwrap(),
            "extra" => fs::write(directory.join("extra"), "extra").unwrap(),
            "symlink" => {
                let link = directory.join(bin).join("alias");
                fs::remove_file(&link).unwrap();
                symlink("../lib/required.dat", link).unwrap();
            }
            "mode" => fs::set_permissions(data, fs::Permissions::from_mode(0o600)).unwrap(),
            "layout" => {
                let path = store.root_path().join("receipts/uv/1.0.0.archive.json");
                let text = fs::read_to_string(&path).unwrap();
                fs::write(
                    path,
                    text.replace("\"bin_dirs\": [", "\"bin_dirs\": [\"lib\","),
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(lock.install_with(&store, None).is_err(), "{change}");
        if change != "layout" {
            assert!(
                crate::EnvLock::capture(&config, &store).is_err(),
                "{change}"
            );
        }
    }
}

#[test]
fn complete_archives_are_immutable_and_failed_reinstallation_preserves_the_current_version() {
    let temp = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temp.path()).unwrap();
    let first = Source(tar_payload("1.0.0", &[], "one"));
    let receipt = store.acquire_with(request("1.0.0"), &first).unwrap();
    assert_eq!(
        store.acquire_with(request("1.0.0"), &first).unwrap(),
        receipt
    );
    store
        .acquire_with(request("2.0.0"), &Source(tar_payload("2.0.0", &[], "two")))
        .unwrap();
    let altered = Source(tar_payload("1.0.0", &[], "different archive"));
    assert!(matches!(
        store.acquire_with(request("1.0.0"), &altered),
        Err(ToolAcquireError::VersionConflict { .. })
    ));
    assert_eq!(
        fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
    fs::write(
        store.root_path().join("installs/uv/1.0.0/share/value"),
        b"changed after install",
    )
    .unwrap();
    assert!(store.acquire_with(request("1.0.0"), &first).is_err());
    assert_eq!(
        fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
}

#[cfg(unix)]
#[test]
fn escaping_cyclic_dangling_and_parent_directory_links_refuse_before_publication() {
    let bin = "bin 空格 '$literal";
    for links in [
        vec![("escape", "../outside", false)],
        vec![("a", "b", false), ("b", "a", false)],
        vec![("dangling", "absent", false)],
        vec![("a", "runtime/b", true), ("b", "runtime/a", true)],
        vec![(bin, "share", false)],
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temp.path().join("store")).unwrap();
        let result = store.acquire_with(
            request("1.0.0"),
            &Source(tar_payload("1.0.0", &links, "one")),
        );
        assert!(result.is_err());
        assert!(!store.root_path().join("current/uv").exists());
        assert!(!store.root_path().join("installs/uv/1.0.0").exists());
        assert!(!temp.path().join("outside").exists());
    }
}

#[test]
fn zip_layouts_and_empty_directories_are_preserved_and_traversal_is_refused() {
    for traversal in [false, true] {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        zip.start_file("runtime/bin/uv", options).unwrap();
        zip.write_all(b"#!/bin/sh\nprintf fixture\n").unwrap();
        zip.add_directory("runtime/empty/", options).unwrap();
        if traversal {
            zip.start_file("../outside", options).unwrap();
            zip.write_all(b"escaped").unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let hash = hex_encode(&Sha256::digest(&bytes));
        let payload = ArchivePayload::new(
            "1.0.0".into(),
            &hash,
            bytes,
            ArchiveFormat::Zip,
            "runtime".into(),
            BTreeMap::from([("uv".into(), "bin/uv".into())]),
            vec!["bin".into()],
        )
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temp.path().join("store")).unwrap();
        let result = store.acquire_with(request("1.0.0"), &Source(payload));
        if traversal {
            assert!(result.is_err());
            assert!(!temp.path().join("outside").exists());
        } else {
            let receipt = result.unwrap();
            assert!(store.root_path().join("installs/uv/1.0.0/empty").is_dir());
            assert!(store
                .installation_paths(&receipt)
                .unwrap()
                .executable()
                .is_file());
        }
    }
}

#[test]
fn archive_debug_omits_payload_bytes_and_admission_checks_integrity_and_layout() {
    let payload = tar_payload("1.0.0", &[], "synthetic-secret-content");
    let debug = format!("{:?}", InstallPayload::Archive(payload.clone()));
    assert!(!debug.contains("synthetic-secret-content"));
    assert!(!debug.contains(&format!("{:?}", payload.bytes)));
    let bad = ArchivePayload::new(
        "1.0.0".into(),
        &"0".repeat(64),
        payload.bytes.clone(),
        ArchiveFormat::TarGz,
        "runtime".into(),
        payload.entrypoints.clone(),
        payload.bin_dirs.clone(),
    );
    assert!(matches!(bad, Err(ToolAcquireError::ChecksumMismatch)));
    for path in ["../outside", "/absolute", "C:/outside"] {
        assert!(ArchivePayload::new(
            "1.0.0".into(),
            &payload.sha256,
            payload.bytes.clone(),
            ArchiveFormat::TarGz,
            "runtime".into(),
            BTreeMap::from([("uv".into(), path.into())]),
            vec!["bin".into()]
        )
        .is_err());
    }
}

#[test]
fn malformed_current_receipts_cannot_redirect_archive_metadata_reads_or_uninstall_another_tool() {
    let temp = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temp.path()).unwrap();
    let source = Source(tar_payload("1.0.0", &[], "one"));
    let receipt = store.acquire_with(request("1.0.0"), &source).unwrap();
    let path = store.root_path().join("receipts/uv.json");
    for (field, value) in [
        ("name", "../outside"),
        ("version", "../outside"),
        ("executable", "../outside"),
        ("schema", "unsupported"),
    ] {
        let mut invalid = serde_json::to_value(&receipt).unwrap();
        invalid
            .as_object_mut()
            .unwrap()
            .insert(field.into(), value.into());
        fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(store.which("uv").is_err());
        assert!(store.remove_sync("uv").is_err());
        assert!(store.root_path().join("installs/uv/1.0.0").is_dir());
    }
}

#[test]
fn source_declared_optional_entrypoints_are_published_only_when_present() {
    let temp = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temp.path()).unwrap();
    let bin = "bin 空格 '$literal";
    let payload = tar_payload("1.0.0", &[], "one")
        .with_optional_entrypoints(BTreeMap::from([(
            "missing".into(),
            PathBuf::from(bin).join("missing"),
        )]))
        .unwrap();
    store
        .acquire_with(request("1.0.0"), &Source(payload))
        .unwrap();
    assert!(!store
        .shim_dir()
        .join(if cfg!(windows) {
            "missing.cmd"
        } else {
            "missing"
        })
        .exists());
}

#[test]
fn oversized_tar_extension_headers_are_refused_before_the_tar_parser_buffers_them() {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_path("././@LongLink").unwrap();
    header.set_entry_type(tar::EntryType::GNULongName);
    header.set_size(64 * 1024 + 1);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, vec![b'a'; 64 * 1024 + 1].as_slice())
        .unwrap();
    let bytes = tar.into_inner().unwrap().finish().unwrap();
    let hash = hex_encode(&Sha256::digest(&bytes));
    let payload = ArchivePayload::new(
        "1.0.0".into(),
        &hash,
        bytes,
        ArchiveFormat::TarGz,
        "runtime".into(),
        BTreeMap::from([("uv".into(), "bin/uv".into())]),
        vec!["bin".into()],
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temp.path()).unwrap();
    assert!(matches!(
        store.acquire_with(request("1.0.0"), &Source(payload)),
        Err(ToolAcquireError::InvalidInstallation(_))
    ));
    assert!(!store.root_path().join("current/uv").exists());
}
