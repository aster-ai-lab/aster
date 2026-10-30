#![cfg(test)]

use super::*;

#[test]
fn conversion_preserves_native_failure_status_and_rejects_inconsistent_status() {
    for code in [7, 23, -9] {
        let output = ToolOutput::error("native failure")
            .with_metadata(serde_json::json!({"exit_code": code}));
        assert_eq!(ToolResult::from(output).exit_code, code);
    }
    for metadata in [
        serde_json::json!({"exit_code": 0}),
        serde_json::json!({"exit_code": "7"}),
        serde_json::json!({"exit_code": i64::MAX}),
        serde_json::json!({}),
    ] {
        assert_eq!(
            ToolResult::from(ToolOutput::error("failure").with_metadata(metadata)).exit_code,
            1
        );
    }
    assert_eq!(
        ToolResult::from(
            ToolOutput::success("success").with_metadata(serde_json::json!({"exit_code": 7}))
        )
        .exit_code,
        0
    );
}

struct NativeFailure;

#[async_trait::async_trait]
impl Tool for NativeFailure {
    fn name(&self) -> &str {
        "native_failure"
    }
    fn description(&self) -> &str {
        "Native process status fixture"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {}, "additionalProperties": false})
    }
    async fn execute(&self, _: &serde_json::Value, _: &ToolContext) -> anyhow::Result<ToolOutput> {
        let mut command = if cfg!(windows) {
            let mut command = std::process::Command::new("cmd.exe");
            command.args(["/d", "/c", "exit 7"]);
            command
        } else {
            let mut command = std::process::Command::new("/bin/sh");
            command.args(["-c", "exit 7"]);
            command
        };
        let status = command.status()?;
        assert_eq!(status.code(), Some(7));
        Ok(ToolOutput::error("native exit seven")
            .with_metadata(serde_json::json!({"exit_code": status.code()})))
    }
}

#[tokio::test]
async fn registry_preserves_the_actual_native_process_exit_status() {
    let root = tempfile::tempdir().unwrap();
    let registry = ToolRegistry::new(root.path().into());
    registry.register(Arc::new(NativeFailure));
    let result = registry
        .execute("native_failure", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(result.exit_code, 7);
    assert_eq!(result.metadata.unwrap()["exit_code"], 7);
}
