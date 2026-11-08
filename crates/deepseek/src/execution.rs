//! Callable XQ base-text pipeline: verified endpoints, all 40 layers and one
//! atomic 53-owner commit. This serial correctness profile does not advertise
//! a serving backend, TP/EP, batching or a measured eight-H200 speedup.

use crate::{
    engram::EngramToken,
    layers::{
        kv::MainKvFormat,
        tensor_error, validate_finite,
        xq::{
            check_cancel,
            engram::metadata::XqEngramMetadata,
            invalid,
            text::{XqTextHead, XqTokenEmbedding},
        },
        CudaLayerDevice,
    },
    model::{sharing::StepIndexSharing, topology::TextArchitecture, VerifiedCheckpointConfig},
    parallel::{XqDeviceMap, XqPlacement, XqStepTransfers},
    state::backbone::BackboneState,
    weights::{
        xq::{load::LoadedXqLayer, storage::XqWeights},
        CheckpointWeights,
    },
};
use aster_power::{
    error::{PowerError, Result},
    inference::{CudaPeerCopyReceipt, CudaPeerGroup, InferenceLimits, RuntimeDevice},
};
use candle_core::{DType, Tensor};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio_util::sync::CancellationToken;

mod greedy;
mod plan;
mod selection;
#[cfg(any(feature = "xq-validation-cache", test))]
pub(crate) mod validation_cache;
pub use greedy::{XqGeneratedTokens, XqGenerationStats, XqGenerationTimings, XqStopReason};
pub use plan::{XqDeviceAdmission, XqPipelinePlan};

pub struct XqNativePipeline<'a> {
    config: VerifiedCheckpointConfig,
    architecture: TextArchitecture,
    metadata: XqEngramMetadata,
    devices: Vec<CudaLayerDevice>,
    embedding: XqTokenEmbedding,
    layers: Vec<LoadedXqLayer<'a>>,
    head: XqTextHead,
    selection: selection::XqGreedySelector,
    peers: CudaPeerGroup,
    plan: XqPipelinePlan,
    limits: InferenceLimits,
    admissions: Vec<XqDeviceAdmission>,
    owner: Arc<()>,
    request_live: Arc<AtomicBool>,
    execution: Mutex<()>,
}

pub struct XqRequestState {
    owner: Arc<()>,
    backbone: BackboneState,
    _lease: RequestLease,
}
impl XqRequestState {
    pub fn position(&self) -> usize {
        self.backbone.position()
    }
    pub fn participant_count(&self) -> usize {
        self.backbone.participant_count()
    }
}

struct RequestLease(Arc<AtomicBool>);
impl RequestLease {
    fn acquire(live: &Arc<AtomicBool>) -> Result<Self> {
        live.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| invalid("serial XQ pipeline already owns its one request-state permit"))?;
        Ok(Self(Arc::clone(live)))
    }
}
impl Drop for RequestLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Logits cover every proposed row; accepted_tokens alone advanced the cache.
/// The argmax is selected on the GPU before the all-owner commit point.
pub struct XqStepOutput {
    pub logits: Tensor,
    pub proposed_tokens: usize,
    pub accepted_tokens: usize,
    pub cache_position: usize,
    pub last_argmax: u32,
    pub executed_layers_by_ordinal: [usize; 8],
    pub peer_copies: Vec<CudaPeerCopyReceipt>,
}

impl<'a> XqNativePipeline<'a> {
    pub fn load(
        weights: &XqWeights,
        tables: &'a CheckpointWeights,
        runtimes: &[RuntimeDevice],
        plan: XqPipelinePlan,
        limits: InferenceLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        Self::load_with_table_source(
            weights,
            crate::weights::engram::XqEngramTableSource::Official(tables),
            runtimes,
            plan,
            limits,
            cancellation,
        )
    }

    pub fn load_with_table_source(
        weights: &XqWeights,
        tables: crate::weights::engram::XqEngramTableSource<'a>,
        runtimes: &[RuntimeDevice],
        plan: XqPipelinePlan,
        limits: InferenceLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let mapping = XqDeviceMap::identity(plan.placement.replica_width());
        Self::load_with_device_map(
            weights,
            tables,
            runtimes,
            &mapping,
            plan,
            limits,
            cancellation,
        )
    }

    /// Load one complete replica on an explicit subset of process CUDA devices.
    /// Placement/statistics stay model-local; admissions and peer receipts name
    /// actual process ordinals. This does not reserve devices across replicas.
    pub fn load_with_device_map(
        weights: &XqWeights,
        tables: crate::weights::engram::XqEngramTableSource<'a>,
        runtimes: &[RuntimeDevice],
        mapping: &XqDeviceMap,
        plan: XqPipelinePlan,
        limits: InferenceLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        plan.validate(&limits)?;
        let config = weights.model_config()?;
        let architecture = config.text_architecture()?;
        let resident = plan.resident_by_device(weights, &architecture, &limits)?;
        let (devices, admissions) =
            plan::admit_mapped_devices(runtimes, mapping, &resident, &plan, &limits)?;
        // All device budgets/capabilities and directed peer permissions are
        // checked before uploading the first model tensor.
        let peers = CudaPeerGroup::new(
            runtimes,
            &mapping.peer_edges(&plan.placement.peer_edges(&architecture)?)?,
            limits.clone(),
        )?;
        let metadata = XqEngramMetadata::load(weights, &limits, cancellation)?;
        let embedding = XqTokenEmbedding::load(
            &devices[plan.placement.embedding_ordinal()],
            weights,
            cancellation,
        )?;
        let mut layers = Vec::with_capacity(40);
        for (id, &ordinal) in plan.placement.layer_ordinals().iter().enumerate() {
            check_cancel(cancellation)?;
            layers.push(LoadedXqLayer::load_with_table_source(
                weights,
                Some(tables),
                &metadata,
                &architecture,
                id,
                &devices[ordinal],
                plan.context_tokens,
                cancellation,
            )?);
        }
        let head = XqTextHead::load(
            &devices[plan.placement.head_ordinal()],
            weights,
            cancellation,
        )?;
        let selection = selection::XqGreedySelector::new(
            &devices[plan.placement.head_ordinal()],
            head.vocabulary_size(),
            cancellation,
        )?;
        for device in &devices {
            device.tensor_device().synchronize().map_err(tensor_error)?;
        }
        check_cancel(cancellation)?;
        Ok(Self {
            config,
            architecture,
            metadata,
            devices,
            embedding,
            layers,
            head,
            selection,
            peers,
            plan,
            limits,
            admissions,
            owner: Arc::new(()),
            request_live: Arc::new(AtomicBool::new(false)),
            execution: Mutex::new(()),
        })
    }
    pub fn model_config(&self) -> &VerifiedCheckpointConfig {
        &self.config
    }
    pub fn placement(&self) -> &XqPlacement {
        &self.plan.placement
    }
    pub fn device_admissions(&self) -> &[XqDeviceAdmission] {
        &self.admissions
    }

    pub fn new_request_state(&self, cancellation: &CancellationToken) -> Result<XqRequestState> {
        check_cancel(cancellation)?;
        let lease = RequestLease::acquire(&self.request_live)?;
        let layer_devices: Vec<_> = self
            .plan
            .placement
            .layer_ordinals()
            .iter()
            .map(|&ordinal| self.devices[ordinal].clone())
            .collect();
        let backbone = BackboneState::new(
            &self.architecture,
            &layer_devices,
            self.plan.context_tokens,
            self.plan.max_step_tokens,
            MainKvFormat::Fp4E4m3,
            self.plan.max_staged_bytes,
        )?;
        check_cancel(cancellation)?;
        Ok(XqRequestState {
            owner: Arc::clone(&self.owner),
            backbone,
            _lease: lease,
        })
    }

    pub fn forward_tokens(
        &self,
        tokens: &[u32],
        state: &mut XqRequestState,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<XqStepOutput> {
        let _serial = self.execution.try_lock().map_err(|_| {
            PowerError::BackendNotAvailable(
                "serial XQ pipeline is busy or its execution lock is poisoned".into(),
            )
        })?;
        self.forward_locked(tokens, state, accepted, cancellation)
    }

    fn forward_locked(
        &self,
        tokens: &[u32],
        state: &mut XqRequestState,
        accepted: usize,
        cancellation: &CancellationToken,
    ) -> Result<XqStepOutput> {
        check_cancel(cancellation)?;
        if !Arc::ptr_eq(&self.owner, &state.owner) {
            return Err(invalid(
                "XQ request state belongs to a different loaded model",
            ));
        }
        plan::validate_token_step(tokens, state.position(), accepted, &self.plan, &self.limits)?;
        // This scope contains only model-owned immutable tensors. No Tensor is
        // accepted from a caller, returned, or mutated in place within it.
        // The scope ends on every return, before any next request/step.
        #[cfg(feature = "xq-validation-cache")]
        let validation_scope =
            validation_cache::Scope::begin(&self.limits, self.plan.max_staged_bytes)?;
        let ngrams = tokens
            .iter()
            .map(|&id| EngramToken::from_raw(id, self.metadata.vocabulary()))
            .collect::<Result<Vec<_>>>()?;
        let mut pending = state.backbone.begin(&ngrams, cancellation)?;
        let positions: Vec<_> = (state.position()..state.position() + tokens.len()).collect();
        let mut sharing = StepIndexSharing::new(&self.limits, &positions)?;
        let mut transfers = XqStepTransfers::new(
            &self.peers,
            &self.limits,
            self.plan.max_peer_bytes,
            self.plan.max_peer_copies,
        )?;
        let mut trunk = self.embedding.initial_continuation(tokens, cancellation)?;
        let mut executed = [0; 8];
        for (id, layer) in self.layers.iter().enumerate() {
            let ordinal = self.plan.placement.layer_ordinals()[id];
            trunk = trunk.move_to(&self.devices[ordinal], &mut transfers, cancellation)?;
            trunk = layer.forward_with_transfers(
                trunk,
                &state.backbone,
                &mut pending,
                &mut sharing,
                &mut transfers,
                cancellation,
            )?;
            executed[ordinal] += 1;
        }
        let mut expected = [0; 8];
        for &ordinal in self.plan.placement.layer_ordinals() {
            expected[ordinal] += 1;
        }
        if !sharing.is_complete() || executed != expected || executed.iter().sum::<usize>() != 40 {
            return Err(invalid(
                "XQ pipeline did not execute its complete forty-layer replica-local schedule",
            ));
        }
        let head_device = &self.devices[self.plan.placement.head_ordinal()];
        trunk = trunk.move_to(head_device, &mut transfers, cancellation)?;
        let (logits, _) = self.head.forward(&trunk, cancellation)?;
        if logits.dtype() != DType::F32
            || logits.dims() != [tokens.len(), crate::tokenizer::RAW_VOCABULARY]
            || !logits.device().same_device(head_device.tensor_device())
        {
            return Err(invalid(
                "XQ pipeline head returned an unbound logits shape/dtype/device",
            ));
        }
        validate_finite(&logits, false, "XQ complete pipeline logits")?;
        let last_argmax = self.selection.select_last(&logits, cancellation)?;
        transfers.validate_complete(cancellation)?;
        // Complete host allocations and observe deferred device errors before
        // publication. Commit itself stages/synchronizes every cache owner.
        let peer_copies = transfers.receipts().to_vec();
        #[cfg(feature = "xq-validation-cache")]
        validation_scope.validate_complete()?;
        state
            .backbone
            .commit_prefix(pending, accepted, cancellation)?;
        Ok(XqStepOutput {
            logits,
            proposed_tokens: tokens.len(),
            accepted_tokens: accepted,
            cache_position: state.position(),
            last_argmax,
            executed_layers_by_ordinal: executed,
            peer_copies,
        })
    }

    pub fn generate_greedy(
        &self,
        prompt: &[u32],
        max_new_tokens: usize,
        cancellation: &CancellationToken,
    ) -> Result<XqGeneratedTokens> {
        self.generate_greedy_streaming(prompt, max_new_tokens, cancellation, |_| Ok(()))
    }

    /// Emits each GPU-selected ID before the next decode step. Callback error
    /// or cancellation ends the request and releases its private cache lease;
    /// a buffered completion must not be replayed as an incremental stream.
    pub fn generate_greedy_streaming(
        &self,
        prompt: &[u32],
        max_new_tokens: usize,
        cancellation: &CancellationToken,
        emit: impl FnMut(u32) -> Result<()>,
    ) -> Result<XqGeneratedTokens> {
        let _serial = self.execution.try_lock().map_err(|_| {
            PowerError::BackendNotAvailable(
                "serial XQ pipeline is busy or its execution lock is poisoned".into(),
            )
        })?;
        greedy::validate_request(prompt, max_new_tokens, &self.plan, &self.limits)?;
        let mut state = self.new_request_state(cancellation)?;
        greedy::run_streaming(
            prompt,
            max_new_tokens,
            &self.plan,
            &self.limits,
            cancellation,
            |tokens| {
                let output = self.forward_locked(tokens, &mut state, tokens.len(), cancellation)?;
                greedy::StepChoice::from_output(output)
            },
            emit,
        )
    }
}

#[cfg(test)]
mod tests;
