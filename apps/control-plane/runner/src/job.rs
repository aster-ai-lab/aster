//! One claimed job: checkout, steps in order, the log stream, the
//! completion.

use std::path::Path;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

use crate::api::{ApiError, RunnerApi};
use crate::checkout::Checkout;
use crate::config::RunnerConfig;
use crate::executor::{
    render_step_script, write_step_script, Cancel, Executor, StepExit, StepRun, GUEST_WORKSPACE,
};
use crate::protocol::{ClaimedJob, ClaimedStep, JobStatus};
use crate::state::create_private_dir;

/// A chunk is sent once this much output is waiting.
pub const LOG_FLUSH_BYTES: usize = 32 * 1024;
/// Under the API's 64 KiB chunk limit.
pub const LOG_CHUNK_MAX_BYTES: usize = 60 * 1024;
/// Output beyond this, while the API is unreachable, is dropped.
const LOG_BACKLOG_MAX_BYTES: usize = 1024 * 1024;
const LOG_FLUSH_EVERY: Duration = Duration::from_secs(1);
const FINAL_FLUSH_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    Completed(JobStatus),
    /// The server ended the job first; the runner reported nothing.
    Ended,
}

enum StepsOutcome {
    Status(JobStatus),
    Ended,
}

pub struct JobRunner<'a, A, C, E> {
    pub api: &'a A,
    pub checkout: &'a C,
    pub executor: &'a E,
    pub config: &'a RunnerConfig,
    pub credential: &'a str,
}

/// A job id becomes a directory name, so only the UUID shape is accepted.
fn valid_job_id(job_id: &str) -> bool {
    job_id.len() == 36 && job_id.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-')
}

fn deadline(expires_at: Option<&str>) -> Instant {
    let remaining = expires_at
        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .and_then(|expires| {
            (expires.with_timezone(&chrono::Utc) - chrono::Utc::now())
                .to_std()
                .ok()
        })
        .unwrap_or(Duration::ZERO);
    Instant::now() + remaining
}

/// Secrets first, then the job's own variables, so a job variable wins.
pub fn step_variables(job: &ClaimedJob, step: &ClaimedStep) -> Vec<(String, String)> {
    let mut variables: Vec<(String, String)> = job
        .secrets
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    for (name, value) in [
        ("ASTER_JOB_ID", job.job_id.as_str()),
        ("ASTER_RUN_ID", job.run_id.as_str()),
        ("ASTER_JOB_NAME", job.name.as_str()),
        ("ASTER_REPOSITORY", job.repository.as_deref().unwrap_or("")),
        ("ASTER_REF", job.git_ref.as_str()),
        ("ASTER_SHA", job.sha.as_str()),
        ("ASTER_STEP_NAME", step.name.as_str()),
        ("ASTER_WORKSPACE", GUEST_WORKSPACE),
        ("ASTER_STEP", step.run.as_str()),
    ] {
        variables.push((name.to_string(), value.to_string()));
    }
    variables
}

impl<A: RunnerApi, C: Checkout, E: Executor> JobRunner<'_, A, C, E> {
    pub async fn run(
        &self,
        job: ClaimedJob,
        shutdown: watch::Receiver<bool>,
    ) -> Result<JobOutcome, String> {
        let token = match job.job_token.as_deref() {
            Some(token) if valid_job_id(&job.job_id) => token.to_string(),
            _ => {
                crate::runner::note(&format!(
                    "job {}: the claim carries no job token or no valid job id",
                    job.job_id
                ));
                return self
                    .complete(&job.job_id, JobStatus::Failure)
                    .await
                    .map(|_| JobOutcome::Completed(JobStatus::Failure));
            }
        };
        let root = self.config.work_dir.join(&job.job_id);
        remove_dir(&root);
        let (output, received) = mpsc::unbounded_channel();
        let (ended_tx, ended_rx) = watch::channel(false);
        let cancel = Cancel {
            ended: ended_rx,
            shutdown,
        };
        let probe_every = Duration::from_secs(self.config.heartbeat_seconds.max(1));
        let (steps, ended) = tokio::join!(
            self.run_steps(&job, &token, &root, output, cancel),
            pump_logs(
                self.api,
                &job.job_id,
                &token,
                probe_every,
                received,
                ended_tx
            ),
        );
        remove_dir(&root);
        match steps {
            StepsOutcome::Ended => Ok(JobOutcome::Ended),
            _ if ended => Ok(JobOutcome::Ended),
            StepsOutcome::Status(status) => self.complete(&job.job_id, status).await,
        }
    }

    async fn complete(&self, job_id: &str, status: JobStatus) -> Result<JobOutcome, String> {
        match self.api.complete(job_id, self.credential, status).await {
            Ok(()) => Ok(JobOutcome::Completed(status)),
            Err(ApiError::Conflict) => Ok(JobOutcome::Ended),
            Err(error) => Err(format!("job {job_id} completion: {error}")),
        }
    }

    async fn run_steps(
        &self,
        job: &ClaimedJob,
        token: &str,
        root: &Path,
        output: mpsc::UnboundedSender<String>,
        cancel: Cancel,
    ) -> StepsOutcome {
        let say = |line: String| {
            let _ = output.send(line);
        };
        let deadline = deadline(job.job_token_expires_at.as_deref());
        let checkout = root.join("checkout");
        let env_dir = root.join("env");
        if let Err(error) = create_private_dir(root).and_then(|()| create_private_dir(&env_dir)) {
            say(format!(
                "==> the job directory cannot be created: {error}\n"
            ));
            return StepsOutcome::Status(JobStatus::Failure);
        }
        let Some(clone_path) = job.clone_path.as_deref() else {
            say("==> the claim names no repository to clone\n".into());
            return StepsOutcome::Status(JobStatus::Failure);
        };
        say(format!("==> checkout {}\n", job.sha));
        let url = format!("{}{clone_path}", self.config.api_url);
        if let Err(error) = self
            .checkout
            .checkout(&url, token, &job.sha, &checkout)
            .await
        {
            say(format!("==> checkout failed: {error}\n"));
            return StepsOutcome::Status(JobStatus::Failure);
        }
        if job.steps.is_empty() {
            say("==> the job has no steps\n".into());
            return StepsOutcome::Status(JobStatus::Failure);
        }
        let short_id = job.job_id.get(..8).unwrap_or("job");
        let mut failed = false;
        for (index, step) in job.steps.iter().enumerate() {
            if failed {
                say(format!("==> step {} skipped\n", step.name));
                continue;
            }
            if cancel.is_set() {
                return cancelled(&cancel, say, &step.name);
            }
            if step.kind != "run" {
                say(format!(
                    "==> step {}: this runner runs only `run` steps; `{}` arrives with artifacts\n",
                    step.name, step.kind
                ));
                failed = true;
                continue;
            }
            let written = render_step_script(&step_variables(job, step))
                .and_then(|script| write_step_script(&env_dir, &script));
            if let Err(error) = written {
                say(format!(
                    "==> step {} cannot be prepared: {error}\n",
                    step.name
                ));
                failed = true;
                continue;
            }
            say(format!("==> step {}\n", step.name));
            let run = StepRun {
                box_name: format!("aster-job-{short_id}-{}", index + 1),
                checkout: &checkout,
                env_dir: &env_dir,
                deadline,
            };
            match self
                .executor
                .run(&run, output.clone(), cancel.clone())
                .await
            {
                Ok(StepExit::Exited(code)) => {
                    say(format!("==> step {} exited {code}\n", step.name));
                    failed = code != 0;
                }
                Ok(StepExit::TimedOut) => {
                    say(format!(
                        "==> step {} timed out: the job's time ran out\n",
                        step.name
                    ));
                    failed = true;
                }
                Ok(StepExit::Cancelled) => return cancelled(&cancel, say, &step.name),
                Err(error) => {
                    say(format!("==> step {} did not run: {error}\n", step.name));
                    failed = true;
                }
            }
        }
        StepsOutcome::Status(if failed {
            JobStatus::Failure
        } else {
            JobStatus::Success
        })
    }
}

fn cancelled(cancel: &Cancel, say: impl Fn(String), step: &str) -> StepsOutcome {
    if cancel.ended() {
        return StepsOutcome::Ended;
    }
    say(format!(
        "==> step {step} cancelled: the runner is shutting down\n"
    ));
    StepsOutcome::Status(JobStatus::Cancelled)
}

fn remove_dir(path: &Path) {
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => crate::runner::note(&format!("{}: {error}", path.display())),
    }
}

/// The largest prefix of at most `max` bytes that ends a line, else the
/// largest that ends a character.
pub fn chunk_end(text: &str, max: usize) -> usize {
    if text.len() <= max {
        return text.len();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    match text[..end].rfind('\n') {
        Some(newline) => newline + 1,
        None => end,
    }
}

struct Pump<'a, A> {
    api: &'a A,
    job_id: &'a str,
    token: &'a str,
    pending: String,
    ended: bool,
    last_sent: Instant,
}

impl<A: RunnerApi> Pump<'_, A> {
    /// Sends what is pending; `false` when the API could not be reached.
    async fn flush(&mut self) -> bool {
        while !self.pending.is_empty() && !self.ended {
            let end = chunk_end(&self.pending, LOG_CHUNK_MAX_BYTES);
            match self
                .api
                .append_log(self.job_id, self.token, &self.pending[..end])
                .await
            {
                Ok(()) => {
                    self.pending.drain(..end);
                    self.last_sent = Instant::now();
                }
                Err(ApiError::Unauthorized | ApiError::Conflict) => {
                    self.ended = true;
                    self.pending.clear();
                }
                Err(ApiError::Other(error)) => {
                    if self.pending.len() > LOG_BACKLOG_MAX_BYTES {
                        crate::runner::note(&format!(
                            "job {}: {} bytes of output dropped: {error}",
                            self.job_id,
                            self.pending.len()
                        ));
                        self.pending.clear();
                    }
                    return false;
                }
            }
        }
        true
    }

    /// A silent step still learns that the server ended the job: an empty
    /// chunk is refused once the job or its token is gone.
    async fn probe(&mut self, every: Duration) {
        if self.ended || !self.pending.is_empty() || self.last_sent.elapsed() < every {
            return;
        }
        match self.api.append_log(self.job_id, self.token, "").await {
            Ok(()) => self.last_sent = Instant::now(),
            Err(ApiError::Unauthorized | ApiError::Conflict) => self.ended = true,
            Err(ApiError::Other(_)) => {}
        }
    }
}

/// Streams the job's output; `true` when the server ended the job.
async fn pump_logs<A: RunnerApi>(
    api: &A,
    job_id: &str,
    token: &str,
    probe_every: Duration,
    mut received: mpsc::UnboundedReceiver<String>,
    ended: watch::Sender<bool>,
) -> bool {
    let mut pump = Pump {
        api,
        job_id,
        token,
        pending: String::new(),
        ended: false,
        last_sent: Instant::now(),
    };
    let mut ticker = tokio::time::interval(LOG_FLUSH_EVERY);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            text = received.recv() => match text {
                Some(text) => {
                    if !pump.ended {
                        pump.pending.push_str(&text);
                    }
                    if pump.pending.len() >= LOG_FLUSH_BYTES {
                        pump.flush().await;
                    }
                }
                None => break,
            },
            _ = ticker.tick() => {
                pump.flush().await;
                pump.probe(probe_every).await;
            }
        }
        if pump.ended && !*ended.borrow() {
            let _ = ended.send(true);
        }
    }
    for attempt in 1..=FINAL_FLUSH_ATTEMPTS {
        if pump.flush().await {
            break;
        }
        if attempt == FINAL_FLUSH_ATTEMPTS {
            crate::runner::note(&format!(
                "job {job_id}: the last {} bytes of output were not delivered",
                pump.pending.len()
            ));
        } else {
            tokio::time::sleep(LOG_FLUSH_EVERY).await;
        }
    }
    pump.ended
}

#[cfg(all(test, unix))]
#[path = "job_tests.rs"]
mod tests;
