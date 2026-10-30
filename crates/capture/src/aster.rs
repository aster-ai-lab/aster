use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

pub const ASTER_CAPTURE_SCHEMA: &str = "aster-capture-1";
pub const ASTER_DRAFT_SESSION: &str = "workbench-home-draft";
const METADATA_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingError {
    Busy,
    Cancelled,
    NotFound,
    PermissionDenied,
    InvalidTarget,
    InvalidTime,
    InvalidBundle,
    InvalidTransition,
    UnsupportedMedia,
    DestinationUnavailable,
    StaleRequest,
    ForeignSession,
    PayloadTooLarge,
    FinalizationFailed,
    ExportFailed,
    CapabilityUnavailable,
}

impl RecordingError {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "recording-busy" => Ok(Self::Busy),
            "recording-cancelled" => Ok(Self::Cancelled),
            "recording-not-found" => Ok(Self::NotFound),
            "permission-denied" => Ok(Self::PermissionDenied),
            "invalid-target" => Ok(Self::InvalidTarget),
            "invalid-time" => Ok(Self::InvalidTime),
            "invalid-bundle" => Ok(Self::InvalidBundle),
            "invalid-transition" => Ok(Self::InvalidTransition),
            "unsupported-media" => Ok(Self::UnsupportedMedia),
            "destination-unavailable" => Ok(Self::DestinationUnavailable),
            "stale-request" => Ok(Self::StaleRequest),
            "foreign-session" => Ok(Self::ForeignSession),
            "payload-too-large" => Ok(Self::PayloadTooLarge),
            "finalization-failed" => Ok(Self::FinalizationFailed),
            "export-failed" => Ok(Self::ExportFailed),
            "desktop-capability-unavailable" => Ok(Self::CapabilityUnavailable),
            _ => Err(format!("unknown recording error: {value}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Busy => "recording-busy",
            Self::Cancelled => "recording-cancelled",
            Self::NotFound => "recording-not-found",
            Self::PermissionDenied => "permission-denied",
            Self::InvalidTarget => "invalid-target",
            Self::InvalidTime => "invalid-time",
            Self::InvalidBundle => "invalid-bundle",
            Self::InvalidTransition => "invalid-transition",
            Self::UnsupportedMedia => "unsupported-media",
            Self::DestinationUnavailable => "destination-unavailable",
            Self::StaleRequest => "stale-request",
            Self::ForeignSession => "foreign-session",
            Self::PayloadTooLarge => "payload-too-large",
            Self::FinalizationFailed => "finalization-failed",
            Self::ExportFailed => "export-failed",
            Self::CapabilityUnavailable => "desktop-capability-unavailable",
        }
    }
}

impl std::fmt::Display for RecordingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for RecordingError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    Idle,
    Selecting,
    Recording,
    Finalizing,
    Review,
    Attaching,
    Exporting,
    Cancelled,
    Failed,
}

impl RecordingState {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "idle" => Ok(Self::Idle),
            "selecting" => Ok(Self::Selecting),
            "recording" => Ok(Self::Recording),
            "finalizing" => Ok(Self::Finalizing),
            "review" => Ok(Self::Review),
            "attaching" => Ok(Self::Attaching),
            "exporting" => Ok(Self::Exporting),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown recording state: {value}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Selecting => "selecting",
            Self::Recording => "recording",
            Self::Finalizing => "finalizing",
            Self::Review => "review",
            Self::Attaching => "attaching",
            Self::Exporting => "exporting",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (Self::Idle, Self::Selecting)
            | (Self::Selecting, Self::Recording | Self::Cancelled | Self::Failed)
            | (Self::Recording, Self::Finalizing | Self::Cancelled | Self::Failed)
            | (Self::Finalizing, Self::Review | Self::Cancelled | Self::Failed)
            | (Self::Review, Self::Attaching | Self::Exporting)
            | (Self::Attaching | Self::Exporting, Self::Review | Self::Failed) => true,
            _ => false,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl CaptureArea {
    pub fn validate(self) -> Result<(), RecordingError> {
        let valid = [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(|value| value.is_finite() && value.abs() <= 100_000.0)
            && self.width >= 16.0
            && self.height >= 16.0;
        if valid {
            Ok(())
        } else {
            Err(RecordingError::InvalidTarget)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureTargetKind {
    Area,
    Window,
    Screen,
    Device,
}

impl CaptureTargetKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "area" => Ok(Self::Area),
            "window" => Ok(Self::Window),
            "screen" => Ok(Self::Screen),
            "device" => Ok(Self::Device),
            _ => Err(format!("unknown capture target: {value}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Area => "area",
            Self::Window => "window",
            Self::Screen => "screen",
            Self::Device => "device",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureDestination {
    Composer,
    Recorded,
    Page,
    Localhost,
    Clipboard,
}

impl CaptureDestination {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "composer" => Ok(Self::Composer),
            "recorded" => Ok(Self::Recorded),
            "page" => Ok(Self::Page),
            "localhost" => Ok(Self::Localhost),
            "clipboard" => Ok(Self::Clipboard),
            _ => Err(format!("unknown capture destination: {value}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Composer => "composer",
            Self::Recorded => "recorded",
            Self::Page => "page",
            Self::Localhost => "localhost",
            Self::Clipboard => "clipboard",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEvidence {
    pub session_id: String,
    pub owner_key: String,
    pub bundle_path: String,
    pub contact_sheet: Vec<u8>,
    pub summary: String,
    pub derivative_revision: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AsterCaptureMetadata {
    pub schema_version: String,
    pub tenant: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_id: Option<String>,
    pub destination: CaptureDestination,
}

impl AsterCaptureMetadata {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == ASTER_CAPTURE_SCHEMA,
            "invalid-bundle"
        );
        ensure!(
            !self.tenant.trim().is_empty() && self.tenant.len() <= 64,
            "invalid-tenant"
        );
        ensure!(
            !self.session_id.trim().is_empty() && self.session_id.len() <= 128,
            "invalid-session"
        );
        ensure!(
            self.draft_id
                .as_deref()
                .is_none_or(|value| !value.trim().is_empty() && value.len() <= 128),
            "invalid-draft"
        );
        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .with_context(|| format!("invalid-bundle: open {}", path.display()))?
            .take(METADATA_LIMIT + 1)
            .read_to_end(&mut bytes)
            .with_context(|| format!("invalid-bundle: read {}", path.display()))?;
        ensure!(bytes.len() <= METADATA_LIMIT as usize, "payload-too-large");
        let metadata: Self = serde_json::from_slice(&bytes).context("invalid-bundle")?;
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn write_new(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).context("invalid-bundle")?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| format!("destination-unavailable: create {}", path.display()))?;
        std::io::Write::write_all(&mut file, &bytes).context("destination-unavailable")?;
        Ok(())
    }

    pub fn bind_session(&mut self, path: &Path, tenant: &str, session_id: &str) -> Result<()> {
        ensure!(self.tenant == tenant, "foreign-session");
        if self.session_id == session_id {
            return Ok(());
        }
        ensure!(self.session_id == ASTER_DRAFT_SESSION, "foreign-session");
        let mut bound = self.clone();
        bound
            .draft_id
            .get_or_insert_with(|| ASTER_DRAFT_SESSION.into());
        bound.session_id = session_id.to_owned();
        bound.validate()?;
        let bytes = serde_json::to_vec_pretty(&bound).context("invalid-bundle")?;
        std::fs::write(path, bytes).context("destination-unavailable")?;
        *self = bound;
        Ok(())
    }
}

#[cfg(test)]
#[path = "aster_tests.rs"]
mod tests;
