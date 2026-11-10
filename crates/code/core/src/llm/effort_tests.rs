use super::*;

#[test]
fn unknown_models_do_not_get_a_native_menu() {
    for id in [
        "relay/gpt-5.4",
        "openai/gpt-5-future",
        "cursor/gpt-5.4",
        "ollama/llama3",
        "openai/gpt-4o",
    ] {
        assert!(native_effort_menu(id).is_empty(), "{id}");
        assert!(!model_sends_native_effort(id), "{id}");
    }
    assert_eq!(
        native_effort_menu("deepseek/deepseek-flash"),
        &["none", "low", "high", "max"]
    );
}

#[test]
fn explicit_invalid_selections_fail_closed() {
    let caps =
        crate::config::documented_reasoning(crate::config::ProviderProtocol::Openai, "gpt-5.4")
            .unwrap();
    assert!(validate_selection(
        crate::config::ProviderProtocol::Openai,
        Some(&caps),
        Some("high")
    )
    .is_ok());
    assert!(validate_selection(
        crate::config::ProviderProtocol::Openai,
        Some(&caps),
        Some("max")
    )
    .is_err());
    assert!(
        validate_selection(crate::config::ProviderProtocol::Openai, None, Some("high")).is_err()
    );
    assert!(validate_selection(
        crate::config::ProviderProtocol::Openai,
        None,
        Some("default")
    )
    .is_ok());
    assert!(validate_selection(crate::config::ProviderProtocol::Openai, None, None).is_ok());
    assert_eq!(resolved_max_tokens(Some(2048), 8192, "high"), 2048);
}
