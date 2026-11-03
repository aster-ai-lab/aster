use super::*;
use crate::{AcquireRequest, EnvConfig, EnvLock, ResolvedTool, ToolStore, VersionRequest};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

pub(super) fn serve(
    listener: TcpListener,
    responses: Vec<(String, Vec<u8>)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for (expected, body) in responses {
            let mut stream = super::test_server::accept(&listener);
            let mut request = String::new();
            BufReader::new(&mut stream).read_line(&mut request).unwrap();
            assert!(
                request.starts_with(&format!("GET {expected} ")),
                "{request}"
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    })
}

fn npm_archive(version: &str) -> Vec<u8> {
    let manifest = serde_json::to_vec(
        &serde_json::json!({ "name":"ntn", "version":version, "bin":{"ntn":"bin/ntn"} }),
    )
    .unwrap();
    let script = format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n").into_bytes();
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    for (name, bytes) in [
        ("package/package.json", manifest),
        ("package/bin/ntn", script),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o755);
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        tar.append_data(&mut header, name, &bytes[..]).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
fn paginated_github_versions_sort_numerically_and_are_available_offline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let first: Vec<_> = (0..100).map(|index| serde_json::json!({ "tag_name":format!("v0.0.{index}"),"draft":false,"prerelease":false })).collect();
    let second = serde_json::json!([{ "tag_name":"v0.0.100","draft":false,"prerelease":false }]);
    let server = serve(
        listener,
        vec![
            (
                "/repos/fixture/tool/releases?per_page=100&page=1".into(),
                serde_json::to_vec(&first).unwrap(),
            ),
            (
                "/repos/fixture/tool/releases?per_page=100&page=2".into(),
                serde_json::to_vec(&second).unwrap(),
            ),
        ],
    );
    let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    let spec = crate::parse_tool_spec("github:fixture/tool").unwrap();
    let listing = store.remote_versions_with(&spec, Some(&source)).unwrap();
    assert_eq!(listing.versions().len(), 101);
    assert_eq!(
        listing
            .resolve(&VersionRequest::parse("prefix:0.0").unwrap())
            .unwrap(),
        "0.0.100"
    );
    server.join().unwrap();
    assert_eq!(store.remote_versions_with(&spec, None).unwrap(), listing);
}

#[test]
fn npm_dist_tag_install_and_lock_remain_resolvable_after_the_server_stops() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let first = npm_archive("1.0.0");
    let next = npm_archive("2.0.0");
    let metadata = serde_json::to_vec(&serde_json::json!({
        "dist-tags": {"latest":"1.0.0","next":"2.0.0"},
        "versions": {
            "1.0.0": {"dist":{"tarball":format!("{address}/ntn-1.0.0.tgz"),"integrity":format!("sha512-{}", STANDARD.encode(Sha512::digest(&first))) }},
            "2.0.0": {"dist":{"tarball":format!("{address}/ntn-2.0.0.tgz"),"integrity":format!("sha512-{}", STANDARD.encode(Sha512::digest(&next))) }},
        }
    })).unwrap();
    let server = serve(
        listener,
        vec![
            ("/ntn".into(), metadata.clone()),
            ("/ntn".into(), metadata),
            ("/ntn-2.0.0.tgz".into(), next),
        ],
    );
    let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 store")).unwrap();
    let receipt = store
        .acquire_requested_with(
            AcquireRequest {
                name: "ntn".into(),
                spec: "npm:ntn".into(),
                version: Some("next".into()),
                executable_name: "ntn".into(),
            },
            &source,
        )
        .unwrap();
    assert_eq!(receipt.version, "2.0.0");
    server.join().unwrap();
    let selected = store
        .installed_for(&ResolvedTool {
            name: "ntn".into(),
            spec: "npm:ntn".into(),
            version: Some("next".into()),
            executable: "ntn".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(selected.version, "2.0.0");
    let config = EnvConfig {
        tools: BTreeMap::from([("ntn".into(), "next".into())]),
        vars: BTreeMap::new(),
    };
    let lock = EnvLock::capture(&config, &store).unwrap();
    let reread = EnvLock::parse(&lock.render()).unwrap();
    assert_eq!(
        reread.resolve(&config).unwrap()[0].version.as_deref(),
        Some("2.0.0")
    );
    assert_eq!(
        reread.install_with(&store, None).unwrap()[0].version,
        "2.0.0"
    );
    #[cfg(unix)]
    {
        let output = std::process::Command::new(store.which("ntn").unwrap().unwrap())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"2.0.0\n");
    }
}

#[test]
fn malformed_provider_prerelease_flags_are_not_treated_as_stable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = serve(
        listener,
        vec![(
            "/repos/fixture/tool/releases?per_page=100&page=1".into(),
            br#"[{"tag_name":"v1.0.0","prerelease":"false"}]"#.to_vec(),
        )],
    );
    let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
    assert!(matches!(
        source.versions(&crate::parse_tool_spec("github:fixture/tool").unwrap()),
        Err(ToolAcquireError::DownloadRejected)
    ));
    server.join().unwrap();
}

#[test]
fn direct_fetch_rejects_unresolved_or_escaping_versions_before_io() {
    let source =
        HttpSource::for_test("http://127.0.0.1:9", "http://127.0.0.1:9", &["test-host"]).unwrap();
    for version in ["prefix:1", "../outside", "1.0/../../outside"] {
        let query = ToolQuery {
            spec: crate::parse_tool_spec("github:fixture/tool").unwrap(),
            version: Some(version.into()),
            executable: "fixture".into(),
        };
        assert!(matches!(
            source.fetch(&query),
            Err(ToolAcquireError::InvalidVersion(_))
        ));
    }
}

#[test]
fn endpoint_credentials_stay_out_of_debug_including_containing_configuration() {
    #[derive(Debug)]
    struct Configuration {
        _policy: FetchPolicy,
        _source: HttpSource,
    }
    let secret = "synthetic-endpoint-secret";
    let endpoint =
        format!("https://{secret}:{secret}@example.invalid/{secret}?token={secret}#{secret}");
    let configuration = Configuration {
        _policy: FetchPolicy {
            github_mirror: endpoint.clone(),
            npm_registry: endpoint.clone(),
            github_token: secret.into(),
        },
        _source: HttpSource::for_test(&endpoint, &endpoint, &["test-host"]).unwrap(),
    };
    let debug = format!("{configuration:?}");
    assert!(!debug.contains(secret));
    assert!(debug.contains("https://example.invalid"));
}
