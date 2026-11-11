use super::*;

const MODEL: &str = "default_model = \"host/model\"\nproviders \"host\" { protocol = \"openai\"\nbase_url = \"https://host.invalid/v1\"\nmodels \"model\" {} }";

#[test]
fn explicit_overlays_keep_workspace_authority_and_native_skill_roots() {
    env_locked(|| {
        std::env::remove_var("ASTER_DEFAULT_MODEL");
        let (user, workspace) = write_user_and_workspace_layers(
            &format!("{MODEL}\nskill_dirs = [\"/host/skills\"]\nsandbox {{ network = \"deny\" }}"),
            "skill_dirs = [\".aster/skills\", \"/untrusted-external/skills\"]\nproviders \"host\" { base_url = \"https://untrusted.invalid\" }\nsandbox { network = \"mediated\"\nnetwork_allow = [\"untrusted.invalid\"] }",
        );
        let workspace_skills = PathBuf::from(".aster/skills");
        std::fs::create_dir(workspace.path().join(&workspace_skills)).unwrap();
        let baseline = load_launch_config(workspace.path().into(), None).unwrap();
        assert_eq!(
            baseline.config.skill_dirs,
            vec![PathBuf::from("/host/skills"), workspace_skills.clone()]
        );
        let overlay = user.path().join("runtime.acl");
        std::fs::write(&overlay, "skill_dirs = [\"/runtime/skills\"]\nproviders \"runtime\" { protocol = \"openai\"\nbase_url = \"https://runtime.invalid/v1\"\nmodels \"bound\" {} }").unwrap();
        let (launch, _) =
            load_launch_config_with_overlays(workspace.path().into(), None, &[], &[overlay])
                .unwrap();
        assert_eq!(launch.model_id, "host/model");
        assert_eq!(
            launch
                .config
                .find_provider("host")
                .unwrap()
                .base_url
                .as_deref(),
            Some("https://host.invalid/v1")
        );
        assert!(launch.config.find_provider("runtime").is_some());
        assert_eq!(
            launch.config.sandbox_network,
            aster_core::sandbox::SandboxNetworkMode::Deny
        );
        assert_eq!(
            launch.config.skill_dirs,
            vec![
                PathBuf::from("/host/skills"),
                PathBuf::from("/runtime/skills"),
                workspace_skills
            ]
        );
    });
}

#[test]
fn overlays_work_without_a_base_and_do_not_change_explicit_base_discovery() {
    env_locked(|| {
        std::env::remove_var("ASTER_DEFAULT_MODEL");
        let home = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", home.path());
        let first = home.path().join("first.acl");
        let second = home.path().join("second.acl");
        std::fs::write(&first, MODEL).unwrap();
        std::fs::write(&second, "default_model = \"runtime/bound\"\nproviders \"runtime\" { protocol = \"openai\"\nmodels \"bound\" {} }").unwrap();
        let (launch, _) = load_launch_config_with_overlays(
            workspace.path().into(),
            None,
            &[],
            &[first.clone(), second.clone()],
        )
        .unwrap();
        assert_eq!(launch.model_id, "runtime/bound");
        assert!(launch.config.find_provider("host").is_some());
        std::fs::create_dir(workspace.path().join(".aster")).unwrap();
        std::fs::write(
            workspace.path().join(".aster/config.acl"),
            "skill_dirs = [\"/workspace/ignored\"]",
        )
        .unwrap();
        let (launch, _) =
            load_launch_config_with_overlays(workspace.path().into(), Some(first), &[], &[second])
                .unwrap();
        assert!(launch.config.skill_dirs.is_empty());
    });
}

#[test]
fn bad_overlay_inputs_are_refused_without_rewriting_configuration() {
    env_locked(|| {
        std::env::remove_var("ASTER_DEFAULT_MODEL");
        let (home, workspace) = write_user_and_workspace_layers(MODEL, "");
        let overlay = home.path().join("overlay.acl");
        for bytes in [
            b"invalid {".to_vec(),
            vec![b' '; MAX_ACL_LAYER_BYTES as usize + 1],
        ] {
            std::fs::write(&overlay, &bytes).unwrap();
            assert!(load_launch_config_with_overlays(
                workspace.path().into(),
                None,
                &[],
                std::slice::from_ref(&overlay)
            )
            .is_err());
            assert_eq!(std::fs::read(&overlay).unwrap(), bytes);
        }
        assert!(load_launch_config_with_overlays(
            workspace.path().into(),
            None,
            &[],
            &[home.path().join("missing.acl")]
        )
        .is_err());
        #[cfg(unix)]
        {
            std::fs::remove_file(&overlay).unwrap();
            std::os::unix::fs::symlink(home.path().join(".aster/config.acl"), &overlay).unwrap();
            assert!(load_launch_config_with_overlays(
                workspace.path().into(),
                None,
                &[],
                &[overlay]
            )
            .is_err());
        }
    });
}
