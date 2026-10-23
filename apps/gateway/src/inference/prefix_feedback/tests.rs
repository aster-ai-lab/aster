use super::*;
use crate::inference::prefix_affinity::PrefixAffinity;
use http_body_util::{BodyExt, Full, StreamBody};
use serde_json::json;

fn finished() -> Value {
    json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{}"}}]},"finish_reason":"tool_calls"}]})
}

fn timing(cached: u64) -> Value {
    json!({"source":"upstream-reported","usage":{"prompt_tokens":1000,"completion_tokens":8,"total_tokens":1008},
        "timings":{"cache_n":cached,"prompt_n":1000-cached,"predicted_n":8,"prompt_ms":250.0,"predicted_ms":100.0}})
}

#[test]
fn fragmented_crlf_multiline_sse_requires_finish_done_and_clean_eof() {
    let mut event = finished();
    event["prism_upstream"] = timing(900);
    let stream = format!(
        ": heartbeat\r\n\r\ndata: {}\r\n\r\ndata: [DONE]\r\n\r\n",
        event
    );
    for width in [1, 2, 7, 1024] {
        let mut observation = Observation::new(true);
        for chunk in stream.as_bytes().chunks(width) {
            observation.push(chunk);
        }
        assert!(observation.complete());
        assert_eq!(observation.timing.unwrap().cached, 900);
    }
    let mut multiline = Observation::new(true);
    multiline.push(b"data: {\"choices\":\n");
    multiline.push(b"data: [{\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n");
    assert!(multiline.complete());
}

#[test]
fn missing_terminal_errors_unknown_finish_and_oversized_observations_stay_cold() {
    for stream in [
        "data: [DONE]\n\n".to_string(),
        format!("data: {}\n\n", finished()),
        format!("data: {}\n\ndata: [DONE]\n", finished()),
        format!(
            "data: {}\n\ndata: {{\"error\":{{\"message\":\"failed\"}}}}\n\ndata: [DONE]\n\n",
            finished()
        ),
        "data: {\"choices\":[{\"finish_reason\":\"new_unsupported_reason\"}]}\n\ndata: [DONE]\n\n"
            .to_string(),
        format!("data: {}\n\ndata: [DONE]\n\ndata: {{}}\n\n", finished()),
        format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            "x".repeat(MAX_EVENT_BYTES),
            finished()
        ),
    ] {
        let mut observation = Observation::new(true);
        observation.push(stream.as_bytes());
        assert!(!observation.complete());
        assert!(observation.buffer.len() <= MAX_EVENT_BYTES);
    }
}

#[test]
fn only_consistent_upstream_extension_trains_costs() {
    assert!(parse_timing(&timing(900)).is_some());
    let mut inconsistent = timing(900);
    inconsistent["usage"]["total_tokens"] = json!(999999);
    assert!(parse_timing(&inconsistent).is_none());
    let mut observation = Observation::new(false);
    let mut value = finished();
    value["usage"] = json!({"total_tokens":999999});
    value["rawUsage"] = timing(900);
    observation.push(&serde_json::to_vec(&value).unwrap());
    assert!(observation.complete());
    assert!(observation.timing.is_none());
}

#[tokio::test]
async fn pass_through_completion_error_and_drop_own_the_lease() {
    let affinity = PrefixAffinity::default();
    let hint = affinity
        .hint(
            "scope",
            json!({"messages":[{"role":"user","content":"x".repeat(4096)}]})
                .as_object()
                .unwrap(),
        )
        .unwrap();
    let signature = [7; 32];
    let candidate = || crate::inference::PrefixTarget {
        target_id: uuid::Uuid::from_u128(1),
        signature,
        load: 0,
    };
    let now = Instant::now();
    let lease = affinity.reserve(&hint, signature, 0, now).unwrap();
    let body = track_prefix_body(
        ResponseBody::boxed(
            Full::new(Bytes::from_static(b"unfinished")).map_err(|never| match never {}),
        ),
        Some(lease),
        true,
        false,
    );
    drop(body);
    assert!(affinity
        .select(&hint, &[candidate()], now)
        .preferred
        .is_none());
    let lease = affinity.reserve(&hint, signature, 0, now).unwrap();
    let body = StreamBody::new(futures_util::stream::iter(vec![Err::<Frame<Bytes>, _>(
        io::Error::other("fixture"),
    )]));
    assert!(
        track_prefix_body(ResponseBody::boxed(body), Some(lease), true, true)
            .collect()
            .await
            .is_err()
    );
    assert!(affinity
        .select(&hint, &[candidate()], now)
        .preferred
        .is_none());
    let lease = affinity.reserve(&hint, signature, 0, now).unwrap();
    let bytes = Bytes::from(serde_json::to_vec(&finished()).unwrap());
    let body = ResponseBody::boxed(Full::new(bytes.clone()).map_err(|never| match never {}));
    assert_eq!(
        track_prefix_body(body, Some(lease), true, false)
            .collect()
            .await
            .unwrap()
            .to_bytes(),
        bytes
    );
    assert!(affinity
        .select(&hint, &[candidate()], now)
        .preferred
        .is_some());
}
