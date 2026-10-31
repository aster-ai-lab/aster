#![cfg(test)]

use super::*;
use crate::loop_checkpoint::LoopCheckpoint;
use crate::session_checkpoint::{SessionCheckpointExportSink, SessionCheckpointExportV1};
use crate::store::{
    FileSessionStore, MemorySessionStore, SessionData, SessionSnapshotV1, SessionStore,
};
use std::sync::atomic::AtomicBool;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Store,
    Export,
}

struct Barrier {
    entered: AtomicBool,
    dropped: Arc<AtomicBool>,
    returned: AtomicBool,
    release: CancellationToken,
}

impl Barrier {
    fn new() -> Self {
        Self {
            entered: AtomicBool::new(false),
            dropped: Arc::new(AtomicBool::new(false)),
            returned: AtomicBool::new(false),
            release: CancellationToken::new(),
        }
    }
    async fn wait(&self) {
        struct Dropped(Arc<AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let _dropped = Dropped(self.dropped.clone());
        self.entered.store(true, Ordering::SeqCst);
        self.release.cancelled().await;
        self.returned.store(true, Ordering::SeqCst);
    }
}

struct StoredBoundary {
    actual: Arc<dyn SessionStore>,
    barrier: Option<Arc<Barrier>>,
}

#[async_trait::async_trait]
impl SessionStore for StoredBoundary {
    async fn save(&self, data: &SessionData) -> anyhow::Result<()> {
        self.actual.save(data).await
    }
    async fn load(&self, id: &str) -> anyhow::Result<Option<SessionData>> {
        self.actual.load(id).await
    }
    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.actual.delete(id).await
    }
    async fn list(&self) -> anyhow::Result<Vec<String>> {
        self.actual.list().await
    }
    async fn exists(&self, id: &str) -> anyhow::Result<bool> {
        self.actual.exists(id).await
    }
    async fn save_snapshot(&self, data: &SessionSnapshotV1) -> anyhow::Result<()> {
        self.actual.save_snapshot(data).await
    }
    async fn load_snapshot(&self, id: &str) -> anyhow::Result<Option<SessionSnapshotV1>> {
        self.actual.load_snapshot(id).await
    }
    async fn save_loop_checkpoint(
        &self,
        id: &str,
        checkpoint: &LoopCheckpoint,
    ) -> anyhow::Result<()> {
        if let Some(barrier) = &self.barrier {
            barrier.wait().await;
        }
        self.actual.save_loop_checkpoint(id, checkpoint).await
    }
    async fn load_loop_checkpoint(&self, id: &str) -> anyhow::Result<Option<LoopCheckpoint>> {
        self.actual.load_loop_checkpoint(id).await
    }
    async fn delete_loop_checkpoint(&self, id: &str) -> anyhow::Result<()> {
        self.actual.delete_loop_checkpoint(id).await
    }
    fn backend_name(&self) -> &str {
        self.actual.backend_name()
    }
}

struct ExportBoundary(Arc<Barrier>);

#[async_trait::async_trait]
impl SessionCheckpointExportSink for ExportBoundary {
    async fn export_checkpoint(&self, checkpoint: SessionCheckpointExportV1) -> anyhow::Result<()> {
        checkpoint.open()?;
        self.0.wait().await;
        Ok(())
    }
}

async fn pending_boundary(stage: Stage, file: bool) {
    let root = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let actual: Arc<dyn SessionStore> = if file {
        Arc::new(FileSessionStore::new(data.path()).await.unwrap())
    } else {
        Arc::new(MemorySessionStore::new())
    };
    let barrier = Arc::new(Barrier::new());
    let store = Arc::new(StoredBoundary {
        actual,
        barrier: (stage == Stage::Store).then(|| barrier.clone()),
    });
    let mut opts = options("goal-run")
        .with_session_store(store)
        .with_auto_save(true)
        .with_permission_policy(
            crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
        );
    if stage == Stage::Export {
        opts = opts.with_session_checkpoint_export_sink(Arc::new(ExportBoundary(barrier.clone())));
    }
    let deadline = now_ms() + 3_000;
    let (agent, session, client) =
        fixture_with(root.path(), true, opts, Some(policy(true, 8, 4, deadline))).await;
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { mut worker, .. }) = session
        .tick_goal(&request(&session, "wake-persistence"))
        .await
        .unwrap()
    else {
        panic!("the original Run must start")
    };
    let entered = tokio::time::timeout(Duration::from_secs(2), async {
        while !barrier.entered.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok();
    tokio::time::sleep(Duration::from_millis(
        deadline.saturating_sub(now_ms()) + 100,
    ))
    .await;
    let still_owned = tokio::time::timeout(Duration::from_millis(20), &mut worker)
        .await
        .is_err();
    let events = session.run_events("wake-persistence").await;
    let notice = events.iter().any(|event| matches!(&event.event, crate::agent::AgentEvent::BudgetThresholdHit { resource, message: Some(message), .. } if resource == "goal_deadline_unix_ms" && message.contains("goal.deadline_expired") && message.contains("settlement is pending")));
    let status = session
        .run_snapshot("wake-persistence")
        .await
        .unwrap()
        .status;
    let dropped_early = barrier.dropped.load(Ordering::SeqCst);
    let calls = client.calls.load(Ordering::SeqCst);
    let next = tokio::time::timeout(
        Duration::from_secs(1),
        session.send("Another run cannot overtake the writer.", None),
    )
    .await;
    let blocked = matches!(next, Ok(Err(crate::error::CodeError::SessionBusy { session_id })) if session_id == "goal-run");
    eprintln!(
        "PERSISTENCE_OWNER_OBSERVATION stage={stage:?} file={file} entered={entered} still_owned={still_owned} dropped_early={dropped_early} notice={notice} blocked={blocked} model_calls={calls} status={status:?}"
    );
    barrier.release.cancel();
    if still_owned {
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .unwrap()
            .unwrap();
    }
    let final_status = session
        .run_snapshot("wake-persistence")
        .await
        .unwrap()
        .status;
    let facts = settlement::facts(root.path());
    session.close().await;
    agent.close().await;
    assert!(
        entered && still_owned && blocked && !dropped_early,
        "an unreturned durable callback cannot be treated as quiescent"
    );
    assert!(
        !status.is_terminal(),
        "the unreturned original owner has not finished publication"
    );
    assert!(barrier.returned.load(Ordering::SeqCst));
    assert_eq!(
        calls, 1,
        "no additional model work while persistence is pending"
    );
    assert_eq!(final_status, crate::run::RunStatus::Cancelled);
    assert_eq!(
        facts
            .iter()
            .filter(|f| f.kind == "aster.goal.slice.settled")
            .count(),
        1
    );
    assert!(!facts.iter().any(|f| f.kind == "aster.goal.completed"));
    assert!(
        notice,
        "deadline expiry must be observable while the original writer still owns cleanup"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pending_checkpoint_write_reports_the_deadline_without_fake_retirement() {
    for file in [false, true] {
        pending_boundary(Stage::Store, file).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pending_export_reports_the_deadline_without_fake_retirement() {
    for file in [false, true] {
        pending_boundary(Stage::Export, file).await;
    }
}
