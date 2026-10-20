#![cfg(test)]

use std::ffi::OsString;

#[test]
fn both_stdio_entry_points_forward_the_same_model_effort_workspace_and_acl() {
    let mut config = super::AgentConfig::default();
    config.default_model_override = Some("mock/exact-model".into());
    config.reasoning_effort_override = Some(a3s_code_shell::sampling::types::ReasoningEffort::High);
    let cwd = std::path::Path::new("workspace with spaces");
    let acl = std::ffi::OsStr::new("config with spaces.acl");
    let interactive = super::sidecar_arguments(&config, cwd, Some("mock/exact-model"), Some(acl));
    let stdio = super::sidecar_arguments(
        &config,
        cwd,
        config.default_model_override.as_deref(),
        Some(acl),
    );
    assert_eq!(interactive, stdio);
    assert_eq!(
        stdio,
        [
            "--reasoning-effort",
            "high",
            "--cwd",
            "workspace with spaces",
            "--model",
            "mock/exact-model",
            "--config",
            "config with spaces.acl"
        ]
        .map(OsString::from)
    );
    config.reasoning_effort_override = None;
    config.models.default_reasoning_effort =
        Some(a3s_code_shell::sampling::types::ReasoningEffort::Low);
    assert_eq!(super::sidecar_arguments(&config, cwd, None, None)[1], "low");
}
