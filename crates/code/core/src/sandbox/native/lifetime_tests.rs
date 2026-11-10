#![cfg(test)]

use super::*;

#[tokio::test]
async fn both_native_apis_execute_uniformly() {
    let workspace = tempfile::tempdir().unwrap();
    let sandbox = NativeBashSandbox::new(workspace.path()).unwrap();
    sandbox.probe().await.expect("probe");
    let output = sandbox
        .exec_command("printf lifetime-ok", "/workspace")
        .await
        .expect("exec");
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, "lifetime-ok");
}

#[test]
fn validate_execution_available_when_constructed() {
    let workspace = tempfile::tempdir().unwrap();
    let sandbox = NativeBashSandbox::new(workspace.path()).unwrap();
    sandbox
        .validate_execution_available()
        .expect("constructed ksandbox admits execution");
}

#[test]
fn resolves_policy_timeout() {
    let workspace = tempfile::tempdir().unwrap();
    let mut policy = aster_bash_policy(workspace.path()).unwrap();
    policy.resources.timeout_ms = 83;
    let sandbox = NativeBashSandbox {
        inner: NativeSandbox::with_policy(workspace.path(), policy).unwrap(),
        mediated: false,
    };
    assert_eq!(sandbox.effective_timeout_ms(1_000).unwrap(), 83);
    assert_eq!(sandbox.effective_timeout_ms(20).unwrap(), 20);
    assert!(sandbox.effective_timeout_ms(0).is_err());
    assert!(sandbox.owns_command_deadline());
}
