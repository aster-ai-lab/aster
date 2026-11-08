//! Atomic commit of a selected set of request-local tensor states. All
//! validation, accepted-prefix allocations and the cancellation check finish
//! before any owner changes. The model composition must supply every affected
//! state, including Engram history; this is not that model composition.

use crate::engram::{EngramCommit, EngramHistory, PreparedNgrams};
use crate::layers::tensor_error;
use aster_power::error::Result;
use candle_core::Device;
use tokio_util::sync::CancellationToken;

use super::index::{IndexCommit, IndexKvState, PreparedIndexKv};
use super::kv::{KvCommit, KvState, PreparedKv};
use super::{
    check_cancel, invalid, CommittedCompression, CompressionCommit, CompressionState,
    PreparedCompression,
};
use crate::model::MAX_CONTEXT_TOKENS;

const MAX_PARTICIPANTS: usize = 128;

pub struct StateCommitBatch<'a> {
    start: usize,
    tokens: usize,
    accepted: usize,
    max_staged_bytes: u64,
    staged_bytes: u64,
    failed: bool,
    mutations: Vec<Mutation<'a>>,
}

enum Mutation<'a> {
    Compression(CompressionCommit<'a>),
    Main(KvCommit<'a>),
    Index(IndexCommit<'a>),
    Engram(EngramCommit<'a>),
}

impl Mutation<'_> {
    fn device(&self) -> Option<&Device> {
        match self {
            Self::Compression(staged) => Some(staged.device()),
            Self::Main(staged) => Some(staged.device()),
            Self::Index(staged) => Some(staged.device()),
            Self::Engram(_) => None,
        }
    }
}

impl<'a> StateCommitBatch<'a> {
    /// `max_staged_bytes` bounds new commit snapshots/results plus conservative
    /// per-participant metadata. It does not reserve total model HBM or count
    /// pre-existing prepared proposals and CUDA allocator workspace.
    pub fn new(
        start: usize,
        tokens: usize,
        accepted: usize,
        max_staged_bytes: u64,
    ) -> Result<Self> {
        if tokens == 0
            || accepted > tokens
            || max_staged_bytes == 0
            || start
                .checked_add(tokens)
                .is_none_or(|end| end > MAX_CONTEXT_TOKENS)
        {
            return Err(invalid("state commit requires a bounded nonempty proposal, valid accepted prefix and positive staged-byte budget"));
        }
        Ok(Self {
            start,
            tokens,
            accepted,
            max_staged_bytes,
            staged_bytes: 0,
            failed: false,
            mutations: Vec::new(),
        })
    }

    pub fn add_compression(
        &mut self,
        target: &'a mut CompressionState,
        proposal: PreparedCompression,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        self.begin(proposal.position, proposal.tokens, cancellation)?;
        self.reserve(target.stage_bytes(self.accepted)?)?;
        let staged = target.stage_prefix(proposal, self.accepted, cancellation)?;
        self.mutations.push(Mutation::Compression(staged));
        self.failed = false;
        Ok(())
    }

    pub fn add_main_kv(
        &mut self,
        target: &'a mut KvState,
        proposal: PreparedKv,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        self.begin(proposal.position, proposal.tokens, cancellation)?;
        self.reserve(target.stage_bytes(self.accepted)?)?;
        let staged = target.stage_prefix(proposal, self.accepted, cancellation)?;
        self.mutations.push(Mutation::Main(staged));
        self.failed = false;
        Ok(())
    }

    pub fn add_index_kv(
        &mut self,
        target: &'a mut IndexKvState,
        proposal: PreparedIndexKv,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        self.begin(proposal.position, proposal.tokens, cancellation)?;
        self.reserve(target.stage_bytes(self.accepted)?)?;
        let staged = target.stage_prefix(proposal, self.accepted, cancellation)?;
        self.mutations.push(Mutation::Index(staged));
        self.failed = false;
        Ok(())
    }

    fn begin(
        &mut self,
        position: usize,
        tokens: usize,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        if self.failed {
            return Err(invalid(
                "state commit batch was poisoned by a previous staging failure",
            ));
        }
        self.failed = true;
        check_cancel(cancellation)?;
        if position != self.start
            || tokens != self.tokens
            || self.mutations.len() >= MAX_PARTICIPANTS
        {
            return Err(invalid("state participants must share one start/token range and stay within the participant bound"));
        }
        Ok(())
    }

    fn reserve(&mut self, bytes: u64) -> Result<()> {
        let metadata = (std::mem::size_of::<Mutation<'static>>()
            + std::mem::size_of::<CommittedCompression>()
            + 64) as u64;
        let bytes = bytes
            .checked_add(metadata)
            .and_then(|bytes| self.staged_bytes.checked_add(bytes))
            .filter(|bytes| *bytes <= self.max_staged_bytes)
            .ok_or_else(|| {
                invalid("atomic state commit exceeds its staged-snapshot byte budget")
            })?;
        // Reserve the bounded host slot before staging tensor allocations.
        self.mutations
            .try_reserve_exact(1)
            .map_err(|_| invalid("state commit cannot reserve its participant metadata"))?;
        self.staged_bytes = bytes;
        Ok(())
    }

    pub fn staged_bytes(&self) -> u64 {
        self.staged_bytes
    }

    pub fn add_engram_history(
        &mut self,
        target: &'a mut EngramHistory,
        proposal: PreparedNgrams,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        self.begin(proposal.position(), proposal.token_count(), cancellation)?;
        self.reserve(0)?;
        let staged = target.stage_prefix(proposal, self.accepted)?;
        self.mutations.push(Mutation::Engram(staged));
        self.failed = false;
        Ok(())
    }

    /// Results are in compression-participant insertion order. Cancellation
    /// linearizes at the final check; applying staged assignments is infallible
    /// and contains no allocation, device operation or mid-commit cancel.
    pub fn commit(mut self, cancellation: &CancellationToken) -> Result<Vec<CommittedCompression>> {
        if self.failed || self.mutations.is_empty() {
            return Err(invalid("state commit batch is empty or poisoned"));
        }
        let mut results = Vec::new();
        results
            .try_reserve_exact(self.mutations.len())
            .map_err(|_| invalid("state commit cannot reserve result metadata"))?;
        // Observe deferred CUDA copy errors before the commit point. Sync each
        // actual device once, without reading any KV payload back to the host.
        for (index, mutation) in self.mutations.iter().enumerate() {
            if let Some(device) = mutation.device() {
                if !self.mutations[..index].iter().any(|previous| {
                    previous
                        .device()
                        .is_some_and(|previous| previous.same_device(device))
                }) {
                    device.synchronize().map_err(tensor_error)?;
                }
            }
        }
        check_cancel(cancellation)?;
        // Keep every old tensor inside its guard until all owners have changed.
        // A CUDA buffer drop/reclamation must not occur between assignments.
        for mutation in &mut self.mutations {
            match mutation {
                Mutation::Compression(staged) => results.push(staged.apply()),
                Mutation::Main(staged) => staged.apply(),
                Mutation::Index(staged) => staged.apply(),
                Mutation::Engram(staged) => staged.apply(),
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests;
