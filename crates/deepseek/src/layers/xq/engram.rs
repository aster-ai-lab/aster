//! XQ Engram: original E4M3/E8M0 sparse table rows, exported F16 fused
//! projection and normalize-first F32 signed-root gate. Xolotl adaptation,
//! ds41_dense.cpp at b3452666e07a6374d06bb8c2185dffa271e8f65f (MIT).

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{check_cancel, invalid, mhc::XqMhcContinuation, rms_math, same_device};
use crate::{
    engram::{EngramLayer, NgramRow},
    layers::{
        engram::lookup::{EngramEmbeddings, EngramTable, PROJECTED_INPUT_WIDTH},
        linear::xq::XqDeviceMatrix,
        tensor_error, validate_finite, CudaLayerDevice,
    },
    weights::{
        xq::{storage::XqWeights, XqTensorType},
        CheckpointWeights,
    },
};

pub mod metadata;

/// Exact hash layer + verified external table + XQ F32 projection. Preparing
/// this contribution changes no request history; the backbone commits later.
pub struct XqEngramLayer<'a> {
    projection: XqEngramProjection,
    table: EngramTable<'a>,
}

impl<'a> XqEngramLayer<'a> {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        metadata: &metadata::XqEngramMetadata,
        checkpoint: &'a CheckpointWeights,
        layer: EngramLayer,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        Self::load_with_table_source(
            device,
            weights,
            metadata,
            crate::weights::engram::XqEngramTableSource::Official(checkpoint),
            layer,
            cancellation,
        )
    }

    pub(crate) fn load_with_table_source(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        metadata: &metadata::XqEngramMetadata,
        source: crate::weights::engram::XqEngramTableSource<'a>,
        layer: EngramLayer,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let table = metadata.bind_table_source(device, source, layer)?;
        let projection = XqEngramProjection::load(device, weights, layer, cancellation)?;
        Ok(Self { projection, table })
    }

    pub fn layer(&self) -> EngramLayer {
        self.projection.layer()
    }
    pub fn hidden_size(&self) -> usize {
        self.projection.hidden_size()
    }
    pub fn retained_bytes(&self) -> u64 {
        self.projection.retained_bytes()
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        self.projection.device()
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        self.projection
            .live_tensor_bytes(tokens, self.table.live_tensor_bytes(tokens)?)
    }

    pub fn forward(
        &self,
        continuation: XqMhcContinuation,
        ngrams: &[NgramRow],
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        check_cancel(cancellation)?;
        same_device(self.device(), continuation.residual())?;
        if continuation.residual().dims() != [ngrams.len(), 4, self.hidden_size()]
            || continuation.residual().dtype() != DType::F32
        {
            return Err(invalid("XQ Engram history rows differ from the F32 trunk"));
        }
        self.live_tensor_bytes(ngrams.len())?;
        let embeddings = self
            .table
            .fetch_ngrams(self.layer(), ngrams, cancellation)?;
        self.projection
            .forward(continuation, &embeddings, cancellation)
    }
}

pub struct XqEngramGate {
    device: CudaLayerDevice,
    query_weight: Tensor,
    key_weight: Tensor,
    hidden: usize,
}

impl XqEngramGate {
    pub fn new(device: CudaLayerDevice, query_weight: Tensor, key_weight: Tensor) -> Result<Self> {
        let (streams, hidden) = query_weight.dims2().map_err(tensor_error)?;
        if streams != 4 || hidden == 0 || hidden > 5120 {
            return Err(invalid("XQ Engram norm requires F32 [4,1..5120] weights"));
        }
        for weight in [&query_weight, &key_weight] {
            same_device(&device, weight)?;
            if weight.dtype() != DType::F32 || weight.dims() != [4, hidden] {
                return Err(invalid("XQ Engram norm weight geometry/dtype differs"));
            }
        }
        device
            .limits()
            .checked_elements(&[4, hidden], "XQ Engram norm")?;
        let retained = hidden as u64 * 32;
        if retained > device.limits().max_resident_weight_bytes {
            return Err(invalid("XQ Engram norms exceed resident parameter budget"));
        }
        device
            .limits()
            .checked_state_bytes(retained * 16 + 4096, "XQ Engram norm snapshot")?;
        let query_weight = query_weight
            .force_contiguous()
            .map_err(tensor_error)?
            .detach();
        let key_weight = key_weight
            .force_contiguous()
            .map_err(tensor_error)?
            .detach();
        validate_finite(&query_weight, false, "XQ Engram query norm")?;
        validate_finite(&key_weight, false, "XQ Engram key norm")?;
        Ok(Self {
            device,
            query_weight,
            key_weight,
            hidden,
        })
    }

    pub fn hidden_size(&self) -> usize {
        self.hidden
    }
    pub fn retained_bytes(&self) -> u64 {
        self.hidden as u64 * 32
    }

    pub fn forward(
        &self,
        residual: &Tensor,
        projected_kv: &Tensor,
        image_mask: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        same_device(&self.device, residual)?;
        let (tokens, streams, hidden) = residual.dims3().map_err(tensor_error)?;
        if streams != 4 || hidden != self.hidden {
            return Err(invalid(
                "XQ Engram residual differs from the bound streams/hidden",
            ));
        }
        self.live_tensor_bytes(tokens)?;
        let output = gate_math(
            residual,
            projected_kv,
            &self.query_weight,
            &self.key_weight,
            image_mask,
        )?;
        check_cancel(cancellation)?;
        Ok(output)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        if tokens == 0 || tokens > self.device.limits().max_context_tokens {
            return Err(invalid(
                "XQ Engram tokens exceed the nonempty context budget",
            ));
        }
        self.device
            .limits()
            .checked_elements(&[tokens, 5, self.hidden], "XQ Engram gate rows")?;
        let bytes = (tokens as u64)
            .checked_mul(self.hidden as u64)
            .and_then(|n| n.checked_mul(512))
            .and_then(|n| n.checked_add(self.retained_bytes() * 16 + 4096))
            .ok_or_else(|| invalid("XQ Engram gate live bytes overflowed"))?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "XQ Engram F32 gate live tensors")
    }
}

pub struct XqEngramProjection {
    device: CudaLayerDevice,
    layer: EngramLayer,
    projection: XqDeviceMatrix,
    gate: XqEngramGate,
    retained_bytes: u64,
}

impl XqEngramProjection {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: EngramLayer,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        let prefix = format!("blk.{}", layer.as_str());
        let mut total = 0_u64;
        for (suffix, kind, shape) in [
            ("engram_kv.weight", XqTensorType::F16, vec![6144, 25600]),
            ("engram_q_norm.weight", XqTensorType::F32, vec![5120, 4]),
            ("engram_k_norm.weight", XqTensorType::F32, vec![5120, 4]),
        ] {
            let tensor = weights.tensor(&format!("{prefix}.{suffix}"))?;
            if tensor.kind != kind || tensor.shape != shape {
                return Err(invalid(
                    "XQ Engram parameter differs from the exact exported schema",
                ));
            }
            total = total
                .checked_add(tensor.bytes)
                .ok_or_else(|| invalid("XQ Engram parameter bytes overflowed"))?;
        }
        if total > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "XQ Engram aggregate parameters exceed resident budget",
            ));
        }
        let projection = XqDeviceMatrix::load(
            device,
            weights,
            &format!("{prefix}.engram_kv.weight"),
            0,
            25600,
            cancellation,
        )?;
        let query = weights.load_f32_parameter(
            device,
            &format!("{prefix}.engram_q_norm.weight"),
            XqTensorType::F32,
            &[5120, 4],
            cancellation,
        )?;
        let key = weights.load_f32_parameter(
            device,
            &format!("{prefix}.engram_k_norm.weight"),
            XqTensorType::F32,
            &[5120, 4],
            cancellation,
        )?;
        let gate = XqEngramGate::new(device.clone(), query, key)?;
        Self::new(device.clone(), layer, projection, gate)
    }

    pub fn new(
        device: CudaLayerDevice,
        layer: EngramLayer,
        projection: XqDeviceMatrix,
        gate: XqEngramGate,
    ) -> Result<Self> {
        if projection.kind() != XqTensorType::F16
            || projection.logical_shape() != [5 * gate.hidden_size(), PROJECTED_INPUT_WIDTH]
        {
            return Err(invalid(
                "XQ Engram projection requires original F16 [5*hidden,6144] rows",
            ));
        }
        for owner in [projection.device(), &gate.device] {
            if !owner.tensor_device().same_device(device.tensor_device())
                || owner.limits() != device.limits()
            {
                return Err(invalid("XQ Engram projection/gate device or limits differ"));
            }
        }
        let retained_bytes = projection
            .retained_weight_bytes()
            .checked_add(gate.retained_bytes())
            .ok_or_else(|| invalid("XQ Engram retained parameter bytes overflowed"))?;
        if retained_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "XQ Engram aggregate parameters exceed resident budget",
            ));
        }
        Ok(Self {
            device,
            layer,
            projection,
            gate,
            retained_bytes,
        })
    }

    pub fn layer(&self) -> EngramLayer {
        self.layer
    }
    pub fn hidden_size(&self) -> usize {
        self.gate.hidden_size()
    }
    pub fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub fn forward(
        &self,
        continuation: XqMhcContinuation,
        embeddings: &EngramEmbeddings,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        check_cancel(cancellation)?;
        let (tokens, streams, hidden) = continuation.residual().dims3().map_err(tensor_error)?;
        same_device(&self.device, continuation.residual())?;
        if streams != 4
            || hidden != self.hidden_size()
            || continuation.residual().dtype() != DType::F32
            || embeddings.activations().data_bytes().dims() != [tokens, PROJECTED_INPUT_WIDTH]
            || !embeddings
                .device()
                .tensor_device()
                .same_device(self.device.tensor_device())
            || embeddings.device().limits() != self.device.limits()
        {
            return Err(invalid(
                "XQ Engram trunk/lookup differs from its geometry/device/limits",
            ));
        }
        self.live_tensor_bytes(tokens, embeddings.live_tensor_bytes())?;
        let decoded = embeddings.activations().decode()?;
        check_cancel(cancellation)?;
        let projected = self
            .projection
            .forward_f32_control(&decoded, cancellation)?
            .0
            .reshape((tokens, 5, hidden))
            .map_err(tensor_error)?;
        let residual = self.gate.forward(
            continuation.residual(),
            &projected,
            embeddings.image_mask(),
            cancellation,
        )?;
        let result = continuation.with_residual(&residual)?;
        check_cancel(cancellation)?;
        Ok(result)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize, lookup_bytes: u64) -> Result<u64> {
        let gate_bytes = self.gate.live_tensor_bytes(tokens)?;
        let bytes = (tokens as u64)
            .checked_mul(PROJECTED_INPUT_WIDTH as u64 * 128)
            .and_then(|n| n.checked_add(gate_bytes))
            .and_then(|n| n.checked_add(lookup_bytes))
            .and_then(|n| n.checked_add(self.retained_bytes))
            .ok_or_else(|| {
                invalid("XQ Engram projection/lookup live bytes overflowed or refused")
            })?;
        self.device
            .limits()
            .checked_state_bytes(bytes, "XQ Engram projection/gate/lookup live tensors")
    }
}

fn gate_math(
    residual: &Tensor,
    projected: &Tensor,
    query_weight: &Tensor,
    key_weight: &Tensor,
    mask: &Tensor,
) -> Result<Tensor> {
    let (tokens, streams, hidden) = residual.dims3().map_err(tensor_error)?;
    if tokens == 0
        || streams != 4
        || hidden == 0
        || hidden > 5120
        || residual.dtype() != DType::F32
        || projected.dtype() != DType::F32
        || projected.dims() != [tokens, 5, hidden]
        || !projected.device().same_device(residual.device())
    {
        return Err(invalid(
            "XQ Engram gate requires F32 residual [T,4,H] and KV [T,5,H]",
        ));
    }
    for weight in [query_weight, key_weight] {
        if weight.dtype() != DType::F32
            || weight.dims() != [4, hidden]
            || !weight.device().same_device(residual.device())
        {
            return Err(invalid(
                "XQ Engram gate requires same-device F32 [4,H] norm weights",
            ));
        }
        validate_finite(weight, false, "XQ Engram gate norm weight")?;
    }
    if mask.dtype() != DType::U8
        || mask.dims() != [tokens]
        || !mask.device().same_device(residual.device())
    {
        return Err(invalid(
            "XQ Engram image mask requires same-device U8 [T] values",
        ));
    }
    if mask
        .max_all()
        .and_then(|t| t.to_scalar::<u8>())
        .map_err(tensor_error)?
        > 1
    {
        return Err(invalid("XQ Engram image mask values must be zero or one"));
    }
    validate_finite(projected, false, "XQ Engram fused KV")?;
    let key = projected
        .narrow(1, 0, 4)
        .and_then(|x| x.contiguous())
        .map_err(tensor_error)?;
    let hn = rms_math(residual, None)?;
    let kn = rms_math(&key, None)?;
    let wqk = query_weight.mul(key_weight).map_err(tensor_error)?;
    validate_finite(&wqk, false, "XQ Engram combined norm weights")?;
    let scale = 1_f32 / (hidden as f32).sqrt();
    let dot = hn
        .broadcast_mul(&wqk.unsqueeze(0).map_err(tensor_error)?)
        .and_then(|x| x.mul(&kn))
        .and_then(|x| x.sum_keepdim(2))
        .and_then(|x| x.affine(f64::from(scale), 0.))
        .map_err(tensor_error)?;
    validate_finite(&dot, false, "XQ Engram normalized gate dot")?;
    let root = dot
        .abs()
        .and_then(|x| x.maximum(1e-6_f32))
        .and_then(|x| x.sqrt())
        .and_then(|x| x.mul(&dot.sign()?))
        .map_err(tensor_error)?;
    let gate = candle_nn::ops::sigmoid(&root).map_err(tensor_error)?;
    let value = projected.narrow(1, 4, 1).map_err(tensor_error)?;
    let updated = gate
        .broadcast_mul(&value)
        .and_then(|x| x.add(residual))
        .map_err(tensor_error)?;
    let output = mask
        .reshape((tokens, 1, 1))
        .and_then(|x| x.broadcast_as(residual.shape()))
        .and_then(|x| x.where_cond(residual, &updated))
        .map_err(tensor_error)?
        .detach();
    validate_finite(&output, false, "XQ Engram F32 residual output")?;
    Ok(output)
}

#[cfg(test)]
pub(crate) mod tests;
