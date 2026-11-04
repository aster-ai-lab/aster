//! Session ownership for explicit background child runs.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex, Weak};

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use crate::capability::{
    CapabilityCatalog, CapabilityCatalogStamp, CapabilityCeiling, CapabilityEffectError,
    CapabilityProjectionLease, EffectSupervisor, ScopeClosePolicy, ScopeCloseReport,
    SessionCapabilityRun,
};
use crate::hooks::{HookEngine, HookExecutor};

pub(crate) struct BackgroundTasks {
    supervisor: EffectSupervisor,
    live: Mutex<BTreeMap<String, Option<String>>>,
}

impl BackgroundTasks {
    pub(super) fn new(session: &str, cancellation: CancellationToken) -> Arc<Self> {
        Arc::new(Self {
            supervisor: EffectSupervisor::new(
                format!("background:{session}"),
                cancellation,
                ScopeClosePolicy::default(),
            ),
            live: Mutex::new(BTreeMap::new()),
        })
    }

    pub(crate) fn live(&self) -> BTreeMap<String, Option<String>> {
        self.live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) async fn close(&self) -> Result<ScopeCloseReport> {
        self.supervisor.close().await.map_err(Into::into)
    }
}

pub(crate) struct BackgroundAdmission {
    owner: Weak<BackgroundTasks>,
    catalog: Arc<CapabilityCatalog>,
    expected: CapabilityCatalogStamp,
    ceiling: CapabilityCeiling,
    cancellation: CancellationToken,
    hooks: Arc<HookEngine>,
    external_hooks: Option<Arc<dyn HookExecutor>>,
    persistence: super::SessionPersistenceContext,
}

impl BackgroundAdmission {
    pub(super) fn new(
        session: &super::AgentSession,
        projection: &CapabilityProjectionLease,
        ceiling: CapabilityCeiling,
        cancellation: CancellationToken,
        hooks: Arc<HookEngine>,
        external_hooks: Option<Arc<dyn HookExecutor>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            owner: Arc::downgrade(&session.background_tasks),
            catalog: Arc::clone(&session.capability_catalog),
            expected: projection.stamp().clone(),
            ceiling,
            cancellation,
            hooks,
            external_hooks,
            persistence: super::SessionPersistenceContext::from_session(session),
        })
    }

    pub(crate) fn persistence(&self) -> &super::SessionPersistenceContext {
        &self.persistence
    }

    pub(crate) fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub(crate) fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub(crate) fn capture(&self, context: &crate::tools::ToolContext) -> Result<BackgroundPlan> {
        context
            .capability_context()
            .ok_or_else(|| anyhow::anyhow!("background invocation has no capability scope"))?
            .ensure_active()?;
        anyhow::ensure!(
            !self.cancellation.is_cancelled(),
            "background invocation cancelled"
        );
        let projection = self.catalog.pin();
        anyhow::ensure!(
            projection.stamp() == &self.expected,
            "background capability generation changed before admission"
        );
        let hooks = Arc::new(self.hooks.snapshot_with_external_hooks([], true)?);
        Ok(BackgroundPlan {
            projection,
            ceiling: self.ceiling.clone(),
            cancellation: self.cancellation.child_token(),
            hooks,
            external_hooks: self.external_hooks.clone(),
        })
    }

    pub(crate) fn spawn<F>(
        &self,
        task_id: String,
        parent_key: Option<String>,
        task: F,
    ) -> Result<()>
    where
        F: Future<Output = Result<(), CapabilityEffectError>> + Send + 'static,
    {
        let owner = self
            .owner
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("background session is closed"))?;
        owner
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(task_id.clone(), parent_key);
        let ticket = BackgroundTicket {
            owner: Arc::downgrade(&owner),
            task_id: task_id.clone(),
        };
        owner
            .supervisor
            .spawn_task(format!("subagent.{task_id}"), async move {
                let _ticket = ticket;
                task.await
            })?;
        Ok(())
    }

    pub(crate) fn live(&self) -> BTreeMap<String, Option<String>> {
        self.owner
            .upgrade()
            .map(|owner| owner.live())
            .unwrap_or_default()
    }
}

// A job must not retain the Session supervisor that owns it.
struct BackgroundTicket {
    owner: Weak<BackgroundTasks>,
    task_id: String,
}
impl Drop for BackgroundTicket {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            owner
                .live
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&self.task_id);
        }
    }
}

pub(crate) struct BackgroundPlan {
    projection: CapabilityProjectionLease,
    ceiling: CapabilityCeiling,
    cancellation: CancellationToken,
    hooks: Arc<HookEngine>,
    external_hooks: Option<Arc<dyn HookExecutor>>,
}
impl BackgroundPlan {
    pub(crate) fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub(crate) async fn admit(
        self,
        task_id: &str,
        workspace: &std::path::Path,
    ) -> Result<(SessionCapabilityRun, Arc<dyn HookExecutor>)> {
        let guard = SessionCapabilityRun::admit(
            self.projection,
            task_id,
            task_id,
            self.ceiling,
            self.cancellation,
            workspace,
        )
        .await?;
        match super::run_hook_executor::RunHookExecutor::new(
            self.external_hooks,
            self.hooks,
            guard.task_spawner(),
        ) {
            Ok(hooks) => Ok((guard, hooks)),
            Err(error) => {
                let closed = guard.close().await;
                anyhow::bail!("background Hook setup failed: {error}; close={closed:?}")
            }
        }
    }
}
