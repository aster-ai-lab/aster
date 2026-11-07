use super::history_seed::with_original_tool_text;
use super::tool_history_tests::{
    append, call, error_result, provider_request, result, ObservedClient,
};
use super::*;

#[tokio::test]
async fn scheduler_public_prose_does_not_duplicate_original_provider_or_summary_text() {
    for count in [1, 2, 8] {
        let workspace = tempfile::tempdir().unwrap();
        let log = FileLog::open(log_dir(workspace.path())).unwrap();
        let (mut lines, public, private) = write_batch(&log, count);
        let note = format!("assistant\n{public}");
        lines.insert(0, note.clone());
        let facts = log.read(THREAD).unwrap();
        for cut in [0, 1, lines.len()] {
            let ([prefix, kept], _) =
                with_original_tool_text(&lines[..cut], &lines[cut..], facts.clone()).unwrap();
            assert_eq!(prefix.join("\n").matches(&public).count(), 1);
            assert_eq!(
                kept.join("\n").matches(&public).count(),
                usize::from(cut < lines.len())
            );
            assert!(!prefix.join("\n").contains(&private));
        }
        assert_eq!(log.read(THREAD).unwrap(), facts);
        let client = Arc::new(ObservedClient::default());
        let messages = provider_request(workspace.path(), &client, "", lines.clone())
            .await
            .unwrap();
        let wire = messages
            .iter()
            .map(Message::text)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(wire.matches(&public).count(), 1);
        assert_eq!(messages.iter().flat_map(Message::tool_calls).count(), count);
        assert_eq!(
            messages
                .iter()
                .filter_map(|m| m.reasoning_content.as_ref())
                .filter(|r| r.as_str() == private)
                .count(),
            1
        );

        let mut separate = vec![note, "user\nA later request.".into()];
        separate.extend(lines.clone());
        let client = Arc::new(ObservedClient::default());
        let messages = provider_request(workspace.path(), &client, "", separate)
            .await
            .unwrap();
        let wire = messages
            .iter()
            .map(Message::text)
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            wire.matches(&public).count(),
            2,
            "an earlier separate message must remain"
        );
        let after = log.read(THREAD).unwrap();
        assert_eq!(&after[..facts.len()], facts);
        assert!(after[facts.len()..]
            .iter()
            .all(|fact| fact.kind == "context.sections"));
    }
}

fn write_batch(log: &FileLog, count: usize) -> (Vec<String>, String, String) {
    let public = "Original public checkpoint. ".repeat(16);
    let private = "SYNTHETIC-CONTINUATION-Ω".repeat(256);
    let calls: Vec<_> = (0..count)
        .map(|index| {
            let mut value = call(&format!("member-{index}"))["call"].clone();
            value["text"] = serde_json::json!(public);
            value["reasoning"] = serde_json::json!(private);
            value
        })
        .collect();
    append(
        log,
        "original-response",
        "model.turn",
        serde_json::json!({
            "kind":"tool", "call":calls[0], "also":calls[1..]
        }),
    );
    let mut lines: Vec<_> = calls
        .iter()
        .map(|call| format!("assistant\ntool read\n{}", call["args"]))
        .collect();
    for (index, call) in calls.iter().enumerate() {
        let output = format!("exact observation {index}");
        let id = call["id"].as_str().unwrap();
        if index + 1 == count && count > 1 {
            error_result(log, &format!("result-{index}"), id, &output);
            lines.push(format!("tool-error\n{output}"));
        } else {
            result(log, &format!("result-{index}"), id, &output);
            lines.push(format!("tool\n{output}"));
        }
    }
    (lines, public, private)
}

#[tokio::test]
async fn original_batch_response_is_sent_once_with_every_retained_call_and_result() {
    let mut failures = Vec::new();
    for count in [1, 2, 3, 8] {
        for retained in 1..=count {
            if retained != 1 && retained != count {
                continue;
            }
            for compacted in [false, true] {
                if !compacted && retained != count {
                    continue;
                }
                let workspace = tempfile::tempdir().unwrap();
                let log = FileLog::open(log_dir(workspace.path())).unwrap();
                let (all, public, private) = write_batch(&log, count);
                let mut lines = all[count - retained..count].to_vec();
                lines.extend_from_slice(&all[2 * count - retained..]);
                let summary = if compacted {
                    "Earlier original instructions."
                } else {
                    ""
                };
                if compacted {
                    for index in 0..2 {
                        append(
                            &log,
                            &format!("cut-{index}"),
                            "compaction.done",
                            serde_json::json!({"summary":summary,"kept":lines}),
                        );
                    }
                }
                let before = log.read(THREAD).unwrap();
                let client = Arc::new(ObservedClient::default());
                let messages = provider_request(workspace.path(), &client, summary, lines)
                    .await
                    .unwrap();
                let ids: Vec<_> = messages
                    .iter()
                    .flat_map(Message::tool_calls)
                    .map(|call| call.id)
                    .collect();
                let expected: Vec<_> = (count - retained..count)
                    .map(|index| format!("member-{index}"))
                    .collect();
                assert_eq!(ids, expected);
                let results: Vec<_> = messages
                    .iter()
                    .flat_map(|message| &message.content)
                    .filter_map(|block| match block {
                        crate::llm::ContentBlock::ToolResult {
                            tool_use_id,
                            is_error,
                            ..
                        } => Some((tool_use_id.clone(), *is_error)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(
                    results.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(results.last().unwrap().1, Some(count > 1));
                let public_copies = messages
                    .iter()
                    .filter(|message| message.text() == public)
                    .count();
                let private_copies = messages
                    .iter()
                    .filter(|message| {
                        message.reasoning_content.as_deref() == Some(private.as_str())
                    })
                    .count();
                eprintln!(
                    "BATCH_HISTORY_MEASUREMENT {}",
                    serde_json::json!({"members":count,"retained":retained,"compacted":compacted,"public_copies":public_copies,"private_copies":private_copies,"provider_message_bytes":serde_json::to_vec(&messages).unwrap().len()})
                );
                if public_copies != 1 || private_copies != 1 {
                    failures.push((count, retained, compacted, public_copies, private_copies));
                }
                let after = log.read(THREAD).unwrap();
                assert!(
                    serde_json::to_value(&before).unwrap()
                        == serde_json::to_value(&after[..before.len()]).unwrap(),
                    "original facts must remain unchanged"
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "one original response must not be duplicated per batch member: {failures:?}"
    );
}

#[test]
fn retained_batch_reserves_one_original_private_continuation() {
    for count in [1, 2, 3, 8] {
        let workspace = tempfile::tempdir().unwrap();
        let log = FileLog::open(log_dir(workspace.path())).unwrap();
        let (lines, _, private) = write_batch(&log, count);
        let ([prefix, kept], tokens) =
            with_original_tool_text(&[], &lines, log.read(THREAD).unwrap()).unwrap();
        assert_eq!(
            tokens,
            private.len().div_ceil(4),
            "one original response with {count} calls"
        );
        assert!(prefix
            .iter()
            .chain(&kept)
            .all(|line| !line.contains(&private)));
    }
}

#[tokio::test]
async fn identical_notes_and_reused_ids_in_distinct_responses_are_not_coalesced() {
    for reused in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let log = FileLog::open(log_dir(workspace.path())).unwrap();
        let ids = if reused {
            ["same", "same"]
        } else {
            ["one", "two"]
        };
        let mut records = Vec::new();
        let mut results = Vec::new();
        for (index, id) in ids.iter().enumerate() {
            append(&log, &format!("response-{index}"), "model.turn", call(id));
            result(&log, &format!("result-{index}"), id, "same observation");
            records.push(format!(
                "assistant\ntool read\n{}",
                call(id)["call"]["args"]
            ));
            results.push("tool\nsame observation".into());
        }
        records.extend(results);
        let client = Arc::new(ObservedClient::default());
        let messages = provider_request(workspace.path(), &client, "", records)
            .await
            .unwrap();
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.reasoning_content.is_some())
                .count(),
            2
        );
        assert_eq!(
            messages
                .iter()
                .filter(|message| !message.tool_calls().is_empty())
                .count(),
            2
        );
    }
}

#[tokio::test]
async fn per_call_metadata_and_user_boundaries_preserve_separate_continuations() {
    for variation in ["public", "private", "user"] {
        let workspace = tempfile::tempdir().unwrap();
        let log = FileLog::open(log_dir(workspace.path())).unwrap();
        let first = call("first")["call"].clone();
        let mut second = call("second")["call"].clone();
        if variation == "public" {
            second["text"] = serde_json::json!("A different public checkpoint.");
        } else if variation == "private" {
            second["reasoning"] = serde_json::json!("SYNTHETIC-DIFFERENT-CONTINUATION");
        }
        append(
            &log,
            "response",
            "model.turn",
            serde_json::json!({"kind":"tool","call":first,"also":[second]}),
        );
        result(&log, "result-first", "first", "first observation");
        error_result(&log, "result-second", "second", "second observation");
        let mut lines = vec![
            format!("assistant\ntool read\n{}", first["args"]),
            "tool\nfirst observation".into(),
        ];
        if variation == "user" {
            lines.push("user\nKeep this intervening instruction.".into());
        }
        lines.extend([
            format!("assistant\ntool read\n{}", second["args"]),
            "tool-error\nsecond observation".into(),
        ]);
        let (_, allowance) =
            with_original_tool_text(&[], &lines, log.read(THREAD).unwrap()).unwrap();
        let client = Arc::new(ObservedClient::default());
        let messages = provider_request(workspace.path(), &client, "", lines)
            .await
            .unwrap();
        let assistants: Vec<_> = messages
            .iter()
            .filter(|message| !message.tool_calls().is_empty())
            .collect();
        assert_eq!(assistants.len(), 2, "{variation}");
        for (actual, original) in assistants.iter().zip([&first, &second]) {
            assert_eq!(actual.text(), original["text"].as_str().unwrap());
            assert_eq!(
                actual.reasoning_content.as_deref(),
                original["reasoning"].as_str()
            );
        }
        assert_eq!(
            allowance,
            assistants
                .iter()
                .map(|message| {
                    message
                        .reasoning_content
                        .as_ref()
                        .unwrap()
                        .len()
                        .div_ceil(4)
                })
                .sum::<usize>(),
            "{variation}"
        );
        if variation == "user" {
            let boundary = messages
                .iter()
                .position(|message| {
                    message.role == "user" && message.text() == "Keep this intervening instruction."
                })
                .unwrap();
            assert_eq!(messages[boundary - 1].role, "user");
            assert_eq!(messages[boundary + 1].tool_calls()[0].id, "second");
        }
    }
}

#[tokio::test]
async fn grouped_batch_keeps_exact_image_and_failed_result_blocks() {
    let workspace = tempfile::tempdir().unwrap();
    let log = FileLog::open(log_dir(workspace.path())).unwrap();
    let first = call("member-0")["call"].clone();
    let second = call("member-1")["call"].clone();
    append(
        &log,
        "response",
        "model.turn",
        serde_json::json!({"kind":"tool","call":first,"also":[second]}),
    );
    let image = crate::llm::Attachment::png(vec![137, 80, 78, 71]);
    let receipt = super::receipts::runner_result_value(
        "image observation",
        false,
        std::slice::from_ref(&image),
        0,
        None,
        None,
        None,
    );
    let body = receipt.as_str().unwrap();
    result(&log, "result-0", "member-0", body);
    error_result(&log, "result-1", "member-1", "exact observation 1");
    let lines = vec![
        format!("assistant\ntool read\n{}", first["args"]),
        format!("assistant\ntool read\n{}", second["args"]),
        format!("tool\n{body}"),
        "tool-error\nexact observation 1".into(),
    ];
    let client = Arc::new(ObservedClient::default());
    let messages = provider_request(workspace.path(), &client, "", lines)
        .await
        .unwrap();
    assert_eq!(messages.iter().flat_map(Message::tool_calls).count(), 2);
    let expected = [
        message_from_folded_tool("member-0", receipt.as_str().unwrap(), false, None, true),
        message_from_folded_tool("member-1", "exact observation 1", true, None, true),
    ];
    let actual: Vec<_> = messages
        .iter()
        .filter(|message| {
            message
                .content
                .iter()
                .any(|block| matches!(block, crate::llm::ContentBlock::ToolResult { .. }))
        })
        .collect();
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
