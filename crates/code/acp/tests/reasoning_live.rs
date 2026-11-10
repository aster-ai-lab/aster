//! Opt-in provider acceptance with configured credentials and captured native controls.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use aster_core::config::{AccountImport, ProviderProtocol, ReasoningMode};
use aster_core::llm::{
    create_client_with_config, default_http_client, HttpClient, HttpResponse, Message,
    StreamingHttpResponse,
};
use async_trait::async_trait;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

struct Capture {
    inner: Arc<dyn HttpClient>,
    calls: Mutex<Vec<Value>>,
}

impl Capture {
    fn record(&self, body: &Value) {
        let mut controls = serde_json::Map::new();
        for field in [
            "model",
            "reasoning_effort",
            "reasoning",
            "output_config",
            "thinking",
            "generationConfig",
        ] {
            if let Some(value) = body.get(field) {
                controls.insert(field.into(), value.clone());
            }
        }
        self.calls.lock().unwrap().push(Value::Object(controls));
    }
}

#[async_trait]
impl HttpClient for Capture {
    async fn post(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        cancel: CancellationToken,
    ) -> Result<HttpResponse> {
        self.record(body);
        self.inner.post(url, headers, body, cancel).await
    }

    async fn post_streaming(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        cancel: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        self.record(body);
        self.inner.post_streaming(url, headers, body, cancel).await
    }
}

fn launch(workspace: &std::path::Path) -> aster_acp::LaunchConfig {
    let config = std::env::var_os("ASTER_EFFORT_LIVE_CONFIG")
        .expect("set ASTER_EFFORT_LIVE_CONFIG to an authorized ACL profile");
    aster_acp::load_launch_config_with_imports(
        workspace.into(),
        Some(config.into()),
        &[AccountImport::Cursor, AccountImport::Codex],
    )
    .unwrap()
    .0
}

#[tokio::test]
#[ignore = "uses an opted-in local profile and account model catalogs"]
async fn current_catalog_retains_capability_sources() {
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let mut counts = std::collections::BTreeMap::new();
    for (provider, model) in launch.config.list_models() {
        let controls = model
            .reasoning_capabilities(provider.model_protocol(model), provider.get_base_url(model));
        let source = controls
            .as_ref()
            .map_or("unknown", |controls| controls.source());
        *counts.entry(source).or_insert(0usize) += 1;
        if provider.resolved_protocol() == ProviderProtocol::Cursor {
            let controls = controls.expect("imported Cursor preset declaration");
            assert_eq!(controls.mode(), ReasoningMode::Preset);
            assert!(controls.levels().is_empty());
        }
    }
    assert!(!counts.is_empty());
    eprintln!("catalog entries by capability source: {counts:?}");
    for id in ["cursor/auto", "codex/gpt-6.1-sol"] {
        let (provider, model) = id.split_once('/').unwrap();
        let Some(config) = launch.config.llm_config(provider, model) else {
            eprintln!("discovery {id}: account unavailable");
            continue;
        };
        match aster_core::llm::model_discovery::discover(&config).await {
            Ok(found) => {
                let controls = found.reasoning.unwrap();
                assert_eq!(controls.source(), "account");
                if provider == "cursor" {
                    assert_eq!(controls.mode(), ReasoningMode::Preset);
                }
                eprintln!(
                    "discovery {id} source={} context={:?} mode={:?} default={:?}",
                    found.source,
                    found.context_tokens,
                    controls.mode(),
                    controls.default_level()
                );
            }
            Err(aster_core::llm::model_discovery::DiscoveryError::Unavailable)
                if provider == "cursor" =>
            {
                eprintln!("discovery {id}: catalog has no fixed limit for the auto preset");
            }
            Err(error) => panic!("{id}: {error}"),
        }
    }
}

#[tokio::test]
#[ignore = "real provider calls; spends quota for each selected model and canonical effort"]
async fn configured_native_efforts_complete() {
    let root = tempfile::tempdir().unwrap();
    let launch = launch(root.path());
    let selected = std::env::var("ASTER_EFFORT_LIVE_MODELS")
        .expect("set comma-separated ASTER_EFFORT_LIVE_MODELS");
    for id in selected.split(',') {
        let (provider_id, model_id) = id.split_once('/').expect("provider/model");
        let mut base = launch
            .config
            .llm_config(provider_id, model_id)
            .expect("configured model and credentials");
        base.reasoning_effort = None;
        match aster_core::llm::model_discovery::discover(&base).await {
            Ok(found) => {
                eprintln!(
                    "discovery {id} source={} context={:?} reasoning={}",
                    found.source,
                    found.context_tokens,
                    found
                        .reasoning
                        .as_ref()
                        .map_or("unreported", |caps| caps.source())
                );
                if let Some(controls) = found.reasoning {
                    base.reasoning_capabilities = Some(controls);
                }
            }
            Err(error) => eprintln!("discovery {id} unavailable: {error}"),
        }
        let controls = base
            .reasoning_capabilities
            .as_ref()
            .expect("known native controls");
        assert!(
            !controls.levels().is_empty(),
            "{id}: selectable native controls"
        );
        let field = match (base.protocol, controls.mode()) {
            (ProviderProtocol::Openai | ProviderProtocol::Glm, _)
            | (ProviderProtocol::Anthropic, ReasoningMode::Effort) => "/reasoning_effort",
            (
                ProviderProtocol::Anthropic,
                ReasoningMode::Adaptive | ReasoningMode::OutputConfig,
            ) => "/output_config/effort",
            (ProviderProtocol::OpenaiResponses | ProviderProtocol::Codex, _) => "/reasoning/effort",
            (ProviderProtocol::Gemini, _) => "/generationConfig/thinkingConfig/thinkingLevel",
            _ => panic!("unsupported live probe control: {id}"),
        };
        let selections =
            std::iter::once(None).chain(controls.levels().iter().map(|level| Some(level.as_str())));
        for selection in selections {
            let capture = Arc::new(Capture {
                inner: default_http_client(),
                calls: Mutex::new(Vec::new()),
            });
            let mut config = base.clone().with_http_client(capture.clone());
            if let Some(selection) = selection {
                config = config.with_reasoning_effort(selection);
            }
            let client = create_client_with_config(config);
            let response = tokio::time::timeout(
                Duration::from_secs(120),
                client.complete(
                    &[Message::user(
                        "Reply with exactly READY. Do not call tools.",
                    )],
                    None,
                    &[],
                ),
            )
            .await
            .expect("provider deadline")
            .unwrap();
            assert_eq!(
                response.text().trim(),
                "READY",
                "{id}/{}",
                selection.unwrap_or("default")
            );
            let calls = capture.calls.lock().unwrap();
            assert!(!calls.is_empty());
            for body in calls.iter() {
                let expected = selection.map(|token| {
                    if base.protocol == ProviderProtocol::Gemini {
                        token.to_uppercase()
                    } else {
                        token.to_string()
                    }
                });
                assert_eq!(
                    body.pointer(field).and_then(Value::as_str),
                    expected.as_deref(),
                    "{id}"
                );
            }
            eprintln!(
                "live {id} effort={} requests={} passed",
                selection.unwrap_or("default"),
                calls.len()
            );
        }
    }
}
