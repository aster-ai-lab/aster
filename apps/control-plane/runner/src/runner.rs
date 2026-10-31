//! Credential bootstrap, heartbeats and the claim loop.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use crate::api::{ApiError, RunnerApi};
use crate::checkout::Checkout;
use crate::config::RunnerConfig;
use crate::executor::Executor;
use crate::job::{JobOutcome, JobRunner};
use crate::state::{self, RunnerState, REGISTRATION_TOKEN_ENV};

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("{0}")]
    Setup(String),
    /// The API refused the runner's credential: exit 2.
    #[error("{0}")]
    Revoked(String),
    #[error("{0}")]
    Job(String),
}

pub fn note(message: &str) {
    eprintln!("aster-runner: {message}");
}

/// The stored credential, else the registration token exchanged for one
/// and stored before anything else happens.
pub async fn ensure_credential<A: RunnerApi>(
    api: &A,
    config: &RunnerConfig,
    registration_token: Option<String>,
) -> Result<RunnerState, RunnerError> {
    if let Some(state) = state::load(&config.state_file).map_err(RunnerError::Setup)? {
        return Ok(state);
    }
    let token = registration_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            RunnerError::Setup(format!(
                "{} holds no credential and {REGISTRATION_TOKEN_ENV} is not set",
                config.state_file.display()
            ))
        })?;
    let beat = match api.heartbeat(&token).await {
        Ok(beat) => beat,
        Err(ApiError::Unauthorized) => {
            return Err(RunnerError::Revoked(
                "the registration token was refused".into(),
            ))
        }
        Err(error) => return Err(RunnerError::Setup(error.to_string())),
    };
    let credential = beat
        .credential
        .ok_or_else(|| RunnerError::Setup("the registration token was already exchanged".into()))?;
    let state = RunnerState {
        runner_id: beat.runner_id,
        credential,
    };
    state::store(&config.state_file, &state).map_err(RunnerError::Setup)?;
    note(&format!("registered as runner {}", state.runner_id));
    Ok(state)
}

/// Waits `duration`; `true` when `stop` was set first.
async fn pause(duration: Duration, stop: &mut watch::Receiver<bool>) -> bool {
    if *stop.borrow_and_update() {
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(duration) => false,
        changed = stop.changed() => changed.is_ok() && *stop.borrow(),
    }
}

pub struct Runner<A, C, E> {
    pub api: Arc<A>,
    pub checkout: C,
    pub executor: E,
    pub config: RunnerConfig,
    pub state: RunnerState,
}

impl<A: RunnerApi + 'static, C: Checkout, E: Executor> Runner<A, C, E> {
    /// Claims and runs jobs until shutdown; with `once`, returns after the
    /// first claimed job.
    pub async fn run(
        &self,
        once: bool,
        shutdown: watch::Receiver<bool>,
    ) -> Result<(), RunnerError> {
        self.beat().await?;
        let (refused_tx, refused) = watch::channel(false);
        let heartbeats = tokio::spawn(heartbeat_loop(
            Arc::clone(&self.api),
            self.state.credential.clone(),
            Duration::from_secs(self.config.heartbeat_seconds),
            refused_tx,
        ));
        let result = self.claim_loop(once, shutdown, refused).await;
        heartbeats.abort();
        result
    }

    async fn beat(&self) -> Result<(), RunnerError> {
        match self.api.heartbeat(&self.state.credential).await {
            Ok(_) => Ok(()),
            Err(ApiError::Unauthorized) => Err(revoked()),
            Err(error) => Err(RunnerError::Setup(error.to_string())),
        }
    }

    async fn claim_loop(
        &self,
        once: bool,
        mut shutdown: watch::Receiver<bool>,
        credential_refused: watch::Receiver<bool>,
    ) -> Result<(), RunnerError> {
        let poll = Duration::from_secs(self.config.poll_seconds);
        loop {
            if *credential_refused.borrow() {
                return Err(revoked());
            }
            if *shutdown.borrow() {
                return Ok(());
            }
            let job = match self.api.claim(&self.state.credential).await {
                Ok(Some(job)) => job,
                Ok(None) => {
                    if pause(poll, &mut shutdown).await {
                        return Ok(());
                    }
                    continue;
                }
                Err(ApiError::Unauthorized) => return Err(revoked()),
                Err(error) => {
                    note(&format!("claim: {error}"));
                    if pause(poll, &mut shutdown).await {
                        return Ok(());
                    }
                    continue;
                }
            };
            let job_id = job.job_id.clone();
            note(&format!("job {job_id}: claimed"));
            let outcome = JobRunner {
                api: self.api.as_ref(),
                checkout: &self.checkout,
                executor: &self.executor,
                config: &self.config,
                credential: &self.state.credential,
            }
            .run(job, shutdown.clone())
            .await;
            match outcome {
                Ok(JobOutcome::Completed(status)) => {
                    note(&format!("job {job_id}: {}", status.as_str()))
                }
                Ok(JobOutcome::Ended) => note(&format!("job {job_id}: ended by the server")),
                Err(error) if once => return Err(RunnerError::Job(error)),
                Err(error) => note(&error),
            }
            if once {
                return Ok(());
            }
        }
    }
}

fn revoked() -> RunnerError {
    RunnerError::Revoked("the runner credential was refused; register the runner again".into())
}

async fn heartbeat_loop<A: RunnerApi>(
    api: Arc<A>,
    credential: String,
    every: Duration,
    refused: watch::Sender<bool>,
) {
    loop {
        tokio::time::sleep(every).await;
        match api.heartbeat(&credential).await {
            Ok(_) => {}
            Err(ApiError::Unauthorized) => {
                let _ = refused.send(true);
                return;
            }
            Err(error) => note(&format!("heartbeat: {error}")),
        }
    }
}

#[cfg(all(test, unix))]
#[path = "runner_tests.rs"]
mod tests;
