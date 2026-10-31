//! Exact XQ weights -> a complete callable decoder layer with uncommitted
//! own-window/compressed/index/history state. Sequential sharing is explicit;
//! a failed layer poisons the whole request proposal. No backend is registered.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::storage::XqWeights;
use crate::{
    layers::{
        attention::projection::{add, resident, validate_owner},
        indexer::{CandidateBlockPolicy, CandidateBlockWidth, SparseIndexer},
        tensor_error,
        xq::{
            attention::sublayer::XqAttentionSublayer,
            check_cancel,
            compressor::XqKvCompressor,
            engram::{metadata::XqEngramMetadata, XqEngramLayer},
            indexer::{XqIndexKeyProjection, XqIndexQueryProjection},
            invalid,
            mhc::XqMhcContinuation,
            moe::XqFfnSublayer,
            same_device,
        },
        CudaLayerDevice,
    },
    model::{
        sharing::StepIndexSharing,
        topology::{LayerBinding, TextArchitecture},
    },
    state::{
        backbone::{BackboneState, PendingBackboneStep},
        CompressionNumerics,
    },
    weights::CheckpointWeights,
};

pub struct LoadedXqLayer<'a> {
    device: CudaLayerDevice,
    binding: LayerBinding,
    attention: XqAttentionSublayer,
    ffn: XqFfnSublayer,
    engram: Option<XqEngramLayer<'a>>,
    compressor: Option<XqKvCompressor>,
    index_query: Option<XqIndexQueryProjection>,
    index_key: Option<XqIndexKeyProjection>,
    indexer: Option<SparseIndexer>,
    resident_upper_bytes: u64,
}

impl<'a> LoadedXqLayer<'a> {
    pub(crate) fn resident_bound(
        weights: &XqWeights,
        architecture: &TextArchitecture,
        layer: usize,
        context: usize,
    ) -> Result<u64> {
        let binding = architecture.layer(layer)?;
        let prefix = format!("blk.{layer}.");
        let mut bytes = 0_u64;
        for (name, tensor) in weights.header().tensors() {
            if name.starts_with(&prefix) {
                bytes = add(bytes, tensor.bytes)?;
            }
        }
        if bytes == 0 {
            return Err(invalid("XQ layer has no tensors in the pinned namespace"));
        }
        let caches = 2
            + usize::from(binding.is_kv_source())
            + usize::from(binding.is_index_topk_source())
            + usize::from(binding.is_index_k_source());
        let rotary = (context as u64)
            .checked_mul(64 * 4)
            .and_then(|n| n.checked_mul(caches as u64))
            .ok_or_else(|| invalid("XQ layer rotary snapshot bytes overflowed"))?;
        add(bytes, rotary)
    }

    pub fn load(
        weights: &XqWeights,
        tables: Option<&'a CheckpointWeights>,
        metadata: &XqEngramMetadata,
        architecture: &TextArchitecture,
        layer: usize,
        device: &CudaLayerDevice,
        context: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        Self::load_with_table_source(
            weights,
            tables.map(crate::weights::engram::XqEngramTableSource::Official),
            metadata,
            architecture,
            layer,
            device,
            context,
            cancellation,
        )
    }

    pub(crate) fn load_with_table_source(
        weights: &XqWeights,
        tables: Option<crate::weights::engram::XqEngramTableSource<'a>>,
        metadata: &XqEngramMetadata,
        architecture: &TextArchitecture,
        layer: usize,
        device: &CudaLayerDevice,
        context: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let binding = architecture.layer(layer)?;
        if binding.compression_numerics() != CompressionNumerics::XqF32 {
            return Err(invalid(
                "XQ decoder loader requires its pinned F32 architecture",
            ));
        }
        if context == 0 || context > device.limits().max_context_tokens {
            return Err(invalid(
                "XQ layer rotary context exceeds its nonempty budget",
            ));
        }
        // A conservative upper bound over the exact pinned layer namespace,
        // including integer partition maps that are actually kept on the host.
        // Each projection owner separately validates exact type/shape/entities.
        let resident_upper_bytes = resident(
            device,
            Self::resident_bound(weights, architecture, layer, context)?,
        )?;
        let construction = resident_upper_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(weights.max_read_bytes()))
            .ok_or_else(|| invalid("XQ layer construction bytes overflowed"))?;
        device
            .limits()
            .checked_state_bytes(construction, "XQ complete layer conservative construction")?;
        let engram = if let Some(e) = binding.engram() {
            let table = tables.ok_or_else(|| {
                invalid("XQ Engram layer is missing its verified external table collection")
            })?;
            let layer = match e.table() {
                0 => crate::engram::EngramLayer::Layer1,
                1 => crate::engram::EngramLayer::Layer14,
                _ => return Err(invalid("XQ Engram table slot is outside exact [1,14] set")),
            };
            Some(XqEngramLayer::load_with_table_source(
                device,
                weights,
                metadata,
                table,
                layer,
                cancellation,
            )?)
        } else {
            None
        };
        let cache = binding.new_rope_cache(device, context, cancellation)?;
        let attention = XqAttentionSublayer::load(
            device,
            weights,
            layer,
            cache.cos_sin().clone(),
            cancellation,
        )?;
        let ffn = XqFfnSublayer::load(device, weights, layer, cancellation)?;
        let compressor = binding
            .is_kv_source()
            .then(|| {
                XqKvCompressor::load(
                    device,
                    weights,
                    layer,
                    cache.cos_sin().clone(),
                    cancellation,
                )
            })
            .transpose()?;
        let index_query = binding
            .is_index_topk_source()
            .then(|| {
                XqIndexQueryProjection::load(
                    device,
                    weights,
                    layer,
                    cache.cos_sin().clone(),
                    cancellation,
                )
            })
            .transpose()?;
        let index_key = binding
            .is_index_k_source()
            .then(|| {
                XqIndexKeyProjection::load(
                    device,
                    weights,
                    layer,
                    cache.cos_sin().clone(),
                    cancellation,
                )
            })
            .transpose()?;
        let indexer = if binding.is_index_topk_source() {
            let policy = binding
                .is_candidate_source()
                .then(|| CandidateBlockPolicy::new(CandidateBlockWidth::Eight, 2048))
                .transpose()?;
            Some(SparseIndexer::new(device.clone(), 32, 128, 512, policy)?)
        } else {
            None
        };
        validate_owner(device, attention.device())?;
        validate_owner(device, ffn.device())?;
        if attention.hidden_size() != ffn.hidden_size() {
            return Err(invalid("XQ attention and FFN hidden widths differ"));
        }
        Ok(Self {
            device: device.clone(),
            binding: binding.clone(),
            attention,
            ffn,
            engram,
            compressor,
            index_query,
            index_key,
            indexer,
            resident_upper_bytes,
        })
    }

    pub fn binding(&self) -> &LayerBinding {
        &self.binding
    }
    pub fn resident_upper_bytes(&self) -> u64 {
        self.resident_upper_bytes
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    fn prepare_live_tensor_bytes(&self, state: &BackboneState, tokens: usize) -> Result<u64> {
        let mut bytes = add(
            add(
                self.attention.prepare_live_tensor_bytes(tokens)?,
                self.ffn.live_tensor_bytes(tokens)?,
            )?,
            self.resident_upper_bytes,
        )?;
        if let Some(engram) = &self.engram {
            bytes = add(bytes, engram.live_tensor_bytes(tokens)?)?;
        }
        if let Some(compressor) = &self.compressor {
            bytes = add(
                bytes,
                compressor.live_tensor_bytes(
                    state.compressed_source(self.binding.layer())?.compression(),
                    tokens,
                )?,
            )?;
        }
        if let Some(query) = &self.index_query {
            bytes = add(bytes, query.live_tensor_bytes(tokens)?)?;
        }
        if let Some(key) = &self.index_key {
            bytes = add(bytes, key.live_tensor_bytes(tokens)?)?;
        }
        let mut packed = state.window(self.binding.layer())?.retained_bytes();
        if let Some(topology) = self.binding.sources() {
            let source = state.compressed_source(topology.kv())?;
            packed = add(
                packed,
                add(
                    source.main().retained_bytes(),
                    source.index().retained_bytes(),
                )?,
            )?;
            if self.index_query.is_some() {
                // Conservative all-candidate F32 decode/selection workspace.
                // Model-wide permits and measured allocator peaks stay separate.
                let candidate = (source.index().row_count() as u64)
                    .checked_add(tokens as u64)
                    .and_then(|n| n.checked_mul(128 * 256))
                    .ok_or_else(|| invalid("XQ index candidate live bytes overflowed"))?;
                bytes = add(bytes, candidate)?;
            }
        }
        let proposed = (tokens as u64)
            .checked_mul(528 + 288 + 68)
            .ok_or_else(|| invalid("XQ proposed packed rows overflowed"))?;
        packed = add(packed, proposed)?;
        let ready_rows = self
            .binding
            .sources()
            .map(|s| {
                state
                    .compressed_source(s.kv())
                    .map(|s| s.main().row_count())
            })
            .transpose()?
            .unwrap_or(0);
        let identities = ready_rows
            .checked_add(tokens)
            .and_then(|n| n.checked_mul(32))
            .and_then(|n| n.checked_add(tokens.checked_mul(64)?))
            .ok_or_else(|| invalid("XQ prepared row metadata bytes overflowed"))?;
        if identities > self.device.limits().max_input_bytes {
            return Err(invalid(
                "XQ prepared row identities exceed input metadata budget",
            ));
        }
        add(
            bytes,
            packed
                .checked_mul(32)
                .ok_or_else(|| invalid("XQ prepared cache view live bytes overflowed"))?,
        )
    }

    pub fn forward_pending(
        &self,
        input: XqMhcContinuation,
        state: &BackboneState,
        pending: &mut PendingBackboneStep,
        sharing: &mut StepIndexSharing,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        let mut transfers = crate::parallel::XqStepTransfers::local(self.device.limits());
        self.forward_with_transfers(input, state, pending, sharing, &mut transfers, cancellation)
    }

    pub(crate) fn forward_with_transfers(
        &self,
        input: XqMhcContinuation,
        state: &BackboneState,
        pending: &mut PendingBackboneStep,
        sharing: &mut StepIndexSharing,
        transfers: &mut crate::parallel::XqStepTransfers<'_>,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        let result = self.forward_step(input, state, pending, sharing, transfers, cancellation);
        if result.is_err() {
            pending.poison();
            sharing.poison();
            transfers.poison();
        }
        result
    }

    fn forward_step(
        &self,
        input: XqMhcContinuation,
        state: &BackboneState,
        pending: &mut PendingBackboneStep,
        sharing: &mut StepIndexSharing,
        transfers: &mut crate::parallel::XqStepTransfers<'_>,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        check_cancel(cancellation)?;
        if pending.is_poisoned()
            || pending.position() != state.position()
            || state.compression_numerics() != CompressionNumerics::XqF32
        {
            return Err(invalid(
                "XQ layer requires the current unpoisoned F32 backbone proposal",
            ));
        }
        sharing.validate_next_layer(&self.binding)?;
        same_device(self.device(), input.residual())?;
        let tokens = pending.token_count();
        if input.residual().dims() != [tokens, 4, self.attention.hidden_size()]
            || input.residual().dtype() != DType::F32
        {
            return Err(invalid("XQ layer trunk differs from the current F32 step"));
        }
        if tokens
            .checked_mul(16)
            .is_none_or(|n| n > self.device.limits().max_input_bytes)
        {
            return Err(invalid(
                "XQ step positions/image metadata exceed input budget",
            ));
        }
        let end = pending
            .position()
            .checked_add(tokens)
            .ok_or_else(|| invalid("XQ step position overflowed"))?;
        let positions: Vec<_> = (pending.position()..end).collect();
        if positions != sharing.positions() {
            return Err(invalid("XQ shared-index and request positions differ"));
        }
        let live = self.prepare_live_tensor_bytes(state, tokens)?;
        self.device
            .limits()
            .checked_state_bytes(live, "XQ complete decoder prepare live tensors")?;
        let image_mask = Tensor::from_vec(
            pending
                .ngrams()
                .iter()
                .map(|r| u8::from(r.is_image()))
                .collect::<Vec<_>>(),
            tokens,
            self.device.tensor_device(),
        )
        .map_err(tensor_error)?;
        let input = match &self.engram {
            Some(e) => e.forward(input, pending.ngrams(), cancellation)?,
            None => input,
        };
        let prepared = self.attention.prepare(input, &positions, cancellation)?;
        let layer = self.binding.layer();
        let window = state.window(layer)?.prepare(
            pending.position(),
            tokens,
            &positions,
            prepared.projected().window_rows().records(),
            cancellation,
        )?;
        pending.stage_window(layer, window, cancellation)?;
        if self.binding.is_kv_source() {
            let source = state.compressed_source(layer)?;
            let compressor = self
                .compressor
                .as_ref()
                .ok_or_else(|| invalid("XQ source is missing its compressor"))?;
            let index_key = self
                .index_key
                .as_ref()
                .ok_or_else(|| invalid("XQ source is missing its index-K projection"))?;
            let compressed = compressor.prepare(
                source.compression(),
                pending.position(),
                prepared.current_hidden(),
                cancellation,
            )?;
            let keys = index_key.forward_compressed(&compressed, cancellation)?;
            let main = source.main().prepare(
                pending.position(),
                tokens,
                compressed.emitted_positions(),
                compressed.rows().records(),
                cancellation,
            )?;
            let index = source.index().prepare(
                pending.position(),
                tokens,
                compressed.emitted_positions(),
                keys.data_bytes(),
                keys.scale_bytes(),
                cancellation,
            )?;
            let (compression, _, _, _) = compressed.into_parts();
            pending.stage_source(layer, compression, main, index, cancellation)?;
        }
        let source = self
            .binding
            .sources()
            .map(|s| pending.compressed_source(s.kv()))
            .transpose()?;
        let main = source
            .map(|s| {
                s.main()
                    .attention_rows_with_transfer(&self.device, transfers, cancellation)
            })
            .transpose()?;
        let main_ready = source.map(|s| s.main().ready_positions().collect::<Vec<_>>());
        let selected = if self.binding.is_index_topk_source() {
            let query = self
                .index_query
                .as_ref()
                .ok_or_else(|| invalid("XQ index source is missing its query projection"))?;
            let indexer = self
                .indexer
                .as_ref()
                .ok_or_else(|| invalid("XQ index source is missing its selector"))?;
            let topology = self
                .binding
                .sources()
                .ok_or_else(|| invalid("XQ index source lacks compressed topology"))?;
            let index_source = pending.compressed_source(topology.index_k())?;
            let keys = index_source.index().index_rows_with_transfer(
                &self.device,
                transfers,
                cancellation,
            )?;
            let query = query.forward(
                prepared.current_hidden(),
                prepared.projected().normalized_query(),
                &positions,
                cancellation,
            )?;
            let ready = main_ready
                .as_deref()
                .ok_or_else(|| invalid("XQ index source has no prepared main rows"))?;
            if index_source.index().ready_positions().collect::<Vec<_>>() != ready {
                return Err(invalid("XQ prepared main/index row identities differ"));
            }
            Some(indexer.select_xq_f32_packed(
                query.rows(),
                &keys,
                query.head_weights(),
                &sharing.index_plan(&self.binding, ready)?,
                cancellation,
            )?)
        } else {
            None
        };
        let window = pending.window(layer)?;
        let window_ready = window.ready_positions().collect::<Vec<_>>();
        let window = window.attention_rows(&self.device)?;
        let plan = sharing.attention_plan(
            &self.binding,
            &window_ready,
            main_ready.as_deref(),
            selected.as_ref(),
            cancellation,
        )?;
        self.device.limits().checked_state_bytes(
            add(
                self.attention.finish_live_tensor_bytes(&plan)?,
                self.prepare_live_tensor_bytes(state, tokens)?,
            )?,
            "XQ complete decoder finish live tensors",
        )?;
        let continued =
            self.attention
                .finish(prepared, &window, main.as_ref(), &plan, cancellation)?;
        self.ffn.forward(continued, Some(&image_mask), cancellation)
    }
}

#[cfg(test)]
pub(crate) mod tests;
