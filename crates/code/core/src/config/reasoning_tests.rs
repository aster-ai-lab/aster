use super::*;
use crate::config::{rewrite_acl_sections, ConfigSection};
use crate::CodeConfig;
use serde_json::json;

#[test]
fn declarations_validate_native_values() {
    for value in [
        json!({"mode":"effort","levels":[]}),
        json!({"mode":"effort","levels":["default"]}),
        json!({"mode":"effort","levels":["ultracode"]}),
        json!({"mode":"effort","levels":["high","high"]}),
        json!({"mode":"budget","levels":["high"]}),
        json!({"mode":"adaptive","levels":["turbo"]}),
        json!({"mode":"effort","levels":["high"],"typo":true}),
    ] {
        assert!(
            serde_json::from_value::<ReasoningCapabilities>(value.clone()).is_err(),
            "{value}"
        );
    }
    for mode in [
        ReasoningMode::Unknown,
        ReasoningMode::Unsupported,
        ReasoningMode::Budget,
        ReasoningMode::Preset,
    ] {
        let caps = ReasoningCapabilities::new(mode, vec![]).unwrap();
        assert!(!caps.supports("high"));
    }
}

#[test]
fn acl_controls_survive_provider_rewrite() {
    let source = r#"default_model = "relay/custom"
providers "relay" {
    protocol = "openai"
    api_key = "fixture"
    models "custom" {
        reasoning = true
        reasoning_control = { mode = "effort", levels = ["low", "high"], default_level = "high" }
    }
}
"#;
    let config = CodeConfig::from_acl(source).unwrap();
    let caps = config.providers[0].models[0]
        .reasoning_capabilities(ProviderProtocol::Openai, None)
        .unwrap();
    assert_eq!(
        caps.levels(),
        &[ReasoningEffort::Low, ReasoningEffort::High]
    );
    assert_eq!(caps.source(), "declared");
    assert_eq!(caps.default_level(), Some(ReasoningEffort::High));
    assert!(caps.validate_effort("max").is_err());
    let rewritten = rewrite_acl_sections(source, &config, &[ConfigSection::Providers]).unwrap();
    let restored = CodeConfig::from_acl(&rewritten).unwrap();
    assert_eq!(
        restored.providers[0].models[0].reasoning_control,
        config.providers[0].models[0].reasoning_control
    );
    assert_eq!(
        config.default_llm_config().unwrap().reasoning_capabilities,
        Some(caps)
    );
}

#[test]
fn unknown_is_distinct_from_unsupported_and_presets() {
    assert!(documented_reasoning(ProviderProtocol::Openai, "gpt-5-future").is_none());
    assert!(documented_reasoning(ProviderProtocol::Cursor, "gpt-5.4-high").is_none());
    assert_eq!(
        documented_reasoning(ProviderProtocol::Openai, "gpt-4o")
            .unwrap()
            .mode(),
        ReasoningMode::Unsupported
    );
    assert_eq!(
        ReasoningCapabilities::preset().mode(),
        ReasoningMode::Preset
    );
    let caps = documented_reasoning(ProviderProtocol::Anthropic, "glm-5.3").unwrap();
    assert_eq!(
        caps.levels(),
        &[
            ReasoningEffort::Low,
            ReasoningEffort::High,
            ReasoningEffort::Max
        ]
    );
    assert_eq!(caps.source(), "documented");
}
