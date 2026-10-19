#![cfg(test)]

//! A stuck blocking worker must not hang the test runner itself.

use super::*;
use crate::tools::{ToolInvocation, ToolResult};
use serde_json::json;
use std::process::{Command, Stdio};

const CASE: &str =
    "tools::program_tool::worker_deadline_tests::post_read_tight_loop_workers_settle";
const MODE_ENV: &str = "ASTER_TEST_PROGRAM_WORKER_MODE";
const MARKER_ENV: &str = "ASTER_TEST_PROGRAM_WORKER_MARKER";

struct ReadWitness {
    inner: Arc<dyn ToolInvoker>,
    read: Arc<tokio::sync::Notify>,
    resume_write: tokio::sync::Notify,
    wait_for_read: bool,
}

#[async_trait]
impl ToolInvoker for ReadWitness {
    async fn invoke(&self, invocation: ToolInvocation, ctx: &ToolContext) -> ToolResult {
        let writing = invocation.name == "write";
        if writing {
            if self.wait_for_read {
                self.resume_write.notified().await;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        let result = self.inner.invoke(invocation, ctx).await;
        assert_eq!(result.exit_code, 0, "controlled native tool must succeed");
        if !writing {
            self.resume_write.notify_one();
        }
        self.read.notify_one();
        result
    }
    fn available_tools(&self) -> Vec<String> {
        vec!["read".into(), "write".into()]
    }
}

async fn worker(mode: &str) {
    let queued_only = matches!(mode, "queued_return" | "queued_throw");
    let succeeds = matches!(mode, "return" | "queued_return");
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("input.txt"), "controlled input\n").unwrap();
    let executor = crate::tools::ToolExecutor::new(workspace.path().display().to_string());
    let read = Arc::new(tokio::sync::Notify::new());
    let invoker = Arc::new(ReadWitness {
        inner: registry_tool_invoker(Arc::clone(executor.registry())),
        read: Arc::clone(&read),
        resume_write: tokio::sync::Notify::new(),
        wait_for_read: !queued_only,
    });
    let released = Arc::downgrade(&invoker);
    let tool = ProgramTool {
        fallback_invoker: invoker,
    };
    drop(executor);
    let ctx = ToolContext::new(workspace.path().to_owned()).with_session_id("owned-program-worker");
    let cancel = ctx.cancellation_token();
    let background_write = matches!(mode, "return" | "throw") || queued_only;
    let source = if background_write {
        format!("async function run(ctx) {{ void ctx.tool('write', {{file_path:'saved.txt',content:'saved-marker\\n'}}); {} {} }}",
            if queued_only { "" } else { "await ctx.readFile('input.txt');" },
            if succeeds { "return {done:true};" } else { "throw new Error('controlled JS failure');" })
    } else {
        "async function run(ctx) { await ctx.readFile('input.txt'); while (true) {} }".to_string()
    };
    let args = json!({"type":"script","allowed_tools":["read","write"],
        "limits":{"timeoutMs":if mode == "deadline" {100} else {30_000}},
        "source":source});
    let task = tokio::spawn(async move { tool.execute(&args, &ctx).await.unwrap() });
    tokio::time::timeout(Duration::from_secs(1), read.notified())
        .await
        .expect("worker must complete a real native call before stopping");
    if mode == "drop" {
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    } else {
        if mode == "cancel" {
            cancel.cancel();
        }
        let output = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("native stop must settle the VM worker")
            .unwrap();
        assert_eq!(output.success, succeeds);
        if !background_write {
            assert!(output.content.contains(if mode == "deadline" {
                "timed out"
            } else {
                "cancelled"
            }));
        }
        let metadata = output.metadata.unwrap();
        assert_eq!(metadata["program"]["tool_calls_complete"], true);
        let calls = metadata["program"]["tool_calls"].as_array().unwrap();
        assert_eq!(
            calls.len(),
            if background_write && !queued_only {
                2
            } else {
                1
            }
        );
        assert!(calls.iter().all(|call| call["success"] == true));
        if !queued_only {
            assert!(calls.iter().any(|call| call["tool_name"] == "read"));
        }
        if background_write {
            assert!(calls.iter().any(|call| call["tool_name"] == "write"));
            assert_eq!(
                std::fs::read(workspace.path().join("saved.txt")).unwrap(),
                b"saved-marker\n"
            );
        }
    }
    // The native callback owns this invoker through ScriptVmState. It cannot
    // disappear while the embedded context or its host futures remain alive.
    tokio::time::timeout(Duration::from_secs(2), async {
        while released.upgrade().is_some() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("VM context and native callback must release after caller stop");
}

#[test]
fn post_read_tight_loop_workers_settle() {
    if let Ok(mode) = std::env::var(MODE_ENV) {
        let (flavor, stop) = mode.split_once(':').unwrap();
        let mut builder = match flavor {
            "single" => tokio::runtime::Builder::new_current_thread(),
            "multi" => {
                let mut builder = tokio::runtime::Builder::new_multi_thread();
                builder.worker_threads(2);
                builder
            }
            _ => panic!("unknown controlled runtime flavor"),
        };
        assert!(matches!(
            stop,
            "deadline" | "cancel" | "drop" | "return" | "throw" | "queued_return" | "queued_throw"
        ));
        let runtime = builder.enable_all().build().unwrap();
        std::fs::write(std::env::var_os(MARKER_ENV).unwrap(), b"started").unwrap();
        runtime.block_on(worker(stop));
        // Runtime Drop joins blocking workers too, before this child can exit.
        drop(runtime);
        return;
    }
    let workspace = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();
    for flavor in ["single", "multi"] {
        for stop in [
            "deadline",
            "cancel",
            "drop",
            "return",
            "throw",
            "queued_return",
            "queued_throw",
        ] {
            if flavor == "single" && !matches!(stop, "deadline" | "cancel" | "drop") {
                continue;
            }
            let mode = format!("{flavor}:{stop}");
            let marker = workspace.path().join(format!("{flavor}-{stop}"));
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", CASE, "--nocapture", "--test-threads=1"])
                .env(MODE_ENV, &mode)
                .env(MARKER_ENV, &marker)
                .stdin(Stdio::null())
                .spawn()
                .unwrap();
            let started = Instant::now();
            let mut entered = None;
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    if !status.success() {
                        failures.push(format!("{mode}: {status}"));
                    }
                    break;
                }
                if entered.is_none() && marker.exists() {
                    entered = Some(Instant::now());
                }
                if started.elapsed() > Duration::from_secs(15)
                    || entered.is_some_and(|time| time.elapsed() > Duration::from_secs(4))
                {
                    child.kill().unwrap();
                    let status = child.wait().unwrap();
                    failures.push(format!(
                        "{mode}: owned child exceeded bound, reaped {status}"
                    ));
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
