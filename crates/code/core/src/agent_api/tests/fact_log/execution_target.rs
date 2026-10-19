#![cfg(test)]

use super::*;
use crate::sandbox::{BashSandbox, SandboxOutput};
use std::sync::atomic::AtomicBool;

struct UnreportedSandbox;

#[async_trait]
impl BashSandbox for UnreportedSandbox {
    async fn exec_command(&self, _: &str, _: &str) -> anyhow::Result<SandboxOutput> {
        anyhow::bail!("this context fixture must never execute a command")
    }

    async fn shutdown(&self) {}
}

struct PendingContext {
    entered: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
}

struct MarkDropped(Arc<AtomicBool>);

impl Drop for MarkDropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl crate::context::ContextProvider for PendingContext {
    fn name(&self) -> &str {
        "pending-execution-context"
    }

    async fn query(
        &self,
        _: &crate::context::ContextQuery,
    ) -> anyhow::Result<crate::context::ContextResult> {
        let _drop = MarkDropped(Arc::clone(&self.dropped));
        self.entered.store(true, Ordering::SeqCst);
        std::future::pending().await
    }
}

async fn capture_unknown_target(pending_context: bool) {
    let workspace = tempfile::tempdir().unwrap();
    let agent = durable_source::source_agent().await;
    let systems = Arc::new(std::sync::Mutex::new(Vec::new()));
    let entered = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let mut options = crate::SessionOptions::new()
        .with_session_id("execution-target-unknown")
        .with_planning_mode(crate::PlanningMode::Disabled)
        .with_auto_delegation_enabled(false)
        .with_continuation(false)
        .with_sandbox_handle(Arc::new(UnreportedSandbox));
    if pending_context {
        options = options
            .with_llm_api_timeout(100)
            .with_context_provider(Arc::new(PendingContext {
                entered: Arc::clone(&entered),
                dropped: Arc::clone(&dropped),
            }));
    }
    let session = agent
        .build_session(
            workspace.path().display().to_string(),
            Arc::new(SkillCapturingClient {
                systems: Arc::clone(&systems),
            }),
            &options,
        )
        .unwrap();
    session
        .send("Describe the current execution target without tools.", None)
        .await
        .unwrap();
    let requests = systems.lock().unwrap().clone();
    assert_eq!(requests.len(), 1, "one actual fact main-model request");
    let system = &requests[0];
    assert!(
        system.contains(&format!(
            "Host platform: {} ({})",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
        "the host must remain explicit in the actual model input: {system}"
    );
    assert!(
        system.contains("Default Bash execution target: unknown (backend did not report OS/shell)"),
        "an unreported bound backend must not inherit host shell claims: {system}"
    );
    assert!(!system.lines().any(|line| line.starts_with("Platform:")));
    if pending_context {
        assert!(entered.load(Ordering::SeqCst));
        assert!(dropped.load(Ordering::SeqCst), "timed-out query is dropped");
    }
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn fact_model_input_preserves_unknown_bound_execution_target() {
    capture_unknown_target(false).await;
}

#[tokio::test]
async fn fact_model_input_keeps_required_target_after_optional_context_timeout() {
    capture_unknown_target(true).await;
}

struct LinuxSandbox;

#[async_trait]
impl BashSandbox for LinuxSandbox {
    fn execution_target(&self) -> crate::sandbox::ShellExecutionTarget {
        crate::sandbox::ShellExecutionTarget::LinuxGuestBash
    }

    async fn exec_command(&self, _: &str, _: &str) -> anyhow::Result<SandboxOutput> {
        anyhow::bail!("the delegated context fixture must not execute commands")
    }

    async fn shutdown(&self) {}
}

struct TargetRequest {
    system: String,
    tools: Vec<ToolDefinition>,
}

struct DelegatingTargetClient {
    mode: crate::tools::ToolPresentationModeV1,
    calls: AtomicUsize,
    requests: std::sync::Mutex<Vec<TargetRequest>>,
}

#[async_trait]
impl LlmClient for DelegatingTargetClient {
    async fn complete(
        &self,
        _: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        self.requests.lock().unwrap().push(TargetRequest {
            system: system.unwrap_or_default().to_string(),
            tools: tools.to_vec(),
        });
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let mut response = streamed_response();
        if call == 0 {
            let task = serde_json::json!({
                "agent":"target-worker",
                "description":"Inspect the bound command target",
                "prompt":"Describe the command target without executing any tools."
            });
            let (name, input) = if self.mode == crate::tools::ToolPresentationModeV1::Code {
                (
                    "program",
                    serde_json::json!({
                        "type":"script", "language":"javascript",
                        "source":format!("async function run(ctx) {{ return await ctx.tool('task', {task}); }}"),
                        "allowed_tools":["task"]
                    }),
                )
            } else {
                ("task", task)
            };
            response.message.content = vec![crate::llm::ContentBlock::ToolUse {
                id: "execution-target-delegate".into(),
                name: name.into(),
                input,
            }];
            response.stop_reason = Some("tool_use".into());
        }
        Ok(response)
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        _: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let response = self.complete(messages, system, tools).await?;
        let (sender, receiver) = mpsc::channel(1);
        sender.send(StreamEvent::Done(response)).await.unwrap();
        Ok(receiver)
    }
}

async fn capture_parent_and_delegated_targets(mode: crate::tools::ToolPresentationModeV1) {
    for pending_context in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let agent = durable_source::source_agent().await;
        let client = Arc::new(DelegatingTargetClient {
            mode,
            calls: AtomicUsize::new(0),
            requests: std::sync::Mutex::new(Vec::new()),
        });
        let entered = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let mut options = crate::SessionOptions::new()
            .with_session_id("execution-target-delegation")
            .with_planning_mode(crate::PlanningMode::Disabled)
            .with_auto_delegation_enabled(false)
            .with_manual_delegation_enabled(true)
            .with_continuation(false)
            .with_sandbox_handle(Arc::new(LinuxSandbox))
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new()
                    .allow_all(&["task", "program", "bash", "read"]),
            )
            .with_tool_presentation_profile(crate::tools::ToolPresentationProfileV1::for_mode(mode))
            .with_worker_agent(
                crate::subagent::WorkerAgentSpec::custom(
                    "target-worker",
                    "Report execution target",
                )
                .with_permissions(
                    crate::permissions::PermissionPolicy::new()
                        .allow_all(&["program", "bash", "read"]),
                )
                .with_prompt("EXECUTION-TARGET-CHILD-ROLE"),
            );
        if pending_context {
            options = options
                .with_llm_api_timeout(100)
                .with_context_provider(Arc::new(PendingContext {
                    entered: Arc::clone(&entered),
                    dropped: Arc::clone(&dropped),
                }));
        }
        let session = agent
            .build_session(
                workspace.path().display().to_string(),
                client.clone(),
                &options,
            )
            .unwrap();
        session
            .send(
                "Delegate one read-only inspection of the Bash execution target.",
                None,
            )
            .await
            .unwrap();
        let requests = client.requests.lock().unwrap();
        assert_eq!(
            requests.len(),
            3,
            "actual parent, task child, parent requests"
        );
        assert!(requests[1].system.contains("EXECUTION-TARGET-CHILD-ROLE"));
        for request in requests.iter() {
            assert!(request.system.contains(&format!(
                "Host platform: {} ({})",
                std::env::consts::OS,
                std::env::consts::ARCH
            )));
            assert_eq!(
                request
                    .system
                    .matches("Default Bash execution target: linux guest; shell: /bin/bash")
                    .count(),
                1
            );
            assert!(!request
                .system
                .lines()
                .any(|line| line.starts_with("Platform:")));
            assert!(!request
                .system
                .contains("Default Bash execution target: host"));
            if mode == crate::tools::ToolPresentationModeV1::Code {
                assert_eq!(request.tools.len(), 1);
                assert_eq!(request.tools[0].name, "program");
                assert!(request.tools[0].description.contains("bash(command)"));
                assert!(!request.tools[0]
                    .description
                    .contains("On Windows this runs"));
            }
        }
        drop(requests);
        if pending_context {
            assert!(entered.load(Ordering::SeqCst));
            assert!(dropped.load(Ordering::SeqCst));
        }
        session.close().await;
        agent.close().await;
    }
}

#[tokio::test]
async fn fact_model_inputs_keep_parent_and_delegated_targets_in_adaptive_mode() {
    capture_parent_and_delegated_targets(crate::tools::ToolPresentationModeV1::Adaptive).await;
}

#[tokio::test]
async fn fact_model_inputs_keep_parent_and_delegated_targets_in_code_mode() {
    capture_parent_and_delegated_targets(crate::tools::ToolPresentationModeV1::Code).await;
}
