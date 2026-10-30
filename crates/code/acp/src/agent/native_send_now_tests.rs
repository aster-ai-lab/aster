#![cfg(any(windows, target_os = "linux"))]

use super::*;
use a3s_code_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, ToolDefinition,
};
use acp::Agent as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct NativeCommandThenAnswer {
    calls: AtomicUsize,
    command: String,
    seen: std::sync::Mutex<Vec<(Vec<Message>, String)>>,
}

impl NativeCommandThenAnswer {
    fn new(command: String) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            command,
            seen: Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for NativeCommandThenAnswer {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        panic!("the original streaming response must not fall back")
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        self.seen
            .lock()
            .unwrap()
            .push((messages.to_vec(), system.unwrap_or_default().to_owned()));
        let message = if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            Message {
                role: "assistant".into(),
                content: vec![ContentBlock::ToolUse {
                    id: "native-cancel-probe".into(),
                    name: "bash".into(),
                    input: serde_json::json!({"command": self.command}),
                }],
                reasoning_content: None,
                transcript_text: None,
                transcript_visibility: a3s_code_core::llm::TranscriptVisibility::Wire,
            }
        } else {
            Message::assistant("NEXT-OWNED")
        };
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.try_send(StreamEvent::Done(LlmResponse {
            message,
            usage: Default::default(),
            stop_reason: None,
            token_logprobs: vec![],
            meta: None,
        }))
        .unwrap();
        Ok(rx)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_now_retires_the_real_native_worker_before_admitting_the_next_turn() {
    tokio::task::LocalSet::new().run_until(async {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        let mut launch = tests::test_launch(Some(root.path().join("sessions")));
        launch.workspace = workspace.clone();
        let mut config = launch.config.clone();
        config.memory = Some(a3s_code_core::memory::MemoryConfig { llm_extraction: false, ..Default::default() });
        let host = A3sCodeAgent::new(launch);
        let id = host.new_session(acp::NewSessionRequest::new(&workspace)).await.unwrap().session_id;
        let old = host.sessions.lock().await.get(id.0.as_ref()).unwrap().session.clone();
        old.close().await;
        let core = a3s_code_core::Agent::from_config(config).await.unwrap();
        let command = if cfg!(windows) {
            "[IO.File]::WriteAllText('started.txt', 'started'); Start-Sleep -Seconds 8; [IO.File]::WriteAllText('late.txt', 'late')"
        } else {
            "printf started > started.txt; sleep 8; printf late > late.txt"
        };
        let client = Arc::new(NativeCommandThenAnswer::new(command.into()));
        let session = Arc::new(core.session_async(workspace.display().to_string(), Some(a3s_code_core::SessionOptions::new()
            .with_llm_client(client.clone())
            .with_session_id(id.0.to_string())
            .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
            .with_continuation(false)
            .with_auto_delegation_enabled(false)
            .with_prompt_slots(a3s_code_core::SystemPromptSlots { extra: Some("HOST-GUIDANCE-KEPT".into()), ..Default::default() })
            .with_permission_policy(a3s_code_core::permissions::PermissionPolicy { default_decision: a3s_code_core::permissions::PermissionDecision::Allow, ..Default::default() })
        )).await.unwrap());
        host.sessions.lock().await.get_mut(id.0.as_ref()).unwrap().session = session.clone();
        let mut first = Box::pin(host.prompt(acp::PromptRequest::new(id.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new("run original command"))])));
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                tokio::select! {
                    result = &mut first => panic!("original completed before cancellation: {result:?}"),
                    _ = tokio::time::sleep(Duration::from_millis(20)) => {
                        if workspace.join("started.txt").is_file() { break; }
                    }
                }
            }
        }).await.expect("actual native command must start");
        let next = host.prompt(acp::PromptRequest::new(id.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new("next exact prompt"))])
            .meta(serde_json::json!({"sendNow": true}).as_object().cloned()));
        let (original, next) = tokio::time::timeout(Duration::from_secs(10), async { tokio::join!(first, next) }).await.expect("native cancellation and next admission must settle");
        assert_eq!(original.unwrap().stop_reason, acp::StopReason::Cancelled);
        let refused = next.expect_err("the previous mutation still needs real verification");
        assert_eq!(refused.data.unwrap()["kind"], "tool_effect_unresolved");
        assert_eq!(client.calls.load(Ordering::SeqCst), 2);
        {
            let seen = client.seen.lock().unwrap();
            assert_eq!(seen[1].0.iter().rev().find(|message| message.role == "user").unwrap().text(), "next exact prompt");
            assert!(seen[1].1.contains("[owed work]") && seen[1].1.contains("started.txt"));
            assert!(seen[1].1.contains("HOST-GUIDANCE-KEPT"));
        }
        assert!(session.runs().await.iter().any(|run| run.status == a3s_code_core::run::RunStatus::Failed));
        assert!(session.cancel_and_settle(Duration::ZERO, Duration::ZERO).await, "the failed turn must release actual admission without clearing its obligations");
        tokio::time::sleep(Duration::from_secs(9)).await;
        assert!(!workspace.join("late.txt").exists());
        assert_eq!(std::fs::read(workspace.join("started.txt")).unwrap(), b"started");
        session.close().await;
    }).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_failed_shell_keeps_both_output_streams_in_the_model_request() {
    let root = tempfile::tempdir().unwrap();
    let stdout = format!("STDOUT-{}", uuid::Uuid::new_v4());
    let stderr = format!("STDERR-{}", uuid::Uuid::new_v4());
    let (name, script, command) = if cfg!(windows) {
        ("probe script.ps1", format!("[Console]::Out.WriteLine('{stdout}')\n[Console]::Error.WriteLine('{stderr}')\n[Console]::Out.WriteLine('CWD=' + (Get-Location).Path)\nexit 7\n"), "& './probe script.ps1'")
    } else {
        ("probe script.sh", format!("printf '%s\\n' '{stdout}'\nprintf '%s\\n' '{stderr}' >&2\nprintf 'CWD=%s\\n' \"$PWD\"\nexit 7\n"), "/bin/sh './probe script.sh'")
    };
    std::fs::write(root.path().join(name), &script).unwrap();
    let client = Arc::new(NativeCommandThenAnswer::new(command.into()));
    let mut config = tests::test_launch(None).config;
    config.memory = Some(a3s_code_core::memory::MemoryConfig {
        llm_extraction: false,
        ..Default::default()
    });
    let core = a3s_code_core::Agent::from_config(config).await.unwrap();
    let session = core
        .session_async(
            root.path().display().to_string(),
            Some(
                a3s_code_core::SessionOptions::new()
                    .with_llm_client(client.clone())
                    .with_continuation(false)
                    .with_auto_delegation_enabled(false)
                    .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
                    .with_permission_policy(a3s_code_core::permissions::PermissionPolicy {
                        default_decision: a3s_code_core::permissions::PermissionDecision::Allow,
                        ..Default::default()
                    }),
            ),
        )
        .await
        .unwrap();
    let mut stream = session
        .stream_with_run("run the controlled failing command", None)
        .await
        .unwrap();
    while let Some(event) = stream.events.recv().await {
        assert!(
            !matches!(event, AgentEvent::Error { .. }),
            "unexpected parent failure: {event:?}"
        );
    }
    stream.worker.await.unwrap();
    let seen = client.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    let results = seen[1]
        .0
        .iter()
        .flat_map(|message| &message.content)
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                content: a3s_code_core::llm::ToolResultContentField::Text(text),
                is_error,
                ..
            } => Some((text, is_error)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(*results[0].1, Some(true));
    assert!(results[0].0.starts_with("Tool exit code: 7\n"));
    assert!(
        results[0].0.contains(&stdout) && results[0].0.contains(&stderr),
        "model received: {:?}",
        results[0].0
    );
    drop(seen);
    assert_eq!(
        std::fs::read(root.path().join(name)).unwrap(),
        script.as_bytes()
    );
    session.close().await;
}
