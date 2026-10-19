#![cfg(test)]

use crate::config::{rewrite_acl_sections, CodeConfig, ConfigSection, LayerAcl};
use crate::tools::{ToolPresentationModeV1, ToolPresentationProfileV1};

#[test]
fn capability_modes_and_disabled_retrieval_roundtrip_through_acl() {
    assert!(CodeConfig::from_acl("")
        .unwrap()
        .tool_presentation_profile
        .is_none());
    for mode in ["adaptive", "direct", "code", "disabled"] {
        let source = format!("tool_presentation {{ mode = \"{mode}\" }}\nworkspace_retrieval {{ enabled = false\nlocal_cpu {{}}\n}}\n");
        let config = CodeConfig::from_acl(&source).unwrap();
        let profile = config.tool_presentation_profile.as_ref().unwrap();
        assert_eq!(profile.mode(), ToolPresentationModeV1::parse(mode).unwrap());
        let rendered = rewrite_acl_sections("", &config, &[ConfigSection::Execution]).unwrap();
        let reopened = CodeConfig::from_acl(&rendered).unwrap();
        assert_eq!(
            reopened.tool_presentation_profile,
            config.tool_presentation_profile
        );
        assert_eq!(reopened.workspace_retrieval, config.workspace_retrieval);
    }
}

#[test]
fn capability_declarations_refuse_ambiguous_or_incomplete_shapes() {
    for source in [
        "tool_presentation {}",
        "tool_presentation { mode = \"CODE\" }",
        "tool_presentation { mode = \"code\", unknown = true }",
        "tool_presentation \"code\" { mode = \"code\" }",
        "tool_presentation { mode = \"code\"\n nested {} }",
        "tool_presentation { mode = \"code\", mode = \"direct\" }",
        "tool_presentation { mode = \"code\", mode: \"direct\" }",
        "tool_presentation { mode = code }",
        "tool_presentation { mode = \"code\" ]",
        "tool_presentation { mode = \"code\" stray }",
        "tool_presentation { mode = \"code\" }\ntool_presentation { mode = \"direct\" }",
        "tool_presentation { mode = \"code\"",
        "workspace_retrieval { enabled = true, enabled = false }",
        "workspace_retrieval { enabled: true, enabled = false }",
        "workspace_retrieval { enabled = true ]",
        "workspace_retrieval { enabled = true\n local_cpu {}\n local_cpu {} }",
        "workspace_retrieval { enabled = true\n local_cpu { nested {} } }",
        "workspace_retrieval { enabled = true\n local_cpu \"untrusted\" {} }",
        "workspace_retrieval { enabled = true\n local_cpu { path = \"/tmp/model\" } }",
        "workspace_retrieval { enabled = true }\nworkspace_retrieval { enabled = false }",
        "workspace_retrieval { enabled = true",
    ] {
        assert!(CodeConfig::from_acl(source).is_err(), "accepted {source}");
    }
}

#[test]
fn later_trusted_capability_layer_overrides_without_duplicating_in_one_layer() {
    let first = "tool_presentation { mode = \"code\" }\nworkspace_retrieval { enabled = true\nlocal_cpu {} }";
    let second =
        "tool_presentation { mode = \"adaptive\" }\nworkspace_retrieval { enabled = false }";
    let config = CodeConfig::from_acl_layers(&[first, second]).unwrap();
    assert_eq!(
        config.tool_presentation_profile,
        Some(ToolPresentationProfileV1::adaptive())
    );
    assert!(!config.workspace_retrieval.unwrap().enabled);
}

#[test]
fn workspace_cannot_select_profile_or_arm_local_retrieval() {
    let trusted =
        "tool_presentation { mode = \"disabled\" }\nworkspace_retrieval { enabled = false }";
    let untrusted = "tool_presentation { mode = \"direct\" }\nworkspace_retrieval { enabled = true\nlocal_cpu {} }";
    for host in ["", trusted] {
        let mut config = CodeConfig::from_acl_layers(&[host, untrusted]).unwrap();
        let withheld = crate::config::withhold_untrusted_host_authoritative_blocks(
            &mut config,
            &[
                LayerAcl {
                    trusted: true,
                    acl: host.into(),
                },
                LayerAcl {
                    trusted: false,
                    acl: untrusted.into(),
                },
            ],
        );
        assert!(withheld.contains(&"tool_presentation"));
        assert!(withheld.contains(&"workspace_retrieval"));
        let expected = CodeConfig::from_acl(host).unwrap();
        assert_eq!(
            config.tool_presentation_profile,
            expected.tool_presentation_profile
        );
        assert_eq!(config.workspace_retrieval, expected.workspace_retrieval);
    }
}
