use super::*;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use aster_use_extension::build_aix_package;
use serde_json::{Value, json};

use super::super::{LocalCapabilities, PluginPin, digest_of, sync_plugins_with_toolchain};

const PACKAGE: &str = "use/aster/tool-probe";

struct Fixture {
    _directory: tempfile::TempDir,
    capabilities: LocalCapabilities,
    agents: PathBuf,
    slug: String,
    bytes: Vec<u8>,
    plugin: ToolchainPlugin,
}

fn artifact() -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in [
        (
            if cfg!(windows) { "probe.exe" } else { "probe" },
            b"#!/bin/sh\nprintf 'owned-runtime' > \"$1\"\n".as_slice(),
        ),
        ("share/data", b"required companion".as_slice()),
    ] {
        archive
            .start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn platform() -> &'static str {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => "aarch64-macos-",
        ("x86_64", "macos") => "x86_64-macos-",
        ("aarch64", "linux") if cfg!(target_env = "musl") => "aarch64-linux-musl",
        ("x86_64", "linux") if cfg!(target_env = "musl") => "x86_64-linux-musl",
        ("aarch64", "linux") => "aarch64-linux-gnu",
        ("x86_64", "linux") => "x86_64-linux-gnu",
        ("aarch64", "windows") => "aarch64-windows-msvc",
        ("x86_64", "windows") => "x86_64-windows-msvc",
        target => panic!("unsupported test target: {target:?}"),
    }
}

impl Fixture {
    fn new(artifact: &[u8]) -> Self {
        let directory = tempfile::Builder::new()
            .prefix("tool plugin 空格 ")
            .tempdir()
            .unwrap();
        let source = directory.path().join("source");
        fs::create_dir(&source).unwrap();
        let text = format!(
            r#"toolchain {{
 schema_version = 1
 name = "probe"
 spec = "github:example/probe"
 version = "0.6.4.post4"
 executable = "probe"
 platform = "{}"
 url = "https://github.com/example/probe/releases/download/v1.0.0/probe.zip"
 length = {}
 sha256 = "{}"
 format = "zip"
 prefix = ""
 bin_dirs = [""]
 executable_mode = "0755"
 command "probe" {{ path = "{}" }}
}}"#,
            platform(),
            artifact.len(),
            digest_of(artifact).trim_start_matches("sha256:"),
            if cfg!(windows) { "probe.exe" } else { "probe" }
        );
        fs::write(source.join("toolchain.acl"), &text).unwrap();
        fs::write(
            source.join("aster-use-extension.acl"),
            r#"extension "aster/tool-probe" {
 schema_version = 3
 version = "1.0.0"
 route = "probe"
 requires_use = ">=0.3.0, <0.4.0"
 actions = ["read"]
 repository { url = "https://github.com/aster-ai-lab/plugins" }
 plugin "toolchain" { entry = "toolchain.acl" }
}"#,
        )
        .unwrap();
        fs::write(source.join("README.md"), "# Tool fixture\n").unwrap();
        let archive = directory.path().join("fixture.aix");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(build_aix_package(&source, &archive))
            .unwrap();
        runtime
            .block_on(ExtensionLifecyclePackage::prepare_local(
                "aster/tool-probe",
                &archive,
                true,
            ))
            .unwrap();
        let tenant = directory.path().join("租户 with spaces");
        let agents = tenant.join("agents");
        fs::create_dir_all(&agents).unwrap();
        Self {
            capabilities: LocalCapabilities::open_at(Some(tenant.join("capabilities"))),
            agents,
            slug: digest_of(b"aster/tool-probe")
                .trim_start_matches("sha256:")
                .into(),
            bytes: fs::read(archive).unwrap(),
            plugin: ToolchainPlugin::parse_acl(&text).unwrap(),
            _directory: directory,
        }
    }

    fn caps(&self) -> &Path {
        self.capabilities.root.as_deref().unwrap()
    }

    fn embedded(mut self, artifact: &[u8]) -> Self {
        let source = self._directory.path().join("source");
        let text = fs::read_to_string(source.join("toolchain.acl"))
            .unwrap()
            .replace(
                "url = \"https://github.com/example/probe/releases/download/v1.0.0/probe.zip\"",
                "archive = \"payload/运行时 with spaces.zip\"",
            );
        fs::write(source.join("toolchain.acl"), text).unwrap();
        fs::create_dir(source.join("payload")).unwrap();
        fs::write(source.join("payload/运行时 with spaces.zip"), artifact).unwrap();
        self.rebuild();
        self
    }

    fn rebuild(&mut self) {
        let source = self._directory.path().join("source");
        let archive = self._directory.path().join("fixture.aix");
        fs::remove_file(&archive).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(build_aix_package(&source, &archive))
            .unwrap();
        self.bytes = fs::read(archive).unwrap();
        self.plugin =
            ToolchainPlugin::parse_acl(&fs::read_to_string(source.join("toolchain.acl")).unwrap())
                .unwrap();
    }

    fn prepare_from_aix(&self) -> io::Result<PreparedToolchain> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let package = runtime
            .block_on(ExtensionLifecyclePackage::prepare_local(
                "aster/tool-probe",
                &self._directory.path().join("fixture.aix"),
                true,
            ))
            .map_err(io::Error::other)?;
        runtime
            .block_on(prepare(&package, Arc::new(self.bytes.clone())))?
            .ok_or_else(|| io::Error::other("toolchain missing from fixture"))
    }

    fn install(&self, prepared: Option<PreparedToolchain>) -> io::Result<()> {
        let pins = BTreeMap::from([(
            self.slug.clone(),
            PluginPin {
                require_aix: true,
                digest: digest_of(&self.bytes),
                package_id: PACKAGE.into(),
            },
        )]);
        sync_plugins_with_toolchain(
            self.caps(),
            &self.agents,
            &pins,
            &|_| Ok(self.bytes.clone()),
            prepared.map(|prepared| (self.slug.as_str(), prepared)),
        )
    }

    fn store(&self) -> ReconcileStore {
        ReconcileStore::open(self.agents.parent().unwrap().join("package-generations")).unwrap()
    }

    fn page(&self) -> Value {
        let bytes = self
            .capabilities
            .registry_request("GET", &["api", "plugins"], "installed=true", &[])
            .unwrap()
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}

#[test]
fn embedded_aix_payload_uses_the_existing_install_query_and_removal_owner() {
    let archive = artifact();
    let fixture = Fixture::new(&archive).embedded(&archive);
    fixture
        .install(Some(fixture.prepare_from_aix().unwrap()))
        .unwrap();
    let publication = fixture.store().current_sync(PACKAGE).unwrap().unwrap();
    let page = fixture.page();
    assert!(
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["runtimeName"] == "aster/tool-probe"
                && row["installed"] == true
                && row["needsRepair"] == false)
    );
    let callable: Value = serde_json::from_slice(
        &super::super::callable_plugin_json(fixture.caps(), &fixture.slug).unwrap(),
    )
    .unwrap();
    assert!(Path::new(callable["surfaces"][0]["entry"].as_str().unwrap()).is_file());
    let lease = fixture.store().acquire_sync(&publication.cursor()).unwrap();
    fixture
        .capabilities
        .registry_request(
            "DELETE",
            &["api", "plugins", "installations", &fixture.slug],
            "",
            br#"{"packageId":"aster/tool-probe"}"#,
        )
        .unwrap()
        .unwrap();
    assert!(fixture.store().current_sync(PACKAGE).unwrap().is_none());
    assert!(publication.owned_directory().is_dir());
    lease.release_sync().unwrap();
    assert!(!publication.owned_directory().exists());
}

#[test]
fn missing_damaged_or_wrong_length_embedded_payload_preserves_the_working_generation() {
    let archive = artifact();
    for change in ["missing", "checksum", "length"] {
        let mut fixture = Fixture::new(&archive).embedded(&archive);
        fixture
            .install(Some(fixture.prepare_from_aix().unwrap()))
            .unwrap();
        let publication = fixture.store().current_sync(PACKAGE).unwrap().unwrap();
        let payload = fixture
            ._directory
            .path()
            .join("source/payload/运行时 with spaces.zip");
        match change {
            "missing" => fs::remove_file(payload).unwrap(),
            "checksum" => {
                let mut damaged = archive.clone();
                damaged[0] ^= 1;
                fs::write(payload, damaged).unwrap();
            }
            "length" => fs::write(payload, &archive[..archive.len() - 1]).unwrap(),
            _ => unreachable!(),
        }
        fixture.rebuild();
        assert!(fixture.prepare_from_aix().is_err(), "{change}");
        assert_eq!(
            fixture
                .store()
                .current_sync(PACKAGE)
                .unwrap()
                .unwrap()
                .cursor(),
            publication.cursor()
        );
    }
}

// Catch a successful package marker masking an absent runtime, or uninstall deleting a leased runtime.
#[test]
fn native_toolchain_install_query_reuse_and_removal_share_the_package_lease() {
    let archive = artifact();
    let fixture = Fixture::new(&archive);
    assert!(fixture.install(None).is_err());
    assert!(fixture.store().current_sync(PACKAGE).unwrap().is_none());
    fixture
        .install(Some(fixture.plugin.prepare_archive(archive).unwrap()))
        .unwrap();
    let publication = fixture.store().current_sync(PACKAGE).unwrap().unwrap();
    let page = fixture.page();
    let row = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["runtimeName"] == "aster/tool-probe")
        .unwrap();
    assert_eq!(row["version"], "1.0.0");
    assert_eq!(row["installedToolVersion"], "0.6.4.post4");
    assert_eq!(row["installed"], true);
    assert_eq!(row["needsRepair"], false);
    let bytes = super::super::callable_plugin_json(fixture.caps(), &fixture.slug).unwrap();
    let callable: Value = serde_json::from_slice(&bytes).unwrap();
    let tool = &callable["surfaces"][0];
    assert_eq!(tool["id"], "toolchain");
    assert_eq!(tool["version"], "0.6.4.post4");
    let executable = PathBuf::from(tool["entry"].as_str().unwrap());
    assert!(executable.starts_with(publication.owned_directory()));
    assert!(executable.is_file());
    #[cfg(unix)]
    {
        let output = fixture.agents.parent().unwrap().join("command effect");
        assert!(
            std::process::Command::new(&executable)
                .arg(&output)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(fs::read(output).unwrap(), b"owned-runtime");
    }
    fixture.install(None).unwrap();
    assert_eq!(
        fixture
            .store()
            .current_sync(PACKAGE)
            .unwrap()
            .unwrap()
            .cursor(),
        publication.cursor()
    );
    let lease = fixture.store().acquire_sync(&publication.cursor()).unwrap();
    fixture
        .capabilities
        .registry_request(
            "DELETE",
            &["api", "plugins", "installations", &fixture.slug],
            "",
            br#"{"packageId":"aster/tool-probe"}"#,
        )
        .unwrap()
        .unwrap();
    assert!(fixture.store().current_sync(PACKAGE).unwrap().is_none());
    assert!(
        fixture.page()["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["runtimeName"] != "aster/tool-probe")
    );
    assert!(executable.is_file());
    lease.release_sync().unwrap();
    assert!(!publication.owned_directory().exists());
}

// Catch archive extraction failure replacing a working publication or its source markers.
#[test]
fn failed_toolchain_update_preserves_the_working_publication_and_source() {
    let archive = artifact();
    let mut fixture = Fixture::new(&archive);
    fixture
        .install(Some(fixture.plugin.prepare_archive(archive).unwrap()))
        .unwrap();
    let original = fixture.store().current_sync(PACKAGE).unwrap().unwrap();
    let marker = fixture
        .caps()
        .join("plugins")
        .join(&fixture.slug)
        .join(super::super::DIGEST_MARKER);
    let digest = fs::read(&marker).unwrap();
    let candidate = Fixture::new(b"pinned bytes that are not a zip");
    fixture.bytes = candidate.bytes;
    let prepared = candidate
        .plugin
        .prepare_archive(b"pinned bytes that are not a zip".to_vec())
        .unwrap();
    assert!(fixture.install(Some(prepared)).is_err());
    assert_eq!(
        fixture.store().current_sync(PACKAGE).unwrap().unwrap(),
        original
    );
    assert_eq!(fs::read(marker).unwrap(), digest);
    assert!(super::super::callable_plugin_json(fixture.caps(), &fixture.slug).is_ok());
    assert!(
        !fixture
            .caps()
            .join("plugins")
            .join(format!(".staging-{}", fixture.slug))
            .exists()
    );
}

// Catch a modified runtime being accepted solely because its local receipt was modified too.
#[test]
fn rewritten_runtime_receipts_require_a_new_verified_generation() {
    let archive = artifact();
    let fixture = Fixture::new(&archive);
    fixture
        .install(Some(
            fixture.plugin.prepare_archive(archive.clone()).unwrap(),
        ))
        .unwrap();
    let original = fixture.store().current_sync(PACKAGE).unwrap().unwrap();
    let lease = fixture.store().acquire_sync(&original.cursor()).unwrap();
    let root = original.owned_directory();
    let companion = root.join("installs/probe/0.6.4.post4/share/data");
    fs::write(&companion, b"changed companion").unwrap();
    assert!(super::super::callable_plugin_json(fixture.caps(), &fixture.slug).is_err());
    assert!(
        fixture.page()["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["runtimeName"] == "aster/tool-probe"
                && row["needsRepair"] == true
                && row["installedToolVersion"].is_null())
    );
    let record_path = root.join("receipts/probe/0.6.4.post4.archive.json");
    let mut record: Value = serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
    record["entries"]["share/data"]["sha256"] =
        json!(digest_of(b"changed companion").trim_start_matches("sha256:"));
    fs::write(&record_path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(
        fixture
            .plugin
            .installed(&ToolStore::open(root).unwrap())
            .unwrap()
            .is_some()
    );
    assert!(super::super::callable_plugin_json(fixture.caps(), &fixture.slug).is_err());
    fixture
        .install(Some(fixture.plugin.prepare_archive(archive).unwrap()))
        .unwrap();
    let repaired = fixture.store().current_sync(PACKAGE).unwrap().unwrap();
    assert_ne!(repaired.cursor(), original.cursor());
    assert_eq!(
        fs::read(
            repaired
                .owned_directory()
                .join("installs/probe/0.6.4.post4/share/data")
        )
        .unwrap(),
        b"required companion"
    );
    assert_eq!(fs::read(companion).unwrap(), b"changed companion");
    assert!(super::super::callable_plugin_json(fixture.caps(), &fixture.slug).is_ok());
    lease.release_sync().unwrap();
    assert!(!original.owned_directory().exists());
}

#[test]
fn toolchain_declaration_cannot_claim_a_different_package_route() {
    let fixture = Fixture::new(&artifact());
    let text = fs::read_to_string(fixture._directory.path().join("source/toolchain.acl")).unwrap();
    let manifest = fs::read_to_string(
        fixture
            ._directory
            .path()
            .join("source/aster-use-extension.acl"),
    )
    .unwrap();
    let manifest =
        ExtensionManifest::parse_acl(&manifest.replace("route = \"probe\"", "route = \"other\""))
            .unwrap();
    assert!(declaration(&manifest, text.as_bytes()).is_err());
}

#[test]
#[ignore = "requires the independently built Notion AIX candidate and a real upstream download"]
fn actual_notion_aix_installs_queries_executes_and_removes_through_the_desktop_owner() {
    let archive = PathBuf::from(std::env::var_os("ASTER_TEST_NTN_AIX").unwrap());
    let bytes = Arc::new(fs::read(&archive).unwrap());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let package = runtime
        .block_on(ExtensionLifecyclePackage::prepare_local(
            "aster/ntn",
            &archive,
            true,
        ))
        .unwrap();
    let prepared = runtime
        .block_on(prepare(&package, Arc::clone(&bytes)))
        .unwrap()
        .unwrap();
    let root = tempfile::Builder::new()
        .prefix("Notion desktop 空格 ")
        .tempdir()
        .unwrap();
    let caps = root.path().join("capabilities");
    let agents = root.path().join("agents");
    fs::create_dir(&agents).unwrap();
    let local = LocalCapabilities::open_at(Some(caps.clone()));
    let slug = digest_of(b"aster/ntn")
        .trim_start_matches("sha256:")
        .to_owned();
    let pins = BTreeMap::from([(
        slug.clone(),
        PluginPin {
            require_aix: true,
            digest: digest_of(&bytes),
            package_id: "use/aster/ntn".into(),
        },
    )]);
    sync_plugins_with_toolchain(
        &caps,
        &agents,
        &pins,
        &|_| Ok(bytes.as_ref().clone()),
        Some((&slug, prepared)),
    )
    .unwrap();
    let callable: Value =
        serde_json::from_slice(&super::super::callable_plugin_json(&caps, &slug).unwrap()).unwrap();
    let tool = &callable["surfaces"][0];
    assert_eq!(tool["name"], "ntn");
    assert_eq!(tool["version"], "0.23.25");
    let output = runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new(tool["entry"].as_str().unwrap())
                .arg("--version")
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
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("0.23.25"));
    let page: Value = serde_json::from_slice(
        &local
            .registry_request("GET", &["api", "plugins"], "installed=true&q=ntn", &[])
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["runtimeName"] == "aster/ntn"
                && row["installed"] == true
                && row["needsRepair"] == false)
    );
    let store = ReconcileStore::open(root.path().join("package-generations")).unwrap();
    let publication = store.current_sync("use/aster/ntn").unwrap().unwrap();
    let lease = store.acquire_sync(&publication.cursor()).unwrap();
    drop(runtime);
    local
        .registry_request(
            "DELETE",
            &["api", "plugins", "installations", &slug],
            "",
            br#"{"packageId":"aster/ntn"}"#,
        )
        .unwrap()
        .unwrap();
    assert!(store.current_sync("use/aster/ntn").unwrap().is_none());
    assert!(publication.owned_directory().is_dir());
    lease.release_sync().unwrap();
    assert!(!publication.owned_directory().exists());
    if let Some(path) = std::env::var_os("ASTER_TEST_NTN_DESKTOP_EVIDENCE") {
        fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "packageId":"aster/ntn", "version":"0.23.25", "aixArchiveSha256":digest_of(&bytes),
                "versionStdout":String::from_utf8_lossy(&output.stdout),
                "downloadPreparedBeforePublication":true,"installedCatalogueQueried":true,
                "nativeCommandExecuted":true,"officialManagementRemovalPassed":true,
                "runtimeRetainedUntilLeaseRelease":true,"permanentInstallation":false,
                "trust":"explicit-local-candidate","officialRegistryPublicationAccepted":false,
                "nativeGuiAccepted":false
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

// Catch incomplete upstream bundles and management records that outlive their package owner.
#[test]
#[ignore = "requires declared AIX candidates and real upstream release downloads"]
fn actual_dependency_aix_candidates_are_queryable_executable_and_removable() {
    verify_actual_dependency_candidates(std::time::Duration::from_secs(10));
}

#[test]
#[ignore = "requires the complete independently built BabelDOC AIX candidate"]
fn actual_embedded_babeldoc_aix_is_queryable_executable_and_removable() {
    verify_actual_dependency_candidates(std::time::Duration::from_secs(120));
}

fn verify_actual_dependency_candidates(command_timeout: std::time::Duration) {
    let input = std::env::var_os("ASTER_TEST_DEPENDENCY_AIX_INPUTS").unwrap();
    let cases: Vec<Value> = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    assert!(!cases.is_empty());
    let root = tempfile::Builder::new()
        .prefix("dependency plugins 空格 ")
        .tempdir()
        .unwrap();
    let caps = root.path().join("capabilities");
    let agents = root.path().join("agents");
    fs::create_dir(&agents).unwrap();
    let local = LocalCapabilities::open_at(Some(caps.clone()));
    let store = ReconcileStore::open(root.path().join("package-generations")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut pins = BTreeMap::new();
    let mut evidence = Vec::new();
    let mut accepted = Vec::new();
    for case in &cases {
        let id = case["packageId"].as_str().unwrap();
        let archive = Path::new(case["aix"].as_str().unwrap());
        let bytes = Arc::new(fs::read(archive).unwrap());
        let package = runtime
            .block_on(ExtensionLifecyclePackage::prepare_local(id, archive, true))
            .unwrap();
        let prepared = runtime
            .block_on(prepare(&package, Arc::clone(&bytes)))
            .unwrap()
            .unwrap();
        assert_eq!(prepared.plugin().name(), case["tool"].as_str().unwrap());
        assert_eq!(
            prepared.plugin().version(),
            case["version"].as_str().unwrap()
        );
        let declaration = prepared.plugin().clone();
        let slug = digest_of(id.as_bytes())
            .trim_start_matches("sha256:")
            .to_owned();
        let package_id = format!("use/{id}");
        pins.insert(
            slug.clone(),
            PluginPin {
                require_aix: true,
                digest: digest_of(&bytes),
                package_id: package_id.clone(),
            },
        );
        sync_plugins_with_toolchain(
            &caps,
            &agents,
            &pins,
            &|requested| {
                assert_eq!(requested, slug);
                Ok(bytes.as_ref().clone())
            },
            Some((&slug, prepared)),
        )
        .unwrap();
        let publication = store.current_sync(&package_id).unwrap().unwrap();
        let tools = ToolStore::open(publication.owned_directory()).unwrap();
        let installed = declaration.installed(&tools).unwrap().unwrap();
        let search_path = std::env::join_paths(installed.paths().bin_dirs()).unwrap();
        let callable: Value =
            serde_json::from_slice(&super::super::callable_plugin_json(&caps, &slug).unwrap())
                .unwrap();
        assert_eq!(callable["packageDigest"], digest_of(&bytes));
        assert!(
            callable["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == "toolchain" && row["version"] == case["version"])
        );
        let mut outputs = Vec::new();
        for command in case["requiredCommands"].as_array().unwrap() {
            let command = command.as_str().unwrap();
            let executable = aster_use_tool_acquire::find_env_executable(
                std::ffi::OsStr::new(command),
                Some(&search_path),
                root.path(),
            )
            .unwrap()
            .unwrap();
            let args: Vec<_> = case["versionArgs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap())
                .collect();
            let output = runtime.block_on(async {
                tokio::time::timeout(
                    command_timeout,
                    tokio::process::Command::new(&executable)
                        .args(args)
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
                "{id}/{command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(
                stdout.contains(case["version"].as_str().unwrap()),
                "{id}/{command}: {stdout}"
            );
            outputs.push(json!({"command":command,"stdout":stdout}));
        }
        let lease = store.acquire_sync(&publication.cursor()).unwrap();
        accepted.push((id.to_owned(), slug, publication, lease));
        evidence
            .push(json!({"packageId":id,"aixArchiveSha256":digest_of(&bytes),"outputs":outputs}));
    }
    drop(runtime);
    let page: Value = serde_json::from_slice(
        &local
            .registry_request("GET", &["api", "plugins"], "installed=true", &[])
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    for (id, _, _, _) in &accepted {
        assert!(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["runtimeName"] == *id
                    && row["installed"] == true
                    && row["needsRepair"] == false)
        );
    }
    for (id, slug, publication, lease) in accepted {
        local
            .registry_request(
                "DELETE",
                &["api", "plugins", "installations", &slug],
                "",
                &serde_json::to_vec(&json!({"packageId":id})).unwrap(),
            )
            .unwrap()
            .unwrap();
        assert!(
            store
                .current_sync(&publication.package_id)
                .unwrap()
                .is_none()
        );
        assert!(publication.owned_directory().is_dir());
        lease.release_sync().unwrap();
        assert!(!publication.owned_directory().exists());
    }
    let page: Value = serde_json::from_slice(
        &local
            .registry_request("GET", &["api", "plugins"], "installed=true", &[])
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    for case in &cases {
        assert!(
            page["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["runtimeName"] != case["packageId"])
        );
    }
    if let Some(path) = std::env::var_os("ASTER_TEST_DEPENDENCY_AIX_EVIDENCE") {
        fs::write(path, serde_json::to_vec_pretty(&json!({
                "results":evidence,"installedCatalogueQueried":true,"allRequestedVersionCommandsExecuted":true,
            "allCandidatesRemoved":true,"runtimeRetainedUntilLeaseRelease":true,
            "permanentInstallation":false,"trust":"explicit-local-candidates",
            "officialRegistryPublicationAccepted":false,"nativeGuiAccepted":false,
            "commandDeadlineSeconds":command_timeout.as_secs(),
            "nativeTarget":format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
        })).unwrap()).unwrap();
    }
}
