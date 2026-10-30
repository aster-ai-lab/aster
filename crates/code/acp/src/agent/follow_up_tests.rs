use super::*;

#[test]
fn follow_up_method_is_a_closed_exact_extension() {
    assert_eq!(
        HostMethod::parse("a3s/followup/generate"),
        Some(HostMethod::FollowUp)
    );
    assert_eq!(HostMethod::FollowUp.as_str(), "a3s/followup/generate");
    assert_eq!(HostMethod::parse("a3s/followup/generate/extra"), None);
}

#[tokio::test]
async fn unknown_session_cannot_generate_follow_up_questions() {
    let agent = A3sCodeAgent::new(crate::agent::tests::test_launch(None));
    let params = serde_json::value::to_raw_value(&serde_json::json!({
        "sessionId": "missing",
        "prompt": "Fix the bug",
        "response": "The bug is fixed"
    }))
    .unwrap();
    let error = agent
        .handle_follow_up(&acp::ExtRequest::new(
            "a3s/followup/generate",
            params.into(),
        ))
        .await;
    assert!(error.is_err());
}
