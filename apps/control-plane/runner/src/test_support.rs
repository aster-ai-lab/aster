#![cfg(test)]
//! Fakes for the API, the checkout and the executor.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::api::{ApiError, RunnerApi};
use crate::checkout::Checkout;
use crate::config::RunnerConfig;
use crate::executor::{Cancel, Executor, StepExit, StepRun};
use crate::protocol::{ClaimedJob, ClaimedStep, Heartbeat, JobStatus};

pub const JOB_ID: &str = "0b9d6c0e-5f4b-4a8e-9f1e-1b2c3d4e5f60";
pub const SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

pub fn config(work_dir: &Path) -> RunnerConfig {
    RunnerConfig {
        api_url: "http://api.test".into(),
        state_file: work_dir.join("state").join("runner_state.acl"),
        work_dir: work_dir.join("jobs"),
        image: "alpine:3.20".into(),
        box_cli: None,
        cpus: 2,
        memory: "2g".into(),
        poll_seconds: 1,
        heartbeat_seconds: 5,
    }
}

pub fn step(name: &str, run: &str) -> ClaimedStep {
    ClaimedStep {
        name: name.into(),
        kind: "run".into(),
        run: run.into(),
    }
}

pub fn job(steps: Vec<ClaimedStep>) -> ClaimedJob {
    ClaimedJob {
        job_id: JOB_ID.into(),
        run_id: "run-1".into(),
        name: "build".into(),
        git_ref: "refs/heads/main".into(),
        sha: SHA.into(),
        steps,
        repository: Some("team/app".into()),
        clone_path: Some("/v1/api/git/team/app.git".into()),
        job_token: Some("job-token".into()),
        job_token_expires_at: Some(
            (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339(),
        ),
        secrets: BTreeMap::from([("DEPLOY_TOKEN".to_string(), "s3cr'et".to_string())]),
    }
}

#[derive(Default)]
pub struct FakeApi {
    /// Registration token -> (runner id, credential).
    pub registrations: Mutex<BTreeMap<String, (String, String)>>,
    pub credentials: Mutex<Vec<String>>,
    pub jobs: Mutex<Vec<ClaimedJob>>,
    /// Append answers, in order; once empty, appends succeed.
    pub append_answers: Mutex<Vec<ApiError>>,
    /// The server ended the job: empty appends (probes) are refused.
    pub refuse_probes: AtomicBool,
    pub logs: Mutex<Vec<String>>,
    pub completions: Mutex<Vec<(String, String, JobStatus)>>,
    pub heartbeats: Mutex<usize>,
}

impl FakeApi {
    pub fn with_credential(credential: &str) -> Self {
        let api = Self::default();
        api.credentials
            .lock()
            .map(|mut credentials| credentials.push(credential.into()))
            .ok();
        api
    }

    pub fn log(&self) -> String {
        self.logs
            .lock()
            .map(|logs| logs.concat())
            .unwrap_or_default()
    }

    pub fn completions(&self) -> Vec<(String, String, JobStatus)> {
        self.completions
            .lock()
            .map(|completions| completions.clone())
            .unwrap_or_default()
    }

    fn known(&self, credential: &str) -> bool {
        self.credentials
            .lock()
            .map(|credentials| credentials.iter().any(|known| known == credential))
            .unwrap_or(false)
    }
}

#[async_trait]
impl RunnerApi for FakeApi {
    async fn heartbeat(&self, token: &str) -> Result<Heartbeat, ApiError> {
        if let Ok(mut count) = self.heartbeats.lock() {
            *count += 1;
        }
        if self.known(token) {
            return Ok(Heartbeat {
                runner_id: "runner-1".into(),
                credential: None,
            });
        }
        let registered = self
            .registrations
            .lock()
            .ok()
            .and_then(|mut registrations| registrations.remove(token));
        match registered {
            Some((runner_id, credential)) => {
                if let Ok(mut credentials) = self.credentials.lock() {
                    credentials.push(credential.clone());
                }
                Ok(Heartbeat {
                    runner_id,
                    credential: Some(credential),
                })
            }
            None => Err(ApiError::Unauthorized),
        }
    }

    async fn claim(&self, credential: &str) -> Result<Option<ClaimedJob>, ApiError> {
        if !self.known(credential) {
            return Err(ApiError::Unauthorized);
        }
        Ok(self.jobs.lock().ok().and_then(|mut jobs| jobs.pop()))
    }

    async fn append_log(&self, job_id: &str, job_token: &str, chunk: &str) -> Result<(), ApiError> {
        assert_eq!((job_id, job_token), (JOB_ID, "job-token"));
        if let Some(answer) = self
            .append_answers
            .lock()
            .ok()
            .and_then(|mut answers| answers.pop())
        {
            return Err(answer);
        }
        if chunk.is_empty() && self.refuse_probes.load(Ordering::SeqCst) {
            return Err(ApiError::Unauthorized);
        }
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(chunk.into());
        }
        Ok(())
    }

    async fn complete(
        &self,
        job_id: &str,
        credential: &str,
        status: JobStatus,
    ) -> Result<(), ApiError> {
        if let Ok(mut completions) = self.completions.lock() {
            completions.push((job_id.into(), credential.into(), status));
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeCheckout {
    pub fail: bool,
    pub calls: Mutex<Vec<(String, String, String)>>,
}

#[async_trait]
impl Checkout for FakeCheckout {
    async fn checkout(&self, url: &str, token: &str, sha: &str, dir: &Path) -> Result<(), String> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push((url.into(), token.into(), sha.into()));
        }
        if self.fail {
            return Err("repository not found".into());
        }
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())
    }
}

pub enum Behaviour {
    Exit(i32),
    /// Waits for the cancel signal.
    UntilCancelled,
    TimedOut,
}

/// One behaviour per step, in order; records each step's `step.sh`.
pub struct FakeExecutor {
    pub output: String,
    pub behaviours: Mutex<Vec<Behaviour>>,
    pub scripts: Mutex<Vec<(String, String, u32)>>,
    pub env_dirs: Mutex<Vec<PathBuf>>,
}

impl FakeExecutor {
    pub fn new(output: &str, mut behaviours: Vec<Behaviour>) -> Self {
        behaviours.reverse();
        Self {
            output: output.into(),
            behaviours: Mutex::new(behaviours),
            scripts: Mutex::new(Vec::new()),
            env_dirs: Mutex::new(Vec::new()),
        }
    }

    pub fn runs(&self) -> usize {
        self.scripts
            .lock()
            .map(|scripts| scripts.len())
            .unwrap_or(0)
    }

    pub fn script(&self, index: usize) -> (String, String, u32) {
        self.scripts
            .lock()
            .ok()
            .and_then(|scripts| scripts.get(index).cloned())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Executor for FakeExecutor {
    async fn run(
        &self,
        step: &StepRun<'_>,
        output: mpsc::UnboundedSender<String>,
        cancel: Cancel,
    ) -> Result<StepExit, String> {
        use std::os::unix::fs::PermissionsExt;
        let path = step.env_dir.join(crate::executor::STEP_SCRIPT);
        let script = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let mode = std::fs::metadata(&path)
            .map_err(|error| error.to_string())?
            .permissions()
            .mode()
            & 0o777;
        if let Ok(mut scripts) = self.scripts.lock() {
            scripts.push((step.box_name.clone(), script, mode));
        }
        if let Ok(mut dirs) = self.env_dirs.lock() {
            dirs.push(step.env_dir.to_path_buf());
        }
        let _ = output.send(self.output.clone());
        let behaviour = self
            .behaviours
            .lock()
            .ok()
            .and_then(|mut behaviours| behaviours.pop())
            .unwrap_or(Behaviour::Exit(0));
        match behaviour {
            Behaviour::Exit(code) => Ok(StepExit::Exited(code)),
            Behaviour::TimedOut => Ok(StepExit::TimedOut),
            Behaviour::UntilCancelled => {
                match tokio::time::timeout(Duration::from_secs(10), cancel.wait()).await {
                    Ok(()) => Ok(StepExit::Cancelled),
                    Err(_) => Err("never cancelled".into()),
                }
            }
        }
    }
}
