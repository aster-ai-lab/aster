#![cfg(test)]

use super::*;

const MODEL: &str =
    "default_model = \"mock/m1\"\nproviders \"mock\" { api_key = \"test-only\"\nmodels \"m1\" {} }";
const HOST: &str = "tool_presentation { mode = \"code\" }\nworkspace_retrieval { enabled = false }";
const WORKSPACE: &str =
    "tool_presentation { mode = \"direct\" }\nworkspace_retrieval { enabled = true\nlocal_cpu {} }";

#[test]
fn capability_settings_are_host_owned_in_actual_layer_loading() {
    env_locked(|| {
        std::env::remove_var("A3S_DEFAULT_MODEL");
        for host in ["", HOST] {
            let (_user, workspace) =
                write_user_and_workspace_layers(&format!("{MODEL}\n{host}"), WORKSPACE);
            let (launch, admission) =
                load_launch_config_with_admission(workspace.path().into(), None).unwrap();
            let expected = CodeConfig::from_acl(host).unwrap();
            assert_eq!(
                launch.config.tool_presentation_profile,
                expected.tool_presentation_profile
            );
            assert_eq!(
                launch.config.workspace_retrieval,
                expected.workspace_retrieval
            );
            assert!(admission.withheld_blocks.contains(&"tool_presentation"));
            assert!(admission.withheld_blocks.contains(&"workspace_retrieval"));
        }
    });
}

#[test]
fn explicit_capability_config_does_not_merge_user_or_workspace_defaults() {
    env_locked(|| {
        std::env::remove_var("A3S_DEFAULT_MODEL");
        let (user, workspace) =
            write_user_and_workspace_layers(&format!("{MODEL}\n{HOST}"), WORKSPACE);
        let explicit = user.path().join("explicit.acl");
        std::fs::write(&explicit, MODEL).unwrap();
        let launch = load_launch_config(workspace.path().into(), Some(explicit)).unwrap();
        assert!(launch.config.tool_presentation_profile.is_none());
        assert!(launch.config.workspace_retrieval.is_none());
    });
}

#[test]
fn invalid_capability_config_refuses_instead_of_falling_back() {
    env_locked(|| {
        std::env::remove_var("A3S_DEFAULT_MODEL");
        for bad in [
            "tool_presentation { mode = \"automatic\" }",
            "tool_presentation { mode = \"code\", mode = \"direct\" }",
            "workspace_retrieval { enabled = true\nlocal_cpu {}\nlocal_cpu {} }",
        ] {
            let (user, workspace) = write_user_and_workspace_layers(MODEL, "");
            let explicit = user.path().join("bad.acl");
            std::fs::write(&explicit, format!("{MODEL}\n{bad}")).unwrap();
            assert!(load_launch_config(workspace.path().into(), Some(explicit)).is_err());
        }
    });
}
