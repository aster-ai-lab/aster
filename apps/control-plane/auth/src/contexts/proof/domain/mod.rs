//! Proof domain: planes, admission, attestations, and store ports.
//!
//! No HTTP framework and no SQL driver in this module.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use uuid::Uuid;

pub mod repository;
pub mod rules;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Plane {
    Member,
    User,
}

impl Plane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::User => "user",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "member" => Some(Self::Member),
            "user" => Some(Self::User),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthCode {
    InvalidCredentials,
    PlaneMismatch,
    NotAdmitted,
    Locked,
    Expired,
    Revoked,
    InvalidChallenge,
    CodeReplay,
    RefreshReplay,
    InvalidPkce,
    InvalidClient,
    InvalidRequest,
    Unauthorized,
    Storage,
}

impl AuthCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid_credentials",
            Self::PlaneMismatch => "plane_mismatch",
            Self::NotAdmitted => "not_admitted",
            Self::Locked => "locked",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
            Self::InvalidChallenge => "invalid_challenge",
            Self::CodeReplay => "code_replay",
            Self::RefreshReplay => "refresh_replay",
            Self::InvalidPkce => "invalid_pkce",
            Self::InvalidClient => "invalid_client",
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::Storage => "storage",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "invalid_credentials" => Some(Self::InvalidCredentials),
            "plane_mismatch" => Some(Self::PlaneMismatch),
            "not_admitted" => Some(Self::NotAdmitted),
            "locked" => Some(Self::Locked),
            "expired" => Some(Self::Expired),
            "revoked" | "ok" => Some(Self::Revoked),
            "invalid_challenge" => Some(Self::InvalidChallenge),
            "code_replay" => Some(Self::CodeReplay),
            "refresh_replay" => Some(Self::RefreshReplay),
            "invalid_pkce" => Some(Self::InvalidPkce),
            "invalid_client" => Some(Self::InvalidClient),
            "invalid_request" => Some(Self::InvalidRequest),
            "unauthorized" => Some(Self::Unauthorized),
            "storage" => Some(Self::Storage),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthError {
    pub code: AuthCode,
    pub message: String,
}

impl AuthError {
    pub fn storage(message: impl Into<String>) -> Self {
        Self {
            code: AuthCode::Storage,
            message: message.into(),
        }
    }

    pub(crate) fn code(code: AuthCode) -> Self {
        Self {
            code,
            message: code.as_str().to_owned(),
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code.as_str())
    }
}

impl std::error::Error for AuthError {}

#[derive(Debug, Clone)]
pub struct AdmitQuery {
    pub plane: Plane,
    pub identifier: String,
    pub org_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct Admission {
    pub subject_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub active: bool,
    pub console_login: bool,
}

impl Admission {
    pub fn unknown() -> Self {
        Self {
            subject_id: None,
            org_id: None,
            active: false,
            console_login: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubjectStatus {
    pub active: bool,
    pub console_login: bool,
    pub org_id: Option<Uuid>,
}

#[async_trait]
pub trait DirectoryAdmission: Send + Sync {
    async fn admit(&self, query: AdmitQuery) -> Result<Admission, AuthError>;
    async fn subject_status(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<SubjectStatus, AuthError>;
}

pub trait Clock: Send + Sync {
    fn unix_secs(&self) -> i64;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn unix_secs(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct ManualClock {
    now: AtomicU64,
}

impl ManualClock {
    pub fn new(now: i64) -> Self {
        Self {
            now: AtomicU64::new(now.max(0) as u64),
        }
    }

    pub fn set(&self, now: i64) {
        self.now.store(now.max(0) as u64, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn unix_secs(&self) -> i64 {
        self.now.load(Ordering::SeqCst) as i64
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    pub subject_id: Uuid,
    pub plane: Plane,
    pub org_id: Option<Uuid>,
    pub auth_time: i64,
    pub amr: Vec<String>,
}

impl Attestation {
    pub fn claim_names(&self) -> Vec<String> {
        let mut names = vec!["sub".to_owned(), "plane".to_owned()];
        if self.org_id.is_some() {
            names.push("org_id".to_owned());
        }
        names.push("auth_time".to_owned());
        names.push("amr".to_owned());
        names
    }
}

#[derive(Debug, Clone)]
pub struct IssuedSession {
    pub token: String,
    pub expires_at: i64,
    pub attestation: Attestation,
}

#[derive(Debug, Clone)]
pub struct IssuedChallenge {
    pub challenge_id: Uuid,
    pub plane: Plane,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub enum LoginOutcome {
    Session(IssuedSession),
    Challenge(IssuedChallenge),
    Denied(AuthCode),
}

#[derive(Debug, Clone)]
pub struct LoginCommand {
    pub plane: Plane,
    pub identifier: String,
    pub org_id: Option<Uuid>,
    pub password: String,
    /// Required for a usable member session. Ignored on the user plane.
    pub computer_id: Option<String>,
}

/// A member computer id is a stable token shared by every app on that machine.
pub fn normalize_computer_id(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim();
    if (8..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-'))
    {
        Some(value.to_owned())
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub enum IntrospectOutcome {
    Active(Attestation),
    Denied(AuthCode),
}

#[derive(Debug, Clone)]
pub struct ForwardAuthDecision {
    pub allowed: bool,
    pub error: Option<AuthCode>,
    pub subject_id: Option<Uuid>,
    pub plane: Option<Plane>,
    pub org_id: Option<Uuid>,
}

pub struct AuthParts {
    pub store: Arc<dyn repository::AuthStore>,
    pub directory: Arc<dyn DirectoryAdmission>,
    pub clock: Arc<dyn Clock>,
}

/// Directory that admits nobody. The process can boot and answer a stable denial.
#[derive(Debug, Default)]
pub struct UnknownDirectory;

#[async_trait]
impl DirectoryAdmission for UnknownDirectory {
    async fn admit(&self, _query: AdmitQuery) -> Result<Admission, AuthError> {
        Ok(Admission::unknown())
    }

    async fn subject_status(
        &self,
        _plane: Plane,
        _subject_id: Uuid,
    ) -> Result<SubjectStatus, AuthError> {
        Ok(SubjectStatus {
            active: false,
            console_login: false,
            org_id: None,
        })
    }
}
