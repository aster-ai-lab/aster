#![cfg(test)]

use super::*;

fn finite_response(frames: Vec<u8>) -> StreamingHttpResponse {
    StreamingHttpResponse {
        status: 200,
        retry_after: None,
        error_body: String::new(),
        byte_stream: Box::pin(futures::stream::iter(
            frames
                .into_iter()
                .map(|byte| Ok(bytes::Bytes::from(vec![byte]))),
        )),
    }
}

async fn read_frames(frames: Vec<u8>) -> Result<String> {
    let (replies, _receiver) = mpsc::channel(1);
    read_run_response(
        finite_response(frames),
        "synthetic-secret",
        CancellationToken::new(),
        replies,
        &[],
        GENERATION_DEADLINE,
    )
    .await
    .map(|output| output.text)
}

fn text_frame(text: &str) -> Vec<u8> {
    connect_envelope(
        &serde_json::to_vec(&json!({
            "interactionUpdate":{"textDelta":{"text":text}}
        }))
        .unwrap(),
    )
}

fn read_tool() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "read".into(),
        description: "Read a workspace file".into(),
        parameters: json!({"type":"object","properties":{"file_path":{"type":"string"}}}),
    }]
}

#[tokio::test]
async fn native_parallel_calls_are_returned_without_executing_cursor_tools() {
    for listed_first in [false, true] {
        let mut frames = Vec::new();
        if listed_first {
            frames.extend(connect_envelope(
                br#"{"interactionUpdate":{"toolRequestsListed":{"callCount":1}}}"#,
            ));
            frames.extend(connect_envelope(
                br#"{"interactionUpdate":{"toolCallCompleted":{}}}"#,
            ));
            frames.extend(connect_envelope(
                br#"{"interactionUpdate":{"toolRequestsListed":{"callCount":2}}}"#,
            ));
        }
        for path in ["space dir/你好.rs", "second.rs"] {
            frames.extend(connect_envelope(
                &serde_json::to_vec(&json!({
                    "execServerMessage":{"mcpArgs":{
                        "providerIdentifier":"aster","toolName":"read","args":{"file_path":path}
                    }}
                }))
                .unwrap(),
            ));
        }
        if !listed_first {
            frames.extend(connect_envelope(
                br#"{"interactionUpdate":{"toolRequestsListed":{"callCount":2}}}"#,
            ));
        }
        let (replies, _receiver) = mpsc::channel(1);
        let output = read_run_response(
            finite_response(frames),
            "synthetic-secret",
            CancellationToken::new(),
            replies,
            &read_tool(),
            GENERATION_DEADLINE,
        )
        .await
        .unwrap();
        assert_eq!(output.calls.len(), 2);
        let mut message = Message::assistant(&output.text);
        message.content.extend(output.calls);
        let calls = message.tool_calls();
        assert_eq!(calls[0].args["file_path"], "space dir/你好.rs");
        assert_ne!(calls[0].id, calls[1].id);
        assert_eq!(calls[1].name, "read");
    }
}

#[test]
fn native_and_legacy_finals_remain_usable_without_reinterpreting_tool_markup() {
    for text in [
        "Completed the task.",
        "```rust\nfn main() {}\n```",
        "{\"answer\":42}",
    ] {
        assert_eq!(
            tool_protocol::response(text, &read_tool()).unwrap().text(),
            text
        );
    }
    assert_eq!(
        tool_protocol::response(r#"{"text":"Done","tool_calls":[]}"#, &read_tool())
            .unwrap()
            .text(),
        "Done"
    );
    assert!(tool_protocol::response(
        r#"{"text":"","tool_calls":[{"name":"shell","arguments":{}}]}"#,
        &read_tool()
    )
    .is_err());
}

#[tokio::test]
async fn an_incomplete_native_batch_is_not_executed_when_the_stream_ends() {
    let mut frames =
        connect_envelope(br#"{"interactionUpdate":{"toolRequestsListed":{"callCount":2}}}"#);
    frames.extend(connect_envelope(br#"{"execServerMessage":{"mcpArgs":{"providerIdentifier":"aster","toolName":"read","args":{}}}}"#));
    let mut end = connect_envelope(b"{}");
    end[0] = 2;
    frames.extend(end);
    let (replies, _receiver) = mpsc::channel(1);
    assert!(read_run_response(
        finite_response(frames),
        "synthetic-secret",
        CancellationToken::new(),
        replies,
        &read_tool(),
        GENERATION_DEADLINE
    )
    .await
    .is_err());
}

#[test]
fn protobuf_usage_counts_accept_strings_and_numbers_without_double_counting_cache() {
    let usage = token_usage(
        &json!({"inputTokens":"20","outputTokens":5,"cacheReadTokens":"7","cacheWriteTokens":3}),
    )
    .unwrap();
    assert_eq!(usage.total_tokens, 25);
    assert_eq!(usage.prompt_tokens, 20);
    assert_eq!(usage.cache_read_tokens, Some(7));
    assert_eq!(usage.cache_write_tokens, Some(3));
    assert!(token_usage(&json!({"inputTokens":1,"cacheReadTokens":2})).is_err());
    assert!(token_usage(&json!({"inputTokens":"-1"})).is_err());
}

#[tokio::test]
async fn error_and_terminal_frames_cannot_dispatch_native_calls() {
    for error_frame in [false, true] {
        let mut frames =
            connect_envelope(br#"{"interactionUpdate":{"toolRequestsListed":{"callCount":1}}}"#);
        let mut value = json!({"execServerMessage":{"mcpArgs":{"providerIdentifier":"aster","toolName":"read","args":{}}}});
        if error_frame {
            value["error"] = json!({"code":"permission_denied"});
        }
        let mut frame = connect_envelope(&serde_json::to_vec(&value).unwrap());
        if !error_frame {
            frame[0] = 2;
        }
        frames.extend(frame);
        let (replies, _receiver) = mpsc::channel(1);
        assert!(read_run_response(
            finite_response(frames),
            "synthetic-secret",
            CancellationToken::new(),
            replies,
            &read_tool(),
            GENERATION_DEADLINE
        )
        .await
        .is_err());
    }
}

fn delayed_frames(frames: Vec<(Duration, Vec<u8>)>) -> StreamingHttpResponse {
    let stream = futures::stream::unfold(frames.into_iter(), |mut frames| async move {
        let Some((delay, frame)) = frames.next() else {
            return std::future::pending().await;
        };
        tokio::time::sleep(delay).await;
        Some((Ok(bytes::Bytes::from(frame)), frames))
    });
    StreamingHttpResponse {
        status: 200,
        retry_after: None,
        error_body: String::new(),
        byte_stream: Box::pin(stream),
    }
}

#[tokio::test]
async fn pauses_inside_a_frame_or_thinking_do_not_complete_a_preamble() {
    for thinking in [false, true] {
        let frame = text_frame("complete");
        let frames = if thinking {
            vec![
                (Duration::ZERO, text_frame("preamble ")),
                (
                    Duration::ZERO,
                    connect_envelope(
                        br#"{"interactionUpdate":{"thinkingDelta":{"text":"reasoning"}}}"#,
                    ),
                ),
                (
                    QUIET_AFTER_TEXT + Duration::from_millis(100),
                    connect_envelope(br#"{"interactionUpdate":{"thinkingCompleted":{}}}"#),
                ),
                (Duration::ZERO, frame),
            ]
        } else {
            vec![
                (Duration::ZERO, text_frame("preamble ")),
                (Duration::ZERO, frame[..6].to_vec()),
                (
                    QUIET_AFTER_TEXT + Duration::from_millis(100),
                    frame[6..].to_vec(),
                ),
            ]
        };
        let mut frames = frames;
        frames.push((
            Duration::ZERO,
            connect_envelope(br#"{"interactionUpdate":{"turnEnded":{}}}"#),
        ));
        let (replies, _receiver) = mpsc::channel(1);
        let output = tokio::time::timeout(
            Duration::from_secs(5),
            read_run_response(
                delayed_frames(frames),
                "synthetic-secret",
                CancellationToken::new(),
                replies,
                &[],
                GENERATION_DEADLINE,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(output.text, "preamble complete");
        assert_eq!(output.reasoning, if thinking { "reasoning" } else { "" });
    }
}

#[tokio::test]
async fn generation_deadlines_never_return_partial_answers() {
    let (replies, _receiver) = mpsc::channel(1);
    let response = delayed_frames(vec![(Duration::ZERO, text_frame("partial"))]);
    assert!(read_run_response(
        response,
        "synthetic-secret",
        CancellationToken::new(),
        replies,
        &[],
        Duration::from_millis(20)
    )
    .await
    .is_err());
    let client = CursorAccountClient::new("synthetic-secret".into(), "model".into());
    assert!(client
        .with_active_generation_timeout(Duration::from_secs(240))
        .is_some());
}

#[tokio::test]
async fn native_tool_calls_can_follow_a_preamble_after_a_quiet_gap() {
    let call = json!({"execServerMessage":{"mcpArgs":{
        "providerIdentifier":"aster","toolName":"read","args":{"file_path":"input.txt"}
    }}});
    let response = delayed_frames(vec![
        (Duration::ZERO, text_frame("I will read input.txt.")),
        (
            QUIET_AFTER_TEXT + Duration::from_millis(100),
            connect_envelope(&serde_json::to_vec(&call).unwrap()),
        ),
        (
            Duration::ZERO,
            connect_envelope(br#"{"interactionUpdate":{"toolRequestsListed":{"callCount":1}}}"#),
        ),
    ]);
    let (replies, _receiver) = mpsc::channel(1);
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        read_run_response(
            response,
            "synthetic-secret",
            CancellationToken::new(),
            replies,
            &read_tool(),
            GENERATION_DEADLINE,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(output.calls.len(), 1);
    assert_eq!(output.text, "I will read input.txt.");
}

#[tokio::test]
async fn fragmented_connect_headers_and_utf8_preserve_the_complete_answer() {
    for trailer in [false, true] {
        let mut frames = text_frame("你好，Cursor");
        let mut terminal = if trailer {
            connect_envelope(br#"{"error":null}"#)
        } else {
            connect_envelope(br#"{"interactionUpdate":{"turnEnded":{}}}"#)
        };
        if trailer {
            terminal[0] = 2;
        }
        frames.extend(terminal);
        assert_eq!(read_frames(frames).await.unwrap(), "你好，Cursor");
    }
}

#[tokio::test]
async fn broken_frames_and_eof_cannot_turn_partial_text_into_success() {
    for suffix in [
        vec![],
        vec![0, 0, 0],
        vec![0, 0, 0, 0, 10, b'{'],
        connect_envelope(br#"{invalid synthetic-secret"#),
    ] {
        let mut frames = text_frame("partial answer");
        frames.extend(suffix);
        let error = read_frames(frames)
            .await
            .expect_err("broken stream must fail");
        assert!(!error.to_string().contains("synthetic-secret"));
    }
}

#[tokio::test]
async fn permanent_connect_errors_do_not_retry_or_echo_secrets() {
    for code in [
        "permission_denied",
        "unauthenticated",
        "invalid_argument",
        "failed_precondition",
    ] {
        let frames = connect_envelope(
            &serde_json::to_vec(&json!({
                "error":{"code":code,"message":"synthetic-secret"}
            }))
            .unwrap(),
        );
        let error = read_frames(frames).await.unwrap_err();
        assert!(
            error
                .downcast_ref::<super::super::NonRetryableLlmError>()
                .is_some(),
            "{code}: {error}"
        );
        assert!(!error.to_string().contains("synthetic-secret"));
    }
}

#[tokio::test]
async fn quota_details_distinguish_exhausted_accounts_from_transient_limits() {
    for retryable in [false, true] {
        let frames = connect_envelope(&serde_json::to_vec(&json!({
            "error":{"code":"resource_exhausted","details":[{
                "type":"aiserver.v1.ErrorDetails", "debug":{
                    "error":"ERROR_RATE_LIMITED_CHANGEABLE",
                    "details":{"title":"Usage limit reached","detail":"Account quota resets next month","isRetryable":retryable}
                }
            }]}
        })).unwrap());
        let error = read_frames(frames).await.unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<super::super::NonRetryableLlmError>()
                .is_some(),
            !retryable
        );
        assert!(error.to_string().contains("Usage limit reached"));
    }
}

fn delayed_response(prefix: String, suffix: String) -> StreamingHttpResponse {
    let stream =
        futures::stream::unfold((0, prefix, suffix), |(step, prefix, suffix)| async move {
            let text = match step {
                0 => &prefix,
                1 => {
                    tokio::time::sleep(QUIET_AFTER_TEXT + Duration::from_millis(250)).await;
                    &suffix
                }
                _ => return std::future::pending().await,
            };
            let frame = connect_envelope(
                &serde_json::to_vec(&json!({
                    "interactionUpdate":{"textDelta":{"text":text}}
                }))
                .unwrap(),
            );
            Some((Ok(bytes::Bytes::from(frame)), (step + 1, prefix, suffix)))
        });
    StreamingHttpResponse {
        status: 200,
        retry_after: None,
        error_body: String::new(),
        byte_stream: Box::pin(stream),
    }
}

#[tokio::test]
async fn a_quiet_gap_inside_a_tool_document_does_not_truncate_or_execute_a_partial_call() {
    let tools = [super::super::types::ToolDefinition {
        name: "read".into(),
        description: "Read a workspace file".into(),
        parameters: json!({"type":"object","required":["file_path"],"properties":{"file_path":{"type":"string"}}}),
    }];
    for fenced in [false, true] {
        let document = json!({"text":"","tool_calls":[{"name":"read","arguments":{"file_path":"example.rs"}}]}).to_string();
        let document = if fenced {
            format!("```json\n{document}\n```")
        } else {
            document
        };
        let split = document.find("example.rs").unwrap() + 4;
        let response = delayed_response(document[..split].into(), document[split..].into());
        let (replies, _reply_rx) = mpsc::channel(1);
        let text = tokio::time::timeout(
            Duration::from_secs(8),
            read_run_response(
                response,
                "synthetic-secret",
                CancellationToken::new(),
                replies,
                &tools,
                GENERATION_DEADLINE,
            ),
        )
        .await
        .expect("a completed document settles after its quiet gap")
        .unwrap();
        let message = tool_protocol::message(&text.text, &tools).unwrap();
        let calls = message.tool_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].args["file_path"], "example.rs");
    }
}

#[tokio::test]
async fn cancellation_remains_observable_while_a_tool_document_is_incomplete() {
    let response = delayed_response("{\"text\":".into(), "\"still incomplete".into());
    let token = CancellationToken::new();
    let cancelled = token.clone();
    let (replies, _reply_rx) = mpsc::channel(1);
    let cancellation = tokio::spawn(async move {
        tokio::time::sleep(QUIET_AFTER_TEXT + Duration::from_millis(100)).await;
        cancelled.cancel();
    });
    let error = read_run_response(
        response,
        "synthetic-secret",
        token,
        replies,
        &read_tool(),
        GENERATION_DEADLINE,
    )
    .await
    .unwrap_err();
    cancellation.await.unwrap();
    assert_eq!(error.to_string(), "cursor account request was cancelled");
}
