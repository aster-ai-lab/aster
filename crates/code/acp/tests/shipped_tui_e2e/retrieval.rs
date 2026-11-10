#![cfg(test)]

use super::*;
use serde_json::{Value, json};

const PROMPT: &str = "RETRIEVAL-ORACLE: find the workspace retrieval probe using hybrid search";
const SEMANTIC_PROMPT: &str = "RETRIEVAL-ORACLE: find configuration loading by meaning";
const EVIDENCE: &str = "VERIFIED_RETRIEVAL_011";
const COMPLETE: &str = "RETRIEVAL-EVIDENCE-VERIFIED";

pub(super) fn response(body: &Value, state: &mut MockState, streaming: bool) -> Option<String> {
    let messages = body["messages"].as_array()?;
    if !messages.iter().any(|message| {
        message["role"] == "user" && message["content"].to_string().contains("RETRIEVAL-ORACLE")
    }) {
        return None;
    }
    if let Some(tool) = messages
        .iter()
        .rev()
        .find(|message| message["role"] == "tool")
    {
        let text = tool["content"].to_string();
        return Some(reply_text(
            if text.contains(EVIDENCE) && text.contains("64 |") {
                COMPLETE
            } else {
                "RETRIEVAL-EVIDENCE-MISSING"
            },
            streaming,
        ));
    }
    let search = body["tools"]
        .as_array()?
        .iter()
        .find(|tool| tool["function"]["name"] == "search");
    let semantic = messages.iter().any(|message| {
        message["role"] == "user" && message["content"].to_string().contains(SEMANTIC_PROMPT)
    });
    let args = if semantic {
        json!({"mode": "semantic", "query": "parse application configuration", "limit": 1})
    } else {
        json!({"mode": "hybrid", "query": "retrieval_probe", "path": "source.rs", "limit": 1})
    };
    Some(match search {
        Some(_) => reply_tool("search", &args.to_string(), state, streaming),
        None => reply_text("RETRIEVAL-TOOL-MISSING", streaming),
    })
}

fn fixture(root: &Path, local_opt_in: bool) -> (PathBuf, PathBuf, PathBuf, Arc<Mutex<MockState>>) {
    let workspace = root.join("workspace");
    let home = root.join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let mut lines = vec!["// unrelated prefix".to_owned(); 79];
    lines[63] = format!("pub fn retrieval_probe() -> &'static str {{ \"{EVIDENCE}\" }}");
    std::fs::write(workspace.join("source.rs"), lines.join("\n")).unwrap();
    let (addr, state) = spawn_mock(&workspace.join("unused"), &workspace.join("unused-late"));
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str("\ntool_presentation { mode = \"direct\" }\n");
    if local_opt_in {
        text.push_str("workspace_retrieval { enabled = true\n local_cpu {}\n}\n");
    }
    std::fs::write(&config, text).unwrap();
    (workspace, home, config, state)
}

async fn stdio_case(local_opt_in: bool, prompt: &str) {
    let root = tempfile::tempdir().unwrap();
    let (workspace, home, config, state) = fixture(root.path(), local_opt_in);
    if prompt == SEMANTIC_PROMPT {
        let mut lines = vec!["// unrelated prefix".to_owned(); 79];
        lines[63] = format!(
            "pub fn load_settings() {{ /* Read application configuration settings from disk. {EVIDENCE} */ }}"
        );
        std::fs::write(workspace.join("source.rs"), lines.join("\n")).unwrap();
        std::fs::write(
            workspace.join("weather.md"),
            "The weather is sunny today.\n",
        )
        .unwrap();
    }
    let missing_model = root.path().join("absent-model");
    let product_home = home.join(".aster");
    let started = Instant::now();
    let agent = spawn_agent_with_env(
        &workspace,
        &config,
        &home,
        &[
            ("ASTER_HOME", product_home.as_path()),
            ("ASTER_BGE_M3_MODEL_DIR", Path::new("")),
            ("ASTER_USE_BGE_HOME", missing_model.as_path()),
        ],
    )
    .await;
    acp::Agent::initialize(
        agent.conn.as_ref(),
        acp::InitializeRequest::new(acp::ProtocolVersion::V1),
    )
    .await
    .unwrap();
    let session =
        acp::Agent::new_session(agent.conn.as_ref(), acp::NewSessionRequest::new(workspace))
            .await
            .unwrap();
    let session_opened = started.elapsed();
    let query_started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(45),
        acp::Agent::prompt(
            agent.conn.as_ref(),
            user_prompt(&session.session_id, prompt),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let query_elapsed = query_started.elapsed();
    assert_eq!(result.stop_reason, acp::StopReason::EndTurn);
    assert!(agent_text(&agent, &session.session_id, 0).contains(COMPLETE));
    let bodies = state.lock().unwrap().bodies.clone();
    let first: Value = serde_json::from_str(first_tool_request(&bodies)).unwrap();
    let search = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["name"] == "search")
        .unwrap();
    let modes = search["function"]["parameters"]["properties"]["mode"]["enum"]
        .as_array()
        .unwrap();
    assert!(modes.contains(&json!("hybrid")));
    // The in-tree light tier (specs/013 P2 wiring) serves the local_cpu
    // opt-in from compile-time bytes: no "missing assets" refusal exists,
    // and semantic mode is offered when the opt-in is present.
    if local_opt_in {
        assert!(modes.contains(&json!("semantic")));
        assert!(
            !missing_model.exists(),
            "session startup installed assets implicitly"
        );
    } else {
        assert!(!modes.contains(&json!("semantic")));
    }
    oracle(
        "shipped_retrieval_stdio",
        if prompt == SEMANTIC_PROMPT {
            "embedded-local-semantic"
        } else if local_opt_in {
            "embedded-local-hybrid"
        } else {
            "model-free-hybrid"
        },
        &format!(
            "actual tool result includes verified line 64; session-open-ms={}; prompt-ms={}",
            session_opened.as_millis(),
            query_elapsed.as_millis()
        ),
    );
    stop_agent(agent).await;
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_retrieval_stdio_without_semantic_model() {
    tokio::task::LocalSet::new()
        .run_until(stdio_case(false, PROMPT))
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_retrieval_stdio_missing_local_assets_keeps_hybrid() {
    tokio::task::LocalSet::new()
        .run_until(stdio_case(true, PROMPT))
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_retrieval_stdio_executes_embedded_semantic_search() {
    tokio::task::LocalSet::new()
        .run_until(stdio_case(true, SEMANTIC_PROMPT))
        .await;
}
