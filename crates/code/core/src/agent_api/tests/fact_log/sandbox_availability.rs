#![cfg(test)]

use super::*;
use crate::sandbox::{BashSandbox, SandboxOutput};

struct UnavailableSandbox {
    ready: Arc<std::sync::atomic::AtomicBool>,
}

#[async_trait]
impl BashSandbox for UnavailableSandbox {
    fn supports_background_execution(&self) -> bool {
        true
    }

    fn validate_execution_available(&self) -> anyhow::Result<()> {
        if !self.ready.load(Ordering::Acquire) {
            anyhow::bail!("test_namespace_unavailable: no qualified command backend")
        }
        Ok(())
    }

    async fn exec_command(&self, _: &str, _: &str) -> anyhow::Result<SandboxOutput> {
        anyhow::bail!("the unavailable backend must not execute")
    }

    async fn shutdown(&self) {}
}

#[tokio::test]
async fn background_and_shell_refusals_leave_one_bounded_recovery_opportunity() {
    for last_tool in ["read", "background", "ls"] {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("recipe.txt"), "verified recipe").unwrap();
        let agent = durable_source::source_agent().await;
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = session_for(
            &agent,
            workspace.path(),
            "background-recovery",
            Arc::new(ScriptedClient {
                seen: Arc::clone(&seen),
                responses: std::sync::Mutex::new(vec![
                    tool_response(
                        "first",
                        "background",
                        serde_json::json!({"action":"start", "name":"dev", "command":"just dev"}),
                    ),
                    tool_response("shell", "bash", serde_json::json!({"command":"just dev"})),
                    tool_response(
                        "last",
                        last_tool,
                        match last_tool {
                            "read" => serde_json::json!({"file_path":"recipe.txt"}),
                            "background" => {
                                serde_json::json!({"action":"start", "name":"another", "command":"just dev"})
                            }
                            _ => serde_json::json!({"path":"."}),
                        },
                    ),
                    text_response("The recipe is verified."),
                ]),
            }),
            &crate::SessionOptions::new()
                .with_sandbox_handle(Arc::new(UnavailableSandbox {
                    ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                }))
                .with_permission_policy(
                    crate::permissions::PermissionPolicy::new()
                        .allow("background(*)")
                        .allow("bash(*)")
                        .allow("read(*)")
                        .deny("ls(*)"),
                )
                .with_max_tool_rounds(0)
                .with_auto_compact(false)
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        );
        let result = session
            .send("Start the dev service or inspect its recipe.", None)
            .await;
        let facts = read_workspace_facts(workspace.path(), session.session_id()).unwrap();
        session.close().await;
        agent.close().await;

        if last_tool == "read" {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("run.tool_refusal_cycle"));
        }
        let requests = seen.lock().unwrap();
        let requests: Vec<_> = requests
            .iter()
            .filter(|request| !request.tools.is_empty())
            .collect();
        assert_eq!(requests.len(), if last_tool == "read" { 4 } else { 3 });
        assert!(requests[1].messages.iter().any(|message| {
            message
                .text()
                .contains("[host tool recovery] The host refused 'background'")
        }));
        assert!(requests[2].messages.iter().any(|message| {
            message
                .text()
                .contains("[host tool recovery] The host refused 'bash'")
        }));

        let receipts: Vec<_> = facts
            .iter()
            .filter(|fact| fact.kind == "tool.result")
            .map(|fact| &fact.payload["receipt"])
            .collect();
        assert_eq!(receipts.len(), 3);
        for receipt in &receipts[..2] {
            assert_eq!(receipt["outcome"], "denied");
            assert!(receipt["exit_code"].is_null());
            assert!(receipt["output"]
                .as_str()
                .unwrap()
                .contains("test_namespace_unavailable"));
        }
        if last_tool == "read" {
            assert_eq!(receipts[2]["outcome"], "succeeded");
            assert!(receipts[2]["output"]
                .as_str()
                .unwrap()
                .contains("verified recipe"));
        } else {
            assert_eq!(receipts[2]["outcome"], "denied");
            assert!(receipts[2]["exit_code"].is_null());
        }
    }
}

#[tokio::test]
async fn the_first_model_request_names_an_unavailable_command_backend() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = durable_source::source_agent().await;
    let systems = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let session = agent
        .build_session(
            workspace.path().to_string_lossy().to_string(),
            Arc::new(SkillCapturingClient {
                systems: Arc::clone(&systems),
            }),
            &crate::SessionOptions::new()
                .with_session_id("unavailable-sandbox-context")
                .with_planning_mode(crate::prompts::PlanningMode::Disabled)
                .with_sandbox_handle(Arc::new(UnavailableSandbox {
                    ready: Arc::clone(&ready),
                }))
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        )
        .unwrap();
    session
        .send("Describe the execution environment.", None)
        .await
        .unwrap();
    let first = systems.lock().unwrap().first().cloned().unwrap();
    assert!(
        first.contains("test_namespace_unavailable: no qualified command backend"),
        "the model must know execution is unavailable before requesting Bash"
    );
    ready.store(true, Ordering::Release);
    session
        .send("Describe the updated execution environment.", None)
        .await
        .unwrap();
    let latest = systems.lock().unwrap().last().cloned().unwrap();
    assert!(
        !latest.contains("test_namespace_unavailable: no qualified command backend"),
        "a recovered backend must replace its stale unavailability context"
    );
    session.close().await;
    agent.close().await;
}
