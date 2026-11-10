//! Opt-in account tests: native Cursor MCP requests execute through Aster tools.

#![cfg(test)]

use std::{sync::Arc, time::Duration};

use aster_core::config::AccountImport;
use aster_core::permissions::{PermissionDecision, PermissionPolicy};
use aster_core::{Agent, AgentEvent, PlanningMode, SessionOptions};

async fn copy_and_verify(model: &str) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .try_init();
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace 空格");
    std::fs::create_dir(&workspace).unwrap();
    let marker = format!("cursor-native-{}\n", uuid::Uuid::new_v4());
    std::fs::write(workspace.join("input.txt"), &marker).unwrap();
    let config = root.path().join("cursor.acl");
    std::fs::write(&config, "import_accounts = [\"cursor\"]\n").unwrap();
    let (launch, _) = aster_acp::load_launch_config_with_imports(
        workspace.clone(),
        Some(config),
        &[AccountImport::Cursor],
    )
    .expect("local Cursor login and catalog");
    let agent = Agent::from_config(launch.config).await.unwrap();
    let mut policy = PermissionPolicy::new().allow_all(&["read", "write"]);
    policy.default_decision = PermissionDecision::Deny;
    let options = SessionOptions::new()
        .with_model(model)
        .with_assistant_final_response(true)
        .with_permission_policy(policy)
        .with_planning_mode(PlanningMode::Disabled)
        .with_auto_delegation_enabled(false)
        .with_manual_delegation_enabled(false)
        .with_max_tool_rounds(8)
        .with_completion_attestor(Arc::new(
            aster_core::completion_attestor::DiskContentAttestor::new(&workspace),
        ));
    let session = agent
        .session_async(workspace.display().to_string(), Some(options))
        .await
        .unwrap();
    let (mut events, handle) = session.stream(
        "Use only read and write. Read input.txt, copy its exact bytes to output.txt using write, then read output.txt to verify the copy and finish. Do not run commands or delegate.",
        None,
    ).await.unwrap();
    let mut calls = Vec::new();
    let result = tokio::time::timeout(Duration::from_secs(240), async {
        while let Some(event) = events.recv().await {
            match event {
                AgentEvent::TurnStart { turn } => println!("{model}: turn {turn}"),
                AgentEvent::ToolStart { name, .. } => println!("{model}: tool {name} started"),
                AgentEvent::ToolEnd {
                    name, exit_code, ..
                } => {
                    assert_eq!(exit_code, 0, "{model}: {name} failed");
                    println!("{model}: tool {name} completed");
                    calls.push(name);
                }
                AgentEvent::End { .. } => return,
                AgentEvent::Error { message } => panic!("{model}: {message}"),
                _ => {}
            }
        }
        panic!("{model}: stream closed without completion");
    })
    .await;
    if result.is_err() {
        handle.abort();
    }
    result.expect("bounded Cursor read/write/verify task");
    tokio::time::timeout(Duration::from_secs(10), handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(workspace.join("output.txt")).unwrap(),
        marker
    );
    assert_eq!(calls, ["read", "write", "read"]);
    println!("{model}: verified exact bytes and read/write/read tool trace");
}

#[tokio::test]
#[ignore = "real Cursor Auto account task; spends account quota"]
async fn auto_copies_and_verifies_actual_workspace_bytes() {
    copy_and_verify("cursor/auto").await;
}

#[tokio::test]
#[ignore = "real Cursor Composer account task; spends account quota"]
async fn composer_copies_and_verifies_actual_workspace_bytes() {
    copy_and_verify("cursor/composer-2.5").await;
}

#[tokio::test]
#[ignore = "real Cursor Auto parent and verifier; spends account quota"]
async fn auto_delegates_verification_of_pending_changes() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .try_init();
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace 空格");
    std::fs::create_dir(&workspace).unwrap();
    let marker = format!("cursor-verifier-{}\n", uuid::Uuid::new_v4());
    std::fs::write(workspace.join("input.txt"), &marker).unwrap();
    let check = "from pathlib import Path\nassert Path('output.txt').read_bytes() == Path('input.txt').read_bytes()\nprint('COPY_VERIFIED')\n";
    std::fs::write(workspace.join("check.py"), check).unwrap();
    let config = root.path().join("cursor.acl");
    std::fs::write(&config, "import_accounts = [\"cursor\"]\n").unwrap();
    let (launch, _) = aster_acp::load_launch_config_with_imports(
        workspace.clone(),
        Some(config),
        &[AccountImport::Cursor],
    )
    .expect("local Cursor login and catalog");
    let agent = Agent::from_config(launch.config).await.unwrap();
    let mut policy = PermissionPolicy::new().allow_all(&["read", "write", "task", "bash"]);
    policy.default_decision = PermissionDecision::Deny;
    let options = SessionOptions::new()
        .with_model("cursor/auto")
        .with_assistant_final_response(true)
        .with_permission_policy(policy)
        .with_planning_mode(PlanningMode::Disabled)
        .with_auto_delegation_enabled(false)
        .with_manual_delegation_enabled(true)
        .with_max_tool_rounds(12)
        .with_completion_attestor(Arc::new(
            aster_core::completion_attestor::DiskContentAttestor::new(&workspace),
        ));
    let session = agent
        .session_async(workspace.display().to_string(), Some(options))
        .await
        .unwrap();
    let (mut events, handle) = session.stream(
        "Read input.txt and write its exact bytes to output.txt. Then use task to run the synchronous verification agent: ask it to execute python3 check.py and report the result. The verifier must run the command; do not run it in the parent. Preserve input.txt and check.py. After the verifier returns, read output.txt and finish. Do not edit other files.",
        None,
    ).await.unwrap();
    let mut delegated = false;
    let mut checked = false;
    let mut verified = false;
    let result = tokio::time::timeout(Duration::from_secs(360), async {
        while let Some(event) = events.recv().await {
            match event {
                AgentEvent::SubagentStart {
                    session_id,
                    parent_session_id,
                    agent,
                    ..
                } => {
                    assert_eq!(agent, "verification");
                    assert_ne!(session_id, parent_session_id);
                    assert!(aster_core::external_observation::session_owns_write(
                        &session_id,
                        &workspace,
                        "output.txt"
                    ));
                    delegated = true;
                    println!(
                        "cursor/auto: verifier owns pending output with an independent session"
                    );
                }
                AgentEvent::SubagentProgress { metadata, .. }
                    if metadata["tool"] == "bash" && metadata["exit_code"] == 0 =>
                {
                    checked = true;
                    println!("cursor/auto: verifier command succeeded");
                }
                AgentEvent::SubagentEnd { success, .. } => {
                    assert!(success, "verification task failed");
                    verified = true;
                }
                AgentEvent::ToolEnd {
                    name, exit_code, ..
                } => {
                    assert_eq!(exit_code, 0, "parent tool {name} failed");
                }
                AgentEvent::Error { message } => panic!("cursor/auto: {message}"),
                AgentEvent::End { .. } => return,
                _ => {}
            }
        }
        panic!("stream closed without completion");
    })
    .await;
    if result.is_err() {
        handle.abort();
    }
    result.expect("bounded Cursor parent and verifier task");
    tokio::time::timeout(Duration::from_secs(10), handle)
        .await
        .unwrap()
        .unwrap();
    assert!(delegated && checked && verified);
    assert_eq!(
        std::fs::read_to_string(workspace.join("output.txt")).unwrap(),
        marker
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("input.txt")).unwrap(),
        marker
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("check.py")).unwrap(),
        check
    );
}
