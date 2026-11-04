use super::*;

#[test]
fn saved_model_and_effort_reload_together_without_rewriting_providers() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("config with spaces 中文.acl");
    let providers = "# preserve provider credentials as expressions\nproviders \"mock\" {\n  api_key = env(\"SYNTHETIC_MODEL_KEY\")\n  models \"old\" {}\n  models \"new\" {}\n}\n";
    std::fs::write(
        &path,
        format!("default_model = \"mock/old\"\nreasoning_effort = \"low\"\n{providers}"),
    )
    .unwrap();
    save_to(&path, Some("mock/new".into()), Some(PickerEffort::Max)).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains(providers), "{saved}");
    let next_process_config = CodeConfig::from_acl(&saved).unwrap();
    assert_eq!(
        next_process_config.default_model.as_deref(),
        Some("mock/new")
    );
    assert_eq!(
        next_process_config.reasoning_effort,
        Some(ReasoningEffort::Max)
    );
    save_to(&path, Some("mock/new".into()), None).unwrap();
    assert_eq!(
        CodeConfig::from_acl(&std::fs::read_to_string(&path).unwrap())
            .unwrap()
            .reasoning_effort,
        Some(ReasoningEffort::Default)
    );
}

#[test]
fn invalid_or_oversized_acl_is_preserved() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("config.acl");
    for source in [
        "providers {".to_string(),
        " ".repeat(MAX_PREFERENCE_CONFIG_BYTES as usize + 1),
    ] {
        std::fs::write(&path, &source).unwrap();
        assert!(save_to(&path, Some("mock/new".into()), Some(PickerEffort::High)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
}
