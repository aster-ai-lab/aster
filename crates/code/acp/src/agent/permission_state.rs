use super::*;

#[derive(Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ClientPermissionRules {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

impl ClientPermissionRules {
    pub(super) fn from_meta(meta: Option<&acp::Meta>) -> acp::Result<Self> {
        let Some(value) = meta.and_then(|meta| meta.get("permissionRules")) else {
            return Ok(Self::default());
        };
        if !value.is_object() {
            return Err(acp::Error::invalid_params().data("permissionRules must be an object"));
        }
        let mut rules: Self = serde_json::from_value(value.clone()).map_err(|error| {
            acp::Error::invalid_params().data(format!("permissionRules: {error}"))
        })?;
        for rule in rules.allow.iter_mut().chain(&mut rules.deny) {
            let trimmed = rule.trim();
            if trimmed.is_empty() {
                return Err(acp::Error::invalid_params()
                    .data("permissionRules must contain non-empty rules"));
            }
            *rule = trimmed.to_string();
        }
        Ok(rules)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum PermissionPosture {
    #[default]
    Default,
    Plan,
    Auto,
    Yolo,
}

impl PermissionPosture {
    pub(super) fn from_meta(meta: Option<&acp::Meta>) -> acp::Result<Option<Self>> {
        let Some(meta) = meta else { return Ok(None) };
        let flag = |key| match meta.get(key) {
            None => Ok(false),
            Some(serde_json::Value::Bool(value)) => Ok(*value),
            Some(_) => Err(acp::Error::invalid_params().data(format!("{key} must be a boolean"))),
        };
        let yolo = flag("yoloMode")?;
        let auto = flag("autoMode")?;
        if let Some(value) = meta.get("permissionPosture") {
            return value
                .as_str()
                .ok_or_else(|| {
                    acp::Error::invalid_params().data("permissionPosture must be a string")
                })
                .and_then(Self::parse)
                .map(Some);
        }
        Ok(if yolo {
            Some(Self::Yolo)
        } else if auto {
            Some(Self::Auto)
        } else if meta.contains_key("yoloMode") || meta.contains_key("autoMode") {
            Some(Self::Default)
        } else {
            None
        })
    }

    pub(super) fn parse(value: &str) -> acp::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "default" => Ok(Self::Default),
            "plan" => Ok(Self::Plan),
            "auto" => Ok(Self::Auto),
            "yolo" => Ok(Self::Yolo),
            _ => Err(acp::Error::invalid_params().data(format!(
                "unknown permission posture {value:?}; expected one of: default, plan, auto, yolo"
            ))),
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Plan => "plan",
            Self::Auto => "auto",
            Self::Yolo => "yolo",
        }
    }

    pub(super) fn modes(self) -> acp::SessionModeState {
        acp::SessionModeState::new(
            self.as_str(),
            vec![
                acp::SessionMode::new("default", "Normal").description("Allow workspace reads; ask before changes and commands; host execution requires approval"),
                acp::SessionMode::new("plan", "Plan")
                    .description("Read and plan without workspace writes"),
                acp::SessionMode::new("auto", "Auto")
                    .description("Allow workspace edits and sandboxed commands; host execution and network grants require approval"),
                acp::SessionMode::new("yolo", "Full access")
                    .description("Run local commands on the host without sandboxing or approval prompts; explicit tool denies still apply"),
            ],
        )
    }
}

#[derive(Default)]
pub(super) struct SessionPermissions {
    pub(super) posture: PermissionPosture,
    pub(super) denies: Vec<String>,
    pub(super) client_rules: ClientPermissionRules,
    research_admission: bool,
}

impl SessionPermissions {
    pub(super) fn with_client_rules(client_rules: ClientPermissionRules) -> Self {
        Self {
            client_rules,
            ..Self::default()
        }
    }

    pub(super) fn policy(&self) -> aster_core::permissions::PermissionPolicy {
        let mut policy = if self.research_admission {
            crate::host_context::research_policy(self.posture.as_str(), &self.denies)
        } else {
            posture_policy(self.posture.as_str(), &self.denies)
        };
        for rule in &self.client_rules.deny {
            policy = policy.deny(rule);
        }
        if self.posture != PermissionPosture::Plan && !self.research_admission {
            for rule in &self.client_rules.allow {
                policy = policy.allow(rule);
            }
        }
        policy
    }
}

impl LiveSession {
    pub(super) fn set_posture(
        &mut self,
        posture: PermissionPosture,
        extra_denies: Vec<String>,
    ) -> acp::Result<()> {
        let mut state = self
            .permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut next = SessionPermissions {
            posture,
            denies: state.denies.clone(),
            client_rules: state.client_rules.clone(),
            research_admission: state.research_admission,
        };
        next.denies.extend(extra_denies);
        let policy = if self.host.knowledge_compile.is_some() {
            crate::knowledge_compile::policy(&next.denies)
        } else {
            next.policy()
        };
        let planning = match (&self.host.knowledge_compile, posture) {
            _ if self.host.source_review => aster_core::PlanningMode::Disabled,
            (None, PermissionPosture::Plan) => aster_core::PlanningMode::Enabled,
            _ => aster_core::PlanningMode::Disabled,
        };
        self.session
            .set_permission_policy(policy)
            .and_then(|()| self.session.set_planning_mode(planning))
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        *self
            .asset_denies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = next.denies.clone();
        *state = next;
        Ok(())
    }
}

/// A research policy lasts only until the core freezes its run admission.
/// Mode changes during that await update the desired posture, not this ceiling.
pub(super) struct ResearchGate {
    session: Arc<AgentSession>,
    permissions: Arc<std::sync::Mutex<SessionPermissions>>,
    armed: bool,
}

impl ResearchGate {
    pub(super) fn enter(live: &LiveSession) -> acp::Result<Self> {
        let mut state = live
            .permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.research_admission = true;
        if let Err(error) = live.session.set_permission_policy(state.policy()) {
            state.research_admission = false;
            return Err(acp::Error::internal_error().data(error.to_string()));
        }
        Ok(Self {
            session: Arc::clone(&live.session),
            permissions: Arc::clone(&live.permissions),
            armed: true,
        })
    }

    pub(super) fn restore(&mut self) -> acp::Result<()> {
        if !self.armed {
            return Ok(());
        }
        let mut state = self
            .permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.research_admission = false;
        if let Err(error) = self.session.set_permission_policy(state.policy()) {
            state.research_admission = true;
            return Err(acp::Error::internal_error().data(error.to_string()));
        }
        self.armed = false;
        Ok(())
    }
}

impl Drop for ResearchGate {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            tracing::error!(%error, "research admission policy restoration failed");
        }
    }
}
