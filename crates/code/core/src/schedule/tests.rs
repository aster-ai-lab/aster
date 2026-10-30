use super::*;
use crate::goal_control::{GoalWaitConditionV1, GoalWaitV1};
use async_trait::async_trait;
use chrono::{Datelike, TimeZone, Timelike};
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::Mutex;

const NOW: u64 = 1_800_000_000_000;

#[derive(Debug, Default)]
struct StaticGoalProvider {
    wakes: Mutex<Vec<GoalUtcWake>>,
    scopes: Mutex<Vec<ScheduleScope>>,
}

impl StaticGoalProvider {
    fn replace(&self, wakes: Vec<GoalUtcWake>) {
        *self.wakes.lock().expect("goal wakes") = wakes;
    }
}

impl GoalUtcWakeProvider for StaticGoalProvider {
    fn open_utc_wakes(&self, scope: &ScheduleScope) -> Result<Vec<GoalUtcWake>, ScheduleError> {
        self.scopes.lock().expect("goal scopes").push(scope.clone());
        Ok(self.wakes.lock().expect("goal wakes").clone())
    }
}

#[derive(Debug, Default)]
struct RecordingDriver {
    requests: Mutex<Vec<ExecutionRequest>>,
    outcomes: Mutex<VecDeque<Result<ExecutionDriverOutcome, ExecutionDriverError>>>,
}

impl RecordingDriver {
    fn push(&self, outcome: Result<ExecutionDriverOutcome, ExecutionDriverError>) {
        self.outcomes
            .lock()
            .expect("driver outcomes")
            .push_back(outcome);
    }

    fn requests(&self) -> Vec<ExecutionRequest> {
        self.requests.lock().expect("driver requests").clone()
    }
}

#[async_trait]
impl ScheduleExecutionDriver for RecordingDriver {
    async fn execute(
        &self,
        request: ExecutionRequest,
    ) -> Result<ExecutionDriverOutcome, ExecutionDriverError> {
        let run_id = format!("run:{}", request.operation_id);
        self.requests.lock().expect("driver requests").push(request);
        self.outcomes
            .lock()
            .expect("driver outcomes")
            .pop_front()
            .unwrap_or(Ok(ExecutionDriverOutcome::Completed { run_id }))
    }
}

struct Fixture {
    scope: ScheduleScope,
    store: Arc<MemoryScheduleStore>,
    goals: Arc<StaticGoalProvider>,
    driver: Arc<RecordingDriver>,
    clock: Arc<ManualScheduleClock>,
    scheduler: RunScheduler,
}

impl Fixture {
    fn new(tenant: &str, workspace: &str) -> Self {
        let scope = scope(tenant, workspace);
        let store = Arc::new(MemoryScheduleStore::new());
        let goals = Arc::new(StaticGoalProvider::default());
        let driver = Arc::new(RecordingDriver::default());
        let clock = Arc::new(ManualScheduleClock::new(NOW));
        let scheduler = RunScheduler::new(
            scope.clone(),
            store.clone(),
            goals.clone(),
            driver.clone(),
            clock.clone(),
            "test-owner",
            60_000,
        )
        .expect("scheduler");
        Self {
            scope,
            store,
            goals,
            driver,
            clock,
            scheduler,
        }
    }
}

fn scope(tenant: &str, workspace: &str) -> ScheduleScope {
    ScheduleScope {
        tenant_id: tenant.to_string(),
        principal_id: "principal".to_string(),
        workspace_id: workspace.to_string(),
    }
}

fn policy() -> ExecutionPolicyIdentity {
    ExecutionPolicyIdentity {
        permission_profile_id: "permission:default".to_string(),
        continuation_policy_id: Some("continuation:goal-v1".to_string()),
        deadline: Some(DeadlinePolicyIdentity {
            policy_id: "deadline:scheduled-v1".to_string(),
            deadline_unix_ms: NOW + 30_000,
        }),
        budget: Some(BudgetPolicyIdentity {
            policy_id: "budget:scheduled-v1".to_string(),
        }),
    }
}

fn execution(attendance: ExecutionAttendance) -> ExecutionTemplate {
    ExecutionTemplate {
        attendance,
        session_id: "session:scheduled".to_string(),
        policy: policy(),
    }
}

fn once(schedule_id: &str, scope: ScheduleScope, due: u64, operation: &str) -> CalendarSchedule {
    CalendarSchedule {
        schedule_id: schedule_id.to_string(),
        scope,
        enabled: true,
        trigger: ScheduleTrigger::Once { at_unix_ms: due },
        missed_policy: MissedPolicy::FireOnce,
        next_due_unix_ms: Some(due),
        operation_id_prefix: operation.to_string(),
        execution: execution(ExecutionAttendance::Background),
    }
}

fn goal(scope: &ScheduleScope, fire_id: &str, due: u64, operation: &str) -> GoalUtcWake {
    GoalUtcWake {
        scope: scope.clone(),
        fire_id: fire_id.to_string(),
        goal_id: "goal:one".to_string(),
        wait: GoalWaitV1 {
            step_id: "step:one".to_string(),
            declared_sequence: 7,
            condition: GoalWaitConditionV1::UtcDue {
                due_unix_ms: NonZeroU64::new(due).expect("nonzero due"),
            },
            observed: None,
        },
        operation_id: operation.to_string(),
        execution: execution(ExecutionAttendance::Background),
    }
}

fn leased_fire(scope: ScheduleScope, expires: u64) -> ScheduleFire {
    let request = ExecutionRequest {
        source: ScheduleSource::GoalUtcWake,
        attendance: ExecutionAttendance::Background,
        session_id: "session:scheduled".to_string(),
        operation_id: "goal-operation".to_string(),
        scope,
        policy: policy(),
    };
    ScheduleFire {
        fire_id: "goal-fire".to_string(),
        operation_id: request.operation_id.clone(),
        request,
        due_unix_ms: NOW - 1,
        state: ScheduleFireState::Leased,
        run_id: None,
        generation: 3,
        lease: Some(ScheduleLease {
            owner_id: "previous-owner".to_string(),
            lease_expires_at_unix_ms: expires,
        }),
        wait: None,
        error: None,
        finished_at_unix_ms: None,
    }
}

#[test]
fn closed_sets_and_cron_subset_reject_unknown_values() {
    assert_eq!(
        ScheduleSource::parse("calendar").unwrap(),
        ScheduleSource::Calendar
    );
    assert_eq!(ScheduleSource::GoalUtcWake.as_str(), "goal_utc_wake");
    assert_eq!(
        ExecutionAttendance::parse("background").unwrap(),
        ExecutionAttendance::Background
    );
    assert_eq!(MissedPolicy::FireOnce.as_str(), "fire_once");
    assert_eq!(
        ScheduleDecision::RecoveryRequired.as_str(),
        "recovery_required"
    );
    assert!(matches!(
        ScheduleSource::parse("prompt-inferred"),
        Err(ScheduleError::UnknownClosedSet { .. })
    ));
    assert!(serde_json::from_str::<ScheduleSource>("\"unknown\"").is_err());

    let valid = ScheduleTrigger::Cron {
        expression: "*/15 9-17 * * MON-FRI".to_string(),
        utc_offset_minutes: 0,
    };
    assert!(valid.validate().is_ok());
    for expression in [
        "@daily",
        "0 * * *",
        "*/0 * * * *",
        "0 0 * JAN *",
        "nope * * * *",
    ] {
        assert!(matches!(
            ScheduleTrigger::Cron {
                expression: expression.to_string(),
                utc_offset_minutes: 0,
            }
            .validate(),
            Err(ScheduleError::InvalidCron(_))
        ));
    }
    assert!(ScheduleTrigger::Interval { every_ms: 999 }
        .validate()
        .is_err());
}

#[test]
fn cron_matches_studio_next_minute_and_weekday_semantics() {
    let after = chrono::Utc.with_ymd_and_hms(2026, 10, 5, 1, 30, 0).unwrap();
    let trigger = ScheduleTrigger::Cron {
        expression: "0 * * * *".to_string(),
        utc_offset_minutes: 0,
    };
    let next = chrono::DateTime::from_timestamp_millis(
        i64::try_from(
            trigger
                .next_after(after.timestamp_millis() as u64)
                .unwrap()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!((next.hour(), next.minute()), (2, 0));

    let weekday = ScheduleTrigger::Cron {
        expression: "30 10 * * 1-5".to_string(),
        utc_offset_minutes: 0,
    };
    let after = chrono::Utc.with_ymd_and_hms(2026, 10, 5, 11, 0, 0).unwrap();
    let next = chrono::DateTime::from_timestamp_millis(
        i64::try_from(
            weekday
                .next_after(after.timestamp_millis() as u64)
                .unwrap()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(next.weekday().num_days_from_monday(), 1);
    assert_eq!((next.hour(), next.minute()), (10, 30));
}

#[tokio::test]
async fn same_instant_calendar_and_goal_share_one_clock_queue_and_request_shape() {
    let mut fixture = Fixture::new("tenant-a", "workspace-a");
    fixture
        .scheduler
        .put_schedule(once(
            "calendar-one",
            fixture.scope.clone(),
            NOW,
            "calendar-operation",
        ))
        .unwrap();
    fixture.goals.replace(vec![goal(
        &fixture.scope,
        "goal-one",
        NOW,
        "goal-operation",
    )]);

    let outcomes = fixture.scheduler.run_due().await.unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.decision == ScheduleDecision::Completed));
    let requests = fixture.driver.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].source, ScheduleSource::Calendar);
    assert_eq!(requests[1].source, ScheduleSource::GoalUtcWake);
    for request in requests {
        assert_eq!(request.attendance, ExecutionAttendance::Background);
        assert_eq!(request.scope, fixture.scope);
        assert_eq!(request.session_id, "session:scheduled");
        assert_eq!(request.policy, policy());
    }
    assert_eq!(fixture.scheduler.queued_len(), 0);
}

#[tokio::test]
async fn duplicate_fire_and_operation_replays_never_execute_twice() {
    let mut fixture = Fixture::new("tenant-a", "workspace-a");
    fixture
        .scheduler
        .put_schedule(once(
            "calendar-one",
            fixture.scope.clone(),
            NOW,
            "shared-operation",
        ))
        .unwrap();
    fixture.goals.replace(vec![goal(
        &fixture.scope,
        "goal-one",
        NOW,
        &format!("shared-operation:{NOW}"),
    )]);

    let first = fixture.scheduler.run_due().await.unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].decision, ScheduleDecision::Completed);
    assert_eq!(first[1].decision, ScheduleDecision::Conflict);
    assert_eq!(fixture.driver.requests().len(), 1);

    fixture.goals.replace(vec![goal(
        &fixture.scope,
        "goal-one",
        NOW,
        &format!("shared-operation:{NOW}"),
    )]);
    let replay = fixture.scheduler.run_due().await.unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].decision, ScheduleDecision::Conflict);
    assert_eq!(fixture.driver.requests().len(), 1);

    fixture.goals.replace(Vec::new());
    let duplicate = fixture.scheduler.run_due().await.unwrap();
    assert!(duplicate.is_empty());
}

#[tokio::test]
async fn duplicate_same_goal_fire_is_idempotent() {
    let mut fixture = Fixture::new("tenant-a", "workspace-a");
    fixture.goals.replace(vec![goal(
        &fixture.scope,
        "goal-one",
        NOW,
        "goal-operation",
    )]);
    assert_eq!(
        fixture.scheduler.run_due().await.unwrap()[0].decision,
        ScheduleDecision::Completed
    );
    assert_eq!(
        fixture.scheduler.run_due().await.unwrap()[0].decision,
        ScheduleDecision::Duplicate
    );
    fixture.goals.replace(vec![goal(
        &fixture.scope,
        "goal-one",
        NOW,
        "changed-operation",
    )]);
    assert_eq!(
        fixture.scheduler.run_due().await.unwrap()[0].decision,
        ScheduleDecision::Conflict
    );
    assert_eq!(fixture.driver.requests().len(), 1);
}

#[tokio::test]
async fn restart_and_stale_lease_require_reconciliation_without_driver_replay() {
    let fixture = Fixture::new("tenant-a", "workspace-a");
    let snapshot = ScheduleSnapshot {
        scope: fixture.scope.clone(),
        generation: 1,
        schedules: BTreeMap::new(),
        fires: BTreeMap::from([(
            "goal-fire".to_string(),
            leased_fire(fixture.scope.clone(), NOW - 1),
        )]),
    };
    fixture.store.commit(0, snapshot).unwrap();
    fixture.goals.replace(vec![goal(
        &fixture.scope,
        "goal-fire",
        NOW - 1,
        "goal-operation",
    )]);

    let mut restarted = RunScheduler::new(
        fixture.scope.clone(),
        fixture.store.clone(),
        fixture.goals.clone(),
        fixture.driver.clone(),
        fixture.clock.clone(),
        "new-owner",
        60_000,
    )
    .unwrap();
    let recovered = restarted.recover_leased().unwrap();
    assert_eq!(recovered.len(), 1);
    assert!(recovered[0].stale_lease);
    assert_eq!(recovered[0].fire.generation, 3);
    let outcomes = restarted.run_due().await.unwrap();
    assert_eq!(outcomes[0].decision, ScheduleDecision::RecoveryRequired);
    assert!(fixture.driver.requests().is_empty());
}

#[tokio::test]
async fn uncertain_driver_outcome_survives_restart_and_is_not_retried() {
    let mut fixture = Fixture::new("tenant-a", "workspace-a");
    fixture.goals.replace(vec![goal(
        &fixture.scope,
        "goal-fire",
        NOW,
        "goal-operation",
    )]);
    fixture
        .driver
        .push(Err(ExecutionDriverError::new("connection lost after send")));
    let first = fixture.scheduler.run_due().await.unwrap();
    assert_eq!(first[0].decision, ScheduleDecision::Uncertain);
    drop(fixture.scheduler);

    let mut restarted = RunScheduler::new(
        fixture.scope.clone(),
        fixture.store.clone(),
        fixture.goals.clone(),
        fixture.driver.clone(),
        fixture.clock.clone(),
        "new-owner",
        60_000,
    )
    .unwrap();
    assert_eq!(restarted.recover_leased().unwrap().len(), 1);
    assert_eq!(
        restarted.run_due().await.unwrap()[0].decision,
        ScheduleDecision::RecoveryRequired
    );
    assert_eq!(fixture.driver.requests().len(), 1);
}

#[tokio::test]
async fn background_needs_input_and_approval_are_typed_waits() {
    let mut fixture = Fixture::new("tenant-a", "workspace-a");
    fixture.goals.replace(vec![
        goal(&fixture.scope, "goal-input", NOW, "goal-input-operation"),
        goal(
            &fixture.scope,
            "goal-approval",
            NOW,
            "goal-approval-operation",
        ),
    ]);
    fixture.driver.push(Ok(ExecutionDriverOutcome::Waiting {
        run_id: "run:input".to_string(),
        wait: ExecutionWait {
            kind: ExecutionWaitKind::NeedsInput,
            wait_id: "input:one".to_string(),
        },
    }));
    fixture.driver.push(Ok(ExecutionDriverOutcome::Waiting {
        run_id: "run:approval".to_string(),
        wait: ExecutionWait {
            kind: ExecutionWaitKind::AwaitingApproval,
            wait_id: "approval:one".to_string(),
        },
    }));

    let outcomes = fixture.scheduler.run_due().await.unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.decision == ScheduleDecision::Waiting));
    let snapshot = fixture.scheduler.snapshot().unwrap();
    let states: Vec<_> = snapshot
        .fires
        .values()
        .map(|fire| (fire.state, fire.wait.clone().map(|wait| wait.kind)))
        .collect();
    assert!(states.contains(&(
        ScheduleFireState::Waiting,
        Some(ExecutionWaitKind::NeedsInput)
    )));
    assert!(states.contains(&(
        ScheduleFireState::Waiting,
        Some(ExecutionWaitKind::AwaitingApproval)
    )));
    assert_eq!(fixture.scheduler.prune_completed(NOW + 1).unwrap(), 0);
    assert_eq!(fixture.scheduler.snapshot().unwrap().fires.len(), 2);
}

#[tokio::test]
async fn tenant_and_workspace_partitions_isolate_records_and_deduplication() {
    let mut first = Fixture::new("tenant-a", "workspace-a");
    let mut second = Fixture::new("tenant-a", "workspace-b");
    let second_store = first.store.clone();
    second.scheduler = RunScheduler::new(
        second.scope.clone(),
        second_store,
        second.goals.clone(),
        second.driver.clone(),
        second.clock.clone(),
        "second-owner",
        60_000,
    )
    .unwrap();
    second.store = first.store.clone();

    first
        .scheduler
        .put_schedule(once(
            "same-schedule",
            first.scope.clone(),
            NOW,
            "same-operation",
        ))
        .unwrap();
    second
        .scheduler
        .put_schedule(once(
            "same-schedule",
            second.scope.clone(),
            NOW,
            "same-operation",
        ))
        .unwrap();
    first.scheduler.run_due().await.unwrap();
    second.scheduler.run_due().await.unwrap();

    assert_eq!(first.driver.requests().len(), 1);
    assert_eq!(second.driver.requests().len(), 1);
    assert_eq!(first.scheduler.snapshot().unwrap().fires.len(), 1);
    assert_eq!(second.scheduler.snapshot().unwrap().fires.len(), 1);
    assert_ne!(
        first.scheduler.snapshot().unwrap().scope,
        second.scheduler.snapshot().unwrap().scope
    );
    assert!(first
        .scheduler
        .put_schedule(once("wrong", second.scope.clone(), NOW, "wrong"))
        .is_err());
    first.goals.replace(vec![goal(
        &second.scope,
        "cross-goal",
        NOW,
        "cross-operation",
    )]);
    assert!(matches!(
        first.scheduler.run_due().await,
        Err(ScheduleError::InvalidField("goal_wake.scope"))
    ));
    assert_eq!(first.driver.requests().len(), 1);
}

#[tokio::test]
async fn enabled_missed_policy_next_due_and_cleanup_are_durable() {
    let mut fixture = Fixture::new("tenant-a", "workspace-a");
    let mut schedule = CalendarSchedule {
        schedule_id: "interval".to_string(),
        scope: fixture.scope.clone(),
        enabled: false,
        trigger: ScheduleTrigger::Interval { every_ms: 1_000 },
        missed_policy: MissedPolicy::FireOnce,
        next_due_unix_ms: Some(NOW - 10_000),
        operation_id_prefix: "interval-operation".to_string(),
        execution: execution(ExecutionAttendance::Background),
    };
    fixture.scheduler.put_schedule(schedule.clone()).unwrap();
    assert!(fixture.scheduler.run_due().await.unwrap().is_empty());

    schedule.enabled = true;
    fixture.scheduler.put_schedule(schedule).unwrap();
    let fired = fixture.scheduler.run_due().await.unwrap();
    assert_eq!(fired.len(), 1);
    assert_eq!(fixture.driver.requests().len(), 1);
    let snapshot = fixture.scheduler.snapshot().unwrap();
    assert_eq!(
        snapshot.schedules["interval"].next_due_unix_ms,
        Some(NOW + 1_000)
    );
    assert_eq!(fixture.scheduler.prune_completed(NOW + 1).unwrap(), 1);
    assert!(fixture.scheduler.snapshot().unwrap().fires.is_empty());

    let mut skipped = once("skip", fixture.scope.clone(), NOW - 1, "skip-operation");
    skipped.trigger = ScheduleTrigger::Interval { every_ms: 5_000 };
    skipped.missed_policy = MissedPolicy::Skip;
    fixture.scheduler.put_schedule(skipped).unwrap();
    let skipped = fixture.scheduler.run_due().await.unwrap();
    assert_eq!(skipped[0].decision, ScheduleDecision::Skipped);
    assert_eq!(
        fixture.scheduler.snapshot().unwrap().schedules["skip"].next_due_unix_ms,
        Some(NOW + 5_000)
    );
    assert_eq!(fixture.driver.requests().len(), 1);
}

#[tokio::test]
async fn file_adapter_restart_preserves_fire_run_policy_and_pruning() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schedule.json");
    let scope = scope("tenant-a", "workspace-a");
    let store = Arc::new(FileScheduleStore::new(&path));
    let goals = Arc::new(StaticGoalProvider::default());
    let driver = Arc::new(RecordingDriver::default());
    let clock = Arc::new(ManualScheduleClock::new(NOW));
    goals.replace(vec![goal(&scope, "goal-one", NOW, "goal-operation")]);
    let mut scheduler = RunScheduler::new(
        scope.clone(),
        store.clone(),
        goals.clone(),
        driver.clone(),
        clock.clone(),
        "first-owner",
        60_000,
    )
    .unwrap();
    scheduler
        .put_schedule(once(
            "calendar-one",
            scope.clone(),
            NOW + 60_000,
            "calendar-operation",
        ))
        .unwrap();
    scheduler.run_due().await.unwrap();
    let before = scheduler.snapshot().unwrap();
    assert_eq!(
        before.fires["goal-one"].run_id.as_deref(),
        Some("run:goal-operation")
    );
    assert_eq!(before.fires["goal-one"].request.policy, policy());
    drop(scheduler);
    drop(store);

    let reopened = Arc::new(FileScheduleStore::new(&path));
    let mut restarted = RunScheduler::new(
        scope.clone(),
        reopened,
        goals,
        driver.clone(),
        clock.clone(),
        "second-owner",
        60_000,
    )
    .unwrap();
    assert_eq!(restarted.snapshot().unwrap(), before);
    clock.set(NOW + 60_000);
    let replayed = restarted.run_due().await.unwrap();
    assert_eq!(replayed.len(), 2);
    assert_eq!(replayed[0].decision, ScheduleDecision::Duplicate);
    assert_eq!(replayed[1].decision, ScheduleDecision::Completed);
    assert_eq!(driver.requests().len(), 2);
    assert_eq!(restarted.prune_completed(NOW + 60_001).unwrap(), 2);
    assert!(restarted.snapshot().unwrap().fires.is_empty());
    assert_eq!(restarted.snapshot().unwrap().generation, 6);
}

#[test]
fn file_adapter_rejects_unknown_cron_and_stale_generation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schedule.json");
    let store = FileScheduleStore::new(&path);
    let scope = scope("tenant-a", "workspace-a");
    let mut snapshot = ScheduleSnapshot::new(scope.clone());
    snapshot.schedules.insert(
        "calendar".to_string(),
        CalendarSchedule {
            schedule_id: "calendar".to_string(),
            scope: scope.clone(),
            enabled: true,
            trigger: ScheduleTrigger::Cron {
                expression: "0 * * * *".to_string(),
                utc_offset_minutes: 0,
            },
            missed_policy: MissedPolicy::FireOnce,
            next_due_unix_ms: Some(NOW),
            operation_id_prefix: "operation".to_string(),
            execution: execution(ExecutionAttendance::Background),
        },
    );
    snapshot.generation = 1;
    store.commit(0, snapshot.clone()).unwrap();
    assert!(matches!(
        store.commit(0, snapshot),
        Err(ScheduleError::Conflict)
    ));

    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["partitions"][0]["schedules"]["calendar"]["trigger"]["expression"] =
        serde_json::Value::String("@daily".to_string());
    std::fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    assert!(matches!(
        store.load(&scope),
        Err(ScheduleError::InvalidCron(_))
    ));
}
