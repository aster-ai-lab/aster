#![cfg(test)]
use super::*;

#[test]
fn native_alignment_approval_http_dto_requires_the_exact_owned_tuple() {
    let body = serde_json::json!({"approved":true,"sessionId":"S","executionId":"E","anchorMessageId":"A","messageId":"M","toolCallId":"T"});
    assert!(serde_json::from_value::<ApprovalAnswer>(body.clone()).is_ok());
    for key in [
        "approved",
        "sessionId",
        "executionId",
        "anchorMessageId",
        "messageId",
        "toolCallId",
    ] {
        let mut value = body.clone();
        value.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<ApprovalAnswer>(value).is_err(),
            "missing {key}"
        );
    }
    for key in ["reason", "updatedInput", "approvalId", "foreign"] {
        let mut value = body.clone();
        value[key] = serde_json::json!("not accepted");
        assert!(
            serde_json::from_value::<ApprovalAnswer>(value).is_err(),
            "unknown {key}"
        );
    }
}
