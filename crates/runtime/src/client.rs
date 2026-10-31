use crate::contract::{
    RuntimeActionRequest, RuntimeApplyRequest, RuntimeCapabilities, RuntimeExecRequest,
    RuntimeExecResult, RuntimeInspection, RuntimeLogChunk, RuntimeLogQuery, RuntimeObservation,
    RuntimePause, RuntimeRemoval, RuntimeResume, RuntimeSnapshot, RuntimeSnapshotRequest,
};
use crate::{RuntimeError, RuntimeResult};
use async_trait::async_trait;

/// Stable provider-neutral lifecycle implemented by every Aster Runtime.
#[async_trait]
pub trait RuntimeClient: Send + Sync {
    async fn capabilities(&self) -> RuntimeResult<RuntimeCapabilities>;

    async fn apply(&self, request: &RuntimeApplyRequest) -> RuntimeResult<RuntimeObservation>;

    async fn inspect(&self, unit_id: &str) -> RuntimeResult<RuntimeInspection>;

    async fn stop(&self, request: &RuntimeActionRequest) -> RuntimeResult<RuntimeInspection>;

    async fn remove(&self, request: &RuntimeActionRequest) -> RuntimeResult<RuntimeRemoval>;

    /// ED3: capture the unit's state to a keyed snapshot before
    /// termination. The default refuses — clients over drivers that
    /// do not advertise
    /// [`RuntimeFeature::StateSnapshot`](crate::contract::RuntimeFeature::StateSnapshot)
    /// never see a planned drain.
    async fn snapshot(&self, request: &RuntimeSnapshotRequest) -> RuntimeResult<RuntimeSnapshot> {
        let _ = request;
        Err(RuntimeError::UnsupportedCapabilities(vec![
            "feature:StateSnapshot".into(),
        ]))
    }

    /// RT1: hold the unit PAUSED — RAM kept node-local, execution
    /// frozen under the keyed request. The default refuses; clients
    /// over drivers that do not advertise
    /// [`RuntimeFeature::KeyedPause`](crate::contract::RuntimeFeature::KeyedPause)
    /// never see a pause-tier demotion.
    async fn pause(&self, request: &RuntimeActionRequest) -> RuntimeResult<RuntimePause> {
        let _ = request;
        Err(RuntimeError::UnsupportedCapabilities(vec![
            "feature:KeyedPause".into(),
        ]))
    }

    /// RT1: resume a paused unit node-local under the keyed request —
    /// the unpause path (no snapshot on the critical path). The
    /// default refuses alongside [`pause`](Self::pause).
    async fn resume(&self, request: &RuntimeActionRequest) -> RuntimeResult<RuntimeResume> {
        let _ = request;
        Err(RuntimeError::UnsupportedCapabilities(vec![
            "feature:KeyedPause".into(),
        ]))
    }

    async fn logs(&self, query: &RuntimeLogQuery) -> RuntimeResult<Vec<RuntimeLogChunk>>;

    async fn exec(&self, request: &RuntimeExecRequest) -> RuntimeResult<RuntimeExecResult>;
}
