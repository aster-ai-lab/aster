use tokio::sync::watch;

use super::*;
use crate::api::ApiError;
use crate::protocol::JobStatus;
use crate::test_support::{
    config, job, step, Behaviour, FakeApi, FakeCheckout, FakeExecutor, JOB_ID, SHA,
};

struct Fixture {
    _dir: tempfile::TempDir,
    config: RunnerConfig,
    api: FakeApi,
    checkout: FakeCheckout,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        Self {
            config: config(dir.path()),
            _dir: dir,
            api: FakeApi::with_credential("credential"),
            checkout: FakeCheckout::default(),
        }
    }

    async fn run(
        &self,
        executor: &FakeExecutor,
        claimed: ClaimedJob,
        shutdown: watch::Receiver<bool>,
    ) -> Result<JobOutcome, String> {
        JobRunner {
            api: &self.api,
            checkout: &self.checkout,
            executor,
            config: &self.config,
            credential: "credential",
        }
        .run(claimed, shutdown)
        .await
    }
}

fn running() -> watch::Receiver<bool> {
    watch::channel(false).1
}

#[tokio::test]
async fn a_passing_job_streams_its_log_and_completes_success() {
    let fixture = Fixture::new();
    let executor = FakeExecutor::new("compiled\n", vec![Behaviour::Exit(0), Behaviour::Exit(0)]);
    let outcome = fixture
        .run(
            &executor,
            job(vec![step("build", "make"), step("test", "make test")]),
            running(),
        )
        .await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Success));
    assert_eq!(
        fixture.api.completions(),
        [(
            JOB_ID.to_string(),
            "credential".to_string(),
            JobStatus::Success
        )]
    );
    assert_eq!(
        fixture.api.log(),
        format!(
            "==> checkout {SHA}\n==> step build\ncompiled\n==> step build exited 0\n\
             ==> step test\ncompiled\n==> step test exited 0\n"
        )
    );
    let calls = fixture.checkout.calls.lock().unwrap().clone();
    assert_eq!(
        calls,
        [(
            "http://api.test/v1/api/git/team/app.git".to_string(),
            "job-token".to_string(),
            SHA.to_string()
        )]
    );
    let (box_name, script, mode) = executor.script(0);
    assert_eq!(box_name, "aster-job-0b9d6c0e-1");
    assert_eq!(mode, 0o600);
    assert!(
        script.starts_with("export DEPLOY_TOKEN='s3cr'\\''et'\n"),
        "{script}"
    );
    assert!(script.contains("export ASTER_SHA='4b825dc642cb6eb9a060e54bf8d69288fbee4904'\n"));
    assert!(script.ends_with("export ASTER_STEP='make'\n"), "{script}");
    assert_eq!(executor.script(1).0, "aster-job-0b9d6c0e-2");
    assert!(executor
        .script(1)
        .1
        .ends_with("export ASTER_STEP='make test'\n"));
    assert!(!fixture.config.work_dir.join(JOB_ID).exists());
}

#[tokio::test]
async fn a_failing_step_skips_the_rest_and_completes_failure() {
    let fixture = Fixture::new();
    let executor = FakeExecutor::new("", vec![Behaviour::Exit(2)]);
    let outcome = fixture
        .run(
            &executor,
            job(vec![step("build", "false"), step("test", "true")]),
            running(),
        )
        .await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Failure));
    assert_eq!(executor.runs(), 1);
    let log = fixture.api.log();
    assert!(
        log.contains("==> step build exited 2\n==> step test skipped\n"),
        "{log}"
    );

    let fixture = Fixture::new();
    let executor = FakeExecutor::new("", vec![Behaviour::TimedOut]);
    let outcome = fixture
        .run(&executor, job(vec![step("build", "sleep 999")]), running())
        .await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Failure));
    assert!(fixture.api.log().contains("==> step build timed out"));
}

#[tokio::test]
async fn a_failed_checkout_runs_nothing() {
    let mut fixture = Fixture::new();
    fixture.checkout.fail = true;
    let executor = FakeExecutor::new("", Vec::new());
    let outcome = fixture
        .run(&executor, job(vec![step("build", "make")]), running())
        .await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Failure));
    assert_eq!(executor.runs(), 0);
    assert!(fixture
        .api
        .log()
        .contains("==> checkout failed: repository not found\n"));
}

#[tokio::test]
async fn unrunnable_claims_fail_without_a_box() {
    let fixture = Fixture::new();
    let executor = FakeExecutor::new("", Vec::new());
    let mut claimed = job(vec![step("build", "make")]);
    claimed.job_token = None;
    let outcome = fixture.run(&executor, claimed, running()).await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Failure));
    assert!(fixture.checkout.calls.lock().unwrap().is_empty());

    let fixture = Fixture::new();
    let mut claimed = job(vec![step("build", "make")]);
    claimed.job_id = "../../etc".into();
    let outcome = fixture.run(&executor, claimed, running()).await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Failure));
    assert!(fixture.checkout.calls.lock().unwrap().is_empty());

    for steps in [
        Vec::new(),
        vec![ClaimedStep {
            name: "deploy".into(),
            kind: "uses".into(),
            run: String::new(),
        }],
    ] {
        let fixture = Fixture::new();
        let outcome = fixture.run(&executor, job(steps), running()).await;
        assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Failure));
    }
    assert_eq!(executor.runs(), 0);
}

#[tokio::test]
async fn a_refused_log_ends_the_job_without_a_completion() {
    for refusal in [ApiError::Conflict, ApiError::Unauthorized] {
        let fixture = Fixture::new();
        fixture.api.append_answers.lock().unwrap().push(refusal);
        let executor = FakeExecutor::new("working\n", vec![Behaviour::UntilCancelled]);
        let outcome = fixture
            .run(
                &executor,
                job(vec![step("build", "make"), step("test", "make test")]),
                running(),
            )
            .await;
        assert_eq!(outcome.unwrap(), JobOutcome::Ended);
        assert!(fixture.api.completions().is_empty());
        assert_eq!(executor.runs(), 1);
    }
}

#[tokio::test]
async fn a_silent_step_learns_the_server_ended_the_job() {
    let mut fixture = Fixture::new();
    fixture.config.heartbeat_seconds = 1;
    fixture
        .api
        .refuse_probes
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let executor = FakeExecutor::new("", vec![Behaviour::UntilCancelled]);
    let outcome = fixture
        .run(&executor, job(vec![step("build", "sleep 300")]), running())
        .await;
    assert_eq!(outcome.unwrap(), JobOutcome::Ended);
    assert!(fixture.api.completions().is_empty());
    assert!(!fixture.config.work_dir.join(JOB_ID).exists());
}

#[tokio::test]
async fn shutdown_completes_the_job_cancelled() {
    let fixture = Fixture::new();
    let (shutdown_tx, shutdown) = watch::channel(false);
    let executor = FakeExecutor::new("", vec![Behaviour::UntilCancelled]);
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let _ = shutdown_tx.send(true);
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    let outcome = fixture
        .run(&executor, job(vec![step("build", "make")]), shutdown)
        .await;
    stopper.abort();
    assert_eq!(
        outcome.unwrap(),
        JobOutcome::Completed(JobStatus::Cancelled)
    );
    assert_eq!(fixture.api.completions()[0].2, JobStatus::Cancelled);
    assert!(fixture
        .api
        .log()
        .ends_with("==> step build cancelled: the runner is shutting down\n"));
}

#[tokio::test]
async fn an_unreachable_api_keeps_the_output_for_the_next_flush() {
    let fixture = Fixture::new();
    fixture
        .api
        .append_answers
        .lock()
        .unwrap()
        .push(ApiError::Other("connection refused".into()));
    let executor = FakeExecutor::new("kept\n", vec![Behaviour::Exit(0)]);
    let outcome = fixture
        .run(&executor, job(vec![step("build", "make")]), running())
        .await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Success));
    assert!(fixture.api.log().contains("kept\n"));
}

#[tokio::test]
async fn large_output_is_sent_in_bounded_chunks() {
    let fixture = Fixture::new();
    let line = "x".repeat(99) + "\n";
    let executor = FakeExecutor::new(&line.repeat(2_000), vec![Behaviour::Exit(0)]);
    let outcome = fixture
        .run(&executor, job(vec![step("build", "make")]), running())
        .await;
    assert_eq!(outcome.unwrap(), JobOutcome::Completed(JobStatus::Success));
    let logs = fixture.api.logs.lock().unwrap().clone();
    assert!(logs.len() >= 4, "{}", logs.len());
    assert!(logs.iter().all(|chunk| chunk.len() <= LOG_CHUNK_MAX_BYTES));
    assert_eq!(fixture.api.log().matches(&line).count(), 2_000);
}

#[test]
fn chunks_end_on_a_line_else_a_character() {
    assert_eq!(chunk_end("short", 10), 5);
    assert_eq!(chunk_end("ab\ncd\nef", 7), 6);
    assert_eq!(chunk_end("abcdef", 4), 4);
    assert_eq!(chunk_end("a\u{e9}\u{e9}", 4), 3);
}

#[test]
fn job_variables_follow_the_secrets() {
    let claimed = job(vec![step("build", "make")]);
    let names: Vec<String> = step_variables(&claimed, &claimed.steps[0])
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        names,
        [
            "DEPLOY_TOKEN",
            "ASTER_JOB_ID",
            "ASTER_RUN_ID",
            "ASTER_JOB_NAME",
            "ASTER_REPOSITORY",
            "ASTER_REF",
            "ASTER_SHA",
            "ASTER_STEP_NAME",
            "ASTER_WORKSPACE",
            "ASTER_STEP",
        ]
    );
}
