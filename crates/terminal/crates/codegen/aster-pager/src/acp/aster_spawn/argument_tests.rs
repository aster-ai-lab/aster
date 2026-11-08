#![cfg(test)]

use std::ffi::OsString;

#[test]
fn both_stdio_entry_points_forward_the_same_model_effort_workspace_and_acl() {
    let mut config = super::AgentConfig::default();
    config.default_model_override = Some("mock/exact-model".into());
    config.reasoning_effort_override = Some(aster_shell::sampling::types::ReasoningEffort::High);
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
        Some(aster_shell::sampling::types::ReasoningEffort::Low);
    let inherited = super::sidecar_arguments(&config, cwd, None, None);
    assert_eq!(
        inherited,
        ["--cwd", "workspace with spaces"].map(OsString::from)
    );
}

#[test]
fn interactive_connect_spawn_args_include_cli_model_override() {
    // Mirrors connect(): CLI override is copied onto AgentConfig then passed
    // as the spawn model so argv carries `--model` even when ACL defaults differ.
    let mut config = super::AgentConfig::default();
    let flags_model = Some("deepseek/deepseek-flash".to_string());
    if let Some(model) = flags_model.clone() {
        config.default_model_override = Some(model);
    }
    let args = super::sidecar_arguments(
        &config,
        std::path::Path::new("/tmp/ws"),
        config.default_model_override.as_deref(),
        None,
    );
    assert!(
        args.windows(2).any(|pair| {
            pair[0] == OsString::from("--model")
                && pair[1] == OsString::from("deepseek/deepseek-flash")
        }),
        "spawn argv must include CLI --model; got {args:?}"
    );
}
