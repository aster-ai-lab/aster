use super::{WorkspaceGate, workspace_command_gate};

#[test]
fn unknown_gate_has_local_opt_in_and_no_auth_diagnosis() {
    let error = workspace_command_gate(None, None)
        .require_enabled()
        .expect_err("unknown gate must refuse activation")
        .to_string();
    assert!(error.contains("feature gate"), "{error}");
    assert!(error.contains("GROK_WORKSPACE_COMMAND=1"), "{error}");
    assert!(!error.contains("network"), "{error}");
    assert!(!error.contains("login"), "{error}");
}

#[test]
fn activation_requires_an_enabled_gate() {
    for (gate, expected) in [
        (WorkspaceGate::Enabled, true),
        (WorkspaceGate::Disabled, false),
        (WorkspaceGate::Unknown, false),
    ] {
        assert_eq!(gate.require_enabled().is_ok(), expected);
    }
}
