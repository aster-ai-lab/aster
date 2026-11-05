use super::*;
use http_body_util::{BodyExt, StreamBody};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn split_body(bytes: &[u8], split: usize) -> ResponseBody {
    let frames = [&bytes[..split], &bytes[split..]]
        .into_iter()
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| Ok::<_, io::Error>(Frame::data(Bytes::copy_from_slice(bytes))))
        .collect::<Vec<_>>();
    ResponseBody::boxed(StreamBody::new(futures_util::stream::iter(frames)))
}

fn text_stream() -> String {
    [
        serde_json::json!({"id":"c1","created":42,"model":"fixture","choices":[{"index":0,"delta":{"content":"你好"},"finish_reason":null}]}).to_string(),
        serde_json::json!({"id":"c1","created":42,"model":"fixture","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}).to_string(),
        "[DONE]".into(),
    ].into_iter().map(|data| format!("data: {data}\r\n\r\n")).collect()
}

#[tokio::test]
async fn parses_done_and_utf8_at_every_transport_split() {
    for newline in ["\n", "\r\n", "\r"] {
        let stream = text_stream().replace("\r\n", newline);
        for split in 0..=stream.len() {
            let bytes = TranslatedStreamBody::to_responses(split_body(stream.as_bytes(), split))
                .collect()
                .await
                .unwrap()
                .to_bytes();
            let text = std::str::from_utf8(&bytes).unwrap();
            let events = text
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .map(|data| serde_json::from_str::<Value>(data).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event["type"] == "response.completed")
                    .count(),
                1,
                "split {split}"
            );
            assert_eq!(
                events.last().unwrap()["response"]["output"][0]["content"][0]["text"],
                "你好"
            );
            assert_eq!(events[0]["response"]["output"], serde_json::json!([]));
            let part_added = events
                .iter()
                .position(|event| event["type"] == "response.content_part.added")
                .unwrap();
            let text_delta = events
                .iter()
                .position(|event| event["type"] == "response.output_text.delta")
                .unwrap();
            assert!(part_added < text_delta);
            assert!(events
                .iter()
                .any(|event| event["type"] == "response.output_text.done"));
            assert!(events
                .iter()
                .any(|event| event["type"] == "response.content_part.done"));
            for (sequence, event) in events.iter().enumerate() {
                assert_eq!(event["sequence_number"], sequence, "split {split}");
            }
            if newline == "\n" && split == 0 {
                println!(
                    "ASTER_RESPONSES_WIRE:{}",
                    serde_json::json!({"case":"text","wire":text})
                );
            }
        }
    }
}

#[tokio::test]
async fn interleaved_tools_survive_actual_sse_framing() {
    let stream = [
        serde_json::json!({"id":"c1","model":"m","created":42,"choices":[{"index":0,"delta":{"tool_calls":[
            {"index":0,"id":"call_0","type":"function","function":{"name":"a","arguments":"{\"city\":"}},
            {"index":1,"id":"call_1","type":"function","function":{"name":"b","arguments":"{\"n\":"}},
        ]},"finish_reason":null}]}),
        serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"2}"}},{"index":0,"function":{"arguments":"\"上海\"}"}}]},"finish_reason":"tool_calls"}]}),
        serde_json::json!({"choices":[],"usage":{"prompt_tokens":13,"completion_tokens":8,"total_tokens":21,"prompt_tokens_details":{"cached_tokens":5},"completion_tokens_details":{"reasoning_tokens":3}}}),
    ].into_iter().map(|event| format!("data: {event}\n\n")).collect::<String>();
    let bytes = TranslatedStreamBody::to_responses(split_body(stream.as_bytes(), 17))
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    let terminal = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str::<Value>(data).unwrap())
        .find(|event| event["type"] == "response.completed")
        .unwrap();
    assert_eq!(terminal["response"]["output"].as_array().unwrap().len(), 2);
    assert_eq!(terminal["response"]["output"][0]["call_id"], "call_0");
    assert_eq!(
        terminal["response"]["output"][0]["arguments"],
        "{\"city\":\"上海\"}"
    );
    assert_eq!(terminal["response"]["output"][1]["call_id"], "call_1");
    assert_eq!(terminal["response"]["output"][1]["arguments"], "{\"n\":2}");
    assert_eq!(
        terminal["response"]["usage"]["input_tokens_details"]["cached_tokens"],
        5
    );
    println!(
        "ASTER_RESPONSES_WIRE:{}",
        serde_json::json!({"case":"tools","wire":text})
    );
}

#[tokio::test]
async fn malformed_and_truncated_sse_are_errors() {
    for bytes in [
        b"data: not-json\n\n".as_slice(),
        b"data: \xff\n\n".as_slice(),
        b"data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n".as_slice(),
        b"data: [DONE]\n\n".as_slice(),
    ] {
        let result = TranslatedStreamBody::to_responses(split_body(bytes, bytes.len() / 2)).collect().await;
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn multiline_data_is_decoded_as_one_event() {
    let stream = b"data: {\"id\":\"c1\",\n\
data: \"model\":\"m\",\"created\":42,\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\n\
data: [DONE]\n\n";
    let bytes = TranslatedStreamBody::to_responses(split_body(stream, 19))
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.contains("response.completed"));
    assert!(text.contains("\"text\":\"ok\""));
}

#[tokio::test]
async fn shared_anthropic_lane_waits_for_usage_after_the_finish_frame() {
    let stream = text_stream().replace("data: [DONE]", "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":13,\"completion_tokens\":8,\"total_tokens\":21}}\r\n\r\ndata: [DONE]");
    let bytes = TranslatedStreamBody::to_anthropic(split_body(stream.as_bytes(), 17))
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    let events = text
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str::<Value>(data).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "message_stop")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .find(|event| event["type"] == "message_delta")
            .unwrap()["usage"]["output_tokens"],
        8
    );
}

#[tokio::test]
async fn source_errors_keep_their_type_and_client_drop_releases_upstream() {
    let source = StreamBody::new(futures_util::stream::iter(vec![Err::<Frame<Bytes>, _>(
        io::Error::new(io::ErrorKind::ConnectionReset, "fixture"),
    )]));
    let error = TranslatedStreamBody::to_responses(ResponseBody::boxed(source))
        .collect()
        .await
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
    let dropped = Arc::new(AtomicBool::new(false));
    let source = HeldOpenBody { frame: Some(Bytes::from_static(b"data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\n")), dropped: dropped.clone() };
    let mut translated = TranslatedStreamBody::to_responses(ResponseBody::boxed(source));
    translated.frame().await.unwrap().unwrap();
    assert!(!dropped.load(Ordering::SeqCst));
    drop(translated);
    assert!(dropped.load(Ordering::SeqCst));
}

struct HeldOpenBody {
    frame: Option<Bytes>,
    dropped: Arc<AtomicBool>,
}

impl Body for HeldOpenBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        match self.frame.take() {
            Some(bytes) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
            None => Poll::Pending,
        }
    }
}

impl Drop for HeldOpenBody {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn protocol_terminal_releases_the_upstream_without_waiting_for_eof() {
    let dropped = Arc::new(AtomicBool::new(false));
    let body = ResponseBody::boxed(HeldOpenBody {
        frame: Some(Bytes::from(text_stream())),
        dropped: dropped.clone(),
    });
    let translated = TranslatedStreamBody::to_responses(body);
    let result =
        tokio::time::timeout(std::time::Duration::from_millis(200), translated.collect()).await;
    assert!(
        result.is_ok(),
        "a protocol-complete stream must not wait for upstream EOF"
    );
    assert!(result.unwrap().is_ok());
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn oversized_complete_or_partial_sse_is_an_error() {
    for suffix in ["", "\n\n"] {
        let data = serde_json::json!({"id":"c1","model":"m","choices":[{"index":0,"delta":{"content":"x".repeat(8 * 1024 * 1024 + 1)},"finish_reason":"stop"}]}).to_string();
        let input = format!("data: {data}{suffix}");
        let result =
            TranslatedStreamBody::to_responses(split_body(input.as_bytes(), input.len() / 2))
                .collect()
                .await;
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
