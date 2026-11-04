//! Canonical legacy observations through the actual ACP memory owner.

#[path = "support/binaries.rs"]
mod binaries;

use aster_durable_memory::MemoryItem;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

async fn request(
    input: &mut tokio::process::ChildStdin,
    output: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: u64,
    method: &str,
    params: Value,
) -> Value {
    let wire = format!(
        "{}\n",
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
    );
    input.write_all(wire.as_bytes()).await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let line = tokio::time::timeout_at(deadline, output.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        if response["id"] == id {
            assert!(response.get("error").is_none(), "{response}");
            return response["result"].clone();
        }
    }
}

#[tokio::test]
async fn real_acp_imports_duplicate_observations_as_one_canonical_memory() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let workspace = root.join("workspace 空间");
    std::fs::create_dir(&workspace).unwrap();
    let directory = root.join("memory");
    let legacy = directory.join("items");
    std::fs::create_dir_all(&legacy).unwrap();
    for (id, content) in [
        ("one", "durable preference for explicit configuration"),
        ("two", " DURABLE   preference for explicit\nconfiguration "),
    ] {
        let mut memory = MemoryItem::new(content).with_tag(id);
        memory.id = id.into();
        std::fs::write(
            legacy.join(format!("{id}.json")),
            serde_json::to_vec(&memory).unwrap(),
        )
        .unwrap();
    }
    let config = root.join("config.acl");
    std::fs::write(&config, format!("providers \"unused\" {{ base_url = \"http://127.0.0.1:9/v1\" api_key = \"fixture\" models \"model\" {{}} }}\ndefault_model = \"unused/model\"\nmemory_dir = {}\n", serde_json::to_string(&directory).unwrap())).unwrap();
    let mut child = tokio::process::Command::new(binaries::acp_binary())
        .arg("--cwd")
        .arg(&workspace)
        .arg("--config")
        .arg(&config)
        .env("ASTER_LOG_FILE", "0")
        .env_remove("ASTER_CONFIG")
        .env_remove("ASTER_DEFAULT_MODEL")
        .current_dir(&workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    request(
        &mut input,
        &mut output,
        1,
        "initialize",
        json!({"protocolVersion":1,"clientCapabilities":{}}),
    )
    .await;
    let page = request(&mut input, &mut output, 2, "_aster/memory/manage", json!({"workspace_root":workspace,"request":{"operation":"list","query":"","memory_type":null,"cursor":null}})).await;
    assert_eq!(page["outcome"]["Ok"]["total"], 1);
    let canonical = &page["outcome"]["Ok"]["items"][0];
    assert!(matches!(canonical["id"].as_str(), Some("one" | "two")));
    let tags = canonical["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&json!("one")) && tags.contains(&json!("two")));
    assert_eq!(canonical["metadata"]["duplicate_count"], "1");
    assert!(directory.join("items.imported").is_dir());
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    let reopened = aster_core::durable_memory_vec::VecMemoryStore::open(&directory).unwrap();
    assert_eq!(reopened.record_count().unwrap(), 1);
}
