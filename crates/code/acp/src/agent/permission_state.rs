use super::*;

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
                acp::SessionMode::new("default", "Normal").description("Ask before changes"),
                acp::SessionMode::new("plan", "Plan")
                    .description("Read and plan without workspace writes"),
                acp::SessionMode::new("auto", "Auto")
                    .description("Allow known reads and ask before changes"),
                acp::SessionMode::new("yolo", "Always approve")
                    .description("Allow execution while preserving explicit denies"),
            ],
        )
    }
}

#[derive(Default)]
pub(super) struct SessionPermissions {
    pub(super) posture: PermissionPosture,
    pub(super) denies: Vec<String>,
    research_admission: bool,
}

impl SessionPermissions {
    fn policy(&self) -> a3s_code_core::permissions::PermissionPolicy {
        if self.research_admission {
            crate::host_context::research_policy(&self.denies)
        } else {
            posture_policy(self.posture.as_str(), &self.denies)
        }
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
            research_admission: state.research_admission,
        };
        next.denies.extend(extra_denies);
        self.session
            .set_permission_policy(next.policy())
            .and_then(|()| {
                self.session
                    .set_planning_mode(if posture == PermissionPosture::Plan {
                        a3s_code_core::PlanningMode::Enabled
                    } else {
                        a3s_code_core::PlanningMode::Disabled
                    })
            })
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
        live.session
            .set_permission_policy(crate::host_context::research_policy(&state.denies))
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        state.research_admission = true;
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
        self.session
            .set_permission_policy(posture_policy(state.posture.as_str(), &state.denies))
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        state.research_admission = false;
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
