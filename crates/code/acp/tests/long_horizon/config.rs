#![cfg(test)]

use a3s_code_core::config::{
    rewrite_acl_sections, CodeConfig, ConfigSection, ModelConfig, ProviderConfig,
};
use std::collections::HashMap;
use std::path::Path;

pub const RELAY_KEY_ENV: &str = "A3S_LONG_HORIZON_RELAY_KEY";

pub fn with_rust_verification(source: &str, workspace: &Path) -> Result<String, String> {
    let mut document = a3s_acl::parse_acl(source).map_err(|_| "fixture ACL parse")?;
    document.blocks.push(a3s_acl::Block {
        name: "verification_contract".into(),
        labels: vec!["rust-local-tests".into()],
        blocks: Vec::new(),
        attributes: [
            (
                "workspace".into(),
                a3s_acl::Value::String(workspace.to_str().ok_or("workspace encoding")?.into()),
            ),
            (
                "command".into(),
                a3s_acl::Value::String(super::fixture::LOCAL_CHECK.into()),
            ),
            (
                "covers".into(),
                a3s_acl::Value::List(
                    [
                        "input.rs",
                        "calc.rs",
                        "display.rs",
                        "CHANGELOG.md",
                        "checks",
                    ]
                    .into_iter()
                    .map(|name| a3s_acl::Value::String(name.into()))
                    .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    });
    let source = a3s_acl::generate_acl(&document);
    CodeConfig::from_acl(&source).map_err(|_| "fixture verification contract validation")?;
    Ok(source)
}

/// Build the child's ACL from an allowlisted set of public model capabilities.
/// User account imports, skill roots and upstream credentials never cross here.
pub fn isolated_acl(
    provider: &ProviderConfig,
    model: &ModelConfig,
    local_endpoint: &str,
    state: &Path,
    max_tool_rounds: usize,
) -> Result<String, String> {
    let endpoint = url::Url::parse(local_endpoint).map_err(|_| "invalid local relay URL")?;
    if endpoint.scheme() != "http"
        || endpoint.host_str() != Some("127.0.0.1")
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.path() != "/v1/chat/completions"
    {
        return Err("relay must be an exact loopback chat-completions endpoint".into());
    }
    if !state.is_absolute() {
        return Err("isolated state root must be absolute".into());
    }
    let mut model = model.clone();
    model.api_key = None;
    model.base_url = None;
    model.headers.clear();
    model.session_id_header = None;
    let config = CodeConfig {
        default_model: Some(format!("{}/{}", provider.name, model.id)),
        providers: vec![ProviderConfig {
            name: provider.name.clone(),
            api_key: Some("fixture-placeholder-replaced-by-env-reference".into()),
            base_url: Some(local_endpoint.into()),
            headers: HashMap::new(),
            session_id_header: None,
            models: vec![model],
        }],
        memory_dir: Some(state.join("memory")),
        sessions_dir: Some(state.join("sessions")),
        user_instructions_dir: Some(state.join("instructions")),
        max_tool_rounds: Some(max_tool_rounds),
        ..CodeConfig::default()
    };
    let source = rewrite_acl_sections(
        "",
        &config,
        &[
            ConfigSection::DefaultModel,
            ConfigSection::Providers,
            ConfigSection::ModelRuntime,
            ConfigSection::Execution,
            ConfigSection::Storage,
            ConfigSection::Memory,
        ],
    )
    .map_err(|_| "isolated fixture ACL failed validation")?;
    // The ACL editor has no personal-instructions section; add its typed
    // assignment through the ACL generator, not Value's unquoted Display.
    let mut document =
        a3s_acl::parse_acl(&source).map_err(|_| "isolated fixture ACL failed validation")?;
    document
        .blocks
        .iter_mut()
        .find(|block| block.name == "providers")
        .ok_or("isolated provider declaration is missing")?
        .attributes
        .insert(
            "api_key".into(),
            a3s_acl::Value::Call(
                "env".into(),
                vec![a3s_acl::Value::String(RELAY_KEY_ENV.into())],
            ),
        );
    document.blocks.push(a3s_acl::Block {
        name: "user_instructions_dir".into(),
        labels: Vec::new(),
        blocks: Vec::new(),
        attributes: [(
            "user_instructions_dir".into(),
            a3s_acl::Value::String(
                state
                    .join("instructions")
                    .to_str()
                    .ok_or("isolated state path must be UTF-8")?
                    .to_owned(),
            ),
        )]
        .into_iter()
        .collect(),
    });
    let source = a3s_acl::generate_acl(&document);
    CodeConfig::from_acl(&source).map_err(|_| "isolated fixture ACL failed validation")?;
    Ok(source)
}
