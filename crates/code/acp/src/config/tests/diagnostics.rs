#![cfg(test)]

use super::*;

#[test]
fn malformed_explicit_acl_identifies_its_actual_source_without_configuration_text() {
    env_locked(|| {
        let workspace = hermetic_workspace();
        let path = workspace.path().join("broken 配置 with spaces.acl");
        let original = b"private_diagnostic_marker = \"fixture-only-secret\"\ndefault_model = [\"unterminated\"\n";
        std::fs::write(&path, original).unwrap();
        let error = load_launch_config(workspace.path().into(), Some(path.clone())).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(message.contains("line 3"), "{message}");
        assert!(!message.contains("fixture-only-secret"), "{message}");
        assert_eq!(std::fs::read(path).unwrap(), original);
    });
}

#[test]
fn malformed_merged_acl_identifies_both_participating_sources() {
    env_locked(|| {
        let (home, workspace) =
            write_user_and_workspace_layers(MOCK_USER_ACL, "default_model = [\"unterminated\"\n");
        let error = load_launch_config(workspace.path().into(), None).unwrap_err();
        let message = format!("{error:#}");
        for root in [home.path(), workspace.path()] {
            assert!(
                message.contains(&root.join(".a3s/config.acl").display().to_string()),
                "{message}"
            );
        }
        assert!(!message.contains("user-key"), "{message}");
    });
}
