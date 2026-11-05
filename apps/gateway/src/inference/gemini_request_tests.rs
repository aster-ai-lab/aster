use super::*;
use crate::inference::openai_request::{collect_json_body_with_model, OPENAI_REQUEST_BODY_LIMIT};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue};
use http_body_util::Full;
use serde_json::{json, Value};

#[test]
fn model_path_decodes_once_and_preserves_alias_identity() {
    for (path, expected) in [
        (
            "/v1beta/models/provider/model:generateContent",
            "provider/model",
        ),
        (
            "/v1/models/provider%2F%E4%B8%AD%2B%25:streamGenerateContent",
            "provider/中+%",
        ),
        (
            "/v1beta/models/provider:model:countTokens",
            "provider:model",
        ),
    ] {
        assert_eq!(GeminiModelCall::parse(path).unwrap().model(), expected);
    }
    for model in [
        "", "../x", "x/../y", "%2e%2e/x", "%FF", "%", "%xx", "a%00", "a%0a", "a%5Cb", "a%3Fb",
        "a%23b", "/x", "x//y", "x/",
    ] {
        assert!(
            GeminiModelCall::parse(&format!("/v1beta/models/{model}:generateContent")).is_err(),
            "{model}"
        );
    }
    assert!(GeminiModelCall::parse("/v1beta/models/m:unknown").is_err());
    assert!(GeminiModelCall::parse(&format!(
        "/v1beta/models/{}:generateContent",
        "a".repeat(256)
    ))
    .is_err());
}

#[test]
fn gemini_wire_and_endpoint_grants_roundtrip_without_aliasing_generation_to_count() {
    use crate::config::{InferenceEndpoint, InferenceProtocol};
    assert_eq!(
        "gemini".parse::<InferenceProtocol>().unwrap(),
        InferenceProtocol::Gemini
    );
    assert_eq!(
        InferenceProtocol::Gemini.authorization_value(" synthetic-key "),
        "synthetic-key"
    );
    for (name, endpoint) in [
        ("gemini-generate", InferenceEndpoint::GeminiGenerate),
        ("gemini-count-tokens", InferenceEndpoint::GeminiCountTokens),
        ("gemini-models", InferenceEndpoint::GeminiModels),
    ] {
        assert_eq!(name.parse::<InferenceEndpoint>().unwrap(), endpoint);
        assert_eq!(
            serde_json::from_value::<InferenceEndpoint>(json!(name)).unwrap(),
            endpoint
        );
        assert_eq!(serde_json::to_value(endpoint).unwrap(), json!(name));
        assert_eq!(endpoint.protocol(), InferenceProtocol::Gemini);
        assert_eq!(endpoint.is_token_count(), name == "gemini-count-tokens");
    }
    assert!("google".parse::<InferenceProtocol>().is_err());
}

#[test]
fn google_errors_keep_http_and_canonical_codes_and_stable_aster_facts() {
    for (code, status) in [
        (400, "INVALID_ARGUMENT"),
        (401, "UNAUTHENTICATED"),
        (403, "PERMISSION_DENIED"),
        (404, "NOT_FOUND"),
        (429, "RESOURCE_EXHAUSTED"),
        (500, "INTERNAL"),
        (501, "UNIMPLEMENTED"),
        (503, "UNAVAILABLE"),
        (504, "DEADLINE_EXCEEDED"),
    ] {
        let bytes = crate::inference::render_native_error(
            crate::inference::NativeWire::Gemini,
            &crate::inference::EnvelopeError {
                status: http::StatusCode::from_u16(code).unwrap(),
                message: "safe message",
                error_code: "stable.code",
            },
        );
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["error"]["status"], status);
        assert_eq!(body["error"]["message"], "safe message");
        assert_eq!(body["error"]["aster_error_code"], "stable.code");
    }
}

#[tokio::test]
async fn path_controls_routing_and_streaming_and_retains_native_parts() {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    let body = Bytes::from_static(br#"{ "model": "forbidden", "stream": false, "contents": [{"parts":[{"text":"hello","thoughtSignature":"opaque"}]}] }"#);
    let call =
        GeminiModelCall::parse("/v1beta/models/provider%2Fallowed:streamGenerateContent").unwrap();
    let request = collect_json_body_with_model(&headers, Full::new(body.clone()), Some(call))
        .await
        .unwrap();
    assert_eq!(request.model_alias(), "provider/allowed");
    assert!(request.stream_requested());
    let routed: Value =
        serde_json::from_slice(&request.routed_body("actual-api-id", None).unwrap()).unwrap();
    let mut expected: Value = serde_json::from_slice(&body).unwrap();
    expected.as_object_mut().unwrap().remove("model");
    expected.as_object_mut().unwrap().remove("stream");
    assert_eq!(routed, expected);
    assert_eq!(request.into_body(), body);
    for body in [Bytes::from_static(b"[]"), Bytes::from_static(b"{broken")] {
        let call = GeminiModelCall::parse("/v1beta/models/m:generateContent").unwrap();
        assert!(
            collect_json_body_with_model(&headers, Full::new(body), Some(call))
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn path_metadata_does_not_reduce_the_eight_mib_body_allowance() {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    let mut body = br#"{"contents":[{"parts":[{"text":""#.to_vec();
    let suffix = br#""}]}]}"#;
    body.resize(OPENAI_REQUEST_BODY_LIMIT - suffix.len(), b'x');
    body.extend_from_slice(suffix);
    let body = Bytes::from(body);
    let call = GeminiModelCall::parse("/v1beta/models/m:generateContent").unwrap();
    let request =
        collect_json_body_with_model(&headers, Full::new(body.clone()), Some(call.clone()))
            .await
            .unwrap();
    assert_eq!(
        request.routed_body("longer-upstream-id", None).unwrap(),
        body
    );
    let too_large = Bytes::from(vec![b' '; OPENAI_REQUEST_BODY_LIMIT + 1]);
    assert_eq!(
        collect_json_body_with_model(&headers, Full::new(too_large), Some(call))
            .await
            .unwrap_err(),
        OpenAiRequestError::BodyTooLarge
    );
}

#[test]
fn target_uri_escapes_the_actual_api_id_and_controls_stream_method_and_alt() {
    let call = GeminiModelCall::parse("/v1beta/models/group/route:streamGenerateContent").unwrap();
    let original = "/v1beta/models/group/route:streamGenerateContent?key=client-secret&alt=json&trace=one&trace=two%20words".parse().unwrap();
    assert_eq!(call.upstream_uri("actual/中+id", &original).unwrap().to_string(), "/v1beta/models/actual%2F%E4%B8%AD+id:streamGenerateContent?trace=one&trace=two+words&alt=sse");
    let call = GeminiModelCall::parse("/v1/models/m:generateContent").unwrap();
    assert_eq!(
        call.upstream_uri("actual", &original)
            .unwrap()
            .path_and_query()
            .unwrap()
            .as_str(),
        "/v1/models/actual:generateContent?trace=one&trace=two+words"
    );
}

#[tokio::test]
async fn full_request_counter_rewrites_its_resource_name_without_dropping_context() {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    for field in ["generateContentRequest", "generate_content_request"] {
        let input = json!({(field):{"model":"models/hidden-model","contents":[{"parts":[{"text":"中","thoughtSignature":"opaque"}]}],"systemInstruction":{"parts":[{"text":"instructions"}]},"tools":[{"functionDeclarations":[{"name":"lookup"}]}]}});
        let call = GeminiModelCall::parse("/v1beta/models/provider/allowed:countTokens").unwrap();
        let request = collect_json_body_with_model(
            &headers,
            Full::new(Bytes::from(input.to_string())),
            Some(call),
        )
        .await
        .unwrap();
        assert_eq!(request.model_alias(), "provider/allowed");
        let mut expected = input;
        expected[field]["model"] = json!("models/actual-api-id");
        let forwarded: Value =
            serde_json::from_slice(&request.routed_body("actual-api-id", None).unwrap()).unwrap();
        assert_eq!(forwarded, expected);
    }
    let call = GeminiModelCall::parse("/v1beta/models/m:countTokens").unwrap();
    let ambiguous =
        Bytes::from_static(br#"{"generateContentRequest":{},"generate_content_request":{}}"#);
    assert_eq!(
        collect_json_body_with_model(&headers, Full::new(ambiguous), Some(call))
            .await
            .unwrap_err(),
        OpenAiRequestError::InvalidBodyShape
    );
}

#[test]
fn google_credentials_are_normalized_once_and_query_secrets_are_removed() {
    for (header, query) in [
        (vec![("x-goog-api-key", "client-key")], "?alt=sse"),
        (vec![("x-api-key", "client-key")], ""),
        (vec![], "?key=client-key&trace=one&trace=two"),
        (
            vec![
                ("authorization", "Bearer client-key"),
                ("x-goog-api-key", "client-key"),
            ],
            "?key=client-key",
        ),
    ] {
        let mut headers = HeaderMap::new();
        for (name, value) in header {
            headers.append(name, value.parse().unwrap());
        }
        let mut uri = format!("/v1beta/models/m:generateContent{query}")
            .parse()
            .unwrap();
        normalize_credentials(&mut headers, &mut uri).unwrap();
        assert_eq!(headers["authorization"], "Bearer client-key");
        assert!(!headers.contains_key("x-goog-api-key"));
        assert!(!uri.to_string().contains("client-key"));
    }
}

#[test]
fn duplicate_conflicting_and_empty_credentials_are_rejected() {
    for (header, query) in [
        (vec![("x-goog-api-key", "a"), ("x-goog-api-key", "a")], ""),
        (vec![("x-goog-api-key", "a")], "?key=b"),
        (
            vec![("authorization", "Bearer a"), ("x-goog-api-key", "b")],
            "",
        ),
        (vec![], "?key=a&key=a"),
        (vec![], "?key="),
        (vec![("x-api-key", "a"), ("x-goog-api-key", "b")], ""),
        (vec![("x-api-key", "a"), ("x-api-key", "a")], ""),
        (
            vec![("authorization", "Bearer a"), ("authorization", "Bearer a")],
            "",
        ),
        (vec![], ""),
    ] {
        let mut headers = HeaderMap::new();
        for (name, value) in header {
            headers.append(name, value.parse().unwrap());
        }
        let mut uri = format!("/v1beta/models/m:generateContent{query}")
            .parse()
            .unwrap();
        assert!(normalize_credentials(&mut headers, &mut uri).is_err());
    }
}

#[test]
fn models_are_paginated_and_tokens_are_bound_to_parameters_and_catalog() {
    let models = [
        "group/一".into(),
        "provider/two".into(),
        "provider/three".into(),
    ];
    let first: Value = serde_json::from_slice(
        &models_body(&models, &"/v1beta/models?pageSize=2".parse().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(first["models"].as_array().unwrap().len(), 2);
    assert_eq!(first["models"][0]["name"], "models/group/一");
    let token = first["nextPageToken"].as_str().unwrap();
    let second: Value = serde_json::from_slice(
        &models_body(
            &models,
            &format!("/v1beta/models?pageSize=2&pageToken={token}")
                .parse()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(second["models"][0]["name"], "models/provider/three");
    assert!(second.get("nextPageToken").is_none());
    assert!(models_body(
        &models,
        &format!("/v1beta/models?pageSize=1&pageToken={token}")
            .parse()
            .unwrap()
    )
    .is_err());
    assert!(models_body(
        &models[..2],
        &format!("/v1beta/models?pageSize=2&pageToken={token}")
            .parse()
            .unwrap()
    )
    .is_err());
    for query in [
        "pageSize=-1",
        "pageSize=x",
        "pageSize=1&pageSize=2",
        "pageToken=bad",
        "other=unsupported",
        "alt=sse",
    ] {
        assert!(
            models_body(&models, &format!("/v1beta/models?{query}").parse().unwrap()).is_err(),
            "{query}"
        );
    }
    let empty: Value =
        serde_json::from_slice(&models_body(&[], &"/v1beta/models".parse().unwrap()).unwrap())
            .unwrap();
    assert_eq!(empty, json!({"models":[]}));
}
