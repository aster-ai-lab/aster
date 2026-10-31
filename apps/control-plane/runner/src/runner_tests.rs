use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use tokio::sync::watch;

use super::*;
use crate::protocol::JobStatus;
use crate::test_support::{config, job, step, Behaviour, FakeApi, FakeCheckout, FakeExecutor};

fn stored(credential: &str) -> RunnerState {
    RunnerState {
        runner_id: "runner-1".into(),
        credential: credential.into(),
    }
}

#[tokio::test]
async fn a_registration_token_is_exchanged_once_and_stored_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let api = FakeApi::default();
    api.registrations.lock().unwrap().insert(
        "registration".into(),
        ("runner-7".into(), "credential-7".into()),
    );
    let state = ensure_credential(&api, &config, Some("registration".into()))
        .await
        .unwrap();
    assert_eq!(state.runner_id, "runner-7");
    let mode = std::fs::metadata(&config.state_file)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(*api.heartbeats.lock().unwrap(), 1);

    let again = ensure_credential(&api, &config, Some("registration".into()))
        .await
        .unwrap();
    assert!(again == state);
    assert_eq!(*api.heartbeats.lock().unwrap(), 1);
}

#[tokio::test]
async fn a_missing_or_refused_registration_does_not_start() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let api = FakeApi::default();
    for token in [None, Some(String::new())] {
        let error = ensure_credential(&api, &config, token).await.err().unwrap();
        assert!(matches!(error, RunnerError::Setup(_)), "{error}");
    }
    let error = ensure_credential(&api, &config, Some("unknown".into()))
        .await
        .err()
        .unwrap();
    assert!(matches!(error, RunnerError::Revoked(_)), "{error}");
    assert!(!config.state_file.exists());
}

#[tokio::test]
async fn once_runs_the_first_claimed_job() {
    let dir = tempfile::tempdir().unwrap();
    let api = Arc::new(FakeApi::with_credential("credential"));
    api.jobs
        .lock()
        .unwrap()
        .push(job(vec![step("build", "make")]));
    let runner = Runner {
        api: Arc::clone(&api),
        checkout: FakeCheckout::default(),
        executor: FakeExecutor::new("built\n", vec![Behaviour::Exit(0)]),
        config: config(dir.path()),
        state: stored("credential"),
    };
    runner.run(true, watch::channel(false).1).await.unwrap();
    assert_eq!(api.completions()[0].2, JobStatus::Success);
    assert!(api.log().contains("built\n"));
}

#[tokio::test]
async fn a_refused_credential_stops_the_runner() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Runner {
        api: Arc::new(FakeApi::default()),
        checkout: FakeCheckout::default(),
        executor: FakeExecutor::new("", Vec::new()),
        config: config(dir.path()),
        state: stored("revoked"),
    };
    let error = runner
        .run(false, watch::channel(false).1)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, RunnerError::Revoked(_)), "{error}");
}

#[tokio::test]
async fn shutdown_stops_an_idle_runner() {
    let dir = tempfile::tempdir().unwrap();
    let api = Arc::new(FakeApi::with_credential("credential"));
    let runner = Runner {
        api: Arc::clone(&api),
        checkout: FakeCheckout::default(),
        executor: FakeExecutor::new("", Vec::new()),
        config: config(dir.path()),
        state: stored("credential"),
    };
    let (shutdown_tx, shutdown) = watch::channel(false);
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let _ = shutdown_tx.send(true);
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        runner.run(false, shutdown),
    )
    .await
    .unwrap()
    .unwrap();
    stopper.abort();
    assert!(api.completions().is_empty());
}
