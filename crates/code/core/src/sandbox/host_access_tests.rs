use super::*;
use crate::permissions::{policy_for_posture, PermissionChecker, PermissionDecision};
use crate::sandbox::native::NativeBashSandbox;
use crate::tools::{Tool, ToolContext};

#[tokio::test]
async fn yolo_runs_actual_host_tools_and_git_while_normal_keeps_its_sandbox() {
    let target =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/yolo-fixtures");
    std::fs::create_dir_all(&target).unwrap();
    let root = tempfile::tempdir_in(target).unwrap();
    let base = ToolContext::new(root.path().to_path_buf())
        .with_sandbox(Arc::new(NativeBashSandbox::new(root.path()).unwrap()));
    let yolo = base
        .clone()
        .with_run_governance(Some(Arc::new(policy_for_posture("yolo"))), None);
    assert!(!yolo.sandbox.as_ref().unwrap().is_isolated());
    let command = "git init -q && printf selected > selected.txt && git add selected.txt && git -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm fixture && git show HEAD:selected.txt && printf '\\n' && uname -s";
    let output = crate::tools::builtin::bash::BashTool
        .execute(&serde_json::json!({"command":command}), &yolo)
        .await
        .unwrap();
    assert!(output.success, "{}", output.content);
    assert!(output.content.contains("selected\n"));
    assert_eq!(output.metadata.as_ref().unwrap()["sandboxed"], false);
    #[cfg(target_os = "macos")]
    assert!(output.content.contains("Darwin"));
    let normal = base.with_run_governance(Some(Arc::new(policy_for_posture("default"))), None);
    assert!(normal.sandbox.as_ref().unwrap().is_isolated());
}

#[tokio::test]
async fn yolo_keeps_custom_sdk_sandboxes_and_explicit_tool_denies() {
    struct CustomSandbox;
    #[async_trait::async_trait]
    impl BashSandbox for CustomSandbox {
        async fn exec_command(&self, _: &str, _: &str) -> anyhow::Result<SandboxOutput> {
            anyhow::bail!("custom host boundary")
        }
        async fn shutdown(&self) {}
    }
    let root = tempfile::tempdir().unwrap();
    let sandbox: Arc<dyn BashSandbox> = Arc::new(CustomSandbox);
    let context = ToolContext::new(root.path().to_path_buf())
        .with_sandbox(sandbox.clone())
        .with_run_governance(Some(Arc::new(policy_for_posture("yolo"))), None);
    assert!(Arc::ptr_eq(context.sandbox.as_ref().unwrap(), &sandbox));
    let policy = policy_for_posture("yolo").deny("bash");
    assert!(policy.allows_unsandboxed_execution());
    assert_eq!(
        policy.check("bash", &serde_json::json!({"command":"echo blocked"})),
        PermissionDecision::Deny
    );
}
