#![cfg(test)]

use super::*;
use serde_json::{json, Value};

struct FailedCheck;

#[async_trait]
impl Tool for FailedCheck {
    fn name(&self) -> &str {
        "failed_check"
    }
    fn description(&self) -> &str {
        "Controlled unsuccessful check"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object"})
    }
    async fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
        assert_ne!(args["panic"], true, "controlled child panic");
        if args["wait_for_cancel"] == true {
            ctx.cancellation_token().cancelled().await;
            if let Some(ms) = args["settle_after_cancel_ms"].as_u64() {
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
        }
        Ok(ToolOutput::error("controlled check failed")
            .with_metadata(json!({"source":"controlled-check"})))
    }
}

async fn execute(source: &str, limits: Value) -> (tempfile::TempDir, ToolOutput) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("input.txt"), "controlled input\n").unwrap();
    let executor = crate::tools::ToolExecutor::new(workspace.path().display().to_string());
    executor.registry().register(Arc::new(FailedCheck));
    let output = executor
        .execute_with_context(
            "program",
            &json!({"type":"script","allowed_tools":["read","write","failed_check"],
                "source":source,"limits":limits}),
            &ToolContext::new(workspace.path().to_owned())
                .with_session_id("program-failure-receipts"),
        )
        .await
        .unwrap();
    assert_ne!(output.exit_code, 0, "parent error must remain an error");
    let output =
        ToolOutput::error(output.output).with_metadata(output.metadata.unwrap_or(Value::Null));
    (workspace, output)
}

fn program(output: &ToolOutput) -> &Value {
    &output.metadata.as_ref().expect("program receipt metadata")["program"]
}

fn assert_call(call: &Value, name: &str, success: bool) {
    assert_eq!(call["tool_name"], name);
    assert_eq!(call["success"], success);
    assert_eq!(call["exit_code"] == 0, success);
    assert!(call["output_bytes"].as_u64().unwrap() > 0);
    assert!(call["original_output_bytes"].as_u64().unwrap() > 0);
    assert!(call["start_offset_ms"].as_u64().is_some());
    assert!(call["duration_ms"].as_u64().is_some());
}

#[tokio::test]
async fn read_then_js_throw_preserves_the_real_child_receipt_and_parent_failure() {
    let (_, output) = execute(
        "async function run(ctx) { await ctx.readFile('input.txt'); throw new Error('controlled JS failure'); }",
        json!({}),
    ).await;
    assert!(output.content.starts_with("program script error:\n"));
    let receipt = program(&output);
    assert_eq!(receipt["success"], false);
    assert_eq!(receipt["tool_calls_complete"], true);
    let calls = receipt["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 1);
    assert_call(&calls[0], "read", true);
    assert_eq!(calls[0]["metadata"]["range"]["returned_lines"], 1);
}

#[tokio::test]
async fn write_then_failed_check_and_throw_preserves_both_child_receipts() {
    let (workspace, output) = execute(
        "async function run(ctx) { await ctx.tool('write', {file_path:'saved.txt',content:'saved-marker\\n'}); const check = await ctx.tool('failed_check', {}); if (check.exitCode !== 0) throw new Error('controlled check rejection'); }",
        json!({}),
    ).await;
    assert_eq!(
        std::fs::read(workspace.path().join("saved.txt")).unwrap(),
        b"saved-marker\n"
    );
    let receipt = program(&output);
    assert_eq!(receipt["success"], false);
    assert_eq!(receipt["tool_calls_complete"], true);
    let calls = receipt["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert_call(&calls[0], "write", true);
    assert_call(&calls[1], "failed_check", false);
    assert_eq!(calls[1]["metadata"]["source"], "controlled-check");
}

#[tokio::test]
async fn deadline_after_read_preserves_completed_child_receipts() {
    let (_, output) = execute(
        "async function run(ctx) { await ctx.readFile('input.txt'); await ctx.tool('failed_check', {wait_for_cancel:true}); }",
        json!({"timeoutMs":100}),
    ).await;
    assert!(output.content.contains("timed out"));
    let calls = program(&output)["tool_calls"].as_array().unwrap();
    assert!(!calls.is_empty());
    assert_call(&calls[0], "read", true);
}

#[tokio::test]
async fn deadline_keeps_ownership_past_grace_and_certifies_the_final_receipts() {
    let started = Instant::now();
    let (_, output) = execute(
        "async function run(ctx) { await ctx.readFile('input.txt'); await ctx.tool('failed_check', {wait_for_cancel:true,settle_after_cancel_ms:600}); }",
        json!({"timeoutMs":50}),
    ).await;
    assert!(started.elapsed() > PROGRAM_CANCELLATION_SETTLE_GRACE);
    assert!(output.content.contains("timed out"));
    assert_eq!(program(&output)["tool_calls_complete"], true);
    let calls = program(&output)["tool_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert_call(&calls[0], "read", true);
    assert_call(&calls[1], "failed_check", false);
}

#[tokio::test]
async fn child_panic_keeps_prior_receipts_without_claiming_an_exact_count() {
    for read_first in [false, true] {
        let source = format!(
            "async function run(ctx) {{ {} await ctx.tool('failed_check', {{panic:true}}); }}",
            if read_first {
                "await ctx.readFile('input.txt');"
            } else {
                ""
            }
        );
        let (_, output) = execute(&source, json!({})).await;
        let receipt = program(&output);
        assert_eq!(receipt["tool_calls_complete"], false);
        let calls = receipt["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), usize::from(read_first));
        if read_first {
            assert_call(&calls[0], "read", true);
        }
    }
}

#[tokio::test]
async fn pre_worker_rejections_publish_native_empty_child_receipts() {
    let workspace = tempfile::tempdir().unwrap();
    let tool = ProgramTool::new(Arc::new(ToolRegistry::new(workspace.path().to_owned())));
    for args in [
        json!({}),
        json!({"type":"unsupported"}),
        json!({"type":"script","language":"typescript"}),
        json!({"type":"script"}),
        json!({"type":"script","path":"missing.txt"}),
        json!({"type":"script","path":"missing.js"}),
        json!({"type":"script","source":"x".repeat(MAX_PROGRAM_SCRIPT_SOURCE_BYTES+1)}),
        json!({"type":"script","source":"import controlled from 'unavailable';"}),
        json!({"type":"script","source":"const noEntrypoint = true;"}),
    ] {
        let output = tool
            .execute(&args, &ToolContext::new(workspace.path().to_owned()))
            .await
            .unwrap();
        assert!(!output.success);
        assert_eq!(program(&output)["success"], false);
        assert_eq!(program(&output)["tool_calls_complete"], true);
        assert_eq!(program(&output)["tool_calls"], json!([]));
    }
}
