#![cfg(test)]

use super::receipts::{runner_interrupted_value, runner_result_value};
use super::*;

#[test]
fn folded_command_results_label_only_digest_bound_retained_streams() {
    let stdout = "actual 中文 output\nCWD=/workspace\n";
    let stderr = "actual error without newline";
    let body = format!("{stdout}{stderr}");
    let metadata = serde_json::json!({"shell_output_streams":crate::tools::builtin::bash::output_streams::capture(stdout, stderr)});
    let receipt = runner_result_value(&body, true, &[], 7, Some(&metadata), None);
    let original = receipt.clone();
    let message = message_from_folded_tool("failed-command", receipt.as_str().unwrap(), false);
    let crate::llm::ContentBlock::ToolResult {
        tool_use_id,
        content,
        is_error,
        trust,
        ..
    } = &message.content[0]
    else {
        panic!("expected original tool result");
    };
    assert_eq!(tool_use_id, "failed-command");
    assert_eq!(*is_error, Some(true));
    assert_eq!(*trust, crate::llm::ToolResultTrustV1::WorkspaceData);
    assert_eq!(content.as_text(), format!("Tool exit code: 7\nCaptured stdout ({} UTF-8 bytes):\n{stdout}\nCaptured stderr ({} UTF-8 bytes):\n{stderr}", stdout.len(), stderr.len()));
    assert_eq!(receipt, original);

    let redacted = runner_result_value("[redacted]\n", true, &[], 7, Some(&metadata), None);
    let message = message_from_folded_tool("redacted-command", redacted.as_str().unwrap(), false);
    let crate::llm::ContentBlock::ToolResult { content, .. } = &message.content[0] else {
        panic!("expected redacted result");
    };
    assert_eq!(content.as_text(), "Tool exit code: 7\n[redacted]\n");
}

#[test]
fn folded_tool_messages_preserve_typed_exit_codes_without_changing_original_receipts() {
    for code in [0, 7, 137] {
        let receipt = runner_result_value("original stdout", code != 0, &[], code, None, None);
        let original = receipt.clone();
        let message = message_from_folded_tool("original-call", receipt.as_str().unwrap(), false);
        let crate::llm::ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } = &message.content[0]
        else {
            panic!("expected original tool-result message")
        };
        assert_eq!(tool_use_id, "original-call");
        let expected = format!("Tool exit code: {code}\noriginal stdout");
        assert_eq!(content, expected.as_str());
        assert_eq!(*is_error, Some(code != 0));
        assert_eq!(
            receipt, original,
            "the original fact remains byte-for-byte unchanged"
        );
    }
    let message = message_from_folded_tool("legacy", "original stdout", false);
    assert!(
        matches!(&message.content[0],crate::llm::ContentBlock::ToolResult { content, .. } if content == "original stdout")
    );
    let silent = runner_result_value("", true, &[], 7, None, None);
    let message = message_from_folded_tool("silent", silent.as_str().unwrap(), true);
    assert!(
        matches!(&message.content[0],crate::llm::ContentBlock::ToolResult { content, .. } if content == "Tool exit code: 7\n")
    );
    let interrupted = runner_interrupted_value("interrupted output");
    let message = message_from_folded_tool("interrupted", interrupted.as_str().unwrap(), true);
    assert!(
        matches!(&message.content[0],crate::llm::ContentBlock::ToolResult { content, .. } if content == "interrupted output")
    );
}

// Mount the original workspace log without initializing any command sandbox.
pub(super) fn surface(workspace: &Path, client: Arc<dyn LlmClient>) -> SessionSurface {
    surface_with_budget(workspace, client, None)
}

pub(super) fn surface_with_budget(
    workspace: &Path,
    client: Arc<dyn LlmClient>,
    budget_guard: Option<Arc<dyn crate::budget::BudgetGuard>>,
) -> SessionSurface {
    surface_with_config(
        workspace,
        client,
        crate::agent::AgentConfig {
            planning_mode: crate::prompts::PlanningMode::Disabled,
            budget_guard,
            ..Default::default()
        },
    )
}

pub(super) fn surface_with_config(
    workspace: &Path,
    client: Arc<dyn LlmClient>,
    config: crate::agent::AgentConfig,
) -> SessionSurface {
    let executor = Arc::new(ToolExecutor::new(workspace.to_string_lossy().into_owned()));
    SessionSurface {
        response_meta: Default::default(),
        agent: crate::agent::AgentLoop::new(
            client,
            executor,
            crate::tools::ToolContext::new(workspace.to_path_buf()),
            config,
        ),
        fact_log: Default::default(),
        session_id: THREAD.into(),
        persistence: None,
        checkpoint: None,
        events: None,
        pending_memory_telemetry: Arc::new(Mutex::new(Vec::new())),
        streamed_text: Arc::new(AtomicBool::new(false)),
        forced_stop: Arc::new(Mutex::new(None)),
        cancel: tokio_util::sync::CancellationToken::new(),
        transcript: Arc::new(Mutex::new(Vec::new())),
        targeted_paths: Arc::new(Mutex::new(Vec::new())),
        usage: Arc::new(Mutex::new(crate::llm::TokenUsage::default())),
        confirmation: None,
        run_store: None,
        run_id: None,
        goal_tick: None,
        ledger: Arc::new(Mutex::new(crate::harness_loop::MutationLedger::default())),
        reports: Arc::new(Mutex::new(Vec::new())),
        run_control: None,
        harness: None,
        host_harness_registry: None,
        host_harness_assembler: None,
    }
}

pub(super) fn append(log: &FileLog, key: &str, kind: &str, payload: serde_json::Value) {
    log.append(
        THREAD,
        &[NewFact {
            key: key.into(),
            kind: kind.into(),
            payload,
        }],
        None,
    )
    .unwrap();
}

pub(super) fn call(id: &str) -> serde_json::Value {
    serde_json::json!({
        "kind": "tool",
        "call": {
            "id": id,
            "name": "read",
            "args": {"file_path": format!("{id}.rs")},
            "needs_confirmation": false,
            "text": "Inspect the original source.",
            "reasoning": "original provider continuation"
        }
    })
}

pub(super) fn result(log: &FileLog, key: &str, id: &str, output: &str) {
    append(
        log,
        key,
        "tool.result",
        serde_json::json!({"toolCallId":id,"ok":true,"output":output}),
    );
}

#[test]
fn compaction_kept_results_retain_original_tool_metadata() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    for id in ["discarded", "kept"] {
        append(&log, &format!("model-{id}"), "model.turn", call(id));
        result(&log, &format!("result-{id}"), id, id);
    }
    append(
        &log,
        "cut",
        "compaction.done",
        serde_json::json!({"summary":"earlier work","kept":[
            "assistant\ntool read\n{\"file_path\":\"kept.rs\"}", "tool\nkept"
        ]}),
    );
    append(&log, "model-fresh", "model.turn", call("fresh"));
    result(&log, "result-fresh", "fresh", "fresh");

    let calls = recorded_tool_calls(workspace.path(), THREAD).unwrap();
    assert_eq!(
        calls.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["kept", "fresh"]
    );
    assert_eq!(calls[0].name, "read");
    assert_eq!(calls[0].args, serde_json::json!({"file_path":"kept.rs"}));
    assert_eq!(calls[0].text, "Inspect the original source.");
    assert_eq!(
        calls[0].reasoning.as_deref(),
        Some("original provider continuation")
    );
}

#[test]
fn repeated_compaction_selects_original_suffix_when_outputs_are_identical() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    for id in ["first", "second"] {
        append(&log, &format!("model-{id}"), "model.turn", call(id));
        result(&log, &format!("result-{id}"), id, "same output");
        append(
            &log,
            &format!("cut-{id}"),
            "compaction.done",
            serde_json::json!({"summary":"earlier work","kept":[
                format!("assistant\ntool read\n{}", call(id)["call"]["args"]),
                "tool\nsame output"
            ]}),
        );
    }
    let calls = recorded_tool_calls(workspace.path(), THREAD).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "second");
}

#[test]
fn closed_scheduler_tool_batches_are_not_assistant_prose() {
    let lines = vec![
        "user\n分析模块".into(),
        "assistant\n先看目录。".into(),
        "assistant\ntool ls\n{\"path\":\"apps\"}".into(),
        "tool\napps".into(),
        "assistant\ntool ls\n{\"path\":\"a\"}".into(),
        "assistant\ntool ls\n{\"path\":\"b\"}".into(),
        "tool\na".into(),
        "tool-error\nmissing".into(),
        "assistant\ntool bash\n{\"command\":\"ls\"}".into(),
    ];
    let indexes = scheduler_tool_record_indexes(&lines);
    assert_eq!(indexes, [2, 4, 5].into_iter().collect());
}

#[test]
fn scheduler_indexes_cover_each_closed_shape() {
    let pair = vec![
        "assistant\ntool read\n{\"file_path\":\"one.rs\"}".into(),
        "tool-error\nrefused".into(),
    ];
    assert_eq!(
        scheduler_tool_record_indexes(&pair),
        [0].into_iter().collect()
    );

    let separated = vec![
        "assistant\ntool read\n{\"file_path\":\"one.rs\"}".into(),
        "tool\none".into(),
        "user\nbetween".into(),
        "assistant\ntool read\n{\"file_path\":\"two.rs\"}".into(),
        "tool\ntwo".into(),
    ];
    assert_eq!(
        scheduler_tool_record_indexes(&separated),
        [0, 3].into_iter().collect()
    );

    let short = vec![
        "assistant\ntool read\n{\"file_path\":\"a.rs\"}".into(),
        "assistant\ntool read\n{\"file_path\":\"b.rs\"}".into(),
        "tool\nonly".into(),
    ];
    assert_eq!(
        scheduler_tool_record_indexes(&short),
        [1].into_iter().collect(),
        "only the suffix whose own result count matches is a closed record"
    );
}

#[tokio::test]
async fn scheduler_tool_record_is_not_sent_as_assistant_prose() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model", "model.turn", call("kept"));
    result(&log, "result", "kept", "kept output");
    let client = Arc::new(ObservedClient::default());
    let (completion, _) = LiveCompletion::new(client.clone(), PermissionPolicy::new(), Vec::new());
    let completion = completion.with_surface(surface(workspace.path(), client.clone()));
    completion
        .complete(
            CompletionRequest {
                system: Vec::new(),
                tools: Vec::new(),
                summary: String::new(),
                messages: vec![
                    "user\n分析模块".into(),
                    "assistant\ntool read\n{\"file_path\":\"kept.rs\"}".into(),
                    "tool\nkept output".into(),
                    "user\n继续".into(),
                    "assistant\ntool bash\n{\"command\":\"ls\"}".into(),
                ],
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
    let inputs = client.0.lock().unwrap();
    let request = &inputs[0];
    assert!(request.iter().any(|message| {
        message
            .tool_calls()
            .iter()
            .any(|call| call.id == "kept" && call.name == "read")
    }));
    assert!(request
        .iter()
        .all(|message| message.text() != "tool read\n{\"file_path\":\"kept.rs\"}"));
    assert!(request.iter().any(|message| {
        message.role == "assistant"
            && message.tool_calls().is_empty()
            && message.text() == "tool bash\n{\"command\":\"ls\"}"
    }));
}

pub(super) fn error_result(log: &FileLog, key: &str, id: &str, output: &str) {
    append(
        log,
        key,
        "tool.result",
        serde_json::json!({"toolCallId":id,"ok":false,"output":output}),
    );
}

pub(super) async fn provider_request(
    workspace: &std::path::Path,
    client: &Arc<ObservedClient>,
    summary: &str,
    messages: Vec<String>,
) -> Result<Vec<Message>, ActorError> {
    let (completion, _) = LiveCompletion::new(client.clone(), PermissionPolicy::new(), Vec::new());
    let completion = completion.with_surface(surface(workspace, client.clone()));
    completion
        .complete(
            CompletionRequest {
                system: Vec::new(),
                tools: Vec::new(),
                summary: summary.to_string(),
                messages,
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await?;
    let inputs = client.0.lock().unwrap();
    Ok(inputs[0].clone())
}

fn tool_call_ids(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .flat_map(|message| message.tool_calls())
        .map(|call| call.id)
        .collect()
}

#[tokio::test]
async fn closed_batch_of_two_reaches_the_provider_as_structured_calls() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model-first", "model.turn", call("first"));
    result(&log, "result-first", "first", "one");
    append(&log, "model-second", "model.turn", call("second"));
    error_result(&log, "result-second", "second", "two");
    let client = Arc::new(ObservedClient::default());
    let request = provider_request(
        workspace.path(),
        &client,
        "",
        vec![
            "user\nnext".into(),
            "assistant\ntool read\n{\"file_path\":\"first.rs\"}".into(),
            "assistant\ntool read\n{\"file_path\":\"second.rs\"}".into(),
            "tool\none".into(),
            "tool-error\ntwo".into(),
            "user\ncontinue".into(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(tool_call_ids(&request), ["first", "second"]);
    assert!(request.iter().all(|message| {
        message.text() != "tool read\n{\"file_path\":\"first.rs\"}"
            && message.text() != "tool read\n{\"file_path\":\"second.rs\"}"
    }));
    assert!(request.iter().any(|message| {
        message.content.iter().any(|block| {
            matches!(
                block,
                crate::llm::ContentBlock::ToolResult {
                    tool_use_id,
                    is_error: Some(true),
                    ..
                } if tool_use_id == "second"
            )
        })
    }));
}

#[tokio::test]
async fn retained_public_prose_survives_without_reopening_its_compacted_tool_call() {
    let checkpoint = "Observed opaque value ξ-7a93; verification remains pending.";
    for latest in ["Inspect the remaining requirement.", checkpoint] {
        let workspace = tempfile::tempdir().unwrap();
        let log = FileLog::open(log_dir(workspace.path())).unwrap();
        let mut older = call("older");
        older["call"]["text"] = serde_json::json!(checkpoint);
        older["call"]["reasoning"] = serde_json::json!("PRIVATE-OLD-CONTINUATION");
        append(&log, "older-model", "model.turn", older);
        result(&log, "older-result", "older", "large original observation");
        let mut retained = call("kept");
        retained["call"]["text"] = serde_json::json!(latest);
        retained["call"]["reasoning"] = serde_json::json!("PRIVATE-KEPT-CONTINUATION");
        append(&log, "kept-model", "model.turn", retained);
        result(&log, "kept-result", "kept", "original requirement");
        let kept = vec![
            format!("assistant\n{checkpoint}"),
            format!("assistant\n{latest}"),
            "assistant\ntool read\n{\"file_path\":\"kept.rs\"}".into(),
            "tool\noriginal requirement".into(),
        ];
        append(
            &log,
            "cut",
            "compaction.done",
            serde_json::json!({"summary":"Earlier notes omitted the checkpoint.","kept":kept}),
        );
        let before = serde_json::to_value(log.read(THREAD).unwrap()).unwrap();
        let client = Arc::new(ObservedClient::default());
        let request = provider_request(workspace.path(), &client, "Earlier notes.", kept)
            .await
            .unwrap();
        assert_eq!(tool_call_ids(&request), ["kept"]);
        let public = request
            .iter()
            .map(Message::text)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            public.matches(checkpoint).count(),
            1 + usize::from(latest == checkpoint)
        );
        assert!(!public.contains("large original observation"));
        assert!(!public.contains("PRIVATE-"));
        assert!(request.iter().all(|message| {
            message.reasoning_content.as_deref() != Some("PRIVATE-OLD-CONTINUATION")
        }));
        assert_eq!(
            request
                .iter()
                .filter(|message| message.reasoning_content.as_deref()
                    == Some("PRIVATE-KEPT-CONTINUATION"))
                .count(),
            1
        );
        let after = log.read(THREAD).unwrap();
        assert_eq!(
            serde_json::to_value(&after[..before.as_array().unwrap().len()]).unwrap(),
            before
        );
    }
}

#[tokio::test]
async fn summary_prefix_does_not_reopen_a_closed_record() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model", "model.turn", call("kept"));
    result(&log, "result", "kept", "kept output");
    let client = Arc::new(ObservedClient::default());
    let request = provider_request(
        workspace.path(),
        &client,
        "SUMMARY-TOKEN",
        vec![
            "assistant\ntool read\n{\"file_path\":\"kept.rs\"}".into(),
            "tool\nkept output".into(),
            "user\ncontinue".into(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(tool_call_ids(&request), ["kept"]);
    assert!(request
        .iter()
        .any(|message| message.role == "user" && message.text().contains("SUMMARY-TOKEN")));
    assert!(request
        .iter()
        .all(|message| message.text() != "tool read\n{\"file_path\":\"kept.rs\"}"));
}

#[tokio::test]
async fn extra_result_line_refuses_before_the_provider() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model", "model.turn", call("kept"));
    result(&log, "result", "kept", "kept output");
    let client = Arc::new(ObservedClient::default());
    let error = provider_request(
        workspace.path(),
        &client,
        "",
        vec![
            "assistant\ntool read\n{\"file_path\":\"kept.rs\"}".into(),
            "tool\nkept output".into(),
            "tool\norphan".into(),
        ],
    )
    .await
    .expect_err("an extra folded result has no call");
    assert!(error.to_string().contains("no original call"));
    assert!(client.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unequal_tool_records_do_not_invent_a_call() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model-first", "model.turn", call("first"));
    result(&log, "result-first", "first", "one");
    append(&log, "model-second", "model.turn", call("second"));
    result(&log, "result-second", "second", "two");
    let client = Arc::new(ObservedClient::default());
    let (completion, _) = LiveCompletion::new(client.clone(), PermissionPolicy::new(), Vec::new());
    let completion = completion.with_surface(surface(workspace.path(), client.clone()));
    let error = completion
        .complete(
            CompletionRequest {
                system: Vec::new(),
                tools: Vec::new(),
                summary: String::new(),
                messages: vec![
                    "user\nnext".into(),
                    "assistant\ntool read\n{\"file_path\":\"first.rs\"}".into(),
                    "assistant\ntool read\n{\"file_path\":\"second.rs\"}".into(),
                    "tool\none".into(),
                ],
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect_err("a folded result is missing for the second call");
    assert!(error
        .to_string()
        .contains("absent from the folded transcript"));
    assert!(client.0.lock().unwrap().is_empty());
}

#[test]
fn actual_folded_entry_point_preserves_assistant_role() {
    let (role, body) = split_folded_message("assistant\n先检查源码，再继续修复。");
    let message = folded_message_with_images(role, &body, Vec::new());
    assert_eq!(message.role, "assistant");
    assert_eq!(message.text(), "先检查源码，再继续修复。");
}

#[test]
fn user_body_cannot_inject_an_assistant_role() {
    let (role, body) = split_folded_message("user\nassistant\nthis is user input");
    let message = folded_message_with_images(role, &body, Vec::new());
    assert_eq!(message.role, "user");
    assert_eq!(message.text(), "assistant\nthis is user input");
}

#[test]
fn missing_original_metadata_never_fabricates_a_tool_call() {
    assert!(recorded_tool_messages(
        &mut Vec::new().into_iter(),
        &mut Vec::new(),
        &mut None,
        "output",
        false
    )
    .is_err());
}

#[test]
fn malformed_original_call_and_result_identity_are_refused() {
    for payload in [
        serde_json::json!({"kind":"tool","call":{"id":"original"}}),
        call("original"),
    ] {
        let workspace = tempfile::tempdir().unwrap();
        let log = FileLog::open(log_dir(workspace.path())).unwrap();
        append(&log, "model", "model.turn", payload);
        result(&log, "result", "different", "output");
        assert!(recorded_tool_calls(workspace.path(), THREAD).is_err());
    }
}

#[test]
fn compaction_cannot_replace_original_result_bytes() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model", "model.turn", call("original"));
    result(&log, "result", "original", "actual");
    append(
        &log,
        "cut",
        "compaction.done",
        serde_json::json!({
            "summary":"earlier work", "kept":["tool\nsubstituted"]
        }),
    );
    assert!(recorded_tool_calls(workspace.path(), THREAD).is_err());
}

#[test]
fn legacy_result_without_id_and_error_role_remain_supported() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model", "model.turn", call("original"));
    append(
        &log,
        "result",
        "tool.result",
        serde_json::json!({"ok":false,"output":"refused"}),
    );
    append(
        &log,
        "cut",
        "compaction.done",
        serde_json::json!({
            "summary":"earlier work", "kept":["tool-error\nrefused"]
        }),
    );
    let mut calls = recorded_tool_calls(workspace.path(), THREAD)
        .unwrap()
        .into_iter();
    let mut messages = Vec::new();
    recorded_tool_messages(&mut calls, &mut messages, &mut None, "refused", true).unwrap();
    assert_eq!(messages.len(), 2);
    let request = &messages[0];
    let response = &messages[1];
    assert_eq!(request.tool_calls()[0].id, "original");
    assert_eq!(request.tool_calls()[0].name, "read");
    assert_eq!(
        request.tool_calls()[0].args,
        serde_json::json!({"file_path":"original.rs"})
    );
    assert_eq!(
        request.reasoning_content.as_deref(),
        Some("original provider continuation")
    );
    assert!(
        matches!(&response.content[0], crate::llm::ContentBlock::ToolResult { tool_use_id, is_error: Some(true), .. } if tool_use_id == "original")
    );
}

#[test]
fn batch_result_order_and_reused_ids_follow_the_original_scheduler() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    let mut batch = call("first");
    batch["also"] = serde_json::json!([call("second")["call"].clone()]);
    append(&log, "model-1", "model.turn", batch);
    result(&log, "result-1", "first", "one");
    result(&log, "result-2", "second", "two");
    append(&log, "model-2", "model.turn", call("first"));
    result(&log, "result-3", "first", "three");
    let calls = recorded_tool_calls(workspace.path(), THREAD).unwrap();
    assert_eq!(
        calls.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["first", "second", "first"]
    );
}

#[derive(Default)]
pub(super) struct ObservedClient(Mutex<Vec<Vec<Message>>>, Mutex<Vec<Option<String>>>);

#[tokio::test]
async fn automatic_summary_sees_public_checkpoint_before_the_retained_tool_result() {
    let root = tempfile::tempdir().unwrap();
    let log = Arc::new(FileLog::open(log_dir(root.path())).unwrap());
    let mut older = call("older");
    older["call"]["text"] = serde_json::json!("");
    append(&log, "older-call", "model.turn", older.clone());
    let output = "older raw observation\n".repeat(2_000);
    result(&log, "older-result", "older", &output);
    let mut next = call("next");
    let checkpoint = "Established checkpoint 値🙂-7a93.";
    next["call"]["text"] = serde_json::json!(checkpoint);
    let mut later = call("later")["call"].clone();
    let next_action = "Next action Ω-3b18.";
    later["text"] = serde_json::json!(next_action);
    next["also"] = serde_json::json!([later]);
    append(&log, "next-call", "model.turn", next.clone());
    result(&log, "next-result", "next", "RETAINED-RAW-OBSERVATION");
    result(&log, "later-result", "later", "LATER-RAW-OBSERVATION");
    let before = serde_json::to_value(log.read(THREAD).unwrap()).unwrap();
    let client = Arc::new(ObservedClient::default());
    let compactor = SessionCompact {
        client: client.clone(),
        history: (log.clone(), THREAD.into()),
        source: None,
        artifacts: None,
        events: None,
        budget: crate::compaction::CompactionBudget::for_auto_compaction(4_096, 0.8, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(0)),
        prompt_tokens: Arc::new(AtomicU64::new(4_000)),
    };
    let kept = vec![
        format!("assistant\ntool read\n{}", next["call"]["args"]),
        format!("assistant\ntool read\n{}", next["also"][0]["args"]),
        "tool\nRETAINED-RAW-OBSERVATION".into(),
        "tool\nLATER-RAW-OBSERVATION".into(),
    ];
    let original_kept = kept.clone();
    compactor
        .compact_with_tail(
            &[
                format!("assistant\ntool read\n{}", older["call"]["args"]),
                format!("tool\n{output}"),
            ],
            &kept,
        )
        .await
        .unwrap();
    let requests = client.0.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let input = requests[0]
        .iter()
        .map(Message::text)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(input.matches(checkpoint).count(), 1);
    assert_eq!(input.matches(next_action).count(), 1);
    assert!(!input.contains("RETAINED-RAW-OBSERVATION"));
    assert!(!input.contains("LATER-RAW-OBSERVATION"));
    assert!(!input.contains("next.rs"));
    assert!(!input.contains("later.rs"));
    assert!(!input.contains("original provider continuation"));
    assert_eq!(kept, original_kept);
    assert_eq!(
        serde_json::to_value(log.read(THREAD).unwrap()).unwrap(),
        before
    );
}

#[async_trait::async_trait]
impl LlmClient for ObservedClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        _: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.0.lock().unwrap().push(messages.to_vec());
        self.1.lock().unwrap().push(system.map(str::to_owned));
        Ok(LlmResponse {
            message: Message::assistant("done"),
            usage: Default::default(),
            stop_reason: None,
            token_logprobs: Vec::new(),
            meta: None,
        })
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> Result<tokio::sync::mpsc::Receiver<crate::llm::StreamEvent>> {
        anyhow::bail!("unused in this hermetic projection test")
    }
}

#[tokio::test]
async fn actual_completion_sends_retained_native_tool_pair_and_assistant_role() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    append(&log, "model", "model.turn", call("kept"));
    result(&log, "result", "kept", "kept output");
    let kept = vec!["tool\nkept output".to_string()];
    append(
        &log,
        "cut",
        "compaction.done",
        serde_json::json!({"summary":"earlier work","kept":kept}),
    );
    let client = Arc::new(ObservedClient::default());
    let (completion, _) = LiveCompletion::new(client.clone(), PermissionPolicy::new(), Vec::new());
    let completion = completion.with_surface(surface(workspace.path(), client.clone()));
    completion
        .complete(
            CompletionRequest {
                system: Vec::new(),
                tools: Vec::new(),
                summary: "earlier work".into(),
                messages: [
                    kept,
                    vec![
                        "assistant\nPrevious reply.".into(),
                        "user\nContinue.".into(),
                    ],
                ]
                .concat(),
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
    let inputs = client.0.lock().unwrap();
    assert_eq!(inputs.len(), 1);
    let request = inputs[0]
        .iter()
        .find(|m| !m.tool_calls().is_empty())
        .unwrap();
    let tool = &request.tool_calls()[0];
    assert_eq!(tool.id, "kept");
    assert_eq!(tool.name, "read");
    assert_eq!(tool.args, serde_json::json!({"file_path":"kept.rs"}));
    assert!(inputs[0]
        .iter()
        .any(|m| m.role == "assistant" && m.text() == "Previous reply."));
    assert!(inputs[0].iter().flat_map(|m| &m.content).any(|b| matches!(b, crate::llm::ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == "kept")));
}

#[tokio::test]
async fn actual_completion_refuses_missing_metadata_before_provider_entry() {
    let workspace = tempfile::tempdir().unwrap();
    let client = Arc::new(ObservedClient::default());
    for mounted in [false, true] {
        let (completion, _) =
            LiveCompletion::new(client.clone(), PermissionPolicy::new(), Vec::new());
        let completion = if mounted {
            completion.with_surface(surface(workspace.path(), client.clone()))
        } else {
            completion
        };
        assert!(completion
            .complete(
                CompletionRequest {
                    system: Vec::new(),
                    tools: Vec::new(),
                    summary: String::new(),
                    messages: vec!["tool\norphan output".into()],
                },
                tokio_util::sync::CancellationToken::new()
            )
            .await
            .is_err());
    }
    assert!(client.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cached_prompt_cannot_replace_a_retained_tool_result() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    let client = Arc::new(ObservedClient::default());
    let (completion, _) = LiveCompletion::new(client.clone(), PermissionPolicy::new(), Vec::new());
    let completion = completion.with_surface(surface(workspace.path(), client.clone()));
    completion
        .complete(
            CompletionRequest {
                system: Vec::new(),
                tools: Vec::new(),
                summary: String::new(),
                messages: vec!["user\nOriginal user request.".into()],
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
    append(&log, "model", "model.turn", call("original"));
    result(&log, "result", "original", "original result");
    append(
        &log,
        "cut",
        "compaction.done",
        serde_json::json!({
            "summary":"earlier work", "kept":["tool\noriginal result"]
        }),
    );
    completion
        .complete(
            CompletionRequest {
                system: Vec::new(),
                tools: Vec::new(),
                summary: "earlier work".into(),
                messages: vec!["tool\noriginal result".into()],
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
    let inputs = client.0.lock().unwrap();
    assert!(inputs.last().unwrap().iter().flat_map(|m| &m.content).any(|b| matches!(b, crate::llm::ContentBlock::ToolResult { tool_use_id, content, .. } if tool_use_id == "original" && content == "original result")));
}

#[tokio::test]
async fn current_verification_follows_the_last_observation_without_changing_the_system_prefix() {
    use crate::verification::{host_report_for_verification_contract, VerificationContractV1};
    for compact in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let log = FileLog::open(log_dir(workspace.path())).unwrap();
        append(&log, "model:1:0", "model.turn", call("kept"));
        let raw = runner_result_value("actual stdout", false, &[], 0, None, None);
        let body = raw.as_str().unwrap();
        let args = aster_effect::tool_args_digest(&call("kept")["call"]["args"]).unwrap();
        let prepared = aster_effect::ToolPreparedV1 {
            schema: aster_effect::TOOL_PREPARED_SCHEMA.into(),
            identity: aster_effect::ToolInvocationIdentityV1 {
                turn: 1,
                cycle: 0,
                tool_call_id: "kept".into(),
                batch_index: 0,
                admission: None,
            },
            tool_name: "read".into(),
            original_args_digest: args.clone(),
            effective_args_digest: args,
            recovery: aster_effect::ToolRecoveryPolicy::NeverReplay,
            baseline: None,
        };
        append(
            &log,
            "prepared",
            "tool.prepared",
            serde_json::json!({"prepared":prepared}),
        );
        let receipt = aster_effect::ToolExecutionResultV1 {
            schema: aster_effect::TOOL_RESULT_SCHEMA.into(),
            identity: prepared.identity.clone(),
            prepared_digest: prepared.digest().unwrap(),
            outcome: aster_effect::ToolExecutionOutcome::Succeeded,
            exit_code: Some(0),
            output: raw.clone(),
            effects: aster_effect::ToolEffectObservationV1::complete(Vec::new()),
        };
        append(
            &log,
            "result",
            "tool.result",
            serde_json::json!({"toolCallId":"kept","ok":true,"output":body,"receipt":receipt}),
        );
        if compact {
            append(
                &log,
                "cut",
                "compaction.done",
                serde_json::json!({
                    "summary":"earlier work", "kept":[format!("tool\n{body}")]
                }),
            );
        }
        let original = read_workspace_facts(workspace.path(), THREAD).unwrap();
        let client = Arc::new(ObservedClient::default());
        let contract =
            VerificationContractV1::new("suite", "run-exact-suite", ["source 甲.rs"]).unwrap();
        let surface = surface_with_config(
            workspace.path(),
            client.clone(),
            crate::agent::AgentConfig {
                verification_contracts: vec![contract.clone()],
                require_assistant_final_response: true,
                ..Default::default()
            },
        );
        let ledger = surface.ledger.clone();
        let reports = surface.reports.clone();
        let (completion, _) =
            LiveCompletion::new(client.clone(), PermissionPolicy::new(), Vec::new());
        let completion = completion.with_surface(surface);
        for state in 0..4 {
            if state != 2 {
                let bytes = format!("revision {state}");
                std::fs::write(workspace.path().join("source 甲.rs"), &bytes).unwrap();
                ledger.lock().unwrap().observe_tool(
                    "write",
                    0,
                    Some(&serde_json::json!({
                        "file_path":"source 甲.rs", "after":bytes
                    })),
                );
            } else {
                let guard = ledger.lock().unwrap();
                let report = host_report_for_verification_contract(
                    &contract,
                    &contract.command,
                    0,
                    &["source 甲.rs".into()],
                    guard.digest(),
                )
                .unwrap();
                reports.lock().unwrap().push(report);
            }
            completion
                .complete(
                    CompletionRequest {
                        system: vec!["stable host policy".into()],
                        tools: Vec::new(),
                        summary: if compact {
                            "earlier work".into()
                        } else {
                            String::new()
                        },
                        messages: vec![format!("tool\n{body}")],
                    },
                    tokio_util::sync::CancellationToken::new(),
                )
                .await
                .unwrap();
            if state != 2 {
                let gate = crate::harness_loop::decide_with_contracts(
                    &ledger.lock().unwrap(),
                    &reports.lock().unwrap(),
                    &[],
                    false,
                    &[],
                    &[contract.clone()],
                );
                assert!(
                    crate::harness_loop::model_verification_context(gate.clone()).is_some(),
                    "fixture must have known unverified effects: {gate:?}"
                );
            }
            let inputs = client.0.lock().unwrap();
            let message = inputs.last().unwrap().last().unwrap();
            let crate::llm::ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
                trust,
                ..
            } = &message.content[0]
            else {
                panic!("verification must not create a fake call or user message");
            };
            assert_eq!(tool_use_id, "kept");
            assert_eq!(*is_error, Some(false));
            assert_eq!(*trust, crate::llm::ToolResultTrustV1::WorkspaceData);
            assert!(content
                .as_text()
                .starts_with("Tool exit code: 0\nactual stdout"));
            if state == 2 {
                assert!(content
                    .as_text()
                    .contains("current workspace changes are verified"));
            } else {
                assert!(content.as_text().contains("[host verification state]"), "state {state}, compact={compact}: current verification must be beside the observation");
                assert!(content.as_text().contains("run-exact-suite"));
            }
        }
        let systems = client.1.lock().unwrap();
        assert_eq!(systems.len(), 4);
        assert!(systems.windows(2).all(|pair| pair[0] == pair[1]));
        assert!(systems.iter().all(|system| !system
            .as_deref()
            .unwrap_or_default()
            .contains("[host verification state]")));
        let after = read_workspace_facts(workspace.path(), THREAD).unwrap();
        assert!(after.len() >= original.len());
        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::to_value(&after[..original.len()]).unwrap()
        );
    }
}

#[test]
fn verification_projection_preserves_images_errors_and_uses_system_for_new_user_input() {
    let image = crate::llm::Attachment::png(vec![137, 80, 78, 71]);
    let original = Message::tool_result_with_images("actual-call", "actual stderr", &[image], true);
    let mut ledger = crate::harness_loop::MutationLedger::default();
    ledger.observe_tool(
        "write",
        0,
        Some(&serde_json::json!({"file_path":"source.rs","after":"changed"})),
    );
    let gate = crate::harness_loop::decide_completion(&ledger, &[], &[], false);
    let mut messages = vec![original.clone()];
    let mut system = Some("stable policy".to_string());
    tool_results::apply_verification_context(&mut messages, &mut system, gate.clone());
    assert_eq!(system.as_deref(), Some("stable policy"));
    let crate::llm::ContentBlock::ToolResult {
        tool_use_id,
        content,
        is_error,
        trust,
        ..
    } = &messages[0].content[0]
    else {
        panic!("original result");
    };
    assert_eq!(tool_use_id, "actual-call");
    assert_eq!(*is_error, Some(true));
    assert_eq!(*trust, crate::llm::ToolResultTrustV1::WorkspaceData);
    let crate::llm::ToolResultContentField::Blocks(blocks) = content else {
        panic!("retain native blocks");
    };
    assert!(matches!(
        &blocks[1],
        crate::llm::ToolResultContent::Image { .. }
    ));
    assert!(content.as_text().contains("[host verification state]"));
    let mut projected = serde_json::to_value(&messages[0]).unwrap();
    projected["content"][0]["content"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert_eq!(projected, serde_json::to_value(&original).unwrap());
    for last in [
        Message::user("new instruction"),
        Message::assistant("pending decision"),
    ] {
        let mut messages = vec![original.clone(), last];
        let before = serde_json::to_value(&messages).unwrap();
        let mut system = None;
        tool_results::apply_verification_context(&mut messages, &mut system, gate.clone());
        assert_eq!(serde_json::to_value(messages).unwrap(), before);
        assert!(system
            .as_deref()
            .unwrap()
            .contains("[host verification state]"));
    }
}

#[test]
fn verified_previous_changes_do_not_mark_a_new_user_request_complete() {
    let mut messages = vec![Message::user("new task")];
    let before = serde_json::to_value(&messages).unwrap();
    let mut system = Some("stable policy".to_string());
    tool_results::apply_verification_context(
        &mut messages,
        &mut system,
        crate::harness_loop::CompletionGate::Allow(
            crate::harness_loop::CompletionTerminal::Verified {
                effect_digest: "previous verified effect".into(),
            },
        ),
    );
    assert_eq!(serde_json::to_value(&messages).unwrap(), before);
    assert_eq!(system.as_deref(), Some("stable policy"));
}

#[test]
fn automatic_summary_restores_public_batch_text_once_without_changing_original_records() {
    let root = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(root.path())).unwrap();
    let mut batch = call("first");
    batch["also"] = serde_json::json!([call("second")["call"].clone()]);
    append(&log, "batch", "model.turn", batch);
    result(&log, "first-result", "first", "first observation");
    result(&log, "second-result", "second", "second observation");
    let lines = vec![
        "user\nKeep the original task.".to_string(),
        format!("assistant\ntool read\n{}", call("first")["call"]["args"]),
        format!("assistant\ntool read\n{}", call("second")["call"]["args"]),
        "tool\nfirst observation".into(),
        "tool\nsecond observation".into(),
    ];
    let facts = log.read(THREAD).unwrap();
    let before = serde_json::to_value(&facts).unwrap();
    let ([projected, kept], continuation_tokens) =
        history_seed::with_original_tool_text(&lines, &[], facts.clone()).unwrap();
    assert!(kept.is_empty());
    assert_eq!(continuation_tokens, 0);
    let note = "assistant\nInspect the original source.";
    assert_eq!(
        projected
            .iter()
            .filter(|line| line.as_str() == note)
            .count(),
        1
    );
    assert_eq!(
        projected
            .iter()
            .filter(|line| line.as_str() != note)
            .cloned()
            .collect::<Vec<_>>(),
        lines
    );
    assert!(!projected
        .join("\n")
        .contains("original provider continuation"));
    assert_eq!(
        serde_json::to_value(log.read(THREAD).unwrap()).unwrap(),
        before
    );
    let mut invalid = facts;
    invalid[1].payload["toolCallId"] = serde_json::json!("different-call");
    assert!(matches!(
        history_seed::with_original_tool_text(&lines, &[], invalid),
        Err(ActorError::Schema(_))
    ));
}

#[test]
fn automatic_summary_and_retained_tail_share_the_original_call_cursor() {
    let root = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(root.path())).unwrap();
    let first = call("prefix-call");
    append(&log, "prefix-model", "model.turn", first.clone());
    result(&log, "prefix-result", "prefix-call", "prefix observation");
    let mut second = call("retained-call");
    second["call"]["text"] = serde_json::json!("Separate retained public checkpoint.");
    append(&log, "tail-model", "model.turn", second.clone());
    result(&log, "tail-result", "retained-call", "retained observation");
    let prefix = vec![
        format!("assistant\ntool read\n{}", first["call"]["args"]),
        "tool\nprefix observation".into(),
    ];
    let tail = vec![
        format!("assistant\ntool read\n{}", second["call"]["args"]),
        "tool\nretained observation".into(),
    ];
    let ([projected, kept], continuation_tokens) =
        history_seed::with_original_tool_text(&prefix, &tail, log.read(THREAD).unwrap()).unwrap();
    assert!(continuation_tokens > 0);
    assert!(projected[0].contains("Inspect the original source."));
    assert_eq!(
        projected.last().unwrap(),
        "assistant\nSeparate retained public checkpoint."
    );
    assert_eq!(kept[0], "assistant\nSeparate retained public checkpoint.");
    assert!(!kept.join("\n").contains("Inspect the original source."));
    assert!(!kept.join("\n").contains("original provider continuation"));
    assert_eq!(&projected[1..projected.len() - 1], prefix);
    assert_eq!(&kept[1..], tail);
}
