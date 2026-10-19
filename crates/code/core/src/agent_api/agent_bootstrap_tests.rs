#![cfg(test)]

use super::*;

#[test]
fn expand_home_leaves_non_home_sources_unchanged() {
    assert_eq!(expand_home("provider \"x\" {}"), "provider \"x\" {}");
}

#[test]
fn load_code_config_rejects_json_inline_config() {
    let err = load_code_config("{\"default_model\":\"x\"}".to_string()).unwrap_err();
    assert!(err.to_string().contains("JSON config is not supported"));
}

#[tokio::test]
async fn agent_bootstrap_allows_a_host_supplied_session_client() {
    build_agent_from_config(CodeConfig::default())
        .await
        .expect("bootstrap should defer model validation to the session");
}

#[tokio::test]
async fn agent_bootstrap_preserves_the_acl_tool_presentation_default() {
    for source in ["", "tool_presentation { mode = \"code\" }"] {
        let config = CodeConfig::from_acl(source).unwrap();
        let expected = config.tool_presentation_profile.clone().unwrap_or_default();
        let agent = build_agent_from_config(config).await.unwrap();
        assert_eq!(agent.config.tool_presentation_profile, expected);
    }
}
