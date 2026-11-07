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
