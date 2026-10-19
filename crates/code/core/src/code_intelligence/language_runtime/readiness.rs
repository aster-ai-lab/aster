//! The built-in rust-analyzer profile's explicit semantic readiness contract.
//!
//! This is a product qualification policy, not a guarantee that an arbitrary
//! semantic query must return a location. Other language servers do not opt in.

use serde::Deserialize;
use serde_json::Value;
use tokio::{sync::watch, time::Instant};
use tokio_util::sync::CancellationToken;

use super::{LanguageRuntimeError, LspProcess};
use crate::code_intelligence::lsp::process::LspProcessState;

pub(super) const SERVER_STATUS: &str = "experimental/serverStatus";

#[derive(Clone, Copy, Debug)]
pub(super) enum ReadinessState {
    Loading,
    Ready,
    Failed(&'static str),
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Health {
    Ok,
    Warning,
    Error,
}

#[derive(Deserialize)]
struct ServerStatus {
    health: Health,
    quiescent: bool,
}

pub(super) fn observe(params: Option<Value>) -> ReadinessState {
    let Some(Ok(status)) = params.map(serde_json::from_value::<ServerStatus>) else {
        return ReadinessState::Failed("invalid rust-analyzer server status");
    };
    match status.health {
        Health::Warning => ReadinessState::Failed("rust-analyzer reported warning health"),
        Health::Error => ReadinessState::Failed("rust-analyzer reported error health"),
        Health::Ok if status.quiescent => ReadinessState::Ready,
        Health::Ok => ReadinessState::Loading,
    }
}

pub(super) async fn wait(
    status: &watch::Receiver<ReadinessState>,
    process: &LspProcess,
    cancellation: &CancellationToken,
    deadline: Instant,
    timeout: std::time::Duration,
) -> Result<(), LanguageRuntimeError> {
    wait_for_state(
        status.clone(),
        process.subscribe_state(),
        cancellation,
        deadline,
        timeout,
    )
    .await
}

pub(super) async fn wait_for_state(
    mut status: watch::Receiver<ReadinessState>,
    mut process: watch::Receiver<LspProcessState>,
    cancellation: &CancellationToken,
    deadline: Instant,
    timeout: std::time::Duration,
) -> Result<(), LanguageRuntimeError> {
    loop {
        if cancellation.is_cancelled() {
            return Err(LanguageRuntimeError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(LanguageRuntimeError::Timeout {
                operation: "semantic_readiness",
                duration: timeout,
            });
        }
        if process.has_changed().is_err() || status.has_changed().is_err() {
            return Err(LanguageRuntimeError::Readiness {
                reason: "rust-analyzer readiness stream closed",
            });
        }
        if !matches!(&*process.borrow_and_update(), LspProcessState::Running) {
            return Err(LanguageRuntimeError::Readiness {
                reason: "rust-analyzer exited before semantic readiness",
            });
        }
        match *status.borrow_and_update() {
            ReadinessState::Ready => return Ok(()),
            ReadinessState::Failed(reason) => {
                return Err(LanguageRuntimeError::Readiness { reason })
            }
            ReadinessState::Loading => {}
        }
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(LanguageRuntimeError::Cancelled),
            result = process.changed() => if result.is_err() {
                return Err(LanguageRuntimeError::Readiness { reason: "rust-analyzer process status stream closed" });
            },
            _ = tokio::time::sleep_until(deadline) => return Err(LanguageRuntimeError::Timeout {
                operation: "semantic_readiness", duration: timeout,
            }),
            result = status.changed() => if result.is_err() {
                return Err(LanguageRuntimeError::Readiness { reason: "rust-analyzer readiness stream closed" });
            },
        }
    }
}
