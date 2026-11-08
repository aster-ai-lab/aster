use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

use sha2::{Digest, Sha256};

fn archive() -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    let command = if cfg!(windows) { "probe.exe" } else { "probe" };
    for (name, content, mode) in [
        (
            command,
            b"#!/bin/sh\nprintf 'plugin-effect' > \"$1\"\n".as_slice(),
            0o755,
        ),
        ("helper", b"helper payload".as_slice(), 0o644),
        ("share/data", b"required data".as_slice(), 0o640),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        archive.append_data(&mut header, name, content).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

fn declaration(bytes: &[u8]) -> String {
    let command = if cfg!(windows) { "probe.exe" } else { "probe" };
    let helper = if cfg!(windows) {
        "helper.exe"
    } else {
        "helper"
    };
    format!(
        r#"toolchain {{
 schema_version = 1
 name = "probe"
 spec = "github:example/probe"
 version = "1.0.0"
 executable = "probe"
 platform = "{}"
 url = "https://github.com/example/probe/releases/download/v1.0.0/probe.tar.gz"
 length = {}
 sha256 = "{}"
 format = "tar.gz"
 prefix = ""
 bin_dirs = [""]
 executable_mode = "0755"
 command "probe" {{ path = "{command}" }}
 command "helper" {{ path = "{helper}" optional = true }}
 command "absent" {{ path = "{}" optional = true }}
}}"#,
        crate::env_lock::platform(),
        bytes.len(),
        crate::store::hex_encode(&Sha256::digest(bytes)),
        if cfg!(windows) {
            "absent.exe"
        } else {
            "absent"
        },
    )
}

fn multi_platform_declaration(bytes: &[u8]) -> String {
    let host = crate::env_lock::platform();
    let other = if host == "x86_64-windows-msvc" {
        "aarch64-macos-"
    } else {
        "x86_64-windows-msvc"
    };
    let command = if cfg!(windows) { "probe.exe" } else { "probe" };
    let other_command = if other == "x86_64-windows-msvc" {
        "probe.exe"
    } else {
        "probe"
    };
    format!(
        r#"toolchain {{
 schema_version = 2
 name = "probe"
 spec = "github:example/probe"
 version = "1.0.0"
 executable = "probe"
 platform "{host}" {{
  url = "https://github.com/example/probe/releases/download/v1.0.0/probe.tar.gz"
  length = {}
  sha256 = "{}"
  format = "tar.gz"
  prefix = ""
  bin_dirs = [""]
  executable_mode = "0755"
  command "probe" {{ path = "{command}" }}
 }}
 platform "{other}" {{
  url = "https://github.com/example/probe/releases/download/v1.0.0/other.zip"
  length = 1
  sha256 = "{}"
  format = "zip"
  prefix = ""
  bin_dirs = ["other"]
  command "probe" {{ path = "other/{other_command}" }}
 }}
}}"#,
        bytes.len(),
        crate::store::hex_encode(&Sha256::digest(bytes)),
        "0".repeat(64),
    )
}

#[test]
fn multi_platform_packages_install_the_host_archive_and_preserve_all_owned_files() {
    let bytes = archive();
    let plugin = ToolchainPlugin::parse_acl(&multi_platform_declaration(&bytes)).unwrap();
    assert_eq!(plugin.platform.as_str(), crate::env_lock::platform());
    assert_eq!(plugin.archive_length(), bytes.len());
    let directory = tempfile::Builder::new()
        .prefix("平台 with spaces ")
        .tempdir()
        .unwrap();
    let store = ToolStore::open(directory.path()).unwrap();
    plugin
        .prepare_archive(bytes)
        .unwrap()
        .install(&store)
        .unwrap();
    let installed = plugin.installed(&store).unwrap().unwrap();
    assert!(installed.paths().executable().is_file());
    assert_eq!(
        std::fs::read(
            store
                .install_dir("probe")
                .unwrap()
                .unwrap()
                .join("share/data")
        )
        .unwrap(),
        b"required data"
    );
    #[cfg(unix)]
    {
        let effect = directory.path().join("实际 command effect");
        assert!(std::process::Command::new(installed.paths().executable())
            .arg(&effect)
            .status()
            .unwrap()
            .success());
        assert_eq!(std::fs::read(effect).unwrap(), b"plugin-effect");
    }
}

#[test]
fn multi_platform_packages_refuse_bad_unselected_entries_duplicates_and_missing_hosts() {
    let text = multi_platform_declaration(&archive());
    for invalid in [
        text.replace("other.zip", "other.zip?token=synthetic-secret"),
        text.replace("path = \"other/", "path = \"../"),
        text.replace("length = 1\n", "length = 0\n"),
        text.replace("other.zip\"", "other.zip\"\n  future = true"),
        text.replace(" schema_version = 2", " schema_version = 2\n url = \"https://github.com/example/probe/releases/download/v1/root.zip\""),
        text.replace("platform \"", "artifact \""),
        text.replace("platform \"", "platform \"unsupported-"),
    ] {
        assert!(ToolchainPlugin::parse_acl(&invalid).is_err(), "{invalid}");
    }
    let host = crate::env_lock::platform();
    let other = if host == "x86_64-windows-msvc" {
        "aarch64-macos-"
    } else {
        "x86_64-windows-msvc"
    };
    let duplicate = text.replace(
        &format!("platform \"{other}\""),
        &format!("platform \"{host}\""),
    );
    assert!(ToolchainPlugin::parse_acl(&duplicate).is_err());
    let third = ["aarch64-macos-", "x86_64-macos-", "aarch64-linux-gnu"]
        .into_iter()
        .find(|value| *value != host && *value != other)
        .unwrap();
    let missing_host = text.replace(
        &format!("platform \"{host}\""),
        &format!("platform \"{third}\""),
    );
    assert!(ToolchainPlugin::parse_acl(&missing_host).is_err());
}

#[test]
#[ignore = "requires the explicitly supplied maintained AIX files and real upstream downloads"]
fn maintained_multi_platform_aix_packages_install_execute_and_remove() {
    use std::io::Read;

    let inputs =
        std::fs::read(std::env::var_os("ASTER_TEST_MULTI_PLATFORM_INPUTS").unwrap()).unwrap();
    let cases: Vec<serde_json::Value> = serde_json::from_slice(&inputs).unwrap();
    assert_eq!(cases.len(), 7);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut results = Vec::new();
    for case in cases {
        let aix = PathBuf::from(case["aix"].as_str().unwrap());
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&aix).unwrap()).unwrap();
        let mut text = String::new();
        archive
            .by_name("toolchain.acl")
            .unwrap()
            .take(MAX_DECLARATION_BYTES as u64 + 1)
            .read_to_string(&mut text)
            .unwrap();
        let plugin = ToolchainPlugin::parse_acl(&text).unwrap();
        assert_eq!(plugin.name(), case["tool"].as_str().unwrap());
        assert_eq!(plugin.version(), case["version"].as_str().unwrap());
        let root = tempfile::Builder::new()
            .prefix("多平台 plugin with spaces ")
            .tempdir()
            .unwrap();
        let store = ToolStore::open(root.path()).unwrap();
        let receipt = plugin.install(&store, &HttpSource::new().unwrap()).unwrap();
        let installed = plugin.installed(&store).unwrap().unwrap();
        let path = std::env::join_paths(installed.paths().bin_dirs()).unwrap();
        let mut commands = Vec::new();
        for command in case["requiredCommands"].as_array().unwrap() {
            let name = command.as_str().unwrap();
            let executable =
                crate::find_env_executable(std::ffi::OsStr::new(name), Some(&path), root.path())
                    .unwrap()
                    .unwrap();
            let arguments = case["versionArgs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap())
                .collect::<Vec<_>>();
            let output = runtime.block_on(async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    tokio::process::Command::new(executable)
                        .args(arguments)
                        .current_dir(root.path())
                        .kill_on_drop(true)
                        .output(),
                )
                .await
                .unwrap()
                .unwrap()
            });
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(stdout.contains(plugin.version()), "{name}: {stdout}");
            commands.push(serde_json::json!({"command":name,"stdout":stdout}));
        }
        store.remove_sync(plugin.name()).unwrap();
        assert!(plugin.installed(&store).unwrap().is_none());
        assert!(store.which(plugin.name()).unwrap().is_none());
        let cached = plugin
            .install_archive(
                &store,
                std::fs::read(case["archive"].as_str().unwrap()).unwrap(),
            )
            .unwrap();
        assert_eq!(cached.sha256, receipt.sha256);
        assert!(plugin.installed(&store).unwrap().is_some());
        store.remove_sync(plugin.name()).unwrap();
        results.push(serde_json::json!({"name":plugin.name(),"version":plugin.version(),"selectedPlatform":plugin.platform.as_str(),"commands":commands,"onlineInstallPassed":true,"offlineReinstallPassed":true,"removalPassed":true}));
    }
    if let Some(path) = std::env::var_os("ASTER_TEST_MULTI_PLATFORM_EVIDENCE") {
        std::fs::write(path, serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    }
}

#[test]
fn prepared_runtime_waits_for_its_owner_and_verifies_all_files_and_layout() {
    let bytes = archive();
    let plugin = ToolchainPlugin::parse_acl(&declaration(&bytes)).unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = ToolStore::open(root.path()).unwrap();
    let prepared = plugin.prepare_archive(bytes).unwrap();
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    let receipt = prepared.install(&store).unwrap();
    let verified = plugin.installed(&store).unwrap().unwrap();
    assert!(verified.digest().starts_with("sha256:"));
    assert!(verified.paths().executable().is_file());
    let directory = store.install_dir("probe").unwrap().unwrap();
    std::fs::write(directory.join("share/data"), b"corrupt companion").unwrap();
    assert!(matches!(
        plugin.installed(&store),
        Err(ToolAcquireError::LockContentMismatch { .. })
    ));
    std::fs::write(directory.join("share/data"), b"required data").unwrap();
    let record_path = store
        .root_path()
        .join("receipts/probe")
        .join(format!("{}.archive.json", receipt.version));
    let original = std::fs::read(&record_path).unwrap();
    let mut record: serde_json::Value = serde_json::from_slice(&original).unwrap();
    record["bin_dirs"] = serde_json::json!([]);
    std::fs::write(&record_path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(matches!(
        plugin.installed(&store),
        Err(ToolAcquireError::InvalidInstallation(_))
    ));
    std::fs::write(record_path, original).unwrap();
    assert_eq!(
        plugin.installed(&store).unwrap().unwrap().digest(),
        verified.digest()
    );
}

#[test]
fn embedded_archives_keep_native_integrity_and_require_the_owning_payload() {
    let bytes = archive();
    let text = declaration(&bytes).replace(
        "url = \"https://github.com/example/probe/releases/download/v1.0.0/probe.tar.gz\"",
        "archive = \"artifacts/运行时 with spaces.tar.gz\"",
    );
    let plugin = ToolchainPlugin::parse_acl(&text).unwrap();
    assert_eq!(
        plugin.embedded_archive(),
        Some(Path::new("artifacts/运行时 with spaces.tar.gz"))
    );
    assert_eq!(plugin.archive_length(), bytes.len());
    assert!(matches!(
        plugin.prepare(&HttpSource::new().unwrap()),
        Err(ToolAcquireError::InvalidToolchainPlugin(_))
    ));
    let root = tempfile::tempdir().unwrap();
    let store = ToolStore::open(root.path()).unwrap();
    let mut damaged = bytes.clone();
    damaged[0] ^= 1;
    assert!(matches!(
        plugin.prepare_archive(damaged),
        Err(ToolAcquireError::ChecksumMismatch)
    ));
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
    plugin
        .prepare_archive(bytes)
        .unwrap()
        .install(&store)
        .unwrap();
    assert!(plugin
        .installed(&store)
        .unwrap()
        .unwrap()
        .paths()
        .executable()
        .is_file());
}

#[test]
fn embedded_source_is_exclusive_portable_and_bounded_independently_of_http() {
    let text = declaration(&archive());
    let url = "url = \"https://github.com/example/probe/releases/download/v1.0.0/probe.tar.gz\"";
    for replacement in [
        "", "archive = true", "archive = \"\"", "archive = \"../escape\"",
        "archive = \"/absolute\"", "archive = \"C:/absolute\"",
        "archive = \"a\\\\b\"",
        "url = \"https://github.com/example/probe/releases/download/v1.0.0/probe.tar.gz\"\narchive = \"runtime.tar.gz\"",
    ] {
        assert!(ToolchainPlugin::parse_acl(&text.replace(url, replacement)).is_err(), "{replacement}");
    }
    let length = format!("length = {}", archive().len());
    let embedded = text.replace(url, "archive = \"runtime.tar.gz\"");
    assert!(ToolchainPlugin::parse_acl(&embedded.replace(&length, "length = 536870912")).is_ok());
    assert!(ToolchainPlugin::parse_acl(&embedded.replace(&length, "length = 536870913")).is_err());
    assert!(ToolchainPlugin::parse_acl(&text.replace(&length, "length = 209715201")).is_err());
}

#[test]
fn npm_artifact_declarations_preserve_source_policy_and_reject_untrusted_origins() {
    let text = declaration(&archive())
        .replace("github:example/probe", "npm:ntn")
        .replace(
            "https://github.com/example/probe/releases/download/v1.0.0/probe.tar.gz",
            "https://registry.npmjs.org/ntn/-/ntn-1.0.0.tgz",
        );
    let plugin = ToolchainPlugin::parse_acl(&text).unwrap();
    let ArchiveSource::Http(expected_url) = &plugin.source else {
        panic!("expected HTTP source");
    };
    let source = HttpSource::with_policy(super::super::FetchPolicy {
        npm_registry: "https://registry.npmmirror.com".into(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        source.prepare_toolchain_download(expected_url).unwrap(),
        "https://registry.npmmirror.com/ntn/-/ntn-1.0.0.tgz"
    );
    for url in [
        "http://registry.npmjs.org/ntn/-/ntn.tgz",
        "https://registry.npmjs.org.evil.example/ntn/-/ntn.tgz",
        "https://user:secret@registry.npmjs.org/ntn/-/ntn.tgz",
        "https://registry.npmjs.org:444/ntn/-/ntn.tgz",
        "https://registry.npmjs.org/ntn/-/ntn.tgz?token=secret",
        "https://registry.npmjs.org/ntn/-/ntn.tgz#fragment",
    ] {
        assert!(
            ToolchainPlugin::parse_acl(&text.replace(expected_url, url)).is_err(),
            "{url}"
        );
    }
}

#[test]
#[ignore = "requires an explicitly provided Notion archive and AIX declaration"]
fn actual_notion_artifact_runs_the_declared_native_binary_and_preserves_its_package() {
    let declaration =
        std::fs::read_to_string(std::env::var_os("ASTER_TEST_NTN_TOOLCHAIN").unwrap()).unwrap();
    let plugin = ToolchainPlugin::parse_acl(&declaration).unwrap();
    assert_eq!(plugin.name(), "ntn");
    let archive_path = std::env::var_os("ASTER_TEST_NTN_ARCHIVE").unwrap();
    assert!(std::fs::metadata(&archive_path).unwrap().len() <= MAX_BYTES as u64);
    let bytes = std::fs::read(&archive_path).unwrap();
    let temporary = tempfile::Builder::new()
        .prefix("Notion plugin 空格 ")
        .tempdir()
        .unwrap();
    let store = ToolStore::open(temporary.path().join("工具 store")).unwrap();
    let receipt = plugin.install(&store, &HttpSource::new().unwrap()).unwrap();
    let installed = store.install_dir("ntn").unwrap().unwrap();
    assert_eq!(receipt.spec, "npm:ntn");
    assert!(installed.join("LICENSE.md").is_file());
    assert!(installed.join("install.cjs").is_file());
    assert!(installed.join("bin/ntn").is_file());
    let paths = store.installation_paths(&receipt).unwrap();
    assert!(paths.executable().starts_with(installed.join("dist")));
    assert_eq!(
        store.installed_receipt("ntn", None).unwrap(),
        Some(receipt.clone())
    );
    let shim = store.which("ntn").unwrap().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let output = runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new(&shim)
                .arg("--version")
                .current_dir(temporary.path())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap()
    });
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(plugin.version()));
    store.remove_sync("ntn").unwrap();
    assert!(store.installed_receipt("ntn", None).unwrap().is_none());
    assert!(store.which("ntn").unwrap().is_none());
    let cached = plugin.install_archive(&store, bytes).unwrap();
    assert_eq!(cached.version, receipt.version);
    assert_eq!(cached.sha256, receipt.sha256);
    assert!(store.which("ntn").unwrap().is_some());
    store.remove_sync("ntn").unwrap();
    assert!(store.installed_receipt("ntn", None).unwrap().is_none());
    if let Some(path) = std::env::var_os("ASTER_TEST_NTN_EVIDENCE") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "name":receipt.name,"spec":receipt.spec,"version":receipt.version,
                "archiveLength":plugin.length,"archiveSha256":plugin.sha256,
                "entry":paths.executable().strip_prefix(&installed).unwrap(),
                "versionStdout":String::from_utf8_lossy(&output.stdout),
                "upstreamInstallerInvokedByHost":false,"permanentInstallation":false,
                "explicitToolRemovalPassed":true,"onlineInstallationPassed":true,
                "offlineReinstallationPassed":true
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

fn serve(bytes: &[u8]) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/archive", listener.local_addr().unwrap());
    let bytes = bytes.to_vec();
    let worker = thread::spawn(move || {
        let mut stream = super::super::test_server::accept(&listener);
        let mut reader = BufReader::new(&mut stream);
        loop {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if line == "\r\n" {
                break;
            }
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            bytes.len()
        )
        .unwrap();
        stream.write_all(&bytes).unwrap();
    });
    (url, worker)
}

#[test]
fn external_plugin_installs_the_complete_archive_through_the_native_http_store() {
    let bytes = archive();
    let mut plugin = ToolchainPlugin::parse_acl(&declaration(&bytes)).unwrap();
    assert_eq!((plugin.name(), plugin.version()), ("probe", "1.0.0"));
    let (url, server) = serve(&bytes);
    let http = HttpSource::for_test(&url, &url, &[]).unwrap();
    // Only the unit fixture replaces the admitted upstream URL with loopback transport.
    plugin.source = ArchiveSource::Http(url);
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 store")).unwrap();
    let receipt = plugin.install(&store, &http).unwrap();
    server.join().unwrap();
    let paths = store.installation_paths(&receipt).unwrap();
    let installed = paths.executable().parent().unwrap();
    assert_eq!(
        std::fs::read(installed.join("share/data")).unwrap(),
        b"required data"
    );
    assert_eq!(
        std::fs::read(installed.join("helper")).unwrap(),
        b"helper payload"
    );
    assert!(!installed.join("absent").exists());
    assert_eq!(
        store.installed_receipt("probe", Some("1.0.0")).unwrap(),
        Some(receipt)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(installed.join("helper"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            std::fs::metadata(installed.join("share/data"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
        let effect = temporary.path().join("实际 effect.txt");
        assert!(std::process::Command::new(paths.executable())
            .arg(&effect)
            .status()
            .unwrap()
            .success());
        assert_eq!(std::fs::read(effect).unwrap(), b"plugin-effect");
    }
}

#[test]
fn invalid_declarations_refuse_unknown_fields_credentials_paths_and_unbounded_work() {
    let text = declaration(&archive());
    for (from, to) in [
        ("schema_version = 1", "schema_version = 2"),
        (
            "schema_version = 1",
            "schema_version = 1\n shell = \"curl\"",
        ),
        ("format = \"tar.gz\"", "format = \"exe\""),
        ("prefix = \"\"", "prefix = \"../outside\""),
        ("bin_dirs = [\"\"]", "bin_dirs = []"),
        ("https://github.com/", "http://github.com/"),
        (
            "https://github.com/",
            "https://synthetic-secret@github.com/",
        ),
        ("https://github.com/", "https://downloads.example/"),
        ("probe.tar.gz\"", "probe.tar.gz?token=synthetic-secret\""),
        ("probe.tar.gz\"", "probe.tar.gz#fragment\""),
        ("version = \"1.0.0\"", "version = \"../bad\""),
        ("executable_mode = \"0755\"", "executable_mode = \"0777\""),
        ("optional = true", "optional = \"true\""),
        ("command \"helper\"", "command \"probe\""),
        ("command \"helper\"", "other \"helper\""),
    ] {
        assert!(text.contains(from), "fixture replacement {from}");
        let error = ToolchainPlugin::parse_acl(&text.replace(from, to)).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
    }
    for length in ["0", "1.5", "209715201"] {
        let old = format!("length = {}", archive().len());
        assert!(
            ToolchainPlugin::parse_acl(&text.replace(&old, &format!("length = {length}"))).is_err()
        );
    }
    assert!(ToolchainPlugin::parse_acl(&" ".repeat(MAX_DECLARATION_BYTES + 1)).is_err());
    assert!(ToolchainPlugin::parse_acl(&format!("{text}\n{text}")).is_err());
}

#[test]
fn wrong_length_checksum_and_missing_required_companions_do_not_publish() {
    let bytes = archive();
    let declaration = declaration(&bytes);
    for change in ["length", "checksum", "companion"] {
        let mut plugin = ToolchainPlugin::parse_acl(&declaration).unwrap();
        match change {
            "length" => plugin.length += 1,
            "checksum" => plugin.sha256 = "a".repeat(64),
            "companion" => {
                plugin
                    .entrypoints
                    .insert("missing".into(), "missing".into());
            }
            _ => unreachable!(),
        }
        let (url, server) = serve(&bytes);
        let http = HttpSource::for_test(&url, &url, &[]).unwrap();
        plugin.source = ArchiveSource::Http(url);
        let temporary = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temporary.path().join("store")).unwrap();
        let error = plugin.install(&store, &http).unwrap_err();
        server.join().unwrap();
        if change == "checksum" {
            assert!(matches!(error, ToolAcquireError::ChecksumMismatch));
        }
        assert!(store
            .installed_receipt("probe", Some("1.0.0"))
            .unwrap()
            .is_none());
        assert!(!store.root_path().join("installs/probe/1.0.0").exists());
        assert!(!store
            .root_path()
            .join("installs/probe/.staging-1.0.0")
            .exists());
        assert!(!store.shim_dir().join("probe").exists());
    }
}

#[test]
fn platform_mismatch_refuses_before_any_download_or_installation() {
    let bytes = archive();
    let mut plugin = ToolchainPlugin::parse_acl(&declaration(&bytes)).unwrap();
    plugin.platform = if cfg!(windows) {
        Platform::MacArm64
    } else {
        Platform::WindowsX64
    };
    let http = HttpSource::for_test("http://127.0.0.1:9", "http://127.0.0.1:9", &[]).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    assert!(matches!(
        plugin.install(&store, &http),
        Err(ToolAcquireError::InvalidToolchainPlugin(_))
    ));
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 0);
}

#[test]
fn debug_omits_artifact_url_paths_from_containing_values() {
    let text = declaration(&archive()).replace("/v1.0.0/", "/synthetic-private-path/");
    let plugin = ToolchainPlugin::parse_acl(&text).unwrap();
    assert!(!format!("{:?}", Some(plugin)).contains("synthetic-private-path"));
}

#[test]
fn downloads_stop_at_the_plugin_declared_length() {
    for known_length in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/archive", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut stream = super::super::test_server::accept(&listener);
            let mut reader = BufReader::new(&mut stream);
            loop {
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
            }
            let length = if known_length {
                "content-length: 4\r\n"
            } else {
                ""
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\n{length}connection: close\r\n\r\nbody"
            )
            .unwrap();
        });
        let mut plugin = ToolchainPlugin::parse_acl(&declaration(&archive())).unwrap();
        plugin.length = 1;
        let http = HttpSource::for_test(&url, &url, &[]).unwrap();
        plugin.source = ArchiveSource::Http(url);
        let temporary = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temporary.path()).unwrap();
        assert!(matches!(
            plugin.install(&store, &http),
            Err(ToolAcquireError::DownloadRejected)
        ));
        server.join().unwrap();
        assert!(store
            .installed_receipt("probe", Some("1.0.0"))
            .unwrap()
            .is_none());
    }
}

#[test]
fn cached_installation_uses_the_same_verifier_without_constructing_an_http_client() {
    let bytes = archive();
    let plugin = ToolchainPlugin::parse_acl(&declaration(&bytes)).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    let receipt = plugin.install_archive(&store, bytes.clone()).unwrap();
    let mut changed = bytes;
    changed[0] ^= 1;
    assert!(matches!(
        plugin.install_archive(&store, changed),
        Err(ToolAcquireError::ChecksumMismatch)
    ));
    assert_eq!(
        store.installed_receipt("probe", None).unwrap(),
        Some(receipt)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
#[ignore = "requires the independently downloaded pinned CUA 0.9.0 archive via ASTER_CUA_ARCHIVE"]
fn cua_cli_v0_9_0_preserves_the_full_producer_bundle_and_runs_its_native_version_command() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let path = std::env::var_os("ASTER_CUA_ARCHIVE")
        .expect("set ASTER_CUA_ARCHIVE to the pinned upstream archive");
    let bytes = std::fs::read(PathBuf::from(path)).unwrap();
    assert_eq!(bytes.len(), 73_285_759);
    assert_eq!(
        crate::store::hex_encode(&Sha256::digest(&bytes)),
        "8d15f4b82694ee8e1c78b419331795bfb3f5fcbab80a7d03ab409a9da89c0bae"
    );
    let text = r#"toolchain {
 schema_version = 1
 name = "cua"
 spec = "github:trycua/cua"
 version = "0.9.0"
 executable = "cua"
 platform = "aarch64-macos-"
 url = "https://github.com/trycua/cua/releases/download/cua-sdk-v0.9.0/cua-cli-0.9.0-darwin-arm64.tar.gz"
 length = 73285759
 sha256 = "8d15f4b82694ee8e1c78b419331795bfb3f5fcbab80a7d03ab409a9da89c0bae"
 format = "tar.gz"
 prefix = ""
 bin_dirs = [""]
 executable_mode = "0755"
 command "cua" { path = "cua" }
 command "cua-vz" { path = "cua-vz" optional = true }
 command "cua-microvm-init" { path = "cua-microvm-init" optional = true }
 command "cua-spacesd-linux-aarch64" { path = "cua-spacesd-linux-aarch64" optional = true }
 command "cua-spacesd-linux-x86_64" { path = "cua-spacesd-linux-x86_64" optional = true }
}"#;
    let plugin = ToolchainPlugin::parse_acl(text).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("CUA 独立工具库")).unwrap();
    let receipt = plugin.install_archive(&store, bytes).unwrap();
    let paths = store.installation_paths(&receipt).unwrap();
    let installed = paths.executable().parent().unwrap();
    let expected = BTreeMap::from([
        ("cua", 148_176_752),
        ("cua-vz", 313_968),
        ("cua-microvm-init", 768_328),
        ("cua-spacesd-linux-aarch64", 28_962_264),
    ]);
    assert_eq!(
        std::fs::read_dir(installed).unwrap().count(),
        expected.len()
    );
    for (name, length) in expected {
        let metadata = std::fs::metadata(installed.join(name)).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.len(), length);
        assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
    }
    let mut process = Command::new(paths.executable())
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while process.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            process.kill().unwrap();
            process.wait().unwrap();
            panic!("pinned native CUA --version exceeded its fixed 20-second deadline");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert!(String::from_utf8(output.stdout).unwrap().contains("0.9.0"));
}
