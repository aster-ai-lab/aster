//! Real stdio ACP process and persistent SDK store; no provider or user state.

#[path = "support/binaries.rs"]
mod binaries;

use a3s_code_core::durable_memory_vec::{MemoryRecord, VecMemoryStore};
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn real_acp_inspects_and_conditionally_forgets_local_memory_without_model_calls() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let workspace = workspace.canonicalize().unwrap();
    let directory = root.path().canonicalize().unwrap().join("tenant-memory");
    let config = root.path().join("config.acl");
    std::fs::write(&config, format!("providers \"unused\" {{ base_url = \"http://127.0.0.1:9/v1\" api_key = \"fixture\" models \"model\" {{}} }}\ndefault_model = \"unused/model\"\nmemory_dir = {}\n", serde_json::to_string(&directory).unwrap())).unwrap();
    let store = VecMemoryStore::open(&directory).unwrap();
    store
        .put(&MemoryRecord {
            id: "synthetic-memory".into(),
            content: "preserve the pinned Rust toolchain".into(),
            tags: vec!["fixture".into()],
            importance: 0.8,
            memory_type: "\"procedural\"".into(),
            timestamp: "2026-10-24T00:00:00Z".into(),
            meta: "{}".into(),
        })
        .unwrap();
    drop(store);

    let mut child = tokio::process::Command::new(binaries::acp_binary())
        .args([
            "--cwd",
            workspace.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
        ])
        .env("A3S_CODE_LOG_FILE", "0")
        .env_remove("A3S_CONFIG")
        .env_remove("A3S_DEFAULT_MODEL")
        .current_dir(&workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    async fn request(
        stdin: &mut tokio::process::ChildStdin,
        lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
        id: u64,
        method: &str,
        params: Value,
    ) -> Value {
        let wire = format!(
            "{}\n",
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
        );
        stdin.write_all(wire.as_bytes()).await.unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            let line = tokio::time::timeout_at(deadline, lines.next_line())
                .await
                .expect("bounded ACP reply")
                .unwrap()
                .expect("ACP stdout");
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["id"] == id {
                assert!(value.get("error").is_none(), "{value}");
                return value["result"].clone();
            }
        }
    }
    request(
        &mut stdin,
        &mut lines,
        1,
        "initialize",
        json!({ "protocolVersion": 1, "clientCapabilities": {} }),
    )
    .await;
    let params = |request: Value| json!({ "workspace_root": workspace, "request": request });
    let page = request(&mut stdin, &mut lines, 2, "_a3s/memory/manage", params(json!({ "operation": "list", "query": "Rust", "memory_type": "procedural", "cursor": null }))).await;
    assert_eq!(page["memory_directory"], json!(directory));
    assert_eq!(page["outcome"]["Ok"]["total"], 1);
    let item = &page["outcome"]["Ok"]["items"][0];
    assert_eq!(item["id"], "synthetic-memory");
    let stale = request(
        &mut stdin,
        &mut lines,
        3,
        "_a3s/memory/manage",
        params(json!({ "operation": "delete", "id": item["id"], "revision": "0".repeat(64) })),
    )
    .await;
    assert_eq!(stale["outcome"]["Err"]["code"], "conflict");
    let deleted = request(
        &mut stdin,
        &mut lines,
        4,
        "_a3s/memory/manage",
        params(json!({ "operation": "delete", "id": item["id"], "revision": item["revision"] })),
    )
    .await;
    assert_eq!(deleted["outcome"]["Ok"]["id"], "synthetic-memory");
    let page = request(
        &mut stdin,
        &mut lines,
        5,
        "_a3s/memory/manage",
        params(json!({ "operation": "list", "query": "", "memory_type": null, "cursor": null })),
    )
    .await;
    assert_eq!(page["outcome"]["Ok"]["total"], 0);
    for (id, layer) in [(6, "working"), (7, "session")] {
        let snapshot = request(
            &mut stdin,
            &mut lines,
            id,
            "_a3s/memory/snapshot",
            json!({"workspace_root": workspace, "layer": layer}),
        )
        .await;
        assert_eq!(snapshot["workspace_root"], json!(workspace));
        assert_eq!(snapshot["memory_directory"], json!(directory));
        assert_eq!(snapshot["layer"], layer);
        assert!(snapshot["items"].as_array().unwrap().is_empty());
    }
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    let reopened = VecMemoryStore::open(&directory).unwrap();
    assert!(reopened.recall("Rust", 10).unwrap().is_empty());
    eprintln!("ORACLE local_memory_real_acp=pass provider_calls=0 stale_delete=refused persisted_recall=empty");
}
