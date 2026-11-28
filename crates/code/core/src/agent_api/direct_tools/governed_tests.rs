use super::*;
use crate::agent::AgentConfig;
use crate::hitl::{ConfirmationManager, ConfirmationPolicy, ConfirmationProvider};
use crate::llm::{LlmClient, LlmResponse, Message, StreamEvent, ToolDefinition};
use crate::permissions::{PermissionChecker, PermissionDecision};
use crate::tools::{Tool, ToolInvocation, ToolOutput};
use crate::{ToolRequestOriginV1, ToolRequestSnapshotV1};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

struct UnusedLlm;

#[async_trait]
impl LlmClient for UnusedLlm {
    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        anyhow::bail!("governed direct-tool tests must not invoke the model")
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
        _cancel_token: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("governed direct-tool tests must not invoke the model")
    }
}

struct StaticPermission(PermissionDecision);

#[test]
fn runtime_posture_and_host_deny_both_govern_direct_calls() {
    use crate::agent_api::agent_loop_runtime::RuntimePermissionChecker;
    use crate::permissions::{policy_for_posture, PermissionPolicy};
    let args = serde_json::json!({"file_path":"fixture.txt", "content":"fixture"});
    for (policy, ceiling, expected) in [
        (
            policy_for_posture("plan"),
            PermissionDecision::Allow,
            PermissionDecision::Deny,
        ),
        (
            policy_for_posture("yolo"),
            PermissionDecision::Deny,
            PermissionDecision::Deny,
        ),
        (
            policy_for_posture("yolo").deny("write"),
            PermissionDecision::Ask,
            PermissionDecision::Deny,
        ),
        (
            PermissionPolicy::new().allow("write"),
            PermissionDecision::Ask,
            PermissionDecision::Allow,
        ),
        (
            PermissionPolicy::new(),
            PermissionDecision::Allow,
            PermissionDecision::Ask,
        ),
    ] {
        let checker =
            RuntimePermissionChecker::for_test(policy, Some(Arc::new(StaticPermission(ceiling))));
        assert_eq!(checker.check("write", &args), expected);
    }
}

#[test]
fn runtime_policy_retains_visibility_and_the_hosts_snapshot_boundary() {
    use crate::agent_api::agent_loop_runtime::RuntimePermissionChecker;
    struct Mutable(std::sync::atomic::AtomicBool);
    impl PermissionChecker for Mutable {
        fn check(&self, _: &str, _: &serde_json::Value) -> PermissionDecision {
            if self.0.load(Ordering::SeqCst) {
                PermissionDecision::Deny
            } else {
                PermissionDecision::Allow
            }
        }
        fn snapshot_for_run(&self) -> Option<Arc<dyn PermissionChecker>> {
            Some(Arc::new(StaticPermission(
                self.check("write", &serde_json::json!({})),
            )))
        }
    }
    let ceiling = Arc::new(Mutable(std::sync::atomic::AtomicBool::new(true)));
    let checker = RuntimePermissionChecker::for_test(
        crate::permissions::policy_for_posture("yolo").deny("write"),
        Some(ceiling.clone()),
    );
    assert!(!checker.expose_to_model("write"));
    let checker = RuntimePermissionChecker::for_test(
        crate::permissions::policy_for_posture("yolo"),
        Some(ceiling.clone()),
    );
    let frozen = checker.snapshot_for_run().unwrap();
    ceiling.0.store(false, Ordering::SeqCst);
    assert_eq!(
        checker.check("write", &serde_json::json!({})),
        PermissionDecision::Allow
    );
    assert_eq!(
        frozen.check("write", &serde_json::json!({})),
        PermissionDecision::Deny
    );
}

#[cfg(unix)]
#[test]
fn runtime_permission_preserves_the_winning_denial_reason() {
    use crate::agent_api::agent_loop_runtime::RuntimePermissionChecker;
    use crate::permissions::{InteractiveToolGuardrail, PermissionPolicy};

    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("tool"), "fixture").unwrap();
    std::fs::write(outside.path().join("tool"), "outside").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("tool"),
        workspace.path().join("tool-link"),
    )
    .unwrap();
    std::os::unix::fs::symlink("tool", workspace.path().join("in-workspace-link")).unwrap();
    let host: Arc<dyn PermissionChecker> =
        Arc::new(InteractiveToolGuardrail::default().with_workspace(workspace.path()));
    let args = serde_json::json!({"command": "test -f ./tool-link"});
    let checker = RuntimePermissionChecker::for_test(
        PermissionPolicy::new().allow("bash"),
        Some(Arc::clone(&host)),
    );
    let frozen = checker.snapshot_for_run().unwrap();
    assert_eq!(frozen.check("bash", &args), PermissionDecision::Deny);
    assert_eq!(
        frozen.denial_code("bash", &args),
        Some("symlink_boundary_escape")
    );

    let in_workspace_args = serde_json::json!({"command": "test -f ./in-workspace-link"});
    assert_ne!(
        frozen.check("bash", &in_workspace_args),
        PermissionDecision::Deny
    );
    assert_eq!(frozen.denial_code("bash", &in_workspace_args), None);

    let args = serde_json::json!({"command": "test -f ./tool"});
    assert_ne!(frozen.check("bash", &args), PermissionDecision::Deny);
    assert_eq!(frozen.denial_code("bash", &args), None);

    let checker =
        RuntimePermissionChecker::for_test(PermissionPolicy::new().deny("bash"), Some(host));
    let args = serde_json::json!({"command": "test -f ./tool-link"});
    assert_eq!(checker.check("bash", &args), PermissionDecision::Deny);
    assert_eq!(checker.denial_code("bash", &args), Some("policy_denied"));
}

impl PermissionChecker for StaticPermission {
    fn check(&self, _tool_name: &str, _args: &serde_json::Value) -> PermissionDecision {
        self.0
    }
}

struct CountingTool {
    calls: Arc<AtomicUsize>,
    confirmation_required: bool,
}

#[async_trait]
impl Tool for CountingTool {
    fn name(&self) -> &str {
        "governed_counting"
    }

    fn description(&self) -> &str {
        "records one governed side effect"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object" })
    }

    fn requires_confirmation(&self, _args: &serde_json::Value) -> bool {
        self.confirmation_required
    }

    async fn execute(&self, _args: &serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::success("counted"))
    }
}

fn runtime(
    decision: PermissionDecision,
    confirmation_required: bool,
    manager: Option<Arc<ConfirmationManager>>,
) -> (DirectToolRuntime, Arc<AtomicUsize>, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let executor = Arc::new(ToolExecutor::new(
        directory.path().to_string_lossy().to_string(),
    ));
    executor.register_dynamic_tool(Arc::new(CountingTool {
        calls: Arc::clone(&calls),
        confirmation_required,
    }));
    let confirmation_manager = manager.map(|manager| manager as Arc<dyn ConfirmationProvider>);
    let session_id = "governed-host-tool-test".to_string();
    let context = ToolContext::new(directory.path().to_path_buf()).with_session_id(&session_id);
    let agent_loop = AgentLoop::new(
        Arc::new(UnusedLlm),
        Arc::clone(&executor),
        context.clone(),
        AgentConfig {
            permission_checker: Some(Arc::new(StaticPermission(decision))),
            confirmation_manager,
            ..AgentConfig::default()
        },
    );
    (
        DirectToolRuntime {
            tool_executor: executor,
            tool_context: context,
            agent_loop,
            session_id,
            session_cancel: CancellationToken::new(),
            closed: Arc::new(AtomicBool::new(false)),
            task_scheduler: None,
            task_priority: crate::task_scheduler::TaskPriority::Interactive,
            provider_quota: None,
            security_provider: None,
        },
        calls,
        directory,
    )
}

async fn resolve_confirmation(manager: &ConfirmationManager, calls: &AtomicUsize, approved: bool) {
    let request = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if let Some(request) = manager
                .pending_confirmation_details()
                .await
                .first()
                .cloned()
            {
                return request;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("governed tool must request confirmation");
    assert_eq!(request.tool_name, "governed_counting");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(manager
        .confirm(
            &request.tool_id,
            approved,
            Some(if approved {
                "approved by test".to_string()
            } else {
                "rejected by test".to_string()
            }),
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn trusted_host_call_preserves_explicit_control_plane_authority() {
    let (runtime, calls, _directory) = runtime(PermissionDecision::Deny, true, None);

    let result = runtime
        .call("governed_counting", serde_json::json!({}))
        .await
        .unwrap();

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn governed_host_call_obeys_permission_before_side_effects() {
    let (runtime, calls, _directory) = runtime(PermissionDecision::Deny, false, None);

    let result = runtime
        .call_governed("governed_counting", serde_json::json!({}))
        .await
        .unwrap();

    assert_ne!(result.exit_code, 0);
    assert!(result.output.contains("Permission denied"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn governed_search_grep_works_without_embeddings_and_obeys_denial() {
    let (allowed, _, allowed_directory) = runtime(PermissionDecision::Allow, false, None);
    std::fs::write(
        allowed_directory.path().join("allowed.txt"),
        "matchword allowed content\n",
    )
    .unwrap();
    assert!(allowed
        .tool_context
        .workspace_services
        .workspace_retrieval()
        .is_none());

    let result = allowed
        .call_governed(
            "search",
            serde_json::json!({"mode": "grep", "query": "matchword", "path": "allowed.txt"}),
        )
        .await
        .unwrap();

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(result.output.contains("allowed.txt"));
    assert!(result.output.contains("matchword allowed content"));
    assert_eq!(result.metadata.as_ref().unwrap()["mode"], "grep");

    let (denied, _, denied_directory) = runtime(PermissionDecision::Deny, false, None);
    std::fs::write(
        denied_directory.path().join("private.txt"),
        "matchword this content must stay private\n",
    )
    .unwrap();
    let result = denied
        .call_governed(
            "search",
            serde_json::json!({"mode": "grep", "query": "matchword", "path": "private.txt"}),
        )
        .await
        .unwrap();

    assert_ne!(result.exit_code, 0);
    assert!(result.output.contains("Permission denied"));
    assert!(!result.output.contains("this content must stay private"));
}

#[tokio::test]
async fn governed_denial_binds_the_request_before_permission_evidence() {
    let (runtime, calls, _directory) = runtime(PermissionDecision::Deny, false, None);
    let arguments = serde_json::json!({"secret": "governed-private-value"});
    let tool_id = "governed-denied-1";
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let event_tx = Some(event_tx);
    let cancellation = CancellationToken::new();

    let result = runtime
        .agent_loop
        .invoke_host_tool(
            ToolInvocation::host_governed(tool_id, "governed_counting", arguments.clone()),
            &runtime.session_id,
            &event_tx,
            &cancellation,
            &runtime.tool_context,
        )
        .await;
    drop(event_tx);

    assert_ne!(result.exit_code, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut events = Vec::new();
    while let Some(event) = event_rx.recv().await {
        events.push(event);
    }
    let request_position = events
        .iter()
        .position(|event| matches!(event, AgentEvent::ToolRequestBound { .. }))
        .expect("denied requests must retain bounded request evidence");
    let denial_position = events
        .iter()
        .position(|event| matches!(event, AgentEvent::PermissionDenied { .. }))
        .expect("governed denial must retain permission evidence");
    assert!(request_position < denial_position);
    assert!(!events
        .iter()
        .any(|event| matches!(event, AgentEvent::ToolExecutionStart { .. })));

    let AgentEvent::ToolRequestBound {
        tool_id: observed_id,
        tool_name,
        snapshot,
    } = &events[request_position]
    else {
        unreachable!()
    };
    let snapshot: &ToolRequestSnapshotV1 = snapshot;
    snapshot
        .validate_against(
            tool_id,
            "governed_counting",
            &arguments,
            ToolRequestOriginV1::HostDirectGoverned,
        )
        .unwrap();
    assert_eq!(observed_id, tool_id);
    assert_eq!(tool_name, "governed_counting");
    assert!(!serde_json::to_string(snapshot)
        .unwrap()
        .contains("governed-private-value"));
}

#[tokio::test]
async fn governed_host_call_requires_approval_before_side_effects() {
    let (event_tx, _) = broadcast::channel(8);
    let manager = Arc::new(ConfirmationManager::new(
        ConfirmationPolicy::enabled(),
        event_tx,
    ));
    let (runtime, calls, _directory) =
        runtime(PermissionDecision::Allow, true, Some(Arc::clone(&manager)));

    let (rejected, ()) = tokio::join!(
        runtime.call_governed("governed_counting", serde_json::json!({})),
        resolve_confirmation(&manager, &calls, false),
    );
    let rejected = rejected.unwrap();
    assert_ne!(rejected.exit_code, 0);
    assert!(rejected.output.contains("REJECTED"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let (approved, ()) = tokio::join!(
        runtime.call_governed("governed_counting", serde_json::json!({})),
        resolve_confirmation(&manager, &calls, true),
    );
    let approved = approved.unwrap();
    assert_eq!(approved.exit_code, 0, "{}", approved.output);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn runtime_upper_argument_allow_stays_visible_without_a_confirmation_route() {
    use crate::agent_api::agent_loop_runtime::RuntimePermissionChecker;
    struct ArgumentAllow;
    impl PermissionChecker for ArgumentAllow {
        fn check(&self, _: &str, args: &serde_json::Value) -> PermissionDecision {
            if args.get("file_path").and_then(serde_json::Value::as_str) == Some("allowed.rs") {
                PermissionDecision::Allow
            } else {
                PermissionDecision::Ask
            }
        }
    }
    let checker = RuntimePermissionChecker::for_test_without_confirmation(
        crate::permissions::PermissionPolicy::new(),
        Some(Arc::new(ArgumentAllow)),
        None,
    );
    assert!(checker.expose_to_model("read"));
}

#[test]
fn runtime_instruction_skill_never_bypasses_its_own_explicit_deny() {
    use crate::agent_api::agent_loop_runtime::RuntimePermissionChecker;
    let registry = Arc::new(crate::skills::SkillRegistry::new());
    registry.register_unchecked(Arc::new(crate::skills::Skill {
        name: "instruction".into(),
        description: "instruction only".into(),
        allowed_tools: None,
        disable_model_invocation: false,
        kind: crate::skills::SkillKind::Instruction,
        content: "read only content".into(),
        tags: Vec::new(),
        version: None,
    }));
    let checker = RuntimePermissionChecker::for_test_without_confirmation(
        crate::permissions::PermissionPolicy::new().deny("Skill"),
        None,
        Some(registry),
    );
    assert!(!checker.expose_to_model("Skill"));
    assert_eq!(
        checker.check("Skill", &serde_json::json!({"skill_name":"instruction"})),
        PermissionDecision::Deny
    );
}
