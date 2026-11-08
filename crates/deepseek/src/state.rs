//! Request-local C1/C2 pre-norm compression state, not a KV cache.
//!
//! Tensor math follows DeepInfer's Apache-2.0 compression reference at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. Rust owns transactions and stores
//! only an unmatched C2 pair half. Norm/RoPE/typed cache quantization must still
//! run on emissions before attention may use them. No backend is registered here.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use aster_power::error::{PowerError, Result};
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Device, Tensor};
use tokio_util::sync::CancellationToken;

use crate::layers::{tensor_error, validate_finite, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

pub mod backbone;
pub mod index;
pub mod kv;
pub mod transaction;

static NEXT_COMPRESSION_STATE: AtomicU64 = AtomicU64::new(1);
const MAX_LATENT_WIDTH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionKind {
    C1,
    C2,
}

impl CompressionKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "c1" => Ok(Self::C1),
            "c2" => Ok(Self::C2),
            _ => Err(invalid(
                "unsupported native compression kind; supported set: c1, c2",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::C1 => "c1",
            Self::C2 => "c2",
        }
    }
    pub fn ratio(self) -> usize {
        match self {
            Self::C1 => 1,
            Self::C2 => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionNumerics {
    OfficialBf16,
    XqF32,
}

impl CompressionNumerics {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "official_bf16" => Ok(Self::OfficialBf16),
            "xq_f32" => Ok(Self::XqF32),
            _ => Err(invalid(
                "unsupported compression numerics; supported set: official_bf16, xq_f32",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OfficialBf16 => "official_bf16",
            Self::XqF32 => "xq_f32",
        }
    }
    fn output_dtype(self) -> DType {
        match self {
            Self::OfficialBf16 => DType::BF16,
            Self::XqF32 => DType::F32,
        }
    }
    fn output_bytes(self) -> usize {
        if self == Self::OfficialBf16 {
            2
        } else {
            4
        }
    }
}

struct PendingPair {
    score: Tensor,
    latent: Tensor,
}

pub struct CompressionState {
    owner: u64,
    generation: u64,
    kind: CompressionKind,
    numerics: CompressionNumerics,
    width: usize,
    position: usize,
    pending: Option<PendingPair>,
    device: Device,
    limits: InferenceLimits,
    context_tokens: usize,
    step_tokens: usize,
    proposal_live: Arc<AtomicBool>,
}

impl CompressionState {
    pub(crate) fn device(&self) -> &Device {
        &self.device
    }
    pub(crate) fn limits(&self) -> &InferenceLimits {
        &self.limits
    }
    pub(crate) fn width(&self) -> usize {
        self.width
    }
    pub(crate) fn max_step_tokens(&self) -> usize {
        self.step_tokens
    }
    pub(crate) fn live_step_bytes(&self, tokens: usize) -> Result<u64> {
        step_peak_bytes_with_numerics(self.kind, self.numerics, self.width, tokens)
    }

    pub fn new(
        device: &CudaLayerDevice,
        kind: CompressionKind,
        width: usize,
        context_tokens: usize,
        step_tokens: usize,
    ) -> Result<Self> {
        Self::from_device(
            device.tensor_device().clone(),
            device.limits().clone(),
            kind,
            width,
            context_tokens,
            step_tokens,
        )
    }

    /// Explicit activation/output profile. The original `new` remains BF16;
    /// XQ carries F32 emissions through norm/RoPE and owns F32 prefix copies.
    pub fn new_with_numerics(
        device: &CudaLayerDevice,
        kind: CompressionKind,
        numerics: CompressionNumerics,
        width: usize,
        context_tokens: usize,
        step_tokens: usize,
    ) -> Result<Self> {
        Self::from_device_with_numerics(
            device.tensor_device().clone(),
            device.limits().clone(),
            kind,
            numerics,
            width,
            context_tokens,
            step_tokens,
        )
    }

    // CPU construction is private to sibling tests. Exported construction
    // requires Power's actual CUDA device boundary.
    fn from_device(
        device: Device,
        limits: InferenceLimits,
        kind: CompressionKind,
        width: usize,
        context_tokens: usize,
        step_tokens: usize,
    ) -> Result<Self> {
        Self::from_device_with_numerics(
            device,
            limits,
            kind,
            CompressionNumerics::OfficialBf16,
            width,
            context_tokens,
            step_tokens,
        )
    }

    fn from_device_with_numerics(
        device: Device,
        limits: InferenceLimits,
        kind: CompressionKind,
        numerics: CompressionNumerics,
        width: usize,
        context_tokens: usize,
        step_tokens: usize,
    ) -> Result<Self> {
        limits.validate()?;
        if width == 0
            || width > MAX_LATENT_WIDTH
            || context_tokens == 0
            || context_tokens > MAX_CONTEXT_TOKENS
            || context_tokens > limits.max_context_tokens
            || step_tokens == 0
            || step_tokens > context_tokens
        {
            return Err(invalid(
                "compression requires width 1..=512 and positive bounded context/step tokens",
            ));
        }
        limits.checked_elements(&[step_tokens + 1, width], "compression working shape")?;
        if step_tokens
            .div_ceil(kind.ratio())
            .checked_mul(std::mem::size_of::<usize>() * 2)
            .is_none_or(|bytes| bytes > limits.max_input_bytes)
        {
            return Err(invalid(
                "compression ready/accepted position metadata exceeds host input budget",
            ));
        }
        limits.checked_state_bytes(
            step_peak_bytes_with_numerics(kind, numerics, width, step_tokens)?,
            "compression step peak",
        )?;
        let owner = NEXT_COMPRESSION_STATE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| invalid("compression state owner sequence exhausted"))?;
        Ok(Self {
            owner,
            generation: 0,
            kind,
            numerics,
            width,
            position: 0,
            pending: None,
            device,
            limits,
            context_tokens,
            step_tokens,
            proposal_live: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn position(&self) -> usize {
        self.position
    }
    pub fn kind(&self) -> CompressionKind {
        self.kind
    }
    pub fn numerics(&self) -> CompressionNumerics {
        self.numerics
    }
    pub fn pending_bytes(&self) -> u64 {
        if self.pending.is_some() {
            (self.width as u64) * 8
        } else {
            0
        }
    }

    /// Inputs are snapshotted on their device. Preparing, cancellation, failure
    /// or dropping the proposal cannot mutate committed state or caller tensors.
    pub fn prepare(
        &self,
        start_position: usize,
        scores: Option<&Tensor>,
        latents: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<PreparedCompression> {
        check_cancel(cancellation)?;
        let (tokens, width) = latents.dims2().map_err(tensor_error)?;
        let end = start_position
            .checked_add(tokens)
            .ok_or_else(|| invalid("compression position overflowed"))?;
        if start_position != self.position
            || width != self.width
            || tokens == 0
            || tokens > self.step_tokens
            || end > self.context_tokens
            || !latents.device().same_device(&self.device)
        {
            return Err(invalid(
                "compression input exceeds its position, shape, device or token bounds",
            ));
        }
        self.limits.checked_state_bytes(
            step_peak_bytes_with_numerics(self.kind, self.numerics, self.width, tokens)?,
            "compression step peak",
        )?;
        self.validate_inputs(scores, latents)?;
        let lease = ProposalLease::acquire(&self.proposal_live, "compression")?;
        let latents = latents.force_contiguous().map_err(tensor_error)?.detach();
        let scores = scores
            .map(|score| {
                score
                    .force_contiguous()
                    .map(|tensor| tensor.detach())
                    .map_err(tensor_error)
            })
            .transpose()?;
        validate_finite(&latents, false, "compression latent")?;
        if let Some(scores) = &scores {
            validate_finite(scores, false, "compression score")?;
        }
        let emissions = self.compute_emissions(scores.as_ref(), &latents)?;
        if let Some(emissions) = &emissions {
            validate_finite(emissions, false, "compressed latent")?;
        }
        let emitted_positions = match self.kind {
            CompressionKind::C1 => (start_position..end).collect(),
            CompressionKind::C2 => {
                let count = (tokens + usize::from(self.pending.is_some())) / 2;
                let first = start_position + usize::from(self.pending.is_none());
                (0..count).map(|index| first + 2 * index).collect()
            }
        };
        check_cancel(cancellation)?;
        Ok(PreparedCompression {
            owner: self.owner,
            generation: self.generation,
            position: self.position,
            tokens,
            latents,
            scores,
            emissions,
            emitted_positions,
            _lease: lease,
        })
    }

    fn validate_inputs(&self, scores: Option<&Tensor>, latents: &Tensor) -> Result<()> {
        match (self.kind,scores) {
            (CompressionKind::C1,None) if latents.dtype()==self.numerics.output_dtype() && self.pending.is_none()=>Ok(()),
            (CompressionKind::C2,Some(scores)) if latents.dtype()==DType::F32 && scores.dtype()==DType::F32
                && scores.shape()==latents.shape() && scores.device().same_device(&self.device)
                && self.pending.is_some()==!self.position.is_multiple_of(2)=>Ok(()),
            _=>Err(invalid("C1 requires its declared BF16/F32 latents without scores; C2 requires matching FP32 scores/latents and a consistent pending pair")),
        }
    }

    fn compute_emissions(
        &self,
        scores: Option<&Tensor>,
        latents: &Tensor,
    ) -> Result<Option<Tensor>> {
        if self.kind == CompressionKind::C1 {
            return Ok(Some(latents.clone()));
        }
        let scores = scores.ok_or_else(|| invalid("C2 compression scores are missing"))?;
        let compute = || -> candle_core::Result<Option<Tensor>> {
            let (scores, latents) = match &self.pending {
                Some(pending) => (
                    Tensor::cat(&[&pending.score, scores], 0)?,
                    Tensor::cat(&[&pending.latent, latents], 0)?,
                ),
                None => (scores.clone(), latents.clone()),
            };
            let rows = latents.dim(0)? / 2;
            if rows == 0 {
                return Ok(None);
            }
            let scores = scores
                .narrow(0, 0, rows * 2)?
                .reshape((rows, 2, self.width))?;
            let latents = latents
                .narrow(0, 0, rows * 2)?
                .reshape((rows, 2, self.width))?;
            let weights = candle_nn::ops::softmax(&scores, 1)?;
            let compressed = latents
                .mul(&weights)?
                .sum(1)?
                .to_dtype(self.numerics.output_dtype())?;
            Ok(Some(compressed.detach()))
        };
        compute().map_err(tensor_error)
    }

    /// Commit exactly an accepted prefix. Rejected rows cannot remain visible
    /// through an output view or the pending row's backing allocation.
    pub fn commit_prefix(
        &mut self,
        proposal: PreparedCompression,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<CommittedCompression> {
        let mut staged = self.stage_prefix(proposal, accepted, cancellation)?;
        staged.device().synchronize().map_err(tensor_error)?;
        check_cancel(cancellation)?;
        Ok(staged.apply())
    }

    fn stage_bytes(&self, accepted: usize) -> Result<u64> {
        if accepted == 0 {
            return Ok(0);
        }
        let position = self
            .position
            .checked_add(accepted)
            .filter(|p| *p <= self.context_tokens)
            .ok_or_else(|| invalid("compression accepted prefix exceeds context capacity"))?;
        let rows = position / self.kind.ratio() - self.position / self.kind.ratio();
        let pending = if self.kind == CompressionKind::C2 && position % 2 == 1 {
            self.width * 8
        } else {
            0
        };
        let per_row = self.width * self.numerics.output_bytes() + std::mem::size_of::<usize>();
        rows.checked_mul(per_row)
            .and_then(|n| n.checked_add(pending))
            .map(|n| n as u64)
            .ok_or_else(|| invalid("compression accepted snapshot bytes overflowed"))
    }

    fn stage_prefix(
        &mut self,
        proposal: PreparedCompression,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<CompressionCommit<'_>> {
        check_cancel(cancellation)?;
        if proposal.owner != self.owner
            || proposal.generation != self.generation
            || proposal.position != self.position
            || accepted > proposal.tokens
        {
            return Err(invalid("compression proposal belongs to another request/generation or has an invalid accepted prefix"));
        }
        if accepted == 0 {
            let position = self.position;
            let generation = self.generation;
            return Ok(CompressionCommit {
                state: self,
                position,
                generation,
                pending: None,
                changed: false,
                result: CommittedCompression {
                    emissions: None,
                    emitted_positions: Vec::new(),
                },
                _lease: proposal._lease,
            });
        }
        let position = self
            .position
            .checked_add(accepted)
            .filter(|position| *position <= self.context_tokens)
            .ok_or_else(|| invalid("compression accepted position exceeds context capacity"))?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| invalid("compression state generation exhausted"))?;
        let rows = position / self.kind.ratio() - self.position / self.kind.ratio();
        let emissions = match (rows, proposal.emissions.as_ref()) {
            (0, _) => None,
            (_, Some(tensor)) => Some(
                tensor
                    .narrow(0, 0, rows)
                    .and_then(|tensor| tensor.force_contiguous())
                    .map_err(tensor_error)?
                    .detach(),
            ),
            _ => {
                return Err(invalid(
                    "compression proposal is missing accepted emissions",
                ))
            }
        };
        let pending = if self.kind == CompressionKind::C2 && !position.is_multiple_of(2) {
            let scores = proposal
                .scores
                .as_ref()
                .ok_or_else(|| invalid("C2 proposal scores are missing"))?;
            Some(PendingPair {
                score: scores
                    .narrow(0, accepted - 1, 1)
                    .and_then(|tensor| tensor.force_contiguous())
                    .map_err(tensor_error)?
                    .detach(),
                latent: proposal
                    .latents
                    .narrow(0, accepted - 1, 1)
                    .and_then(|tensor| tensor.force_contiguous())
                    .map_err(tensor_error)?
                    .detach(),
            })
        } else {
            None
        };
        let emitted_positions = proposal
            .emitted_positions
            .get(..rows)
            .ok_or_else(|| invalid("compression proposal is missing accepted positions"))?
            .to_vec();
        check_cancel(cancellation)?;
        Ok(CompressionCommit {
            state: self,
            position,
            generation,
            pending,
            changed: true,
            result: CommittedCompression {
                emissions,
                emitted_positions,
            },
            _lease: proposal._lease,
        })
    }
}

struct CompressionCommit<'a> {
    state: &'a mut CompressionState,
    position: usize,
    generation: u64,
    pending: Option<PendingPair>,
    changed: bool,
    result: CommittedCompression,
    _lease: ProposalLease,
}

impl CompressionCommit<'_> {
    fn device(&self) -> &Device {
        &self.state.device
    }
    fn apply(&mut self) -> CommittedCompression {
        if self.changed {
            self.state.position = self.position;
            self.state.generation = self.generation;
            std::mem::swap(&mut self.state.pending, &mut self.pending);
            self.changed = false;
        }
        std::mem::replace(
            &mut self.result,
            CommittedCompression {
                emissions: None,
                emitted_positions: Vec::new(),
            },
        )
    }
}

pub struct PreparedCompression {
    owner: u64,
    generation: u64,
    position: usize,
    tokens: usize,
    scores: Option<Tensor>,
    latents: Tensor,
    emissions: Option<Tensor>,
    emitted_positions: Vec<usize>,
    _lease: ProposalLease,
}

struct ProposalLease {
    live: Arc<AtomicBool>,
}

impl ProposalLease {
    fn acquire(live: &Arc<AtomicBool>, kind: &str) -> Result<Self> {
        live.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                PowerError::InvalidRequest(format!(
                    "{kind} state already has an outstanding proposal"
                ))
            })?;
        Ok(Self {
            live: Arc::clone(live),
        })
    }
}

impl Drop for ProposalLease {
    fn drop(&mut self) {
        self.live.store(false, Ordering::Release);
    }
}

impl PreparedCompression {
    pub fn emissions(&self) -> Option<&Tensor> {
        self.emissions.as_ref()
    }
    pub fn emitted_positions(&self) -> &[usize] {
        &self.emitted_positions
    }
}

pub struct CommittedCompression {
    emissions: Option<Tensor>,
    emitted_positions: Vec<usize>,
}

impl CommittedCompression {
    pub fn emissions(&self) -> Option<&Tensor> {
        self.emissions.as_ref()
    }
    pub fn emitted_positions(&self) -> &[usize] {
        &self.emitted_positions
    }
}

/// Conservative per-state live peak, including caller inputs, owned snapshots,
/// pending row, pair concatenation, reduction/softmax working tensors and the
/// accepted-prefix copy. Power's later model-wide permit must aggregate states.
/// CUDA runtime/allocator overhead is not measured by this tensor estimate.
fn step_peak_bytes(kind: CompressionKind, width: usize, tokens: usize) -> Result<u64> {
    let units = if kind == CompressionKind::C1 {
        6_u64
    } else {
        24
    };
    (tokens as u64)
        .checked_add(2)
        .and_then(|rows| rows.checked_mul(width as u64))
        .and_then(|elements| elements.checked_mul(4))
        .and_then(|bytes| bytes.checked_mul(units))
        .ok_or_else(|| invalid("compression live-byte budget overflowed"))
}

fn step_peak_bytes_with_numerics(
    kind: CompressionKind,
    numerics: CompressionNumerics,
    width: usize,
    tokens: usize,
) -> Result<u64> {
    let base = step_peak_bytes(kind, width, tokens)?;
    if kind != CompressionKind::C1 || numerics != CompressionNumerics::XqF32 {
        return Ok(base);
    }
    // F32 C1 keeps larger input/snapshot/accepted buffers. At width one,
    // proposal/accepted position arrays also matter; the BF16 bound cannot
    // be reused just because C1 performs no weighted-pair arithmetic.
    let extra = (tokens as u64)
        .checked_add(2)
        .and_then(|n| n.checked_mul(width as u64))
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(|| invalid("F32 C1 snapshot budget overflowed"))?;
    base.checked_add(extra)
        .ok_or_else(|| invalid("F32 C1 peak budget overflowed"))
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}

#[cfg(test)]
mod tests;
