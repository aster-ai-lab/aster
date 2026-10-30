use super::super::{collect_json_body, collect_proxy_json_body};
use super::*;
use bytes::Bytes;
use http::{header::CONTENT_TYPE, HeaderMap};
use http_body_util::Full;
use serde_json::json;

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers
}

fn canonical() -> Value {
    json!({"model":"external-alias","stream":true,"temperature":0.0,
      "messages":[
       {"role":"system","content":"private instructions"},
       {"role":"user","content":[{"type":"text","text":"literal agent and messageId"},{"type":"image_url","image_url":{"url":"https://images.invalid/example.png"}}]},
       {"role":"assistant","content":null,"reasoning_content":"retained reasoning","tool_calls":[{"id":"call-1","type":"function","function":{"name":"echo","arguments":"{\"agent\":\"semantic argument\",\"usage\":7}"}}]},
       {"role":"tool","tool_call_id":"call-1","name":"echo","content":"tool output"}
      ],"metadata":{"agent":"request-level metadata retained"}})
}

fn annotated() -> Value {
    let mut input = canonical();
    // Literal client replay metadata makes the expected wire history independent
    // of the normalizer's registry and implementation.
    for (index, annotations) in [
        (
            0,
            json!({"agent":{"untrusted":true},"messageId":"replay-row"}),
        ),
        (
            1,
            json!({"conversationRequestId":"front-end-only","startsNewUserRequest":true}),
        ),
        (
            2,
            json!({"model":"ui-display","requestModelId":"ui-model-id","requestModelName":"ui-model-name","stepSeq":4,"traceId":"ui-trace","argumentsDisplayText":"ui-display-arguments","queuePosition":1,"queueTotal":2}),
        ),
        (
            3,
            json!({"rawUsage":{"untrusted":1},"usage":{"untrusted":2}}),
        ),
    ] {
        input["messages"][index]
            .as_object_mut()
            .unwrap()
            .extend(annotations.as_object().unwrap().clone());
    }
    input
}

#[tokio::test]
async fn frontend_annotations_are_normalized_once_for_every_native_model_target() {
    let input = annotated();
    let bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
    let request = collect_json_body(&headers(), Full::new(bytes))
        .await
        .unwrap();
    let expected = canonical().as_object().unwrap().clone();
    assert_eq!(request.document, expected);
    assert_eq!(
        serde_json::from_slice::<Value>(request.body_bytes()).unwrap(),
        Value::Object(expected.clone())
    );
    for model in [
        "prism-27b",
        "deepseek-v41-flash-1017-xq-1m",
        "another-provider-model",
    ] {
        let routed: Value =
            serde_json::from_slice(&request.routed_body(model, None).unwrap()).unwrap();
        assert_eq!(routed["model"], model);
        assert_eq!(routed["messages"], expected["messages"]);
        assert_eq!(routed["metadata"], expected["metadata"]);
    }
}

#[tokio::test]
async fn ordinary_proxy_uses_the_same_normalized_message_history() {
    let input = annotated();
    let bytes = Bytes::from(serde_json::to_vec(&input).unwrap());
    let native = collect_json_body(&headers(), Full::new(bytes.clone()))
        .await
        .unwrap();
    let proxy = collect_proxy_json_body(&headers(), Full::new(bytes))
        .await
        .unwrap();
    assert_eq!(proxy.into_body(), native.into_body());
}

#[test]
fn only_explicit_message_annotations_are_removed_and_normalization_is_idempotent() {
    let mut document = annotated().as_object().unwrap().clone();
    let original = document.clone();
    for key in MESSAGE_ANNOTATIONS {
        document["messages"][0][*key] = json!({"arbitrary":"client-only value"});
    }
    document["messages"][0]["provider_specific_control"] =
        json!({"must":"remain visible to backend validation"});
    assert!(normalize(&mut document));
    assert!(!normalize(&mut document));
    let messages = document["messages"].as_array().unwrap();
    for message in messages {
        for key in MESSAGE_ANNOTATIONS {
            assert!(message.get(*key).is_none());
        }
    }
    assert_eq!(
        messages[0]["provider_specific_control"],
        json!({"must":"remain visible to backend validation"})
    );
    for (index, keys) in [
        (0, vec!["role", "content"]),
        (1, vec!["role", "content"]),
        (
            2,
            vec!["role", "content", "reasoning_content", "tool_calls"],
        ),
        (3, vec!["role", "name", "content", "tool_call_id"]),
    ] {
        for key in keys {
            assert_eq!(messages[index][key], original["messages"][index][key]);
        }
    }
    assert_eq!(document["metadata"], original["metadata"]);
}

#[tokio::test]
async fn unannotated_requests_keep_original_bytes_and_unknown_fields() {
    let body=Bytes::from_static(br#"{ "model":"local", "messages":[{"role":"user","content":"hello","provider_specific_control":true}],"stream":false }"#);
    let native = collect_json_body(&headers(), Full::new(body.clone()))
        .await
        .unwrap();
    let proxy = collect_proxy_json_body(&headers(), Full::new(body.clone()))
        .await
        .unwrap();
    assert_eq!(native.into_body(), body);
    assert_eq!(proxy.into_body(), body);
}

#[test]
fn malformed_messages_are_never_repaired_or_reinterpreted() {
    for value in [
        json!(null),
        json!("history"),
        json!({"agent":"not a message list"}),
        json!([null, "row", 7]),
    ] {
        let mut doc = json!({"messages":value}).as_object().unwrap().clone();
        let original = doc.clone();
        assert!(!normalize(&mut doc));
        assert_eq!(doc, original);
    }
}

#[tokio::test]
async fn normalized_wire_body_still_obeys_the_fixed_byte_limit() {
    use super::super::{OpenAiRequestError, OPENAI_REQUEST_BODY_LIMIT};
    // 1e2 expands to 100.0 when serde emits canonical numeric values.
    let numbers = "1e2,".repeat(OPENAI_REQUEST_BODY_LIMIT / 4 - 100);
    let body = Bytes::from(format!(
        r#"{{"model":"local","messages":[{{"role":"user","content":"hi","agent":"annotation"}}],"values":[{numbers}1e2]}}"#
    ));
    assert!(body.len() < OPENAI_REQUEST_BODY_LIMIT);
    assert_eq!(
        collect_json_body(&headers(), Full::new(body.clone()))
            .await
            .unwrap_err(),
        OpenAiRequestError::BodyTooLarge
    );
    assert_eq!(
        collect_proxy_json_body(&headers(), Full::new(body))
            .await
            .unwrap_err(),
        OpenAiRequestError::BodyTooLarge
    );
}

#[tokio::test]
async fn queue_annotations_on_message_25_preserve_tool_replay_for_native_and_proxy_dispatch() {
    let mut history = Vec::new();
    for turn in 0..12 {
        history.push(json!({"role":"user","content":format!("calibration {turn}")}));
        history.push(json!({"role":"assistant","content":"acknowledged"}));
    }
    history.extend([
        json!({"role":"user","content":"inspect queue data"}),
        json!({"role":"assistant","content":null,"reasoning_content":"retained","tool_calls":[{"id":"call-queue","type":"function","function":{"name":"inspect_queue","arguments":"{\"queuePosition\":3,\"queueTotal\":9}"}}]}),
        json!({"role":"tool","tool_call_id":"call-queue","content":"semantic queuePosition and queueTotal"}),
        json!({"role":"user","content":"finish"}),
    ]);
    for stream in [false, true] {
        let expected = json!({"model":"client-alias","messages":history,"stream":stream,
            "metadata":{"queuePosition":5,"queueTotal":7}});
        let mut annotated = expected.clone();
        annotated["messages"][25]["queuePosition"] = json!(1);
        annotated["messages"][25]["queueTotal"] = json!(2);
        let bytes = Bytes::from(serde_json::to_vec(&annotated).unwrap());
        let native = collect_json_body(&headers(), Full::new(bytes.clone()))
            .await
            .unwrap();
        let proxy = collect_proxy_json_body(&headers(), Full::new(bytes))
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(native.body_bytes()).unwrap(),
            expected
        );
        assert_eq!(proxy.into_body(), native.clone().into_body());
        for target in ["deepseek-v41-flash-1017-xq-1m", "prism-27b"] {
            let routed: Value =
                serde_json::from_slice(&native.routed_body(target, None).unwrap()).unwrap();
            assert_eq!(routed["model"], target);
            assert_eq!(routed["messages"], expected["messages"]);
            assert_eq!(routed["metadata"], expected["metadata"]);
        }
    }
}
