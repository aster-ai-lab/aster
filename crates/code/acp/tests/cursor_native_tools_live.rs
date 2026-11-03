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
