//! The model catalog, model selection, and reasoning-effort budgets.

use super::*;

impl AsterCodeAgent {
    pub(super) fn available_model_infos(&self) -> Vec<acp::ModelInfo> {
        let mut infos = Vec::new();
        let detected = self
            .detected_models
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (provider, model) in self.launch.config.list_models() {
            let id = format!("{}/{}", provider.name, model.id);
            let display_name = if model.name.trim().is_empty() {
                model.id.clone()
            } else {
                model.name.clone()
            };
            let name = model_picker_label(&provider.name, &display_name);
            let description = Some(model_picker_description(&provider.name));
            let mut info = acp::ModelInfo::new(acp::ModelId::new(id.clone()), name);
            if let Some(description) = description {
                info = info.description(description);
            }
            let lookup = detected.get(&id);
            let discovered = lookup.and_then(|model| model.data.as_ref());
            let baseline = model.reasoning_capabilities(
                provider.model_protocol(model),
                provider.get_base_url(model),
            );
            let controls = discovered
                .and_then(|data| data.reasoning.clone())
                .or_else(|| baseline.clone());
            let referenced_default = baseline
                .as_ref()
                .and_then(|caps| caps.default_level())
                .filter(|level| {
                    controls
                        .as_ref()
                        .is_some_and(|caps| caps.supports(level.as_str()))
                });
            let mut meta = reasoning_control_meta(controls);
            if !meta.contains_key("defaultReasoningEffort") {
                if let Some(level) = referenced_default {
                    meta.insert("defaultReasoningEffort".into(), level.as_str().into());
                    meta.insert(
                        "defaultReasoningEffortSource".into(),
                        baseline
                            .as_ref()
                            .map_or("unknown", |caps| caps.source())
                            .into(),
                    );
                }
            }
            meta.insert(
                "capabilitiesStatus".into(),
                lookup
                    .map_or("unqueried", |model| model.status.as_str())
                    .into(),
            );
            let configured = (model.limit.context > 0).then_some(model.limit.context as usize);
            let reported = discovered
                .and_then(|data| data.context_tokens)
                .map(|tokens| tokens as usize);
            let window = reported
                .or(configured)
                .or_else(|| aster_core::documented_context_tokens(&id));
            let source = if reported.is_some() {
                discovered.map_or("unknown", |data| data.source)
            } else if configured.is_some() {
                "configured"
            } else if window.is_some() {
                "documented"
            } else {
                "unknown"
            };
            meta.insert("contextWindowSource".into(), source.into());
            meta.insert("contextWindowKnown".into(), window.is_some().into());
            if let Some(window) = window {
                meta.insert("totalContextTokens".into(), window.into());
            }
            if !model.modalities.input.is_empty() {
                meta.insert(
                    "inputModalities".into(),
                    serde_json::json!(model.modalities.input),
                );
            } else if model.attachment {
                meta.insert("acceptsImages".into(), true.into());
            }
            info = info.meta(Some(meta));
            infos.push(info);
        }
        if infos.is_empty() && !self.launch.model_id.is_empty() {
            let id = self.launch.model_id.clone();
            let info = acp::ModelInfo::new(acp::ModelId::new(id.clone()), id.clone())
                .meta(Some(reasoning_control_meta(None)));
            infos.push(info);
        }
        infos
    }

    pub(super) fn model_state_with(
        &self,
        current: &str,
        effort_token: Option<&str>,
    ) -> acp::SessionModelState {
        let mut available = self.available_model_infos();
        let current_id = if available
            .iter()
            .any(|info| info.model_id.0.as_ref() == current)
        {
            current.to_string()
        } else {
            available
                .first()
                .map(|info| info.model_id.0.to_string())
                .unwrap_or_else(|| current.to_string())
        };
        if let Some(token) = effort_token.filter(|token| self.model_supports_effort(current, token))
        {
            if let Some(info) = available
                .iter_mut()
                .find(|info| info.model_id.0.as_ref() == current_id)
            {
                let mut meta = info.meta.clone().unwrap_or_default();
                meta.insert(
                    "reasoningEffort".into(),
                    serde_json::Value::String(token.to_string()),
                );
                info.meta = Some(meta);
            }
        }
        acp::SessionModelState::new(acp::ModelId::new(current_id), available)
    }

    /// The model picker as a session config option: clients on ACP schemas
    /// without `SessionModelState` (Harbor's Python SDK) select through it.
    pub(super) fn model_config_options(
        models: &acp::SessionModelState,
    ) -> Vec<acp::SessionConfigOption> {
        let options: Vec<acp::SessionConfigSelectOption> = models
            .available_models
            .iter()
            .map(|info| {
                acp::SessionConfigSelectOption::new(info.model_id.0.to_string(), info.name.clone())
            })
            .collect();
        vec![acp::SessionConfigOption::select(
            MODEL_CONFIG_ID,
            "Model",
            models.current_model_id.0.to_string(),
            options,
        )
        .category(acp::SessionConfigOptionCategory::Model)]
    }

    pub(super) fn known_model(&self, model_id: &str) -> bool {
        self.launch
            .config
            .list_models()
            .iter()
            .any(|(provider, model)| format!("{}/{}", provider.name, model.id) == model_id)
            || model_id == self.launch.model_id
    }
}

/// Effort selection for one session.
///
/// `token` is the provider effort parameter when the model has one.
/// `thinking_budget` controls provider reasoning independently of tool counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EffortLimits {
    pub(super) token: &'static str,
    pub(super) thinking_budget: usize,
    pub(super) max_parallel_tasks: usize,
    pub(super) max_continuation_turns: u32,
}

/// Advertise only native controls declared by the resolved model catalog.
pub(super) fn reasoning_control_meta(
    capabilities: Option<aster_core::config::ReasoningCapabilities>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut meta = serde_json::Map::new();
    let levels = capabilities
        .as_ref()
        .map(|caps| caps.levels())
        .unwrap_or_default();
    meta.insert(
        "supportsReasoningEffort".into(),
        (!levels.is_empty()).into(),
    );
    meta.insert(
        "reasoningControlKnown".into(),
        capabilities
            .as_ref()
            .is_some_and(|caps| caps.mode() != aster_core::config::ReasoningMode::Unknown)
            .into(),
    );
    if let Some(caps) = &capabilities {
        meta.insert("reasoningControl".into(), caps.mode().as_str().into());
        meta.insert("reasoningSource".into(), caps.source().into());
        if let Some(level) = caps.default_level() {
            meta.insert("defaultReasoningEffort".into(), level.as_str().into());
            meta.insert("defaultReasoningEffortSource".into(), caps.source().into());
        }
    }
    meta.insert("reasoningEffortReset".into(), true.into());
    meta.insert("reasoningEfforts".into(), serde_json::json!(levels.iter().map(|level| {
        let token = level.as_str();
        serde_json::json!({"value":token,"id":token,"label":token,"description":effort_level_description(token)})
    }).collect::<Vec<_>>()));
    meta
}

impl AsterCodeAgent {
    pub(super) fn model_supports_effort(&self, id: &str, token: &str) -> bool {
        if let Some(controls) = self
            .detected_models
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .and_then(|model| model.data.as_ref())
            .and_then(|data| data.reasoning.as_ref())
        {
            return controls.supports(token);
        }
        self.launch
            .config
            .list_models()
            .iter()
            .find(|(provider, model)| format!("{}/{}", provider.name, model.id) == id)
            .and_then(|(provider, model)| {
                model.reasoning_capabilities(
                    provider.model_protocol(model),
                    provider.get_base_url(model),
                )
            })
            .is_some_and(|caps| caps.supports(token))
    }

    pub(super) fn select_model_effort(
        &self,
        model: &str,
        meta: Option<&acp::Meta>,
        current: Option<EffortLimits>,
    ) -> acp::Result<Option<EffortLimits>> {
        let selected = selected_effort(meta, current)?;
        let explicit = meta
            .and_then(|meta| meta.get("reasoningEffort"))
            .is_some_and(|value| !value.is_null() && value.as_str() != Some("default"));
        if let Some(selection) = selected {
            if self.model_supports_effort(model, selection.token) {
                return Ok(Some(selection));
            }
            if explicit {
                return Err(acp::Error::invalid_params().data(format!(
                    "model {model} does not offer reasoning effort {}",
                    selection.token
                )));
            }
        }
        Ok(None)
    }
}

pub(super) fn effort_level_description(level: &str) -> &'static str {
    match level {
        "default" => "Reset reasoning effort to the model default",
        "low" => "Faster, lighter reasoning",
        "medium" => "Balanced reasoning",
        "high" => "Heavy reasoning",
        "xhigh" => "Extended reasoning",
        "max" => "Maximum reasoning",
        _ => "Reasoning effort",
    }
}

/// Depth steer from aster `BudgetProfile.guideline`. Medium has none.
pub(super) fn effort_guideline(token: &str) -> Option<&'static str> {
    match token {
        "low" => Some(
            "[effort: low] Favor speed and minimalism. Answer directly, make the smallest \
change that works (reading enough surrounding code to change it safely), and \
keep verification proportionate: still run the narrowest build/test/type-check \
that covers what you touched — just don't add checks or scope the task didn't \
warrant. Don't gold-plate.",
        ),
        "high" => Some(
            "[effort: high] Favor depth. Reason through the approach before acting. \
Depth applies to the user's outcome, not to rediscovering the running product. After \
changes, verify the narrow path you touched (build / test / type-check) and \
check the obvious edge cases, then re-read your own diff for correctness before \
finishing.",
        ),
        "xhigh" => Some(
            "[effort: xhigh] Resolve material uncertainty using authoritative inputs \
and checks for the changed behavior and its concrete failure modes. Compare an \
alternative only when the evidence exposes a relevant tradeoff. Review the diff \
for correctness and simplicity. Keep scope and acceptance criteria unchanged; \
stop once the requested outcome and required checks are complete.",
        ),
        "max" => Some(
            "[effort: max] Use the depth needed for the task's actual risk. Resolve \
uncertain assumptions before consequential changes and test the affected \
boundaries with independent evidence. Finish all requested work, including \
necessary callers and failure cases. Reopen a settled choice only for new \
evidence. Keep scope and acceptance criteria unchanged; additional reasoning, \
alternatives or checks are not a quota.",
        ),
        "ultracode" => Some(
            "[ultracode] Scripted tool fan-out is available — you decide whether a turn needs \
it. Match the effort to the task: answer trivial or conversational input (a \
greeting, a single question, a one-step edit) directly, with no plan and no \
fan-out. When a task genuinely needs many coordinated tool calls, call the \
`program` tool with one sandboxed JavaScript PTC script.",
        ),
        _ => None,
    }
}

pub(super) fn effort_limits(token: &str) -> Option<EffortLimits> {
    let (token, thinking_budget) = match token.to_ascii_lowercase().as_str() {
        "none" => ("none", 1024),
        "minimal" => ("minimal", 1536),
        "low" => ("low", 2048),
        "medium" => ("medium", 8192),
        "high" => ("high", 16_384),
        "xhigh" => ("xhigh", 32_768),
        "max" => ("max", 65_536),
        "ultracode" => ("ultracode", 65_536),
        _ => return None,
    };
    Some(EffortLimits {
        token,
        thinking_budget,
        max_parallel_tasks: HOST_PARALLEL_TASKS,
        max_continuation_turns: HOST_CONTINUATION_TURNS,
    })
}

pub(super) fn effort_limits_from_meta(meta: Option<&acp::Meta>) -> Option<EffortLimits> {
    let effort = meta
        .and_then(|m| m.get("reasoningEffort"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    effort_limits(effort)
}

/// Omission/null preserves the running selection. The desktop `default` sentinel
/// explicitly removes overrides; unsupported values must not silently retain
/// a previous (possibly expensive) selection.
pub(super) fn selected_effort(
    meta: Option<&acp::Meta>,
    current: Option<EffortLimits>,
) -> acp::Result<Option<EffortLimits>> {
    let Some(value) = meta.and_then(|meta| meta.get("reasoningEffort")) else {
        return Ok(current);
    };
    if value.is_null() {
        return Ok(current);
    }
    if value.as_str() == Some("default") {
        return Ok(None);
    }
    effort_limits_from_meta(meta).map(Some).ok_or_else(|| {
        acp::Error::invalid_params().data(
            "Unsupported reasoning effort; expected default, none, minimal, low, medium, high, xhigh, max or ultracode.",
        )
    })
}

/// Map pager `/effort` `_meta.reasoningEffort` onto the provider effort token.
/// The pager effort surface lands with its slice; kept linked.
#[allow(dead_code)]
pub(super) fn reasoning_effort_token(meta: Option<&acp::Meta>) -> Option<&'static str> {
    effort_limits_from_meta(meta).map(|limits| limits.token)
}
