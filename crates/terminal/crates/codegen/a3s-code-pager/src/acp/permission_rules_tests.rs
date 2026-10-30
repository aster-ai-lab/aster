use super::*;

#[test]
fn cli_permission_wire_preserves_original_rules_only_for_the_sidecar() {
    let flags = ConnectFlags {
        permission_allow_rules: vec!["Bash(cargo test:*)".into(), "Read(中文 文件.txt)".into()],
        permission_deny_rules: vec!["Write(*)".into()],
        ..Default::default()
    };
    let meta = build_initialize_meta(&flags, CapabilityTarget::Sidecar);
    assert_eq!(
        meta["permissionRules"]["allow"],
        serde_json::json!(flags.permission_allow_rules)
    );
    assert_eq!(
        meta["permissionRules"]["deny"],
        serde_json::json!(["Write(*)"])
    );
    assert!(
        build_initialize_meta(&flags, CapabilityTarget::Leader)
            .get("permissionRules")
            .is_none()
    );
    assert!(
        build_initialize_meta(&ConnectFlags::default(), CapabilityTarget::Sidecar)
            .get("permissionRules")
            .is_none()
    );
}
