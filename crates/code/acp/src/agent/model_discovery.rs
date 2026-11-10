//! Asynchronous selected-model discovery and the shared effective configuration.

use super::*;
use aster_core::llm::model_discovery::ModelDiscovery;

#[derive(Clone, Copy, Default)]
pub(super) enum DetectionStatus {
    Loading,
    Ready,
    Unreported,
    #[default]
    Unavailable,
}

impl DetectionStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Unreported => "unreported",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Default)]
pub(super) struct DetectedModel {
    pub(super) data: Option<ModelDiscovery>,
    pub(super) status: DetectionStatus,
}

impl AsterCodeAgent {
    pub(super) async fn refresh_model(&self, id: &str, force: bool) {
        if !self.client_model_discovery.get() {
            return;
        }
        if !force
            && self
                .detected_models
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(id)
                .is_some_and(|model| matches!(model.status, DetectionStatus::Ready))
        {
            return;
        }
        let Some((provider, model)) = id.split_once('/') else {
            return;
        };
        let Some(config) = self.launch.config.llm_config(provider, model) else {
            return;
        };
        self.detected_models
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(id.into())
            .or_default()
            .status = DetectionStatus::Loading;
        self.emit_model_catalog().await;
        let result = aster_core::llm::model_discovery::discover(&config).await;
        {
            let mut models = self
                .detected_models
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let found = models.entry(id.into()).or_default();
            match result {
                Ok(data) => {
                    found.status = if data.context_tokens.is_some() || data.reasoning.is_some() {
                        DetectionStatus::Ready
                    } else {
                        DetectionStatus::Unreported
                    };
                    found.data = Some(data);
                }
                Err(error) => {
                    found.status = DetectionStatus::Unavailable;
                    tracing::warn!(model = id, reason = %error, "model capability discovery unavailable");
                }
            }
        }
        self.emit_model_catalog().await;
    }

    pub(super) fn effective_model_config(&self, id: &str) -> aster_core::CodeConfig {
        let mut config = self.launch.config.clone();
        let models = self
            .detected_models
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(data) = models.get(id).and_then(|model| model.data.as_ref()) else {
            return config;
        };
        if let Some((provider_id, model_id)) = id.split_once('/') {
            if let Some(model) = config
                .providers
                .iter_mut()
                .find(|provider| provider.name == provider_id)
                .and_then(|provider| {
                    provider
                        .models
                        .iter_mut()
                        .find(|model| model.id == model_id)
                })
            {
                if let Some(reasoning) = &data.reasoning {
                    model.reasoning = !matches!(
                        reasoning.mode(),
                        aster_core::config::ReasoningMode::Unknown
                            | aster_core::config::ReasoningMode::Unsupported
                            | aster_core::config::ReasoningMode::Preset
                    );
                    model.reasoning_control = Some(reasoning.clone());
                }
                if let Some(context) = data.context_tokens {
                    model.limit.context = context;
                }
            }
        }
        config
    }

    pub(super) fn apply_detected_model(
        &self,
        id: &str,
        session_id: &str,
        mut options: SessionOptions,
    ) -> acp::Result<SessionOptions> {
        let data = self
            .detected_models
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .and_then(|model| model.data.clone());
        let Some(data) = data else {
            return Ok(options);
        };
        let config = self.effective_model_config(id);
        if let Some(window) = data.context_tokens {
            options = options.with_max_context_tokens(window as usize);
        }
        if let Some(reasoning) = &data.reasoning {
            options.reasoning_effort = options
                .reasoning_effort
                .filter(|token| reasoning.supports(token));
        }
        let client = aster_core::resolve_session_llm_client(&config, &options, Some(session_id))
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let client = data
            .output_tokens
            .and_then(|limit| client.with_output_cap(limit))
            .unwrap_or(client);
        Ok(options.with_llm_client(client))
    }

    pub(super) async fn emit_model_catalog(&self) {
        let Some(client) = self.client.borrow().clone() else {
            return;
        };
        let current = self.model_id.lock().await.clone();
        let effort = self.effort.lock().await.map(|effort| effort.token);
        let state = self.model_state_with(&current, effort);
        let Ok(raw) = serde_json::value::to_raw_value(&state) else {
            return;
        };
        if let Err(error) = client
            .ext_notification(acp::ExtNotification::new("aster/models/update", raw.into()))
            .await
        {
            tracing::debug!(%error, "model catalog notification unavailable");
        }
    }
}
