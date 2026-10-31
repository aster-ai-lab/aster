#![cfg(test)]

use super::*;

const FIXTURE: &str = r#"default_model = "fixture-x/model"
// Nested blocks and braces in strings must not truncate the provider.
providers "fixture-x" { apiKey = "key-{literal}" baseUrl = "https://fixture.invalid/v1"
    models "model" { name = "Fixture" }
}
memory { llm_extraction = false }
"#;

fn bound_value<'a>(value: &Value, prepared: &'a Prepared) -> Option<&'a str> {
    let Value::Call(_, args) = value else {
        return None;
    };
    prepared
        .environment
        .get(args.first()?.as_str()?)?
        .as_deref()
}

#[test]
fn nested_acl_round_trip_preserves_other_blocks_and_env_calls() {
    let prepared = prepare(FIXTURE, |_| None).expect("fixture");
    assert_eq!(prepared.api_key.as_deref(), Some("key-{literal}"));
    let rendered = aster_acl::generate(&prepared.document);
    assert!(!rendered.contains("key-{literal}"));
    let parsed = aster_acl::parse(&rendered).expect("generated ACL");
    assert!(parsed.blocks.iter().any(|block| block.name == "memory"));
    let provider = parsed
        .blocks
        .iter()
        .find(|block| block.name == "providers")
        .expect("provider");
    assert_eq!(
        bound_value(&provider.attributes["apiKey"], &prepared),
        Some("key-{literal}")
    );
    assert_eq!(
        bound_value(&provider.blocks[0].attributes["base_url"], &prepared),
        Some("https://fixture.invalid/v1")
    );
}

#[test]
fn a_selected_model_override_does_not_replace_sibling_model_credentials() {
    let source = FIXTURE.replace(
        "models \"model\" { name = \"Fixture\" }",
        "models \"model\" { api_key = \"selected-model-key\" } models \"sibling\" {}",
    );
    let prepared = prepare(&source, |_| None).unwrap();
    let provider = prepared
        .document
        .blocks
        .iter()
        .find(|block| block.name == "providers")
        .unwrap();
    assert_eq!(
        bound_value(&provider.attributes["apiKey"], &prepared),
        Some("key-{literal}")
    );
    assert_eq!(
        bound_value(&provider.blocks[0].attributes["api_key"], &prepared),
        Some("selected-model-key")
    );
    assert!(!provider.blocks[1].attributes.contains_key("api_key"));
}

#[test]
fn the_selected_model_export_cannot_replace_other_environment_references() {
    let source = format!(
        r#"{FIXTURE}
providers "other" {{
  api_key = env("ASTER_OPENAI_API_KEY")
  base_url = env("ASTER_OPENAI_BASE_URL")
  models "alternate" {{}}
}}
"#
    );
    let prepared = prepare(&source, |name| match name {
        "ASTER_OPENAI_API_KEY" => Some("other-provider-key".into()),
        "ASTER_OPENAI_BASE_URL" => Some("https://other.invalid/v1".into()),
        _ => None,
    })
    .unwrap();
    let other = prepared
        .document
        .blocks
        .iter()
        .find(|block| block.labels.first().is_some_and(|name| name == "other"))
        .unwrap();
    assert_eq!(
        bound_value(&other.attributes["api_key"], &prepared),
        Some("other-provider-key")
    );
    assert_eq!(
        bound_value(&other.attributes["base_url"], &prepared),
        Some("https://other.invalid/v1")
    );
    assert_eq!(
        prepared.environment["ASTER_OPENAI_API_KEY"].as_deref(),
        Some("other-provider-key")
    );
    let selected = &prepared
        .document
        .blocks
        .iter()
        .find(|block| block.labels.first().is_some_and(|name| name == "fixture-x"))
        .unwrap()
        .blocks[0];
    assert_eq!(
        bound_value(&selected.attributes["api_key"], &prepared),
        Some("key-{literal}")
    );
}

#[test]
fn model_overrides_and_explicit_provider_env_have_correct_precedence() {
    let text = FIXTURE.replace(
        "name = \"Fixture\"",
        "name = \"Fixture\" api_key = \"model-key\"",
    );
    let model = prepare(&text, |_| None).expect("model override");
    assert_eq!(model.api_key.as_deref(), Some("model-key"));
    let host = prepare(&text, |name| {
        (name == "ASTER_FIXTURE_X_API_KEY").then(|| "host-key".to_owned())
    })
    .expect("host override");
    assert_eq!(host.api_key.as_deref(), Some("host-key"));
}

#[test]
fn unused_providers_model_headers_and_mcp_environment_are_not_copied_to_disk() {
    let source = format!(
        r#"{FIXTURE}
providers "other" {{
  api_key = "other-bearer"
  base_url = "https://other.invalid/v1?credential=private"
  headers = {{ Authorization = "Bearer header-value" }}
  models "alternate" {{ api_key = "alternate-bearer" headers = {{ "X-Key" = "model-header" }} }}
}}
mcp_servers "fixture" {{ transport = "stdio" command = "fixture-process" env = {{ AUTH_DATA = "mcp-env-value" }} }}
"#
    );
    CodeConfig::from_acl(&source).expect("valid synthetic MCP/provider configuration");
    let prepared = prepare(&source, |_| None).expect("multi-provider fixture");
    let rendered = aster_acl::generate(&prepared.document);
    for secret in [
        "other-bearer",
        "https://other.invalid/v1?credential=private",
        "Bearer header-value",
        "alternate-bearer",
        "model-header",
        "mcp-env-value",
    ] {
        assert!(
            !rendered.contains(secret),
            "literal credential reached the ACL"
        );
        assert!(
            prepared
                .environment
                .values()
                .any(|value| value.as_deref() == Some(secret)),
            "synthetic credential was lost from the child handoff: {secret}"
        );
    }
    assert!(rendered.contains("fixture-process"));
    assert!(rendered.contains("other"));
}

#[test]
fn existing_env_names_are_forwarded_and_cannot_collide_with_generated_names() {
    let source = FIXTURE.replace(
        "name = \"Fixture\"",
        "name = \"Fixture\" headers = { A = env(\"ASTER_ACL_VALUE_2\") B = \"literal-header\" }",
    );
    let prepared = prepare(&source, |name| {
        (name == "ASTER_ACL_VALUE_2").then(|| "existing-env-value".to_owned())
    })
    .unwrap();
    assert_eq!(
        prepared
            .environment
            .get("ASTER_ACL_VALUE_2")
            .and_then(|value| value.as_deref()),
        Some("existing-env-value")
    );
    assert!(prepared
        .environment
        .values()
        .any(|value| value.as_deref() == Some("literal-header")));
    assert!(!aster_acl::generate(&prepared.document).contains("literal-header"));
}

#[test]
fn nested_credential_expressions_cannot_leave_literals_on_disk() {
    let source = format!(
        r#"{FIXTURE}
providers "unused" {{
  api_key = external_credential("nested-key", env("NESTED_KEY"))
  base_url = env(external_name("nested-name"))
  headers = {{ Authorization = external_header("nested-header", env("NESTED_HEADER", "nested-fallback")) }}
  models "alternate" {{}}
}}
"#
    );
    let prepared = prepare(&source, |name| match name {
        "NESTED_KEY" => Some("existing-nested-key".into()),
        "NESTED_HEADER" => Some("existing-nested-header".into()),
        _ => None,
    })
    .unwrap();
    let rendered = aster_acl::generate(&prepared.document);
    for secret in [
        "nested-key",
        "nested-header",
        "nested-fallback",
        "nested-name",
    ] {
        assert!(
            !rendered.contains(secret),
            "credential expression left a literal in the ACL"
        );
        assert!(prepared
            .environment
            .values()
            .any(|value| value.as_deref() == Some(secret)));
    }
    for (name, value) in [
        ("NESTED_KEY", "existing-nested-key"),
        ("NESTED_HEADER", "existing-nested-header"),
    ] {
        assert_eq!(prepared.environment[name].as_deref(), Some(value));
    }
}

#[test]
fn missing_credentials_remain_missing() {
    let text = r#"default_model = "fixture-x/model" providers "fixture-x" { models "model" {} }"#;
    let prepared = prepare(text, |_| None).expect("unconfigured fixture");
    assert!(prepared.api_key.is_none());
    assert!(prepared.base_url.is_none());
}

#[test]
fn an_unrelated_provider_environment_cannot_fill_missing_credentials() {
    let text = r#"default_model = "fixture-x/model" providers "fixture-x" { models "model" {} }"#;
    let prepared = prepare(text, |name| match name {
        "MINIMAX_API_KEY" => Some("unrelated-provider-key".into()),
        "MINIMAX_BASE_URL" => Some("https://unrelated.invalid/v1".into()),
        _ => None,
    })
    .unwrap();
    assert!(prepared.api_key.is_none());
    assert!(prepared.base_url.is_none());
}

#[test]
fn duplicate_default_provider_is_refused() {
    let duplicate = format!("{FIXTURE}\nproviders \"fixture-x\" {{ models \"model\" {{}} }}");
    assert!(prepare(&duplicate, |_| None).is_err());
}

#[test]
fn duplicate_default_model_is_refused() {
    let duplicate = FIXTURE.replace(
        "models \"model\" { name = \"Fixture\" }",
        "models \"model\" {} models \"model\" {}",
    );
    assert!(prepare(&duplicate, |_| None).is_err());
}

#[test]
fn quoted_multiline_unicode_credentials_survive_only_in_the_environment() {
    let key = "fixture-\"\\\n密钥";
    let source = FIXTURE.replace("\"key-{literal}\"", &serde_json::to_string(key).unwrap());
    let prepared = prepare(&source, |_| None).unwrap();
    let provider = prepared
        .document
        .blocks
        .iter()
        .find(|block| block.name == "providers")
        .unwrap();
    assert_eq!(
        bound_value(&provider.attributes["apiKey"], &prepared),
        Some(key)
    );
    assert_eq!(
        bound_value(&provider.blocks[0].attributes["api_key"], &prepared),
        Some(key)
    );
    assert!(!aster_acl::generate(&prepared.document).contains("密钥"));
}

#[test]
fn invalid_acl_error_does_not_echo_credentials() {
    let error = prepare("providers { api_key = \"secret-token", |_| None)
        .err()
        .expect("invalid");
    assert_eq!(error.to_string(), "invalid ACL configuration");
    assert!(Mode::parse("unknown").is_err());
    assert_eq!(Mode::parse("check").expect("mode").as_str(), "check");
}

#[test]
fn input_is_bounded_and_requires_acl_extension() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("input.acl");
    fs::write(&path, FIXTURE).expect("fixture");
    assert_eq!(read_config(&path).expect("read"), FIXTURE);
    fs::write(&path, vec![b' '; MAX_CONFIG_BYTES as usize + 1]).expect("oversized");
    assert!(read_config(&path).is_err());
    assert!(read_config(dir.path()).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_is_refused_before_open() {
    let dir = tempfile::tempdir().expect("dir");
    let real = dir.path().join("real.acl");
    let link = dir.path().join("link.acl");
    fs::write(&real, FIXTURE).expect("fixture");
    std::os::unix::fs::symlink(real, &link).expect("symlink");
    assert!(read_config(&link).is_err());
}
