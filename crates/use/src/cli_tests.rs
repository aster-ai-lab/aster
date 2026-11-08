use super::*;

#[tokio::test]
async fn version_json_exposes_a_typed_data_payload_for_consumers() {
    let output = run(vec!["--version".to_string(), "--json".to_string()])
        .await
        .unwrap();

    assert_eq!(output.json["schemaVersion"], 1);
    assert_eq!(output.json["ok"], true);
    assert_eq!(output.json["data"]["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn capabilities_include_only_the_builtin_domains() {
    let output = run(vec!["capabilities".to_string(), "--json".to_string()])
        .await
        .unwrap();
    let domains = output.json["data"]["domains"].as_array().unwrap();
    assert!(domains.is_empty());
    assert!(output.json["data"]["externalSurfaces"]
        .as_array()
        .unwrap()
        .iter()
        .all(|surface| surface != "flow"));
    assert!(output.json["data"].get("customJsonRpc").is_none());
}

#[tokio::test]
async fn capability_snapshot_unifies_built_ins_without_rpc_envelopes() {
    let output = run(vec![
        "capability".to_string(),
        "snapshot".to_string(),
        "--json".to_string(),
    ])
    .await
    .unwrap();
    let registry = &output.json["data"]["registry"];
    let capabilities = registry["capabilities"].as_array().unwrap();
    assert!(capabilities.iter().all(|capability| !matches!(
        capability["id"].as_str(),
        Some("use/browser" | "use/ocr" | "use/box")
    )));
    assert!(capabilities
        .iter()
        .all(|capability| capability["route"] != "office"));
    assert_eq!(registry["revision"].as_str().unwrap().len(), 64);
    assert!(output.json.get("jsonrpc").is_none());
}

#[tokio::test]
async fn component_status_uses_cli_json_contract() {
    let output = run(vec![
        "component".to_string(),
        "status".to_string(),
        "browser".to_string(),
        "--json".to_string(),
    ])
    .await;
    let error = match output {
        Ok(_) => panic!("removed builtin status must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, "use.builtin_removed");
}

#[tokio::test]
async fn component_usage_lists_upgrade_as_a_supported_lifecycle_action() {
    let error = match run(vec!["component".to_string()]).await {
        Ok(_) => panic!("component without an action must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, "use.cli.invalid_usage");
    assert_eq!(
        error.message,
        "component requires list, status, install, upgrade, or uninstall"
    );
}

#[test]
fn package_offline_flag_is_explicit_and_non_repeatable() {
    let install = vec![
        "install".to_string(),
        "acme/root".to_string(),
        "--offline".to_string(),
        "--registry-name".to_string(),
        "fixture".to_string(),
    ];
    validate_component_install_options(&install).unwrap();
    assert!(flag_argument(&install, "--offline").unwrap());

    let mut duplicate = install.clone();
    duplicate.push("--offline".to_string());
    validate_component_install_options(&duplicate).unwrap();
    let error = flag_argument(&duplicate, "--offline").unwrap_err();
    assert_eq!(error.code, "use.cli.invalid_usage");
    assert_eq!(error.message, "--offline may be provided only once");

    let upgrade = vec![
        "upgrade".to_string(),
        "acme/root".to_string(),
        "--offline".to_string(),
    ];
    validate_component_upgrade_options(&upgrade).unwrap();
    assert!(flag_argument(&upgrade, "--offline").unwrap());
}

#[test]
fn package_commands_select_persisted_sources_without_inline_trust_overrides() {
    let selected = vec![
        "install".to_string(),
        "acme/root".to_string(),
        "--registry-name".to_string(),
        "packages".to_string(),
    ];
    validate_component_install_options(&selected).unwrap();

    for option in [
        "--registry-url",
        "--trust-root",
        "--trusted-root",
        "--cache-max-bytes",
    ] {
        let mut inline = selected.clone();
        inline.extend([option.to_string(), "value".to_string()]);
        let error = validate_component_install_options(&inline).unwrap_err();
        assert_eq!(error.code, "use.cli.invalid_usage");
        assert!(error.message.contains("unknown component install option"));
    }
}
