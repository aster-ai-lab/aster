use super::*;

#[test]
fn explicit_bounded_mcp_timeout_survives_acp_http_projection() {
    let server:acp::McpServer=serde_json::from_value(serde_json::json!({"type":"http","name":"fixture","url":"http://127.0.0.1:9/mcp","headers":[],"_meta":{"aster/toolTimeoutSecs":1830}})).unwrap();
    let projected = acp_mcp_to_config(&server).unwrap();
    assert_eq!(projected.tool_timeout_secs, 1830);
    assert_eq!(projected.name, "fixture");
}

#[test]
fn invalid_declared_timeouts_refuse_instead_of_falling_back() {
    for value in [
        serde_json::json!(0),
        serde_json::json!(3601),
        serde_json::json!(-1),
        serde_json::json!("1830"),
    ] {
        let server:acp::McpServer=serde_json::from_value(serde_json::json!({"type":"http","name":"fixture","url":"http://127.0.0.1:9/mcp","headers":[],"_meta":{"aster/toolTimeoutSecs":value}})).unwrap();
        assert!(acp_mcp_to_config(&server).is_none());
    }
}

#[test]
fn absent_timeout_retains_existing_sixty_second_contract() {
    let server:acp::McpServer=serde_json::from_value(serde_json::json!({"type":"http","name":"fixture","url":"http://127.0.0.1:9/mcp","headers":[]})).unwrap();
    assert_eq!(acp_mcp_to_config(&server).unwrap().tool_timeout_secs, 60);
}
