//! Human proof library. The process entrypoint is `main.rs`.
//!
//! Feature `client` (default via `server`): domain DTOs + [`HttpAuthClient`].
//! Feature `server`: stores, Boot HTTP surface, argon2 — required by the binary.

mod client;
mod contexts;

#[cfg(test)]
mod architecture;

pub use client::{HttpAuthClient, ProofClient};
pub use contexts::proof::directory_contract::{
    parse_directory_http_body, DirectoryAdmitRequest, DirectoryDecision, DirectoryStatusRequest,
};
pub use contexts::proof::domain::repository::AuthStore;
pub use contexts::proof::domain::{
    Admission, AdmitQuery, Attestation, AuthCode, AuthError, AuthParts, Clock, DirectoryAdmission,
    ForwardAuthDecision, IntrospectOutcome, IssuedSession, LoginCommand, LoginOutcome, Plane,
    SubjectStatus, SystemClock, UnknownDirectory,
};

#[cfg(feature = "server")]
pub use contexts::proof::application::oidc::{decode_claims, OidcClient};
#[cfg(feature = "server")]
pub use contexts::proof::application::service::AuthService;
#[cfg(feature = "server")]
pub use contexts::proof::infrastructure::directory::HttpDirectory;
#[cfg(feature = "server")]
pub use contexts::proof::infrastructure::memory::MemoryAuthStore;
#[cfg(feature = "server")]
pub use contexts::proof::infrastructure::postgres::PostgresAuthStore;
#[cfg(feature = "server")]
pub use contexts::proof::infrastructure::sqlite::SqliteAuthStore;
#[cfg(feature = "server")]
pub use contexts::proof::interfaces::http::{
    router, serve, serve_https, serve_with, AppState, InternalGuard, ProofController,
};
