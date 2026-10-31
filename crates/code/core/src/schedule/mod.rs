//! Durable wall-clock scheduling for calendar fires and Goal UTC wakes.
//!
//! This module owns due/fire/lease semantics only. Capacity admission remains
//! with [`crate::task_scheduler::TaskScheduler`], and model execution remains
//! behind [`ScheduleExecutionDriver`]. Calling [`RunScheduler::run_due`] is the
//! only wall-clock driver; this module does not spawn a timer or an execution
//! loop.

use crate::goal_control::{GoalWaitConditionV1, GoalWaitV1};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

mod cron;
mod store;
#[cfg(test)]
mod tests;

pub use store::{FileScheduleStore, MemoryScheduleStore};

pub const SCHEDULE_STORE_SCHEMA_V1: &str = "aster.code.schedule-store.v1";
pub const SCHEDULE_STORE_MAX_BYTES: usize = 16 * 1024 * 1024;

macro_rules! closed_set {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }

            pub fn parse(value: &str) -> Result<Self, ScheduleError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(ScheduleError::UnknownClosedSet {
                        set: stringify!($name), value: value.to_owned()
                    }),
                }
            }
        }

        impl TryFrom<String> for $name {
            type Error = ScheduleError;
            fn try_from(value: String) -> Result<Self, Self::Error> { Self::parse(&value) }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self { value.as_str().to_owned() }
        }
    };
}

closed_set!(ScheduleSource {
    Calendar => "calendar", GoalUtcWake => "goal_utc_wake",
});
closed_set!(ExecutionAttendance {
    Interactive => "interactive", Background => "background",
});
closed_set!(ScheduleTriggerKind {
    Once => "once", Interval => "interval", Cron => "cron",
});
closed_set!(MissedPolicy {
    FireOnce => "fire_once", Skip => "skip",
});
closed_set!(ScheduleFireState {
    Leased => "leased", Waiting => "waiting", Completed => "completed",
    Failed => "failed", Uncertain => "uncertain",
});
closed_set!(ExecutionWaitKind {
    NeedsInput => "needs_input", AwaitingApproval => "awaiting_approval",
});
closed_set!(ScheduleDecision {
    Admitted => "admitted", Completed => "completed", Failed => "failed",
    Waiting => "waiting", Duplicate => "duplicate", Deferred => "deferred",
    Skipped => "skipped", Conflict => "conflict",
    RecoveryRequired => "recovery_required", Uncertain => "uncertain",
});

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScheduleError {
    #[error("unknown {set} value '{value}'")]
    UnknownClosedSet { set: &'static str, value: String },
    #[error("schedule field `{0}` is invalid")]
    InvalidField(&'static str),
    #[error("cron expression is outside the supported five-field subset: {0}")]
    InvalidCron(String),
    #[error("schedule store schema is unsupported")]
    UnsupportedSchema,
    #[error("schedule generation or operation identity conflicts with durable state")]
    Conflict,
    #[error("schedule store failed: {0}")]
    Store(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("scheduled execution driver failed: {message}")]
pub struct ExecutionDriverError {
    pub message: String,
}

impl ExecutionDriverError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Runtime-cell isolation boundary. Stores and deduplication never cross it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleScope {
    pub tenant_id: String,
    pub principal_id: String,
    pub workspace_id: String,
}

impl ScheduleScope {
    pub fn validate(&self) -> Result<(), ScheduleError> {
        validate_id("tenant_id", &self.tenant_id)?;
        validate_id("principal_id", &self.principal_id)?;
        validate_id("workspace_id", &self.workspace_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeadlinePolicyIdentity {
    pub policy_id: String,
    pub deadline_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetPolicyIdentity {
    pub policy_id: String,
}

/// Existing Core policy identities; no permission or budget is inferred here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPolicyIdentity {
    pub permission_profile_id: String,
    pub continuation_policy_id: Option<String>,
    pub deadline: Option<DeadlinePolicyIdentity>,
    pub budget: Option<BudgetPolicyIdentity>,
}

impl ExecutionPolicyIdentity {
    pub fn validate(&self) -> Result<(), ScheduleError> {
        validate_id("permission_profile_id", &self.permission_profile_id)?;
        if let Some(policy_id) = &self.continuation_policy_id {
            validate_id("continuation_policy_id", policy_id)?;
        }
        if let Some(deadline) = &self.deadline {
            validate_id("deadline.policy_id", &deadline.policy_id)?;
        }
        if let Some(budget) = &self.budget {
            validate_id("budget.policy_id", &budget.policy_id)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionTemplate {
    pub attendance: ExecutionAttendance,
    pub session_id: String,
    pub policy: ExecutionPolicyIdentity,
}

impl ExecutionTemplate {
    pub fn validate(&self) -> Result<(), ScheduleError> {
        validate_id("session_id", &self.session_id)?;
        self.policy.validate()
    }
}

/// The single execution shape emitted for both wall-clock sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRequest {
    pub source: ScheduleSource,
    pub attendance: ExecutionAttendance,
    pub scope: ScheduleScope,
    pub session_id: String,
    pub operation_id: String,
    pub policy: ExecutionPolicyIdentity,
}

impl ExecutionRequest {
    pub fn validate(&self) -> Result<(), ScheduleError> {
        self.scope.validate()?;
        validate_id("session_id", &self.session_id)?;
        validate_id("operation_id", &self.operation_id)?;
        self.policy.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScheduleTrigger {
    Once {
        at_unix_ms: u64,
    },
    Interval {
        every_ms: u64,
    },
    Cron {
        expression: String,
        utc_offset_minutes: i32,
    },
}

impl ScheduleTrigger {
    pub fn kind(&self) -> ScheduleTriggerKind {
        match self {
            Self::Once { .. } => ScheduleTriggerKind::Once,
            Self::Interval { .. } => ScheduleTriggerKind::Interval,
            Self::Cron { .. } => ScheduleTriggerKind::Cron,
        }
    }

    pub fn validate(&self) -> Result<(), ScheduleError> {
        match self {
            Self::Once { .. } => Ok(()),
            Self::Interval { every_ms } if *every_ms >= 1_000 => Ok(()),
            Self::Interval { .. } => Err(ScheduleError::InvalidField("interval.every_ms")),
            Self::Cron {
                expression,
                utc_offset_minutes,
            } => cron::validate(expression, *utc_offset_minutes),
        }
    }

    /// Compute the next occurrence strictly after the supplied instant. A
    /// one-shot has no next occurrence; its initial `next_due` is `at_unix_ms`.
    pub fn next_after(&self, after_unix_ms: u64) -> Result<Option<u64>, ScheduleError> {
        self.validate()?;
        match self {
            Self::Once { .. } => Ok(None),
            Self::Interval { every_ms } => after_unix_ms
                .checked_add(*every_ms)
                .map(Some)
                .ok_or(ScheduleError::InvalidField("next_due_unix_ms")),
            Self::Cron {
                expression,
                utc_offset_minutes,
            } => cron::next_after(expression, *utc_offset_minutes, after_unix_ms),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarSchedule {
    pub schedule_id: String,
    pub scope: ScheduleScope,
    pub enabled: bool,
    pub trigger: ScheduleTrigger,
    pub missed_policy: MissedPolicy,
    pub next_due_unix_ms: Option<u64>,
    /// Host-supplied operation identity prefix. The typed occurrence appends
    /// its UTC due instant; prompt text is never inspected.
    pub operation_id_prefix: String,
    pub execution: ExecutionTemplate,
}

impl CalendarSchedule {
    pub fn validate(&self) -> Result<(), ScheduleError> {
        validate_id("schedule_id", &self.schedule_id)?;
        self.scope.validate()?;
        self.trigger.validate()?;
        validate_id("operation_id_prefix", &self.operation_id_prefix)?;
        self.execution.validate()?;
        if let (ScheduleTrigger::Once { at_unix_ms }, Some(next_due_unix_ms)) =
            (&self.trigger, self.next_due_unix_ms)
        {
            if *at_unix_ms != next_due_unix_ms {
                return Err(ScheduleError::InvalidField("next_due_unix_ms"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleLease {
    pub owner_id: String,
    pub lease_expires_at_unix_ms: u64,
}

/// Durable occurrence identity and its operation/Run association.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleFire {
    pub fire_id: String,
    pub operation_id: String,
    pub request: ExecutionRequest,
    pub due_unix_ms: u64,
    pub state: ScheduleFireState,
    pub run_id: Option<String>,
    pub generation: u64,
    pub lease: Option<ScheduleLease>,
    pub wait: Option<ExecutionWait>,
    pub error: Option<String>,
    pub finished_at_unix_ms: Option<u64>,
}

impl ScheduleFire {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            ScheduleFireState::Completed | ScheduleFireState::Failed
        )
    }
}

/// One store partition. `generation` fences every compare-and-swap commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleSnapshot {
    pub scope: ScheduleScope,
    pub generation: u64,
    pub schedules: BTreeMap<String, CalendarSchedule>,
    pub fires: BTreeMap<String, ScheduleFire>,
}

impl ScheduleSnapshot {
    pub fn new(scope: ScheduleScope) -> Self {
        Self {
            scope,
            generation: 0,
            schedules: BTreeMap::new(),
            fires: BTreeMap::new(),
        }
    }
}

/// Atomic durable contract used by both adapters and embedders.
///
/// `commit` must compare the persisted generation with `expected_generation`
/// and persist `next` atomically, or return [`ScheduleError::Conflict`]. It
/// must not merge records or fall back to another store.
pub trait ScheduleStore: Send + Sync {
    fn load(&self, scope: &ScheduleScope) -> Result<ScheduleSnapshot, ScheduleError>;
    fn commit(&self, expected_generation: u64, next: ScheduleSnapshot)
        -> Result<(), ScheduleError>;
}

/// Typed adapter boundary for hosts that own Goal facts. Implementations
/// project open `GoalWaitConditionV1::UtcDue` waits; the Goal engine is not
/// modified and no additional timer is installed.
pub trait GoalUtcWakeProvider: Send + Sync {
    fn open_utc_wakes(&self, scope: &ScheduleScope) -> Result<Vec<GoalUtcWake>, ScheduleError>;
}

#[derive(Debug, Clone)]
pub struct GoalUtcWake {
    pub scope: ScheduleScope,
    pub fire_id: String,
    pub goal_id: String,
    pub wait: GoalWaitV1,
    pub operation_id: String,
    pub execution: ExecutionTemplate,
}

impl GoalUtcWake {
    pub fn due_unix_ms(&self) -> Result<Option<u64>, ScheduleError> {
        if self.wait.observed.is_some() {
            return Ok(None);
        }
        match &self.wait.condition {
            GoalWaitConditionV1::UtcDue { due_unix_ms } => Ok(Some(due_unix_ms.get())),
            _ => Err(ScheduleError::InvalidField("goal_wait.condition")),
        }
    }
}

#[derive(Debug, Default)]
pub struct NoGoalUtcWakeProvider;

impl GoalUtcWakeProvider for NoGoalUtcWakeProvider {
    fn open_utc_wakes(&self, _scope: &ScheduleScope) -> Result<Vec<GoalUtcWake>, ScheduleError> {
        Ok(Vec::new())
    }
}

pub trait ScheduleClock: Send + Sync {
    fn now_unix_ms(&self) -> u64;
}

#[derive(Debug, Default)]
pub struct SystemScheduleClock;

impl ScheduleClock for SystemScheduleClock {
    fn now_unix_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            })
    }
}

#[derive(Debug, Default)]
pub struct ManualScheduleClock {
    now_unix_ms: AtomicU64,
}

impl ManualScheduleClock {
    pub fn new(now_unix_ms: u64) -> Self {
        Self {
            now_unix_ms: AtomicU64::new(now_unix_ms),
        }
    }

    pub fn set(&self, now_unix_ms: u64) {
        self.now_unix_ms.store(now_unix_ms, Ordering::Release);
    }

    pub fn advance(&self, delta_ms: u64) -> u64 {
        self.now_unix_ms.fetch_add(delta_ms, Ordering::AcqRel) + delta_ms
    }
}

impl ScheduleClock for ManualScheduleClock {
    fn now_unix_ms(&self) -> u64 {
        self.now_unix_ms.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionWait {
    pub kind: ExecutionWaitKind,
    pub wait_id: String,
}

impl ExecutionWait {
    pub fn validate(&self) -> Result<(), ScheduleError> {
        validate_id("wait_id", &self.wait_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionDriverOutcome {
    Completed {
        run_id: String,
    },
    Failed {
        run_id: String,
        error: String,
    },
    Waiting {
        run_id: String,
        wait: ExecutionWait,
    },
    /// The driver cannot prove whether side effects settled. The scheduler
    /// persists this state and requires reconciliation instead of retrying.
    Uncertain {
        run_id: Option<String>,
        error: String,
    },
}

#[async_trait]
pub trait ScheduleExecutionDriver: Send + Sync {
    async fn execute(
        &self,
        request: ExecutionRequest,
    ) -> Result<ExecutionDriverOutcome, ExecutionDriverError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleOutcome {
    pub decision: ScheduleDecision,
    pub fire_id: String,
    pub operation_id: String,
    pub run_id: Option<String>,
    pub wait: Option<ExecutionWait>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRecovery {
    pub fire: ScheduleFire,
    pub stale_lease: bool,
}

#[derive(Debug, Clone)]
struct QueuedExecution {
    fire_id: String,
    due_unix_ms: u64,
    request: ExecutionRequest,
    calendar: Option<CalendarSchedule>,
}

struct ScheduleAdmission {
    decision: ScheduleDecision,
    fire: ScheduleFire,
}

/// One scoped wall-clock queue for calendar and Goal UTC wake admissions.
pub struct RunScheduler {
    scope: ScheduleScope,
    store: Arc<dyn ScheduleStore>,
    goal_provider: Arc<dyn GoalUtcWakeProvider>,
    driver: Arc<dyn ScheduleExecutionDriver>,
    clock: Arc<dyn ScheduleClock>,
    owner_id: String,
    lease_duration_ms: u64,
    queue: BTreeMap<(u64, String), QueuedExecution>,
}

impl RunScheduler {
    pub fn new(
        scope: ScheduleScope,
        store: Arc<dyn ScheduleStore>,
        goal_provider: Arc<dyn GoalUtcWakeProvider>,
        driver: Arc<dyn ScheduleExecutionDriver>,
        clock: Arc<dyn ScheduleClock>,
        owner_id: impl Into<String>,
        lease_duration_ms: u64,
    ) -> Result<Self, ScheduleError> {
        scope.validate()?;
        let owner_id = owner_id.into();
        validate_id("owner_id", &owner_id)?;
        if lease_duration_ms == 0 {
            return Err(ScheduleError::InvalidField("lease_duration_ms"));
        }
        Ok(Self {
            scope,
            store,
            goal_provider,
            driver,
            clock,
            owner_id,
            lease_duration_ms,
            queue: BTreeMap::new(),
        })
    }

    pub fn scope(&self) -> &ScheduleScope {
        &self.scope
    }

    pub fn queued_len(&self) -> usize {
        self.queue.len()
    }

    pub fn snapshot(&self) -> Result<ScheduleSnapshot, ScheduleError> {
        self.store.load(&self.scope)
    }

    pub fn put_schedule(&self, schedule: CalendarSchedule) -> Result<(), ScheduleError> {
        schedule.validate()?;
        if schedule.scope != self.scope {
            return Err(ScheduleError::InvalidField("schedule.scope"));
        }
        let mut next = self.store.load(&self.scope)?;
        next.schedules
            .insert(schedule.schedule_id.clone(), schedule);
        self.commit(next)
    }

    pub fn set_schedule_enabled(
        &self,
        schedule_id: &str,
        enabled: bool,
    ) -> Result<(), ScheduleError> {
        let mut next = self.store.load(&self.scope)?;
        let schedule = next
            .schedules
            .get_mut(schedule_id)
            .ok_or(ScheduleError::InvalidField("schedule_id"))?;
        schedule.enabled = enabled;
        self.commit(next)
    }

    /// Collect both source types using one clock read and one due-ordered queue.
    /// Returned decisions are non-executing skips; admissions occur in
    /// [`Self::run_due`].
    pub fn collect_due(&mut self) -> Result<Vec<ScheduleOutcome>, ScheduleError> {
        let now = self.clock.now_unix_ms();
        let snapshot = self.store.load(&self.scope)?;
        let mut outcomes = Vec::new();
        for schedule in snapshot.schedules.values() {
            if schedule.scope != self.scope {
                return Err(ScheduleError::InvalidField("schedule.scope"));
            }
            schedule.validate()?;
            if !schedule.enabled {
                continue;
            }
            let Some(due) = schedule.next_due_unix_ms else {
                continue;
            };
            if due > now {
                continue;
            }
            let fire_id = calendar_fire_id(&schedule.schedule_id, due);
            if schedule.missed_policy == MissedPolicy::Skip {
                let mut next = self.store.load(&self.scope)?;
                let current = next
                    .schedules
                    .get_mut(&schedule.schedule_id)
                    .ok_or(ScheduleError::InvalidField("schedule_id"))?;
                if current != schedule {
                    return Err(ScheduleError::Conflict);
                }
                current.next_due_unix_ms = schedule.trigger.next_after(now)?;
                self.commit(next)?;
                outcomes.push(outcome(
                    ScheduleDecision::Skipped,
                    fire_id,
                    calendar_operation_id(&schedule.operation_id_prefix, due),
                ));
                continue;
            }
            self.queue_candidate(QueuedExecution {
                fire_id,
                due_unix_ms: due,
                request: ExecutionRequest {
                    source: ScheduleSource::Calendar,
                    attendance: schedule.execution.attendance,
                    scope: self.scope.clone(),
                    session_id: schedule.execution.session_id.clone(),
                    operation_id: calendar_operation_id(&schedule.operation_id_prefix, due),
                    policy: schedule.execution.policy.clone(),
                },
                calendar: Some(schedule.clone()),
            })?;
        }

        for wake in self.goal_provider.open_utc_wakes(&self.scope)? {
            if wake.scope != self.scope {
                return Err(ScheduleError::InvalidField("goal_wake.scope"));
            }
            validate_id("goal_id", &wake.goal_id)?;
            validate_id("fire_id", &wake.fire_id)?;
            validate_id("operation_id", &wake.operation_id)?;
            wake.execution.validate()?;
            let Some(due) = wake.due_unix_ms()? else {
                continue;
            };
            if due > now {
                continue;
            }
            self.queue_candidate(QueuedExecution {
                fire_id: wake.fire_id,
                due_unix_ms: due,
                request: ExecutionRequest {
                    source: ScheduleSource::GoalUtcWake,
                    attendance: wake.execution.attendance,
                    scope: self.scope.clone(),
                    session_id: wake.execution.session_id,
                    operation_id: wake.operation_id,
                    policy: wake.execution.policy,
                },
                calendar: None,
            })?;
        }
        Ok(outcomes)
    }

    /// Collect and drain all currently due work in `(due, fire_id)` order.
    /// Driver calls are serialized through this single queue.
    pub async fn run_due(&mut self) -> Result<Vec<ScheduleOutcome>, ScheduleError> {
        let mut outcomes = self.collect_due()?;
        while let Some((_, candidate)) = self.queue.pop_first() {
            let admission = self.admit(candidate)?;
            if admission.decision != ScheduleDecision::Admitted {
                outcomes.push(outcome_from_fire(admission.decision, &admission.fire));
                continue;
            }
            let request = admission.fire.request.clone();
            let driven = self.driver.execute(request).await;
            let (decision, run_id, wait, error, terminal) = match driven {
                Ok(ExecutionDriverOutcome::Completed { run_id }) => {
                    validate_id("run_id", &run_id)?;
                    (ScheduleDecision::Completed, Some(run_id), None, None, true)
                }
                Ok(ExecutionDriverOutcome::Failed { run_id, error }) => {
                    validate_id("run_id", &run_id)?;
                    (
                        ScheduleDecision::Failed,
                        Some(run_id),
                        None,
                        Some(error),
                        true,
                    )
                }
                Ok(ExecutionDriverOutcome::Waiting { run_id, wait }) => {
                    validate_id("run_id", &run_id)?;
                    wait.validate()?;
                    (
                        ScheduleDecision::Waiting,
                        Some(run_id),
                        Some(wait),
                        None,
                        false,
                    )
                }
                Ok(ExecutionDriverOutcome::Uncertain { run_id, error }) => {
                    if let Some(run_id) = &run_id {
                        validate_id("run_id", run_id)?;
                    }
                    (
                        ScheduleDecision::Uncertain,
                        run_id,
                        None,
                        Some(error),
                        false,
                    )
                }
                Err(error) => (
                    ScheduleDecision::Uncertain,
                    None,
                    None,
                    Some(error.message),
                    false,
                ),
            };
            let fire = self.settle(
                &admission.fire.fire_id,
                admission.fire.generation,
                decision,
                run_id,
                wait,
                error,
                terminal,
            )?;
            outcomes.push(outcome_from_fire(decision, &fire));
        }
        Ok(outcomes)
    }

    /// Recover leased and uncertain occurrences for explicit reconciliation.
    /// No driver call or automatic retry is performed, including stale leases.
    pub fn recover_leased(&self) -> Result<Vec<ScheduleRecovery>, ScheduleError> {
        let now = self.clock.now_unix_ms();
        let snapshot = self.store.load(&self.scope)?;
        Ok(snapshot
            .fires
            .values()
            .filter(|fire| {
                matches!(
                    fire.state,
                    ScheduleFireState::Leased | ScheduleFireState::Uncertain
                )
            })
            .map(|fire| ScheduleRecovery {
                stale_lease: fire.state == ScheduleFireState::Uncertain
                    || fire
                        .lease
                        .as_ref()
                        .is_some_and(|lease| lease.lease_expires_at_unix_ms <= now),
                fire: fire.clone(),
            })
            .collect())
    }

    /// Remove only known terminal records. Waiting and uncertain executions
    /// remain durable and cannot be silently cleaned into a retry.
    pub fn prune_completed(&self, before_unix_ms: u64) -> Result<usize, ScheduleError> {
        let mut next = self.store.load(&self.scope)?;
        let before = next.fires.len();
        next.fires.retain(|_, fire| {
            !(fire.is_terminal()
                && fire
                    .finished_at_unix_ms
                    .is_some_and(|finished| finished < before_unix_ms))
        });
        let removed = before - next.fires.len();
        if removed > 0 {
            self.commit(next)?;
        }
        Ok(removed)
    }

    fn queue_candidate(&mut self, candidate: QueuedExecution) -> Result<(), ScheduleError> {
        candidate.request.validate()?;
        if candidate.request.scope != self.scope {
            return Err(ScheduleError::InvalidField("request.scope"));
        }
        validate_id("fire_id", &candidate.fire_id)?;
        let key = (candidate.due_unix_ms, candidate.fire_id.clone());
        self.queue.entry(key).or_insert(candidate);
        Ok(())
    }

    fn admit(&self, candidate: QueuedExecution) -> Result<ScheduleAdmission, ScheduleError> {
        let now = self.clock.now_unix_ms();
        let mut next = self.store.load(&self.scope)?;
        if let Some(existing) = next.fires.get(&candidate.fire_id) {
            let decision = if existing.operation_id != candidate.request.operation_id
                || existing.request != candidate.request
            {
                ScheduleDecision::Conflict
            } else if matches!(
                existing.state,
                ScheduleFireState::Leased | ScheduleFireState::Uncertain
            ) {
                ScheduleDecision::RecoveryRequired
            } else {
                ScheduleDecision::Duplicate
            };
            return Ok(ScheduleAdmission {
                decision,
                fire: existing.clone(),
            });
        }
        if let Some(existing) = next
            .fires
            .values()
            .find(|fire| fire.operation_id == candidate.request.operation_id)
        {
            return Ok(ScheduleAdmission {
                decision: ScheduleDecision::Conflict,
                fire: existing.clone(),
            });
        }
        if let Some(expected) = &candidate.calendar {
            let current = next
                .schedules
                .get(&expected.schedule_id)
                .ok_or(ScheduleError::InvalidField("schedule_id"))?;
            if current != expected
                || !current.enabled
                || current.next_due_unix_ms != Some(candidate.due_unix_ms)
            {
                return Ok(ScheduleAdmission {
                    decision: ScheduleDecision::Deferred,
                    fire: unattributed_fire(&candidate),
                });
            }
        }

        let fire = ScheduleFire {
            fire_id: candidate.fire_id.clone(),
            operation_id: candidate.request.operation_id.clone(),
            request: candidate.request,
            due_unix_ms: candidate.due_unix_ms,
            state: ScheduleFireState::Leased,
            run_id: None,
            generation: 1,
            lease: Some(ScheduleLease {
                owner_id: self.owner_id.clone(),
                lease_expires_at_unix_ms: now
                    .checked_add(self.lease_duration_ms)
                    .ok_or(ScheduleError::InvalidField("lease_expires_at_unix_ms"))?,
            }),
            wait: None,
            error: None,
            finished_at_unix_ms: None,
        };
        if let Some(expected) = &candidate.calendar {
            let schedule = next
                .schedules
                .get_mut(&expected.schedule_id)
                .ok_or(ScheduleError::InvalidField("schedule_id"))?;
            schedule.next_due_unix_ms = schedule.trigger.next_after(now)?;
        }
        next.fires.insert(fire.fire_id.clone(), fire.clone());
        self.commit(next)?;
        Ok(ScheduleAdmission {
            decision: ScheduleDecision::Admitted,
            fire,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn settle(
        &self,
        fire_id: &str,
        generation: u64,
        decision: ScheduleDecision,
        run_id: Option<String>,
        wait: Option<ExecutionWait>,
        error: Option<String>,
        terminal: bool,
    ) -> Result<ScheduleFire, ScheduleError> {
        let mut next = self.store.load(&self.scope)?;
        let fire = next
            .fires
            .get_mut(fire_id)
            .ok_or(ScheduleError::InvalidField("fire_id"))?;
        if fire.state != ScheduleFireState::Leased
            || fire.generation != generation
            || fire.lease.as_ref().map(|lease| &lease.owner_id) != Some(&self.owner_id)
        {
            return Err(ScheduleError::Conflict);
        }
        fire.state = match decision {
            ScheduleDecision::Completed => ScheduleFireState::Completed,
            ScheduleDecision::Failed => ScheduleFireState::Failed,
            ScheduleDecision::Waiting => ScheduleFireState::Waiting,
            ScheduleDecision::Uncertain => ScheduleFireState::Uncertain,
            _ => return Err(ScheduleError::InvalidField("decision")),
        };
        fire.run_id = run_id;
        fire.wait = wait;
        fire.error = error;
        fire.lease = None;
        fire.finished_at_unix_ms = terminal.then(|| self.clock.now_unix_ms());
        let fire = fire.clone();
        self.commit(next)?;
        Ok(fire)
    }

    fn commit(&self, mut next: ScheduleSnapshot) -> Result<(), ScheduleError> {
        if next.scope != self.scope {
            return Err(ScheduleError::InvalidField("snapshot.scope"));
        }
        let expected = next.generation;
        next.generation = expected
            .checked_add(1)
            .ok_or(ScheduleError::InvalidField("generation"))?;
        validate_snapshot(&next)?;
        self.store.commit(expected, next)
    }
}

pub fn calendar_fire_id(schedule_id: &str, due_unix_ms: u64) -> String {
    format!("calendar:{}:{schedule_id}:{due_unix_ms}", schedule_id.len())
}

fn calendar_operation_id(prefix: &str, due_unix_ms: u64) -> String {
    format!("{prefix}:{due_unix_ms}")
}

fn outcome(decision: ScheduleDecision, fire_id: String, operation_id: String) -> ScheduleOutcome {
    ScheduleOutcome {
        decision,
        fire_id,
        operation_id,
        run_id: None,
        wait: None,
    }
}

fn outcome_from_fire(decision: ScheduleDecision, fire: &ScheduleFire) -> ScheduleOutcome {
    ScheduleOutcome {
        decision,
        fire_id: fire.fire_id.clone(),
        operation_id: fire.operation_id.clone(),
        run_id: fire.run_id.clone(),
        wait: fire.wait.clone(),
    }
}

fn unattributed_fire(candidate: &QueuedExecution) -> ScheduleFire {
    ScheduleFire {
        fire_id: candidate.fire_id.clone(),
        operation_id: candidate.request.operation_id.clone(),
        request: candidate.request.clone(),
        due_unix_ms: candidate.due_unix_ms,
        state: ScheduleFireState::Waiting,
        run_id: None,
        generation: 0,
        lease: None,
        wait: None,
        error: None,
        finished_at_unix_ms: None,
    }
}

fn validate_id(field: &'static str, value: &str) -> Result<(), ScheduleError> {
    if value.is_empty()
        || value.len() > 512
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
    {
        return Err(ScheduleError::InvalidField(field));
    }
    Ok(())
}

pub(crate) fn validate_snapshot(snapshot: &ScheduleSnapshot) -> Result<(), ScheduleError> {
    snapshot.scope.validate()?;
    for (schedule_id, schedule) in &snapshot.schedules {
        if schedule_id != &schedule.schedule_id || schedule.scope != snapshot.scope {
            return Err(ScheduleError::InvalidField("schedules"));
        }
        schedule.validate()?;
    }
    let mut operation_ids = std::collections::BTreeSet::new();
    for (fire_id, fire) in &snapshot.fires {
        if fire_id != &fire.fire_id
            || fire.operation_id != fire.request.operation_id
            || fire.request.scope != snapshot.scope
            || fire.generation == 0
            || !operation_ids.insert(&fire.operation_id)
        {
            return Err(ScheduleError::InvalidField("fires"));
        }
        validate_id("fire_id", &fire.fire_id)?;
        fire.request.validate()?;
        if let Some(run_id) = &fire.run_id {
            validate_id("run_id", run_id)?;
        }
        if let Some(wait) = &fire.wait {
            wait.validate()?;
        }
        match fire.state {
            ScheduleFireState::Leased => {
                let lease = fire
                    .lease
                    .as_ref()
                    .ok_or(ScheduleError::InvalidField("lease"))?;
                validate_id("lease.owner_id", &lease.owner_id)?;
            }
            ScheduleFireState::Waiting if fire.lease.is_none() && fire.wait.is_some() => {}
            ScheduleFireState::Completed | ScheduleFireState::Failed
                if fire.lease.is_none()
                    && fire.run_id.is_some()
                    && fire.finished_at_unix_ms.is_some() => {}
            ScheduleFireState::Uncertain if fire.lease.is_none() => {}
            _ => return Err(ScheduleError::InvalidField("fire.state")),
        }
    }
    Ok(())
}
