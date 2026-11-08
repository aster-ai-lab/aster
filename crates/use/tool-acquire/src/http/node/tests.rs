use super::*;
use crate::ToolSource;
use std::net::TcpListener;

#[test]
fn provider_declared_lts_aliases_and_latest_resolve_from_official_metadata_shape() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let metadata = br#"[{"version":"v22.3.0","lts":"Jod"},{"version":"v24.1.0","lts":"Krypton"},{"version":"v26.0.0","lts":false},{"version":"v27.0.0-alpha.0.0.0","lts":false}]"#.to_vec();
    let server =
        crate::http::version_tests::serve(listener, vec![("/index.json".into(), metadata)]);
    let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
    let versions = source
        .versions(&crate::parse_tool_spec("core:node").unwrap())
        .unwrap();
    server.join().unwrap();
    for (request, version) in [
        ("latest", "26.0.0"),
        ("lts", "24.1.0"),
        ("lts/*", "24.1.0"),
        ("lts/jod", "22.3.0"),
    ] {
        assert_eq!(
            versions
                .resolve(&VersionRequest::parse(request).unwrap())
                .unwrap(),
            version
        );
    }
}

#[cfg(all(
    any(target_arch = "x86_64", target_arch = "aarch64"),
    any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", target_env = "gnu")
    )
))]
#[test]
fn official_signature_is_checked_before_an_altered_runtime_archive_is_refused() {
    let target = NodeTarget::current().unwrap();
    let filename = format!(
        "node-v20.11.0-{}-{}.{}",
        target.os,
        target.arch,
        if target.windows { "zip" } else { "tar.gz" }
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = crate::http::version_tests::serve(
        listener,
        vec![
            (
                "/v20.11.0/SHASUMS256.txt".into(),
                include_bytes!("testdata/node_v20.11.0_SHASUMS256.txt").to_vec(),
            ),
            (
                "/v20.11.0/SHASUMS256.txt.sig".into(),
                include_bytes!("testdata/node_v20.11.0_SHASUMS256.txt.sig").to_vec(),
            ),
            (
                format!("/v20.11.0/{filename}"),
                b"altered runtime archive".to_vec(),
            ),
        ],
    );
    let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
    let result = source.fetch_installation(&ToolQuery {
        spec: crate::parse_tool_spec("core:node").unwrap(),
        version: Some("20.11.0".into()),
        executable: "node".into(),
    });
    server.join().unwrap();
    assert!(matches!(result, Err(ToolAcquireError::ChecksumMismatch)));
}

#[test]
fn native_version_admission_and_complete_layout_protocol_refuse_before_io() {
    let source =
        HttpSource::for_test("http://127.0.0.1:9", "http://127.0.0.1:9", &["test-host"]).unwrap();
    for version in ["not-a-node-version", "1.2.3/../../outside"] {
        assert!(matches!(
            source.fetch_installation(&ToolQuery {
                spec: crate::parse_tool_spec("core:node").unwrap(),
                version: Some(version.into()),
                executable: "node".into(),
            }),
            Err(ToolAcquireError::InvalidVersion(_))
        ));
    }
    assert_eq!(node_version("20.11.0-rc.1").unwrap(), "20.11.0-rc.1");
    assert!(matches!(
        source.fetch(&ToolQuery {
            spec: crate::parse_tool_spec("core:node").unwrap(),
            version: Some("20.11.0".into()),
            executable: "node".into()
        }),
        Err(ToolAcquireError::RuntimeLayoutRequired { .. })
    ));
}

#[test]
#[cfg(all(
    any(target_arch = "x86_64", target_arch = "aarch64"),
    any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", target_env = "gnu")
    )
))]
fn invalid_signatures_refuse_before_requesting_a_runtime_archive() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = crate::http::version_tests::serve(
        listener,
        vec![
            (
                "/v20.11.0/SHASUMS256.txt".into(),
                include_bytes!("testdata/node_v20.11.0_SHASUMS256.txt").to_vec(),
            ),
            (
                "/v20.11.0/SHASUMS256.txt.sig".into(),
                b"invalid signature".to_vec(),
            ),
        ],
    );
    let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
    let result = source.fetch_installation(&ToolQuery {
        spec: crate::parse_tool_spec("core:node").unwrap(),
        version: Some("20.11.0".into()),
        executable: "node".into(),
    });
    server.join().unwrap();
    assert!(matches!(result, Err(ToolAcquireError::NodeSignature(_))));
}

#[test]
#[ignore = "downloads the signed official Node runtime and runs its Node/npm entrypoints"]
fn signed_official_node_runs_native_stdlib_and_npm_without_downloading_an_installed_version_again()
{
    use crate::{AcquireRequest, ToolPayload, ToolSpec, ToolStore};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    struct CountedSource {
        http: HttpSource,
        downloads: AtomicUsize,
        metadata: AtomicUsize,
    }
    impl ToolSource for CountedSource {
        fn fetch(&self, query: &ToolQuery) -> Result<ToolPayload, ToolAcquireError> {
            self.http.fetch(query)
        }
        fn fetch_installation(
            &self,
            query: &ToolQuery,
        ) -> Result<InstallPayload, ToolAcquireError> {
            self.downloads.fetch_add(1, Ordering::SeqCst);
            self.http.fetch_installation(query)
        }
        fn latest(&self, spec: &ToolSpec) -> Result<String, ToolAcquireError> {
            self.metadata.fetch_add(1, Ordering::SeqCst);
            self.http.latest(spec)
        }
        fn versions(&self, spec: &ToolSpec) -> Result<VersionListing, ToolAcquireError> {
            self.metadata.fetch_add(1, Ordering::SeqCst);
            self.http.versions(spec)
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("isolated home");
    std::fs::create_dir(&home).unwrap();
    let store = ToolStore::open(temp.path().join("运行时 store")).unwrap();
    let source = CountedSource {
        http: HttpSource::new().unwrap(),
        downloads: AtomicUsize::new(0),
        metadata: AtomicUsize::new(0),
    };
    let request = || AcquireRequest {
        name: "node".into(),
        spec: "core:node".into(),
        version: Some("20.11.0".into()),
        executable_name: "node".into(),
    };
    let cold = Instant::now();
    let receipt = store.acquire_with(request(), &source).unwrap();
    let cold_ms = cold.elapsed().as_millis();
    let paths = store.installation_paths(&receipt).unwrap();
    let path = std::env::join_paths(
        paths.bin_dirs().iter().cloned().chain(
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>()),
        ),
    )
    .unwrap();
    let mut node = Command::new(paths.executable());
    node.env_clear()
        .current_dir(&home)
        .env("HOME", &home)
        .env("PATH", &path)
        .env("NODE_PROBE_ROOT", temp.path());
    for name in ["SystemRoot", "COMSPEC", "PATHEXT", "TEMP", "TMP", "TMPDIR"] {
        if let Some(value) = std::env::var_os(name) {
            node.env(name, value);
        }
    }
    let output = node.args(["-e", "const fs=require('node:fs');const path=require('node:path');const crypto=require('node:crypto');fs.writeFileSync(path.join(process.env.NODE_PROBE_ROOT,'native-effect'),'executed');console.log(JSON.stringify({version:process.version,hash:crypto.createHash('sha256').update('aster-native-node').digest('hex')}));"]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result.get("version").and_then(|value| value.as_str()),
        Some("v20.11.0")
    );
    use sha2::{Digest, Sha256};
    assert_eq!(
        result.get("hash").and_then(|value| value.as_str()),
        Some(crate::store::hex_encode(&Sha256::digest(b"aster-native-node")).as_str())
    );
    assert_eq!(
        std::fs::read(temp.path().join("native-effect")).unwrap(),
        b"executed"
    );
    let npm = paths
        .bin_dirs()
        .first()
        .unwrap()
        .join(if cfg!(windows) { "npm.cmd" } else { "npm" });
    let mut npm_command = Command::new(npm);
    npm_command
        .env_clear()
        .current_dir(&home)
        .env("HOME", &home)
        .env("NPM_CONFIG_USERCONFIG", home.join("user.npmrc"))
        .env("NPM_CONFIG_GLOBALCONFIG", home.join("global.npmrc"))
        .env("PATH", &path);
    for name in ["SystemRoot", "COMSPEC", "PATHEXT", "TEMP", "TMP", "TMPDIR"] {
        if let Some(value) = std::env::var_os(name) {
            npm_command.env(name, value);
        }
    }
    let output = npm_command.arg("--version").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(matches!(
        versions::Versioning::new(String::from_utf8(output.stdout).unwrap().trim()),
        Some(versions::Versioning::Ideal(_))
    ));
    let warm = Instant::now();
    assert_eq!(
        store.acquire_requested_with(request(), &source).unwrap(),
        receipt
    );
    assert_eq!(source.downloads.load(Ordering::SeqCst), 1);
    assert_eq!(source.metadata.load(Ordering::SeqCst), 0);
    println!(
        "{}",
        serde_json::json!({"version":"20.11.0","os":std::env::consts::OS,"arch":std::env::consts::ARCH,"profile":"test/debug","cold_install_ms":cold_ms,"warm_install_ms":warm.elapsed().as_millis(),"distribution_fetches":source.downloads.load(Ordering::SeqCst),"metadata_queries":source.metadata.load(Ordering::SeqCst)})
    );
}
