use super::*;

#[test]
fn cli_permission_headless_initialize_carries_the_same_compact_rules() {
    let request = build_headless_init_request(
        None,
        None,
        &["Bash(cargo test:*)".into()],
        &["Write(*)".into()],
    );
    let meta = request.meta.unwrap();
    assert_eq!(
        meta["permissionRules"],
        serde_json::json!({"allow":["Bash(cargo test:*)"],"deny":["Write(*)"]})
    );
    assert_eq!(meta["startupHints"]["nonInteractive"], true);
    assert!(
        build_headless_init_request(None, None, &[], &[])
            .meta
            .unwrap()
            .get("permissionRules")
            .is_none()
    );
}
