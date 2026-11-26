//! A shell that never starts is a known refusal. It must not book an
//! unresolved external operation, and it must not freeze a later tool.
//! A git status list past the stamp cap is still a complete read.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::*;

struct SpawnRefusingSandbox;

#[async_trait]
impl crate::sandbox::BashSandbox for SpawnRefusingSandbox {
    async fn exec_command(
        &self,
        _command: &str,
        _guest_workspace: &str,
    ) -> anyhow::Result<crate::sandbox::SandboxOutput> {
        Err(crate::sandbox::process_host::process_host_spawn_refused(
            std::io::Error::new(std::io::ErrorKind::NotFound, "bash"),
        ))
    }

    async fn shutdown(&self) {}
}

struct RunningSandbox;

#[async_trait]
impl crate::sandbox::BashSandbox for RunningSandbox {
    async fn exec_command(
        &self,
        _command: &str,
        _guest_workspace: &str,
    ) -> anyhow::Result<crate::sandbox::SandboxOutput> {
        Err(anyhow::anyhow!(
            "capture process-host bash output: pipe closed after the child started"
        ))
    }

    async fn shutdown(&self) {}
}

#[tokio::test]
async fn a_shell_that_never_starts_does_not_block_a_later_tool() {
    let root = tempfile::tempdir().unwrap();
    let (tools, _) = executor_tools_with(root.path(), Some(Arc::new(SpawnRefusingSandbox)));
    let mut refused = request("bash-missing", json!({"command": "echo hi"}));
    refused.call.name = "bash".into();
    let prepared = prepare_tools(
        Arc::clone(&tools),
        refused,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let refused_result = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_ne!(
        refused_result.outcome,
        ToolExecutionOutcome::Unknown,
        "a shell that was never spawned has no external effect to reconcile"
    );
    assert!(refused_result
        .effects
        .unresolved_external_operations
        .is_empty());
    assert!(!tools
        .ledger
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .has_uncertain_tool_invocations());

    let later = prepare_tools(
        Arc::clone(&tools),
        request("later-write", json!({"write": true})),
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let later_result = execute_prepared(Arc::clone(&tools), later).await;
    assert_eq!(later_result.outcome, ToolExecutionOutcome::Succeeded);
    assert_eq!(
        std::fs::read(root.path().join("changed.txt")).unwrap(),
        b"exact bytes"
    );
}

#[tokio::test]
async fn a_capture_failure_after_the_child_starts_stays_unknown() {
    let root = tempfile::tempdir().unwrap();
    let (tools, _) = executor_tools_with(root.path(), Some(Arc::new(RunningSandbox)));
    let mut started = request("bash-capture", json!({"command": "echo hi"}));
    started.call.name = "bash".into();
    let prepared = prepare_tools(
        Arc::clone(&tools),
        started,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let result = execute_prepared(tools, prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Unknown);
    assert!(!result.effects.unresolved_external_operations.is_empty());
}

fn git_init(root: &std::path::Path) {
    let output = std::process::Command::new("git")
        .args(["init"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn a_write_past_the_stamp_cap_does_not_freeze_the_ledger() {
    let root = tempfile::tempdir().unwrap();
    git_init(root.path());
    for index in 0..=crate::porcelain::FILE_WALK_CAP {
        std::fs::write(root.path().join(format!("f{index:04}.txt")), "x").unwrap();
    }
    let (tools, _) = executor_tools(root.path());
    let prepared = prepare_tools(
        Arc::clone(&tools),
        request("dirty-write", json!({"write": true})),
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let baseline = prepared
        .prepared
        .baseline
        .clone()
        .expect("mutation baseline");
    assert!(
        serde_json::to_vec(&baseline).unwrap().len() < 4 * 1024,
        "prepared baseline inlined the dirty status list"
    );
    assert_eq!(
        baseline["git"]["status_count"],
        crate::porcelain::FILE_WALK_CAP + 1
    );
    assert_eq!(baseline["git"]["status_complete"], true);
    assert_eq!(baseline["git"]["stamp_read_failed"], false);
    assert!(baseline["git"].get("porcelain").is_none());
    let result = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Succeeded);
    assert_eq!(
        result.effects.completeness,
        aster_effect::ToolObservationCompleteness::Incomplete
    );
    assert!(result.effects.budget_exceeded);
    assert!(result
        .effects
        .changes
        .iter()
        .any(|change| change.path == "changed.txt"));
    assert!(!tools
        .ledger
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .has_uncertain_tool_invocations());
}

#[tokio::test]
async fn a_shell_that_never_starts_on_a_missed_status_does_not_block_a_later_tool() {
    let root = tempfile::tempdir().unwrap();
    git_init(root.path());
    std::fs::write(root.path().join(".git/index"), b"not-an-index").unwrap();
    let (tools, _) = executor_tools_with(root.path(), Some(Arc::new(SpawnRefusingSandbox)));
    let mut refused = request("bash-unreadable", json!({"command": "echo hi"}));
    refused.call.name = "bash".into();
    let prepared = prepare_tools(
        Arc::clone(&tools),
        refused,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let refused_result = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(refused_result.outcome, ToolExecutionOutcome::Failed);
    assert_eq!(
        refused_result.effects.completeness,
        aster_effect::ToolObservationCompleteness::Complete
    );
    assert!(refused_result.effects.changes.is_empty());
    assert!(!tools
        .ledger
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .has_uncertain_tool_invocations());

    let later = prepare_tools(
        Arc::clone(&tools),
        request("later-write", json!({"write": true})),
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let later_result = execute_prepared(Arc::clone(&tools), later).await;
    assert_eq!(later_result.outcome, ToolExecutionOutcome::Succeeded);
    assert_eq!(
        std::fs::read(root.path().join("changed.txt")).unwrap(),
        b"exact bytes"
    );
    assert!(
        !tools
            .ledger
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .has_uncertain_tool_invocations(),
        "a captured write stays settled when only the status read was missed"
    );
}

#[tokio::test]
async fn missing_execution_evidence_overrides_a_path_budget() {
    async fn spill(incomplete_evidence: bool) -> (ToolExecutionResultV1, bool) {
        let root = tempfile::tempdir().unwrap();
        let (tools, _) = executor_tools(root.path());
        let mut args = json!({"spill": true});
        if incomplete_evidence {
            args["incomplete_evidence"] = json!(true);
        }
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request("spill", args),
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
        let result = execute_prepared(Arc::clone(&tools), prepared).await;
        let uncertain = tools
            .ledger
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .has_uncertain_tool_invocations();
        (result, uncertain)
    }

    let (budget, uncertain) = spill(false).await;
    assert!(budget.validate().is_ok());
    assert!(budget.effects.budget_exceeded);
    assert!(budget.effects.changes.is_empty());
    assert!(!uncertain);
    let (missed, uncertain) = spill(true).await;
    assert!(missed.validate().is_ok());
    assert!(!missed.effects.budget_exceeded);
    assert_eq!(
        missed.effects.completeness,
        aster_effect::ToolObservationCompleteness::Incomplete
    );
    assert!(missed.effects.changes.is_empty());
    assert!(uncertain);
}

struct SpawnRefusingRunner;

#[async_trait]
impl crate::workspace::WorkspaceCommandRunner for SpawnRefusingRunner {
    async fn exec(
        &self,
        _request: crate::workspace::CommandRequest,
    ) -> anyhow::Result<crate::workspace::CommandOutput> {
        Err(crate::tools::builtin::bash::host_shell_spawn_refused(
            std::io::Error::new(std::io::ErrorKind::NotFound, "bash"),
        ))
    }
}

struct PostSpawnFailingRunner;

#[async_trait]
impl crate::workspace::WorkspaceCommandRunner for PostSpawnFailingRunner {
    async fn exec(
        &self,
        _request: crate::workspace::CommandRequest,
    ) -> anyhow::Result<crate::workspace::CommandOutput> {
        anyhow::bail!("Failed to capture shell output: pipe closed after child started")
    }
}

#[tokio::test]
async fn host_escalated_spawn_refusal_books_failed_with_null_exit_code_and_allows_writes() {
    let root = tempfile::tempdir().unwrap();
    let base = crate::workspace::WorkspaceServices::local(root.path());
    let services =
        crate::workspace::WorkspaceServicesBuilder::new(base.workspace_ref().clone(), base.fs())
            .command_runner(Arc::new(SpawnRefusingRunner))
            .build();
    let (base_tools, _) = executor_tools(root.path());
    let mut tools = base_tools.as_ref().clone();
    tools.context = tools.context.with_workspace_services(services);
    let tools = Arc::new(tools);
    let mut refused = request(
        "host-bash-missing",
        json!({
            "command": "echo hi",
            "sandbox_permissions": "require_escalated",
            "justification": "Exercise explicit OS shell spawn refusal on host runner",
        }),
    );
    refused.call.name = "bash".into();
    refused.call.needs_confirmation = true;
    let prepared = prepare_tools(
        Arc::clone(&tools),
        refused,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let refused_result = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(refused_result.outcome, ToolExecutionOutcome::Failed);
    assert_eq!(refused_result.exit_code, None);
    assert_eq!(
        refused_result.effects,
        ToolEffectObservationV1::complete(Vec::new())
    );
    assert!(!tools
        .ledger
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .has_uncertain_tool_invocations());

    let later = prepare_tools(
        Arc::clone(&tools),
        request("later-write", json!({"write": true})),
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let later_result = execute_prepared(Arc::clone(&tools), later).await;
    assert_eq!(later_result.outcome, ToolExecutionOutcome::Succeeded);
    assert_eq!(
        std::fs::read(root.path().join("changed.txt")).unwrap(),
        b"exact bytes"
    );
}

#[tokio::test]
async fn host_escalated_post_spawn_failure_books_unknown_and_blocks_later_writes() {
    let root = tempfile::tempdir().unwrap();
    let base = crate::workspace::WorkspaceServices::local(root.path());
    let services =
        crate::workspace::WorkspaceServicesBuilder::new(base.workspace_ref().clone(), base.fs())
            .command_runner(Arc::new(PostSpawnFailingRunner))
            .build();
    let (base_tools, _) = executor_tools(root.path());
    let mut tools = base_tools.as_ref().clone();
    tools.context = tools.context.with_workspace_services(services);
    let tools = Arc::new(tools);
    let mut uncertain = request(
        "host-bash-capture-fail",
        json!({
            "command": "echo hi",
            "sandbox_permissions": "require_escalated",
            "justification": "Exercise post-spawn failure on host runner",
        }),
    );
    uncertain.call.name = "bash".into();
    uncertain.call.needs_confirmation = true;
    let prepared = prepare_tools(
        Arc::clone(&tools),
        uncertain,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let uncertain_result = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(uncertain_result.outcome, ToolExecutionOutcome::Unknown);
    assert!(!uncertain_result
        .effects
        .unresolved_external_operations
        .is_empty());
    assert!(tools
        .ledger
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .has_uncertain_tool_invocations());

    let blocked = prepare_tools(
        Arc::clone(&tools),
        request("blocked-write", json!({"write": true})),
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    .unwrap();
    let blocked_result = execute_prepared(Arc::clone(&tools), blocked).await;
    assert_eq!(blocked_result.outcome, ToolExecutionOutcome::Denied);
    assert!(!root.path().join("changed.txt").exists());
}
