//! The existing local ACP effort contract, independent of provider/model names.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReasoningEffort {
    Default,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl ReasoningEffort {
    pub(super) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "default" => Ok(Self::Default),
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::Xhigh),
            "max" => Ok(Self::Max),
            _ => Err("Unsupported local reasoning effort; expected default, none, minimal, low, medium, high, xhigh or max.".into()),
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }

    pub(super) fn from_body(body: &Value) -> Result<Option<Self>, PlaneError> {
        match body.get("reasoningEffort") {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(value)) => Self::parse(value).map(Some).map_err(PlaneError::bad),
            Some(_) => Err(PlaneError::bad(
                "Invalid local reasoning effort: expected a string.",
            )),
        }
    }
}

/// A model/effort replacement resets ACP posture. A failed request leaves
/// caches unknown; only accepted settings can suppress a later request.
pub(super) async fn apply_turn_settings(
    runtime: &SessionRuntime,
    effort: Option<ReasoningEffort>,
    permission_mode: &str,
) -> Result<(), String> {
    if let Some(wanted) = effort {
        let mut current = runtime.reasoning_effort.lock().await;
        if *current != Some(wanted) {
            let (provider, model) = split_model_id(&runtime.model)
                .ok_or_else(|| "The session model is not providerId::modelId.".to_owned())?;
            let changed = runtime
                .client
                .request(
                    "session/set_model",
                    json!({
                        "sessionId": runtime.acp_session,
                        "modelId": format!("{provider}/{model}"),
                        "_meta": {"reasoningEffort": wanted.as_str()},
                    }),
                )
                .await;
            runtime.mode.lock().await.clear();
            match changed {
                Ok(_) => *current = Some(wanted),
                Err(error) => {
                    *current = None;
                    return Err(format!(
                        "Could not apply reasoning effort, so this turn was not started. {error}"
                    ));
                }
            }
        }
    }
    apply_mode(runtime, permission_mode).await
}
