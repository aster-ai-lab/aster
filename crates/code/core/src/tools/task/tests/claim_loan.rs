use super::*;
use crate::external_observation::{claim_write, session_owns_write};
use crate::sandbox::{BashSandbox, SandboxOutput};

struct VerifierSandbox {
    root: std::path::PathBuf,
    called: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl BashSandbox for VerifierSandbox {
    async fn exec_command(&self, _: &str, _: &str) -> Result<SandboxOutput> {
        self.called.store(true, Ordering::SeqCst);
        assert!(!session_owns_write(
            "verify-parent",
            &self.root,
            "pending.txt"
        ));
        assert_eq!(
            std::fs::read_to_string(self.root.join("pending.txt"))?,
            "changed\n"
        );
        Ok(SandboxOutput {
            stdout: "VERIFIED\n".into(),
            stderr: String::new(),
            exit_code: 0,
        })
    }
    async fn shutdown(&self) {}
}

#[tokio::test]
async fn verifier_checks_uncommitted_parent_files() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("pending.txt"), "changed\n").unwrap();
    claim_write("verify-parent", root.path(), "pending.txt").unwrap();
    let called = Arc::new(AtomicBool::new(false));
    let sandbox = Arc::new(VerifierSandbox {
        root: root.path().to_path_buf(),
        called: called.clone(),
    });
    let mut parent = redacting_parent_context();
    parent.sandbox_handle = Some(sandbox);
    let client = Arc::new(MockLlmClient::new(vec![
        MockLlmClient::tool_call_response(
            "verify-call",
            "bash",
            serde_json::json!({"command":"echo check"}),
        ),
        MockLlmClient::text_response("Verified"),
    ]));
    let executor = TaskExecutor::new(
        Arc::new(AgentRegistry::new()),
        client,
        root.path().display().to_string(),
    )
    .with_parent_context(parent);
    let result = executor
        .execute(
            TaskParams {
                agent: "verification".into(),
                description: "Check pending file".into(),
                prompt: "Run the check".into(),
                background: false,
                max_steps: Some(3),
                output_schema: None,
            },
            None,
            Some("verify-parent"),
        )
        .await
        .unwrap();
    assert!(result.success, "{}", result.output);
    assert!(called.load(Ordering::SeqCst), "verifier command never ran");
    assert_ne!(result.session_id, "verify-parent");
    assert!(session_owns_write(
        "verify-parent",
        root.path(),
        "pending.txt"
    ));
    crate::external_observation::release_session("verify-parent");
}
