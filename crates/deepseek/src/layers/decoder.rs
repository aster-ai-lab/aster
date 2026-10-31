//! Complete shifted decoder layer, optionally intervening with Engram before
//! attention pre. Source/index/KV planning occurs between prepare and finish;
//! BackboneState commits the complete request only after all layers succeed.
//! DeepInfer Apache-2.0 hc.py/hc_blocks.py composition at revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. No model loop is registered here.

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::attention::projection::{add, check_cancel, resident, validate_owner};
use super::{
    attention::{
        sublayer::{AttentionSublayer, PreparedAttentionSublayer},
        SparseAttentionPlan,
    },
    engram::{lookup::EngramEmbeddings, projection::EngramProjection},
    invalid,
    kv::MainKvRows,
    mhc::MhcContinuation,
    sublayer::MoeSublayer,
    tensor_error, CudaLayerDevice,
};
use crate::model::topology::LayerBinding;

pub struct TextDecoderLayer {
    device: CudaLayerDevice,
    layer: usize,
    attention: AttentionSublayer,
    ffn: MoeSublayer,
    engram: Option<EngramProjection>,
    resident_bytes: u64,
}

impl TextDecoderLayer {
    pub fn new(
        device: CudaLayerDevice,
        binding: &LayerBinding,
        attention: AttentionSublayer,
        ffn: MoeSublayer,
        engram: Option<EngramProjection>,
    ) -> Result<Self> {
        if attention.hidden_size() != ffn.hidden_size()
            || engram.is_some() != binding.engram().is_some()
            || engram
                .as_ref()
                .is_some_and(|e| e.hidden_size() != attention.hidden_size())
        {
            return Err(invalid(
                "decoder halves/Engram geometry or explicit layer placement differ",
            ));
        }
        for owner in [attention.device(), ffn.device()]
            .into_iter()
            .chain(engram.as_ref().map(EngramProjection::device))
        {
            validate_owner(&device, owner)?;
        }
        let resident_bytes = resident(
            &device,
            add(
                add(attention.retained_bytes(), ffn.retained_weight_bytes())?,
                engram
                    .as_ref()
                    .map_or(0, EngramProjection::retained_weight_bytes),
            )?,
        )?;
        Ok(Self {
            device,
            layer: binding.layer(),
            attention,
            ffn,
            engram,
            resident_bytes,
        })
    }
    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub fn initial_continuation(&self, embedding: &Tensor) -> Result<MhcContinuation> {
        if self.layer != 0 {
            return Err(invalid(
                "only backbone layer zero initializes the shifted trunk",
            ));
        }
        self.attention.initial_shifted(embedding)
    }

    pub fn prepare(
        &self,
        continuation: MhcContinuation,
        positions: &[usize],
        embeddings: Option<&EngramEmbeddings>,
        image_mask: Option<&Tensor>,
        cancellation: &CancellationToken,
    ) -> Result<PreparedDecoderLayer> {
        check_cancel(cancellation)?;
        let tokens = continuation.residual().dims3().map_err(tensor_error)?.0;
        if embeddings.is_some() != self.engram.is_some() {
            return Err(invalid(
                "decoder requires Engram embeddings exactly at its bound Engram layer",
            ));
        }
        let engram_bytes = match (&self.engram, embeddings) {
            (Some(e), Some(x)) => e.live_tensor_bytes(tokens, x.live_tensor_bytes())?,
            _ => 0,
        };
        let live = add(
            add(
                self.attention.prepare_live_tensor_bytes(tokens)?,
                self.ffn.live_tensor_bytes(tokens)?,
            )?,
            add(engram_bytes, self.resident_bytes)?,
        )?;
        self.device
            .limits()
            .checked_state_bytes(live, "complete decoder prepare live tensors")?;
        let inferred = embeddings.map(EngramEmbeddings::image_mask);
        let image_mask = freeze_mask(&self.device, image_mask.or(inferred), tokens)?;
        if let (Some(actual), Some(expected)) = (image_mask.as_ref(), inferred) {
            if actual
                .eq(expected)
                .and_then(|t| t.to_dtype(DType::U32))
                .and_then(|t| t.sum_all())
                .and_then(|t| t.to_scalar::<u32>())
                .map_err(tensor_error)?
                != tokens as u32
            {
                return Err(invalid(
                    "decoder image mask differs from its Engram hash rows",
                ));
            }
        }
        let continuation = match (&self.engram, embeddings) {
            (Some(e), Some(x)) => e.forward(continuation, x, cancellation)?,
            _ => continuation,
        };
        let attention = self
            .attention
            .prepare(continuation, positions, cancellation)?;
        Ok(PreparedDecoderLayer {
            layer: self.layer,
            attention,
            image_mask,
        })
    }

    pub fn finish(
        &self,
        prepared: PreparedDecoderLayer,
        window: &MainKvRows,
        compressed: Option<&MainKvRows>,
        plan: &SparseAttentionPlan,
        cancellation: &CancellationToken,
    ) -> Result<MhcContinuation> {
        check_cancel(cancellation)?;
        if prepared.layer != self.layer {
            return Err(invalid("prepared decoder belongs to a different layer"));
        }
        let tokens = prepared.attention.projected().positions().len();
        let live = add(
            add(
                self.attention.finish_live_tensor_bytes(tokens, plan)?,
                self.ffn.live_tensor_bytes(tokens)?,
            )?,
            self.resident_bytes,
        )?;
        self.device
            .limits()
            .checked_state_bytes(live, "complete decoder finish live tensors")?;
        let continued =
            self.attention
                .finish(prepared.attention, window, compressed, plan, cancellation)?;
        self.ffn
            .forward(continued, prepared.image_mask.as_ref(), cancellation)
    }
}

pub struct PreparedDecoderLayer {
    layer: usize,
    attention: PreparedAttentionSublayer,
    image_mask: Option<Tensor>,
}
impl PreparedDecoderLayer {
    pub fn attention(&self) -> &PreparedAttentionSublayer {
        &self.attention
    }
}

fn freeze_mask(
    device: &CudaLayerDevice,
    mask: Option<&Tensor>,
    tokens: usize,
) -> Result<Option<Tensor>> {
    let Some(mask) = mask else { return Ok(None) };
    if mask.dims() != [tokens]
        || mask.dtype() != DType::U8
        || !mask.device().same_device(device.tensor_device())
    {
        return Err(invalid(
            "decoder image mask requires same-device boolean U8 token rows",
        ));
    }
    let mask = mask.force_contiguous().map_err(tensor_error)?.detach();
    if mask
        .max_all()
        .and_then(|t| t.to_scalar::<u8>())
        .map_err(tensor_error)?
        > 1
    {
        return Err(invalid("decoder image mask values must be zero or one"));
    }
    Ok(Some(mask))
}

#[cfg(test)]
mod tests;
