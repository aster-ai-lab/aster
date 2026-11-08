//! Request-local quantized main/window KV transactions. Preparation never
//! changes committed rows. Accepted prefixes copy only visible bytes; window
//! proposals retain the whole step for prefill, and commit keeps the last128.
//! This correctness storage uses immutable concatenation, not an in-place
//! paged allocator or cross-layer/model-wide atomic commit implementation.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use aster_power::error::{PowerError, Result};
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Device, Tensor};
use tokio_util::sync::CancellationToken;

use crate::layers::kv::{MainKvFormat, MainKvRows, MainKvRowsData, MAIN_KV_DIM};
use crate::layers::{tensor_error, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

use super::{check_cancel, invalid, ProposalLease};

static NEXT_KV_OWNER: AtomicU64 = AtomicU64::new(1);
const WINDOW_ROWS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvCacheKind {
    Window,
    C1,
    C2,
}

impl KvCacheKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "swa" => Ok(Self::Window),
            "c1" => Ok(Self::C1),
            "c2" => Ok(Self::C2),
            _ => Err(invalid(
                "unsupported KV cache kind; supported set: swa, c1, c2",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Window => "swa",
            Self::C1 => "c1",
            Self::C2 => "c2",
        }
    }
    fn ratio(self) -> usize {
        if self == Self::C2 {
            2
        } else {
            1
        }
    }
    fn emitted(self, start: usize, tokens: usize) -> usize {
        (start + tokens) / self.ratio() - start / self.ratio()
    }
    fn ready_position(self, first: usize, row: usize) -> usize {
        if self == Self::C2 {
            row * 2 + 1
        } else {
            first + row
        }
    }
}

pub struct KvState {
    owner: u64,
    generation: u64,
    kind: KvCacheKind,
    format: MainKvFormat,
    device: Device,
    limits: InferenceLimits,
    context: usize,
    step: usize,
    position: usize,
    records: Tensor,
    proposal_live: Arc<AtomicBool>,
}

impl KvState {
    pub fn new(
        device: &CudaLayerDevice,
        kind: KvCacheKind,
        format: MainKvFormat,
        context: usize,
        step: usize,
    ) -> Result<Self> {
        Self::from_device(
            device.tensor_device().clone(),
            device.limits().clone(),
            kind,
            format,
            context,
            step,
        )
    }

    // Public construction requires actual CUDA. CPU access is test-private.
    fn from_device(
        device: Device,
        limits: InferenceLimits,
        kind: KvCacheKind,
        format: MainKvFormat,
        context: usize,
        step: usize,
    ) -> Result<Self> {
        limits.validate()?;
        if context == 0
            || context > MAX_CONTEXT_TOKENS
            || context > limits.max_context_tokens
            || step == 0
            || step > context
            || (kind == KvCacheKind::Window && format != MainKvFormat::Fp8Ue8m0)
        {
            return Err(invalid(
                "KV state requires positive bounded context/step and FP8 format for SWA",
            ));
        }
        let working_rows = if kind == KvCacheKind::Window {
            (WINDOW_ROWS + step).min(context)
        } else {
            context / kind.ratio()
        };
        if working_rows > 0 {
            limits.checked_elements(
                &[working_rows, format.bytes_per_token()],
                "KV working records",
            )?;
        }
        let bytes = (working_rows as u64)
            .checked_mul((format.bytes_per_token() * 8 + MAIN_KV_DIM * 16) as u64)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| invalid("KV state byte bound overflowed"))?;
        // Includes snapshots, proposal/accepted copies and the codec's view
        // validation tensors; allocator and aggregate model permits are separate.
        limits.checked_state_bytes(bytes, "KV transaction peak tensors")?;
        let owner = NEXT_KV_OWNER
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| invalid("KV owner sequence exhausted"))?;
        let records = Tensor::zeros((0, format.bytes_per_token()), DType::U8, &device)
            .map_err(tensor_error)?;
        Ok(Self {
            owner,
            generation: 0,
            kind,
            format,
            device,
            limits,
            context,
            step,
            position: 0,
            records,
            proposal_live: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn position(&self) -> usize {
        self.position
    }
    pub fn kind(&self) -> KvCacheKind {
        self.kind
    }
    pub fn row_count(&self) -> usize {
        self.records.dims()[0]
    }
    pub fn retained_bytes(&self) -> u64 {
        (self.row_count() * self.format.bytes_per_token()) as u64
    }
    pub fn ready_positions(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        let first = if self.kind == KvCacheKind::Window {
            self.position - self.row_count()
        } else {
            0
        };
        (0..self.row_count()).map(move |row| self.kind.ready_position(first, row))
    }

    /// Raw encoded records are validated against this state's explicit format
    /// and snapshotted. Readiness must match exactly the C1/C2/window schedule.
    pub fn prepare(
        &self,
        start: usize,
        tokens: usize,
        ready_positions: &[usize],
        encoded_records: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<PreparedKv> {
        check_cancel(cancellation)?;
        if start != self.position
            || tokens == 0
            || tokens > self.step
            || start
                .checked_add(tokens)
                .is_none_or(|end| end > self.context)
            || !encoded_records.device().same_device(&self.device)
        {
            return Err(invalid(
                "KV proposal has a wrong position/device or exceeds step/context capacity",
            ));
        }
        let count = self.kind.emitted(start, tokens);
        let first_row = start / self.kind.ratio();
        if ready_positions.len() != count
            || ready_positions.iter().enumerate().any(|(row, ready)| {
                *ready
                    != if self.kind == KvCacheKind::C2 {
                        (first_row + row) * 2 + 1
                    } else {
                        start + row
                    }
            })
            || encoded_records.dims() != [count, self.format.bytes_per_token()]
        {
            return Err(invalid(
                "KV proposal rows/readiness differ from its exact compression schedule",
            ));
        }
        let lease = ProposalLease::acquire(&self.proposal_live, "KV")?;
        let snapshot = MainKvRowsData::new(self.format, encoded_records, &self.limits)?;
        let records = if count == 0 {
            self.records.clone()
        } else if self.row_count() == 0 {
            snapshot.records().clone()
        } else {
            Tensor::cat(&[&self.records, snapshot.records()], 0)
                .map_err(tensor_error)?
                .detach()
        };
        check_cancel(cancellation)?;
        Ok(PreparedKv {
            owner: self.owner,
            generation: self.generation,
            position: start,
            tokens,
            old_rows: self.row_count(),
            kind: self.kind,
            format: self.format,
            first_ready: if self.kind == KvCacheKind::Window {
                start - self.row_count()
            } else {
                0
            },
            records,
            limits: self.limits.clone(),
            _lease: lease,
        })
    }

    pub fn commit_prefix(
        &mut self,
        proposal: PreparedKv,
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
        self.position
            .checked_add(accepted)
            .filter(|p| *p <= self.context)
            .ok_or_else(|| invalid("KV accepted prefix exceeds context capacity"))?;
        let rows = self.row_count() + self.kind.emitted(self.position, accepted);
        let retained = if self.kind == KvCacheKind::Window {
            rows.min(WINDOW_ROWS)
        } else {
            rows
        };
        Ok((retained * self.format.bytes_per_token()) as u64)
    }

    pub(super) fn stage_prefix(
        &mut self,
        proposal: PreparedKv,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<KvCommit<'_>> {
        check_cancel(cancellation)?;
        if proposal.owner != self.owner
            || proposal.generation != self.generation
            || proposal.position != self.position
            || proposal.kind != self.kind
            || proposal.format != self.format
            || proposal.old_rows != self.row_count()
            || accepted > proposal.tokens
        {
            return Err(invalid("KV proposal belongs to another request/generation or has an invalid accepted prefix"));
        }
        if accepted == 0 {
            let position = self.position;
            let generation = self.generation;
            return Ok(KvCommit {
                state: self,
                position,
                generation,
                records: None,
                applied: false,
                _lease: proposal._lease,
            });
        }
        let position = self
            .position
            .checked_add(accepted)
            .filter(|end| *end <= self.context)
            .ok_or_else(|| invalid("KV accepted prefix exceeds context capacity"))?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("KV generation exhausted"))?;
        let rows = self.row_count() + self.kind.emitted(self.position, accepted);
        let retained = if self.kind == KvCacheKind::Window {
            rows.min(WINDOW_ROWS)
        } else {
            rows
        };
        let records = proposal
            .records
            .narrow(0, rows - retained, retained)
            .and_then(|value| value.force_contiguous())
            .map_err(tensor_error)?
            .detach();
        check_cancel(cancellation)?;
        Ok(KvCommit {
            state: self,
            position,
            generation,
            records: Some(records),
            applied: false,
            _lease: proposal._lease,
        })
    }

    /// The returned public row handle is a separate validated snapshot, so a
    /// consumer cannot mutate the committed buffer through a tensor alias.
    pub fn attention_rows(&self, device: &CudaLayerDevice) -> Result<MainKvRows> {
        bind_rows(
            device,
            &self.device,
            &self.limits,
            self.format,
            &self.records,
        )
    }
}

pub struct PreparedKv {
    owner: u64,
    generation: u64,
    pub(super) position: usize,
    pub(super) tokens: usize,
    old_rows: usize,
    kind: KvCacheKind,
    format: MainKvFormat,
    first_ready: usize,
    records: Tensor,
    limits: InferenceLimits,
    _lease: ProposalLease,
}

pub(super) struct KvCommit<'a> {
    state: &'a mut KvState,
    position: usize,
    generation: u64,
    records: Option<Tensor>,
    applied: bool,
    _lease: ProposalLease,
}
impl KvCommit<'_> {
    pub(super) fn device(&self) -> &Device {
        &self.state.device
    }
    pub(super) fn apply(&mut self) {
        if self.applied {
            return;
        }
        if let Some(records) = self.records.as_mut() {
            self.state.position = self.position;
            self.state.generation = self.generation;
            std::mem::swap(&mut self.state.records, records);
        }
        self.applied = true;
    }
}

impl PreparedKv {
    pub(crate) fn attention_rows_with_transfer(
        &self,
        device: &CudaLayerDevice,
        transfers: &mut crate::parallel::XqStepTransfers<'_>,
        cancellation: &CancellationToken,
    ) -> Result<MainKvRows> {
        if self.limits != *device.limits() {
            return Err(invalid(
                "peer KV view resource limits differ from its owner",
            ));
        }
        let records = transfers.copy_frozen(&self.records, device, cancellation)?;
        MainKvRows::from_records(device, self.format, &records)
    }
    pub fn row_count(&self) -> usize {
        self.records.dims()[0]
    }
    pub fn ready_positions(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        (0..self.row_count()).map(move |row| self.kind.ready_position(self.first_ready, row))
    }
    pub fn attention_rows(&self, device: &CudaLayerDevice) -> Result<MainKvRows> {
        bind_rows(
            device,
            self.records.device(),
            &self.limits,
            self.format,
            &self.records,
        )
    }
}

fn bind_rows(
    device: &CudaLayerDevice,
    source: &Device,
    limits: &InferenceLimits,
    format: MainKvFormat,
    records: &Tensor,
) -> Result<MainKvRows> {
    if !source.same_device(device.tensor_device()) || limits != device.limits() {
        return Err(PowerError::InvalidRequest(
            "KV read view differs from its owning device/limits".into(),
        ));
    }
    MainKvRows::from_records(device, format, records)
}

#[cfg(test)]
mod tests;
