#![cfg(test)]
use super::tests::{TOKEN, request, start_in_memory};
use serde_json::{Value, json};

#[test]
fn pending_form_http_route_authenticates_and_refuses_unbound_submissions() {
    let base = start_in_memory();
    let id = "a".repeat(64);
    let path = format!("/api/agent-forms/{id}");
    let body = json!({"binding":{"protocolVersion":1,"requestId":id,"revision":format!("sha256:{}", "b".repeat(64)),
        "owner":{"sessionId":"missing","executionId":"model","attemptId":1,"anchorMessageId":"assistant","acpSessionId":"acp","promptId":"prompt"}},
        "values":{"count":3,"ready":false,"topic":"Work"}}).to_string();
    assert_eq!(request(&base, "POST", &path, None, Some(&body)).0, 403);
    let (status, response) = request(&base, "POST", &path, Some(TOKEN), Some(&body));
    assert_eq!(status, 200, "{response}");
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap()["ok"],
        false
    );
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap()["requestId"],
        id
    );
    let legacy = json!({"values":{"count":3}}).to_string();
    assert_eq!(
        request(&base, "POST", &path, Some(TOKEN), Some(&legacy)).0,
        400
    );
    let mut extra: Value = serde_json::from_str(&body).unwrap();
    extra["unexpected"] = json!(true);
    assert_eq!(
        request(&base, "POST", &path, Some(TOKEN), Some(&extra.to_string())).0,
        400
    );
}
