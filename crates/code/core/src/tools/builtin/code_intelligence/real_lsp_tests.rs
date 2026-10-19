//! Explicit native-server qualification; excluded from hermetic test runs.

use std::{collections::HashSet, path::Path, process::Command, sync::Arc, time::Duration};

use bytes::BytesMut;
use serde_json::{json, Value};
use tokio_util::codec::Decoder;

use crate::{
    code_intelligence::{
        lsp::{codec::LspCodec, process::test_trace::Capture},
        CodeIntelligenceState, LocalCodeIntelligence, WorkspaceCodeIntelligence,
    },
    tools::{builtin::register_builtins, registry::ToolRegistry, ArtifactStoreLimits},
    workspace::{ManifestWorkspaceBackend, WorkspaceFileSystem, WorkspaceServices},
};

// Read only PID, PPID, and executable name for this test's direct children.
// These observations never signal a process or inspect other users' servers.
fn owned_rust_analyzer_pids() -> Vec<u32> {
    let children = Command::new("pgrep")
        .args(["-P", &std::process::id().to_string()])
        .output()
        .expect("pgrep must be available for owned child discovery");
    assert!(matches!(children.status.code(), Some(0 | 1)));
    let pids = String::from_utf8(children.stdout)
        .expect("child process IDs must be UTF-8")
        .split_whitespace()
        .map(|pid| pid.parse::<u32>().unwrap().to_string())
        .collect::<Vec<_>>();
    if pids.is_empty() {
        return Vec::new();
    }
    let output = Command::new("ps")
        .args(["-p", &pids.join(","), "-o", "pid=,ppid=,comm="])
        .output()
        .expect("ps must be available for native process lifecycle evidence");
    assert!(matches!(output.status.code(), Some(0 | 1)));
    let own_pid = std::process::id();
    String::from_utf8(output.stdout)
        .expect("process metadata must be UTF-8")
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse::<u32>().ok()?;
            let parent = fields.next()?.parse::<u32>().ok()?;
            if parent != own_pid {
                return None;
            }
            let executable = fields.collect::<Vec<_>>().join(" ");
            (Path::new(&executable).file_name()?.to_str()? == "rust-analyzer").then_some(pid)
        })
        .collect()
}

fn process_still_exists(pid: u32) -> bool {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "pid="])
        .output()
        .expect("ps must be available to verify the owned child was reaped");
    assert!(matches!(output.status.code(), Some(0 | 1)));
    !output.stdout.iter().all(u8::is_ascii_whitespace)
}

fn protocol_messages(bytes: &[u8]) -> Vec<(Value, usize)> {
    let mut codec = LspCodec::default();
    let mut bytes = BytesMut::from(bytes);
    let length = bytes.len();
    let mut messages = Vec::new();
    while let Some(message) = codec.decode(&mut bytes).unwrap() {
        messages.push((message, length - bytes.len()));
    }
    messages
}

fn write_native_diagnostics(capture: &Capture, root: &Path) {
    let Some(output) = std::env::var_os("A3S_CODE_LSP_SMOKE_EVIDENCE") else {
        return;
    };
    let streams = capture.snapshot();
    let outgoing = protocol_messages(&streams[0].bytes);
    let incoming = protocol_messages(&streams[1].bytes);
    let (observations, observations_truncated) = capture.observations();
    let mut protocol_metadata = Vec::new();
    for (stream, messages) in [(0, &outgoing), (1, &incoming)] {
        for (frame_index, (message, end)) in messages.iter().enumerate() {
            let complete = observations
                .iter()
                .find(|observation| {
                    observation.stream == stream
                        && observation.offset_start < *end
                        && observation.offset_end >= *end
                })
                .map(|observation| observation.elapsed_micros);
            protocol_metadata.push(json!({
                "stream": stream, "frame_index": frame_index, "end_offset": end,
                "observed_complete_micros": complete,
                "method": message.get("method"), "id": message.get("id"),
                "result_count": message.get("result").and_then(Value::as_array).map(Vec::len),
                "health": message.get("params").and_then(|params| params.get("health")),
                "quiescent": message.get("params").and_then(|params| params.get("quiescent")),
            }));
        }
    }
    let outgoing: Vec<_> = outgoing.into_iter().map(|(message, _)| message).collect();
    let incoming: Vec<_> = incoming.into_iter().map(|(message, _)| message).collect();
    let client_ids: HashSet<_> = outgoing
        .iter()
        .filter(|message| {
            matches!(
                message["method"].as_str(),
                Some("initialize" | "textDocument/definition")
            )
        })
        .map(|message| message["id"].to_string())
        .collect();
    let server_ids: HashSet<_> = incoming
        .iter()
        .filter(|message| message["method"] == "workspace/configuration")
        .map(|message| message["id"].to_string())
        .collect();
    let outgoing: Vec<_> = outgoing
        .into_iter()
        .filter(|message| {
            matches!(
                message["method"].as_str(),
                Some("initialize" | "textDocument/definition" | "workspace/didChangeWatchedFiles")
            ) || server_ids.contains(&message["id"].to_string())
        })
        .collect();
    let incoming: Vec<_> = incoming
        .into_iter()
        .filter(|message| {
            client_ids.contains(&message["id"].to_string())
                || matches!(
                    message["method"].as_str(),
                    Some(
                        "workspace/configuration"
                            | "window/logMessage"
                            | "window/showMessage"
                            | "experimental/serverStatus"
                    )
                )
        })
        .collect();
    let metadata: Vec<_> = streams
        .iter()
        .map(|stream| {
            json!({"retained_bytes": stream.bytes.len(), "total_bytes": stream.total_bytes,
                "truncated": stream.total_bytes > stream.bytes.len()})
        })
        .collect();
    let evidence = json!({
        "scope": "only this self-authored temporary Rust fixture; no user workspace or model",
        "temp_root": root,
        "stream_order": ["stdin", "stdout", "stderr"],
        "stream_limit_bytes": 65536,
        "streams": metadata,
        "io_observations": observations,
        "io_observations_truncated": observations_truncated,
        "protocol_metadata": protocol_metadata,
        "time_semantics": "monotonic client IO completion observations, not native processing timestamps or a performance sample",
        "client_to_server": outgoing,
        "server_to_client": incoming,
        "stderr": String::from_utf8_lossy(&streams[2].bytes),
    });
    let output = Path::new(&output);
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(output, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "requires an explicitly installed rust-analyzer; run with the pinned RUSTUP_TOOLCHAIN"]
async fn real_rust_analyzer_tools_start_lazily_navigate_and_reap() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let workspace = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(workspace.path()).unwrap();
    let trace = Capture::register(&root);
    let version = Command::new("rust-analyzer")
        .arg("--version")
        .current_dir(&root)
        .output()
        .expect("rust-analyzer must be explicitly installed before this smoke test");
    assert!(
        version.status.success(),
        "rust-analyzer is unavailable in the temporary workspace's toolchain"
    );
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"native_lsp_smoke\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .unwrap();
    let source = "pub fn answer() -> u32 { 42 }\npub fn use_answer() -> u32 { answer() }\n";
    std::fs::write(root.join("src/lib.rs"), source).unwrap();

    let backend = ManifestWorkspaceBackend::new(&root);
    let manifest = backend.manifest();
    let mut snapshots = manifest.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        while manifest.snapshot().version == 0 {
            snapshots.recv().await.unwrap();
        }
    })
    .await
    .expect("tiny workspace manifest must finish its initial scan");
    let file_system: Arc<dyn WorkspaceFileSystem> = backend.clone();
    let provider = LocalCodeIntelligence::start("native-lsp-smoke", manifest, file_system)
        .await
        .unwrap();
    let semantic: Arc<dyn WorkspaceCodeIntelligence> = provider.clone();
    let services =
        WorkspaceServices::local_with_manifest_backend(backend).with_code_intelligence(semantic);
    assert!(services.workspace_retrieval().is_none());
    let registry = ToolRegistry::with_artifact_limits_and_workspace_services(
        root.clone(),
        ArtifactStoreLimits::default(),
        Arc::clone(&services),
    );
    register_builtins(&registry, &services);
    let mut status = provider.subscribe_status();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let dormant = status.borrow().languages.iter().any(|language| {
                language.language.as_str() == "rust"
                    && language.state == CodeIntelligenceState::Starting
                    && !language.capabilities.document_symbols
            });
            if dormant {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .expect("local provider must prepare a dormant Rust language slot");
    assert!(owned_rust_analyzer_pids().is_empty());

    let outline = registry
        .execute(
            "code_symbols",
            &json!({"operation": "outline", "path": "src/lib.rs"}),
        )
        .await
        .unwrap();
    let running = owned_rust_analyzer_pids();
    let navigation = registry
        .execute(
            "code_navigation",
            &json!({"operation": "definition", "path": "src/lib.rs", "line": 1, "character": 31}),
        )
        .await
        .unwrap();
    provider.shutdown().await;
    provider.shutdown().await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while running.iter().any(|pid| process_still_exists(*pid)) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("provider shutdown must reap its rust-analyzer child");
    write_native_diagnostics(&trace, &root);

    assert_eq!(
        running.len(),
        1,
        "first semantic query must start one native server"
    );
    assert_eq!(outline.exit_code, 0, "native outline must succeed");
    let outline: Value = serde_json::from_str(&outline.output).unwrap();
    assert!(outline["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|symbol| symbol["name"] == "answer"));
    assert_eq!(
        navigation.exit_code, 0,
        "native definition navigation must succeed"
    );
    let navigation: Value = serde_json::from_str(&navigation.output).unwrap();
    assert!(navigation["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|location| {
            location["path"] == "src/lib.rs" && location["range"]["start"]["line"] == 0
        }));
    assert_eq!(
        provider.subscribe_status().borrow().state,
        CodeIntelligenceState::Unavailable
    );
    assert!(owned_rust_analyzer_pids().is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        source
    );
}
