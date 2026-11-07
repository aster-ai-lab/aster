//! Compatibility stub for the retired in-tree Aster OS sandbox.
//!
//! Confinement and MicroVM isolation live in external **kbox**. This crate keeps
//! the historical API surface so the TUI/shell continue to compile; every
//! enforcement entry point is a no-op and profiles resolve to [`ProfileName::Off`].

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;

static CONFIGURED_PROFILE: OnceLock<String> = OnceLock::new();
static AUTO_ALLOW_BASH: OnceLock<bool> = OnceLock::new();

/// Child-process network helpers (no-op).
pub mod child_net {
    use std::process::Command as StdCommand;
    use tokio::process::Command as TokioCommand;

    pub fn restrict_child_network(_cmd: &mut TokioCommand) {}

    pub fn restrict_child_network_std(_cmd: &mut StdCommand) {}
}

/// Built-in profile names retained for config parsing compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProfileName {
    Workspace,
    Devbox,
    ReadOnly,
    Strict,
    #[default]
    Off,
    Custom(String),
}

impl ProfileName {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Workspace => "workspace",
            Self::Devbox => "devbox",
            Self::ReadOnly => "read-only",
            Self::Strict => "strict",
            Self::Off => "off",
            Self::Custom(name) => name.as_str(),
        }
    }
}

impl std::fmt::Display for ProfileName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProfileName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value.trim().to_ascii_lowercase().as_str() {
            "workspace" => Self::Workspace,
            "devbox" => Self::Devbox,
            "read-only" | "readonly" => Self::ReadOnly,
            "strict" => Self::Strict,
            "off" | "none" | "" => Self::Off,
            other => Self::Custom(other.to_string()),
        })
    }
}

/// No conflicting sandbox config files are consulted.
pub fn sandbox_profile_conflicts(_workspace: &Path) -> Vec<String> {
    Vec::new()
}

pub fn requires_hook_write_deny(_profile: &ProfileName, _workspace: &Path) -> bool {
    false
}

pub fn requires_read_deny(_profile: &ProfileName, _workspace: &Path) -> bool {
    false
}

pub fn requires_data_write_deny(_profile: &ProfileName, _workspace: &Path) -> bool {
    false
}

pub fn is_inside_bwrap() -> bool {
    false
}

pub fn trust_bwrap_marker_for_devbox() -> bool {
    false
}

pub fn should_restrict_child_network() -> bool {
    false
}

pub fn should_auto_allow_bash() -> bool {
    *AUTO_ALLOW_BASH.get().unwrap_or(&false)
}

pub fn set_auto_allow_bash(enabled: bool) {
    let _ = AUTO_ALLOW_BASH.set(enabled);
}

pub fn set_configured_profile(name: impl Into<String>) {
    let _ = CONFIGURED_PROFILE.set(name.into());
}

pub fn configured_profile_name() -> Option<&'static str> {
    CONFIGURED_PROFILE.get().map(|value| value.as_str())
}

pub fn requested_confinement_profile() -> Option<&'static str> {
    None
}

pub fn is_active() -> bool {
    false
}

pub fn profile_name() -> Option<&'static str> {
    None
}

pub fn log_violation(_target: &str, _operation: &str) {}

pub fn flush() {}

pub fn bwrap_reexec_for_profile(
    _profile: &ProfileName,
    _workspace: &Path,
) -> anyhow::Result<Option<std::process::Command>> {
    Ok(None)
}

pub fn verify_hook_write_deny_enforced() -> Result<(), String> {
    Ok(())
}

pub fn verify_read_deny_enforced(
    _profile: &ProfileName,
    _workspace: &Path,
) -> Result<(), String> {
    Ok(())
}

pub fn verify_data_write_deny_enforced(
    _profile: &ProfileName,
    _workspace: &Path,
) -> Result<(), String> {
    Ok(())
}

/// No-op manager retained for shell startup compatibility.
#[derive(Debug)]
pub struct SandboxManager {
    profile: ProfileName,
    workspace: PathBuf,
}

impl SandboxManager {
    pub fn new(profile: ProfileName, workspace: impl Into<PathBuf>) -> Self {
        Self {
            profile,
            workspace: workspace.into(),
        }
    }

    pub fn profile(&self) -> &ProfileName {
        &self.profile
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn apply(&mut self, _workspace: &Path) -> anyhow::Result<()> {
        tracing::debug!("aster-confine stub: apply is a no-op; use external kbox for isolation");
        Ok(())
    }

    pub fn is_applied(&self) -> bool {
        false
    }

    pub fn install(&mut self) {
        // No kernel confinement to install.
    }
}
