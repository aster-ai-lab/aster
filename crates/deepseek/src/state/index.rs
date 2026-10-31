//! Request-local packed index-K cache. C1/C2 readiness follows the main
//! compressor, but data stays E2M1/group-32 UE8M0 in separate byte sections.
//! Prepared and staged states own snapshots; rejected prefixes never publish.

use std::sync::atomic::AtomicBool;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use aster_power::error::Result;
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Device, Tensor};
use tokio_util::sync::CancellationToken;

use super::{check_cancel, invalid, CompressionKind, ProposalLease};
use crate::layers::quant::{Fp4IndexRows, Fp4IndexRowsData, INDEX_HEAD_DIM};
use crate::layers::{tensor_error, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

static NEXT_INDEX_OWNER: AtomicU64 = AtomicU64::new(1);
const DATA_BYTES: usize = INDEX_HEAD_DIM / 2;
const SCALE_BYTES: usize = INDEX_HEAD_DIM / 32;

pub struct IndexKvState {
    owner: u64,
    generation: u64,
    position: usize,
    kind: CompressionKind,
    context: usize,
    step: usize,
    device: Device,
    limits: InferenceLimits,
    data: Tensor,
    scales: Tensor,
    proposal_live: Arc<AtomicBool>,
}

impl IndexKvState {
    pub fn new(
        device: &CudaLayerDevice,
        kind: CompressionKind,
        context: usize,
        step: usize,
    ) -> Result<Self> {
        Self::from_device(
            device.tensor_device().clone(),
            device.limits().clone(),
            kind,
            context,
            step,
        )
    }

    fn from_device(
        device: Device,
        limits: InferenceLimits,
        kind: CompressionKind,
        context: usize,
        step: usize,
    ) -> Result<Self> {
        limits.validate()?;
        if context == 0
            || context > MAX_CONTEXT_TOKENS
            || context > limits.max_context_tokens
            || step == 0
            || step > context
        {
            return Err(invalid(
                "index KV requires positive bounded context and step",
            ));
        }
        let rows = context / kind.ratio();
        if rows > 0 {
            limits.checked_elements(&[rows, INDEX_HEAD_DIM], "index KV capacity")?;
        }
        let bytes = (rows as u64)
            .checked_mul((INDEX_HEAD_DIM * 16 + (DATA_BYTES + SCALE_BYTES) * 8) as u64)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("index KV live tensor estimate overflowed"))?;
        limits.checked_state_bytes(bytes, "index KV transaction live tensors")?;
        let owner = NEXT_INDEX_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| invalid("index KV owner sequence exhausted"))?;
        Ok(Self {
            owner,
            generation: 0,
            position: 0,
            kind,
            context,
            step,
            data: Tensor::zeros((0, DATA_BYTES), DType::U8, &device).map_err(tensor_error)?,
            scales: Tensor::zeros((0, SCALE_BYTES), DType::U8, &device).map_err(tensor_error)?,
            device,
            limits,
            proposal_live: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn position(&self) -> usize {
        self.position
    }
    pub fn kind(&self) -> CompressionKind {
        self.kind
    }
    pub fn row_count(&self) -> usize {
        self.data.dims()[0]
    }
    pub fn retained_bytes(&self) -> u64 {
        (self.row_count() * (DATA_BYTES + SCALE_BYTES)) as u64
    }
    pub fn ready_positions(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        (0..self.row_count()).map(move |row| (row + 1) * self.kind.ratio() - 1)
    }

    pub fn prepare(
        &self,
        start: usize,
        tokens: usize,
        ready_positions: &[usize],
        data: &Tensor,
        scales: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<PreparedIndexKv> {
        check_cancel(cancellation)?;
        if start != self.position
            || tokens == 0
            || tokens > self.step
            || start
                .checked_add(tokens)
                .is_none_or(|end| end > self.context)
            || !data.device().same_device(&self.device)
        {
            return Err(invalid(
                "index KV proposal has wrong position/device or exceeds step/context",
            ));
        }
        let count = (start + tokens) / self.kind.ratio() - self.row_count();
        if ready_positions.len() != count
            || data.dims() != [count, DATA_BYTES]
            || scales.dims() != [count, SCALE_BYTES]
            || ready_positions
                .iter()
                .enumerate()
                .any(|(row, ready)| *ready != (self.row_count() + row + 1) * self.kind.ratio() - 1)
        {
            return Err(invalid(
                "index KV proposal differs from its exact compressed readiness/shape",
            ));
        }
        let lease = ProposalLease::acquire(&self.proposal_live, "index KV")?;
        let snapshot = Fp4IndexRowsData::new(&self.limits, data, scales)?;
        let (data, scales) = if count == 0 {
            (self.data.clone(), self.scales.clone())
        } else if self.row_count() == 0 {
            (
                snapshot.data_bytes().clone(),
                snapshot.scale_bytes().clone(),
            )
        } else {
            (
                Tensor::cat(&[&self.data, snapshot.data_bytes()], 0)
                    .map_err(tensor_error)?
                    .detach(),
                Tensor::cat(&[&self.scales, snapshot.scale_bytes()], 0)
                    .map_err(tensor_error)?
                    .detach(),
            )
        };
        check_cancel(cancellation)?;
        Ok(PreparedIndexKv {
            owner: self.owner,
            generation: self.generation,
            position: start,
            tokens,
            kind: self.kind,
            old_rows: self.row_count(),
            data,
            scales,
            limits: self.limits.clone(),
            _lease: lease,
        })
    }

    pub fn commit_prefix(
        &mut self,
        proposal: PreparedIndexKv,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        let mut staged = self.stage_prefix(proposal, accepted, cancellation)?;
        staged.device().synchronize().map_err(tensor_error)?;
        check_cancel(cancellation)?;
        staged.apply();
        Ok(())
    }

    pub(super) fn stage_bytes(&self, accepted: usize) -> Result<u64> {
        if accepted == 0 {
            return Ok(0);
        }
        let position = self
            .position
            .checked_add(accepted)
            .filter(|p| *p <= self.context)
            .ok_or_else(|| invalid("index KV accepted position exceeds context"))?;
        Ok((position / self.kind.ratio() * (DATA_BYTES + SCALE_BYTES)) as u64)
    }

    pub(super) fn stage_prefix(
        &mut self,
        proposal: PreparedIndexKv,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<IndexCommit<'_>> {
        check_cancel(cancellation)?;
        if proposal.owner != self.owner
            || proposal.generation != self.generation
            || proposal.position != self.position
            || proposal.kind != self.kind
            || proposal.old_rows != self.row_count()
            || accepted > proposal.tokens
        {
            return Err(invalid("index KV proposal belongs to another request/generation or has an invalid accepted prefix"));
        }
        let position = self
            .position
            .checked_add(accepted)
            .filter(|p| *p <= self.context)
            .ok_or_else(|| invalid("index KV accepted position exceeds context"))?;
        let generation = if accepted == 0 {
            self.generation
        } else {
            self.generation
                .checked_add(1)
                .ok_or_else(|| invalid("index KV generation exhausted"))?
        };
        let records = if accepted == 0 {
            None
        } else {
            let rows = position / self.kind.ratio();
            Some((
                proposal
                    .data
                    .narrow(0, 0, rows)
                    .and_then(|t| t.force_contiguous())
                    .map_err(tensor_error)?
                    .detach(),
                proposal
                    .scales
                    .narrow(0, 0, rows)
                    .and_then(|t| t.force_contiguous())
                    .map_err(tensor_error)?
                    .detach(),
            ))
        };
        check_cancel(cancellation)?;
        Ok(IndexCommit {
            state: self,
            position,
            generation,
            records,
            applied: false,
            _lease: proposal._lease,
        })
    }

    pub fn index_rows(&self, device: &CudaLayerDevice) -> Result<Fp4IndexRows> {
        bind_rows(device, &self.device, &self.limits, &self.data, &self.scales)
    }
}

pub struct PreparedIndexKv {
    owner: u64,
    generation: u64,
    pub(super) position: usize,
    pub(super) tokens: usize,
    kind: CompressionKind,
    old_rows: usize,
    data: Tensor,
    scales: Tensor,
    limits: InferenceLimits,
    _lease: ProposalLease,
}
impl PreparedIndexKv {
    pub(crate) fn index_rows_with_transfer(
        &self,
        device: &CudaLayerDevice,
        transfers: &mut crate::parallel::XqStepTransfers<'_>,
        cancellation: &CancellationToken,
    ) -> Result<Fp4IndexRows> {
        if self.limits != *device.limits() {
            return Err(invalid("peer index KV limits differ from its owner"));
        }
        let data = transfers.copy_frozen(&self.data, device, cancellation)?;
        let scales = transfers.copy_frozen(&self.scales, device, cancellation)?;
        Fp4IndexRows::from_packed(device, &data, &scales)
    }
    pub fn row_count(&self) -> usize {
        self.data.dims()[0]
    }
    pub fn ready_positions(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        (0..self.row_count()).map(move |row| (row + 1) * self.kind.ratio() - 1)
    }
    pub fn index_rows(&self, device: &CudaLayerDevice) -> Result<Fp4IndexRows> {
        bind_rows(
            device,
            self.data.device(),
            &self.limits,
            &self.data,
            &self.scales,
        )
    }
}

pub(super) struct IndexCommit<'a> {
    state: &'a mut IndexKvState,
    position: usize,
    generation: u64,
    records: Option<(Tensor, Tensor)>,
    applied: bool,
    _lease: ProposalLease,
}
impl IndexCommit<'_> {
    pub(super) fn device(&self) -> &Device {
        &self.state.device
    }
    pub(super) fn apply(&mut self) {
        if self.applied {
            return;
        }
        if let Some((data, scales)) = self.records.as_mut() {
            self.state.position = self.position;
            self.state.generation = self.generation;
            std::mem::swap(&mut self.state.data, data);
            std::mem::swap(&mut self.state.scales, scales);
        }
        self.applied = true;
    }
}

fn bind_rows(
    device: &CudaLayerDevice,
    source: &Device,
    limits: &InferenceLimits,
    data: &Tensor,
    scales: &Tensor,
) -> Result<Fp4IndexRows> {
    if !source.same_device(device.tensor_device()) || limits != device.limits() {
        return Err(invalid(
            "index KV view differs from its owning device/limits",
        ));
    }
    Fp4IndexRows::from_packed(device, data, scales)
}

#[cfg(test)]
mod tests;
