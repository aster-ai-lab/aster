//! Complete base-text request-state collection: 40 own-layer windows,
//! compression/main/index states at sources 2/8/14/20, and one shared Engram
//! history. A proposal cannot commit a subset. Model computation and placement
//! still supply the real projected values; this owner does not execute layers.

use std::sync::{atomic::AtomicBool, Arc};

use aster_power::error::Result;
use tokio_util::sync::CancellationToken;

use super::{
    check_cancel,
    index::{IndexKvState, PreparedIndexKv},
    invalid,
    kv::{KvState, PreparedKv},
    transaction::StateCommitBatch,
    CommittedCompression, CompressionNumerics, CompressionState, PreparedCompression,
    ProposalLease,
};
use crate::{
    engram::{EngramHistory, EngramToken, NgramRow, PreparedNgrams},
    layers::{kv::MainKvFormat, CudaLayerDevice},
    model::topology::TextArchitecture,
};

const LAYERS: usize = 40;
const SOURCE_LAYERS: [usize; 4] = [2, 8, 14, 20];

pub struct BackboneState {
    windows: Vec<KvState>,
    sources: Vec<CompressedSourceState>,
    history: EngramHistory,
    max_staged_bytes: u64,
    proposal_live: Arc<AtomicBool>,
    compression_numerics: CompressionNumerics,
}

pub struct CompressedSourceState {
    layer: usize,
    compression: CompressionState,
    main: KvState,
    index: IndexKvState,
}

impl CompressedSourceState {
    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn compression(&self) -> &CompressionState {
        &self.compression
    }
    pub fn main(&self) -> &KvState {
        &self.main
    }
    pub fn index(&self) -> &IndexKvState {
        &self.index
    }
}

impl BackboneState {
    /// One explicit CUDA owner per layer, with repeats for layer placement.
    /// This is not an eight-card allocation/collective or aggregate HBM permit.
    /// max_staged_bytes covers the atomic commit's accepted snapshots/metadata;
    /// pre-existing states, proposals and allocator workspaces are separate.
    pub fn new(
        architecture: &TextArchitecture,
        layer_devices: &[CudaLayerDevice],
        context: usize,
        step: usize,
        format: MainKvFormat,
        max_staged_bytes: u64,
    ) -> Result<Self> {
        if layer_devices.len() != LAYERS
            || architecture.layers().len() != LAYERS
            || max_staged_bytes == 0
        {
            return Err(invalid("base backbone state requires 40 explicit layer devices and a positive staged-byte budget"));
        }
        if architecture.compression_numerics() == CompressionNumerics::XqF32
            && format != MainKvFormat::Fp4E4m3
        {
            return Err(invalid(
                "XQ backbone compressed cache requires FP4 E4M3 scales",
            ));
        }
        let mut windows = Vec::with_capacity(LAYERS);
        let mut sources = Vec::with_capacity(SOURCE_LAYERS.len());
        for (binding, device) in architecture.layers().iter().zip(layer_devices) {
            windows.push(binding.new_window_state(device, context, step)?);
            if binding.is_kv_source() {
                if !binding.is_index_k_source() {
                    return Err(invalid(
                        "official backbone requires colocated main/index-K source schedules",
                    ));
                }
                sources.push(CompressedSourceState {
                    layer: binding.layer(),
                    compression: binding.new_compression_state(device, context, step)?,
                    main: binding.new_compressed_kv_state(device, format, context, step)?,
                    index: binding.new_index_kv_state(device, context, step)?,
                });
            }
        }
        if sources.iter().map(|s| s.layer).collect::<Vec<_>>() != SOURCE_LAYERS {
            return Err(invalid(
                "backbone state sources differ from official layers 2,8,14,20",
            ));
        }
        Ok(Self {
            windows,
            sources,
            history: EngramHistory::new(context, step)?,
            max_staged_bytes,
            proposal_live: Arc::new(AtomicBool::new(false)),
            compression_numerics: architecture.compression_numerics(),
        })
    }

    pub fn position(&self) -> usize {
        self.history.position()
    }

    pub fn compression_numerics(&self) -> CompressionNumerics {
        self.compression_numerics
    }
    pub fn participant_count(&self) -> usize {
        self.windows.len() + 3 * self.sources.len() + 1
    }
    pub fn window(&self, layer: usize) -> Result<&KvState> {
        self.windows
            .get(layer)
            .ok_or_else(|| invalid("backbone window layer must be 0..39"))
    }
    pub fn compressed_source(&self, layer: usize) -> Result<&CompressedSourceState> {
        self.sources
            .iter()
            .find(|s| s.layer == layer)
            .ok_or_else(|| invalid("compressed state requires an explicit source layer: 2,8,14,20"))
    }

    pub fn begin(
        &self,
        tokens: &[EngramToken],
        cancellation: &CancellationToken,
    ) -> Result<PendingBackboneStep> {
        check_cancel(cancellation)?;
        self.validate_positions()?;
        let lease = ProposalLease::acquire(&self.proposal_live, "backbone")?;
        let ngrams = self.history.prepare(tokens)?;
        Ok(PendingBackboneStep {
            start: self.position(),
            tokens: tokens.len(),
            ngrams,
            windows: (0..LAYERS).map(|_| None).collect(),
            sources: (0..SOURCE_LAYERS.len()).map(|_| None).collect(),
            failed: false,
            _lease: lease,
        })
    }

    pub fn commit_prefix(
        &mut self,
        pending: PendingBackboneStep,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<Vec<CommittedCompression>> {
        self.stage_prefix(pending, accepted, cancellation)?
            .commit(cancellation)
    }

    pub fn stage_prefix<'a>(
        &'a mut self,
        pending: PendingBackboneStep,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<BackboneCommit<'a>> {
        check_cancel(cancellation)?;
        self.validate_positions()?;
        if pending.failed
            || pending.start != self.position()
            || pending.windows.len() != self.windows.len()
            || pending.sources.len() != self.sources.len()
            || pending.windows.iter().any(Option::is_none)
            || pending.sources.iter().any(Option::is_none)
        {
            return Err(invalid("backbone commit requires every window/source participant in one complete, unpoisoned current step"));
        }
        let PendingBackboneStep {
            start,
            tokens,
            ngrams,
            windows,
            sources,
            _lease,
            ..
        } = pending;
        let mut batch = StateCommitBatch::new(start, tokens, accepted, self.max_staged_bytes)?;
        // Histories, all 40 windows and every source's three states are required.
        // Any late staging error drops all guards; no target has changed yet.
        batch.add_engram_history(&mut self.history, ngrams, cancellation)?;
        for (target, proposal) in self.windows.iter_mut().zip(windows) {
            batch.add_main_kv(
                target,
                proposal.ok_or_else(|| invalid("backbone window proposal is missing"))?,
                cancellation,
            )?;
        }
        for (target, proposal) in self.sources.iter_mut().zip(sources) {
            let proposal =
                proposal.ok_or_else(|| invalid("backbone source proposal is missing"))?;
            batch.add_compression(&mut target.compression, proposal.compression, cancellation)?;
            batch.add_main_kv(&mut target.main, proposal.main, cancellation)?;
            batch.add_index_kv(&mut target.index, proposal.index, cancellation)?;
        }
        Ok(BackboneCommit { batch, _lease })
    }

    fn validate_positions(&self) -> Result<()> {
        let position = self.position();
        if self.windows.iter().any(|s| s.position() != position)
            || self.sources.iter().any(|s| {
                s.compression.position() != position
                    || s.main.position() != position
                    || s.index.position() != position
            })
        {
            return Err(invalid("backbone request state positions have diverged"));
        }
        Ok(())
    }
}

pub struct PendingBackboneStep {
    start: usize,
    tokens: usize,
    ngrams: PreparedNgrams,
    windows: Vec<Option<PreparedKv>>,
    sources: Vec<Option<PreparedSourceState>>,
    failed: bool,
    _lease: ProposalLease,
}

pub struct PreparedSourceState {
    compression: PreparedCompression,
    main: PreparedKv,
    index: PreparedIndexKv,
}

impl PreparedSourceState {
    pub fn compression(&self) -> &PreparedCompression {
        &self.compression
    }
    pub fn main(&self) -> &PreparedKv {
        &self.main
    }
    pub fn index(&self) -> &PreparedIndexKv {
        &self.index
    }
}

impl PendingBackboneStep {
    pub(crate) fn poison(&mut self) {
        self.failed = true;
    }
    pub fn is_poisoned(&self) -> bool {
        self.failed
    }

    pub fn position(&self) -> usize {
        self.start
    }
    pub fn token_count(&self) -> usize {
        self.tokens
    }
    pub fn ngrams(&self) -> &[NgramRow] {
        self.ngrams.rows()
    }
    pub fn window(&self, layer: usize) -> Result<&PreparedKv> {
        if self.failed {
            return Err(invalid("backbone proposal is poisoned"));
        }
        self.windows
            .get(layer)
            .and_then(Option::as_ref)
            .ok_or_else(|| invalid("backbone window proposal is missing or layer is outside 0..39"))
    }
    pub fn compressed_source(&self, layer: usize) -> Result<&PreparedSourceState> {
        if self.failed {
            return Err(invalid("backbone proposal is poisoned"));
        }
        let slot = SOURCE_LAYERS
            .iter()
            .position(|&id| id == layer)
            .ok_or_else(|| invalid("backbone source must be layer 2,8,14,20"))?;
        self.sources[slot]
            .as_ref()
            .ok_or_else(|| invalid("backbone source proposal has not been prepared"))
    }

    pub fn stage_window(
        &mut self,
        layer: usize,
        proposal: PreparedKv,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        self.begin_stage(cancellation)?;
        let slot = self
            .windows
            .get_mut(layer)
            .ok_or_else(|| invalid("backbone window layer must be 0..39"))?;
        if slot.is_some() || proposal.position != self.start || proposal.tokens != self.tokens {
            return Err(invalid(
                "backbone window is duplicated or has a different proposal range",
            ));
        }
        *slot = Some(proposal);
        self.failed = false;
        Ok(())
    }

    pub fn stage_source(
        &mut self,
        layer: usize,
        compression: PreparedCompression,
        main: PreparedKv,
        index: PreparedIndexKv,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        self.begin_stage(cancellation)?;
        let slot = SOURCE_LAYERS
            .iter()
            .position(|&id| id == layer)
            .ok_or_else(|| invalid("backbone source must be layer 2,8,14,20"))?;
        if self.sources[slot].is_some()
            || compression.position != self.start
            || compression.tokens != self.tokens
            || main.position != self.start
            || main.tokens != self.tokens
            || index.position != self.start
            || index.tokens != self.tokens
        {
            return Err(invalid(
                "backbone source is duplicated or has a different proposal range",
            ));
        }
        self.sources[slot] = Some(PreparedSourceState {
            compression,
            main,
            index,
        });
        self.failed = false;
        Ok(())
    }

    fn begin_stage(&mut self, cancellation: &CancellationToken) -> Result<()> {
        if self.failed {
            return Err(invalid(
                "backbone proposal was poisoned by a previous staging failure",
            ));
        }
        self.failed = true;
        check_cancel(cancellation)
    }
}

pub struct BackboneCommit<'a> {
    batch: StateCommitBatch<'a>,
    _lease: ProposalLease,
}

impl BackboneCommit<'_> {
    pub fn staged_bytes(&self) -> u64 {
        self.batch.staged_bytes()
    }
    pub fn commit(self, cancellation: &CancellationToken) -> Result<Vec<CommittedCompression>> {
        let Self { batch, _lease } = self;
        batch.commit(cancellation)
    }
}

#[cfg(test)]
pub(crate) mod tests;
