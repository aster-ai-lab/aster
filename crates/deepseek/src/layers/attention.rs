//! Unfused sparse MQA control math over explicitly decoded logical values.
//!
//! The model must supply its actual norm/RoPE/quantize-dequantize results. This
//! module does not substitute unquantized BF16 for a cache format, choose an
//! indexer's top-k, or implement a packed cache. Public execution requires CUDA.
//! Sink logits participate in the softmax denominator with a zero value vector.
//! This agrees with the MIT FlashMLA reference `tests/ref.py` at revision
//! 2e5429fc5653bab6e081f09477126f731882a6a9 (Copyright 2025 DeepSeek).

use std::collections::BTreeSet;

use aster_power::error::Result;
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::kv::{MainKvFormat, MainKvRows, MainKvRowsData, MAIN_KV_DIM};
use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

pub mod branch;
pub mod output;
pub mod projection;
pub mod sublayer;

#[cfg(test)]
mod test_support;

const MAX_WINDOW: usize = 128;
const MAX_HEADS: usize = 64;
const MAX_HEAD_DIM: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvSource {
    Window,
    Compressed,
}

impl KvSource {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "swa" => Ok(Self::Window),
            "compressed" => Ok(Self::Compressed),
            _ => Err(invalid(
                "unsupported attention KV source; supported set: swa, compressed",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Window => "swa",
            Self::Compressed => "compressed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KvCandidate {
    source: KvSource,
    ready_position: usize,
}

impl KvCandidate {
    pub fn new(source: KvSource, ready_position: usize) -> Self {
        Self {
            source,
            ready_position,
        }
    }
}

/// One request's explicit candidate rows. Different source rows may share a
/// position; a physical row may occur only once in a query's selection.
pub struct SparseAttentionPlan {
    selections: Vec<Vec<u32>>,
    query_positions: Vec<usize>,
    kv_rows: usize,
    max_selected: usize,
    packed_window_rows: Option<usize>,
}

impl SparseAttentionPlan {
    pub fn new(
        limits: &InferenceLimits,
        window_tokens: usize,
        candidates: &[KvCandidate],
        query_positions: &[usize],
        selections: &[Vec<usize>],
    ) -> Result<Self> {
        limits.validate()?;
        if window_tokens == 0
            || window_tokens > MAX_WINDOW
            || limits.max_context_tokens > MAX_CONTEXT_TOKENS
            || query_positions.is_empty()
            || query_positions.len() > limits.max_context_tokens
            || query_positions.len() != selections.len()
            || candidates.len() > limits.max_context_tokens * 2 + window_tokens
            || query_positions
                .iter()
                .any(|position| *position >= limits.max_context_tokens)
            || query_positions.windows(2).any(|pair| pair[0] >= pair[1])
            || candidates
                .iter()
                .any(|candidate| candidate.ready_position >= limits.max_context_tokens)
        {
            return Err(invalid(
                "attention plan exceeds its window, context, row or position bounds",
            ));
        }
        let max_selected = selections.iter().map(Vec::len).max().unwrap_or(0);
        let entries = selections
            .iter()
            .try_fold(0_usize, |sum, row| sum.checked_add(row.len()))
            .ok_or_else(|| invalid("attention selection count overflowed"))?;
        // Include caller metadata, owned u32 indices and per-row duplicate
        // validation scratch. This is a bound, not a model-wide reservation.
        let plan_bytes = entries
            .checked_mul(12)
            .and_then(|bytes| {
                query_positions
                    .len()
                    .checked_mul(56)
                    .and_then(|rows| bytes.checked_add(rows))
            })
            .and_then(|bytes| {
                candidates
                    .len()
                    .checked_mul(16)
                    .and_then(|rows| bytes.checked_add(rows))
            })
            .and_then(|bytes| {
                max_selected
                    .checked_mul(64)
                    .and_then(|scratch| bytes.checked_add(scratch))
            })
            .and_then(|bytes| bytes.checked_add(256))
            .ok_or_else(|| invalid("attention plan byte count overflowed"))?;
        if plan_bytes > limits.max_graph_plan_bytes {
            return Err(invalid("attention plan exceeds its metadata byte budget"));
        }
        for (position, selection) in query_positions.iter().zip(selections) {
            let mut seen = BTreeSet::new();
            for row in selection {
                let candidate = candidates
                    .get(*row)
                    .ok_or_else(|| invalid("attention index exceeds its candidate rows"))?;
                if !seen.insert(*row)
                    || candidate.ready_position > *position
                    || (candidate.source == KvSource::Window
                        && position - candidate.ready_position >= window_tokens)
                {
                    return Err(invalid("attention selection duplicates a row or exposes a future/expired window row"));
                }
            }
        }
        let selections = selections
            .iter()
            .map(|row| {
                row.iter()
                    .map(|index| {
                        u32::try_from(*index)
                            .map_err(|_| invalid("attention row index exceeds u32"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .collect::<Result<_>>()?;
        let window_rows = candidates
            .iter()
            .take_while(|candidate| candidate.source == KvSource::Window)
            .count();
        let packed_window_rows = candidates[window_rows..]
            .iter()
            .all(|candidate| candidate.source == KvSource::Compressed)
            .then_some(window_rows);
        Ok(Self {
            selections,
            query_positions: query_positions.to_vec(),
            kv_rows: candidates.len(),
            max_selected,
            packed_window_rows,
        })
    }

    pub fn query_positions(&self) -> &[usize] {
        &self.query_positions
    }

    pub(crate) fn live_tensor_bytes(&self, heads: usize, dim: usize) -> Result<u64> {
        validate_geometry(heads, dim)?;
        attention_peak_bytes(
            self.query_positions.len(),
            heads,
            dim,
            self.kv_rows,
            self.max_selected,
        )
    }
}

pub struct SparseMqaAttention {
    device: CudaLayerDevice,
    heads: usize,
    head_dim: usize,
    sink_logits: Tensor,
}

impl SparseMqaAttention {
    /// HF attention sinks are effective FP32 runtime logits. They are preserved
    /// without multiplying by the QK softmax scale.
    pub fn new(
        device: CudaLayerDevice,
        heads: usize,
        head_dim: usize,
        sink_logits: Tensor,
    ) -> Result<Self> {
        validate_geometry(heads, head_dim)?;
        if !sink_logits.device().same_device(device.tensor_device())
            || sink_logits.dtype() != DType::F32
            || sink_logits.dims() != [heads]
        {
            return Err(invalid(
                "attention sinks require same-device FP32 [heads] runtime logits",
            ));
        }
        let sink_logits = sink_logits
            .force_contiguous()
            .map_err(tensor_error)?
            .detach();
        validate_finite(&sink_logits, false, "attention sink")?;
        Ok(Self {
            device,
            heads,
            head_dim,
            sink_logits,
        })
    }

    /// Correctness control with one ragged query at a time, not a fused/batched
    /// performance kernel. KV supplies the same vector as both K and V (MQA).
    pub fn forward_decoded(
        &self,
        query: &Tensor,
        decoded_kv: &Tensor,
        plan: &SparseAttentionPlan,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        if !query.device().same_device(self.device.tensor_device())
            || query.dims().get(1..) != Some(&[self.heads, self.head_dim][..])
        {
            return Err(invalid(
                "attention query differs from its bound CUDA geometry",
            ));
        }
        attend_math(
            query,
            decoded_kv,
            &self.sink_logits,
            plan,
            self.device.limits(),
            cancellation,
        )
    }

    /// Main/window records keep their distinct physical formats. Selection
    /// metadata already passed the causal/window validator; only selected raw
    /// rows are gathered and decoded, preserving the original selection order.
    pub fn forward_packed(
        &self,
        query: &Tensor,
        window: &MainKvRows,
        compressed: Option<&MainKvRows>,
        plan: &SparseAttentionPlan,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        if !query.device().same_device(self.device.tensor_device())
            || query.dims().get(1..) != Some(&[self.heads, self.head_dim][..])
        {
            return Err(invalid(
                "packed attention query differs from its bound CUDA geometry/device",
            ));
        }
        let keys = PackedAttentionRows {
            window: window.rows_data(),
            compressed: compressed.map(MainKvRows::rows_data),
        };
        attend_math(
            query,
            &keys,
            &self.sink_logits,
            plan,
            self.device.limits(),
            cancellation,
        )
    }

    /// Explicit XQ correctness profile: F32 query/output, the original packed
    /// activation grid, then F16 KV at the pinned ggml attention boundary.
    /// This is unfused F32 math, not a claim of ggml kernel equivalence or
    /// FlashAttention performance. Only the selected cache rows are decoded.
    pub fn forward_xq_f32_packed(
        &self,
        query: &Tensor,
        window: &MainKvRows,
        compressed: Option<&MainKvRows>,
        plan: &SparseAttentionPlan,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        if !query.device().same_device(self.device.tensor_device())
            || query.dims().get(1..) != Some(&[self.heads, self.head_dim][..])
        {
            return Err(invalid(
                "XQ attention query differs from its bound device/geometry",
            ));
        }
        let keys = XqPackedAttentionRows(PackedAttentionRows {
            window: window.rows_data(),
            compressed: compressed.map(MainKvRows::rows_data),
        });
        attend_math_typed(
            query,
            &keys,
            &self.sink_logits,
            plan,
            self.device.limits(),
            DType::F32,
            cancellation,
        )
    }
}

trait AttentionKeys {
    fn dimensions(&self) -> Result<(usize, usize)>;
    fn supported_dtype(&self) -> bool;
    fn device(&self) -> &candle_core::Device;
    fn validate_plan(&self, _plan: &SparseAttentionPlan) -> Result<()> {
        Ok(())
    }
    fn gather(&self, selection: &[u32], limits: &InferenceLimits) -> Result<Tensor>;
}

impl AttentionKeys for Tensor {
    fn dimensions(&self) -> Result<(usize, usize)> {
        self.dims2().map_err(tensor_error)
    }
    fn supported_dtype(&self) -> bool {
        self.dtype() == DType::BF16
    }
    fn device(&self) -> &candle_core::Device {
        Tensor::device(self)
    }
    fn gather(&self, selection: &[u32], _limits: &InferenceLimits) -> Result<Tensor> {
        let indices =
            Tensor::from_slice(selection, selection.len(), self.device()).map_err(tensor_error)?;
        self.index_select(&indices, 0)
            .and_then(|value| value.detach().to_dtype(DType::F32))
            .map_err(tensor_error)
    }
}

struct PackedAttentionRows<'a> {
    window: &'a MainKvRowsData,
    compressed: Option<&'a MainKvRowsData>,
}

struct XqPackedAttentionRows<'a>(PackedAttentionRows<'a>);

impl AttentionKeys for XqPackedAttentionRows<'_> {
    fn dimensions(&self) -> Result<(usize, usize)> {
        self.0.dimensions()
    }
    fn supported_dtype(&self) -> bool {
        true
    }
    fn device(&self) -> &candle_core::Device {
        self.0.device()
    }
    fn validate_plan(&self, plan: &SparseAttentionPlan) -> Result<()> {
        self.0.validate_plan(plan)?;
        if self
            .0
            .compressed
            .is_some_and(|data| data.format() != MainKvFormat::Fp4E4m3)
        {
            return Err(invalid(
                "XQ compressed attention rows require FP4 E4M3 scales",
            ));
        }
        Ok(())
    }
    fn gather(&self, selection: &[u32], limits: &InferenceLimits) -> Result<Tensor> {
        // Existing packed grids decode exactly into BF16 before F32; XQ then
        // explicitly rounds K and V to F16, as ds41_dense.cpp's k4 does.
        self.0
            .gather(selection, limits)?
            .to_dtype(DType::F16)
            .and_then(|x| x.to_dtype(DType::F32))
            .map_err(tensor_error)
    }
}

impl AttentionKeys for PackedAttentionRows<'_> {
    fn dimensions(&self) -> Result<(usize, usize)> {
        Ok((
            self.window.row_count() + self.compressed.map_or(0, MainKvRowsData::row_count),
            MAIN_KV_DIM,
        ))
    }
    fn supported_dtype(&self) -> bool {
        true
    }
    fn device(&self) -> &candle_core::Device {
        self.window.device()
    }
    fn validate_plan(&self, plan: &SparseAttentionPlan) -> Result<()> {
        if self.window.format() != MainKvFormat::Fp8Ue8m0
            || plan.packed_window_rows != Some(self.window.row_count())
            || self
                .compressed
                .is_some_and(|data| !data.device().same_device(self.window.device()))
        {
            return Err(invalid("packed attention requires FP8 window rows followed by bound compressed rows on the same device"));
        }
        Ok(())
    }
    fn gather(&self, selection: &[u32], limits: &InferenceLimits) -> Result<Tensor> {
        let window_count = self.window.row_count() as u32;
        let mut window = Vec::new();
        let mut compressed = Vec::new();
        let mut order = Vec::with_capacity(selection.len());
        for index in selection {
            if *index < window_count {
                order.push((false, window.len()));
                window.push(*index);
            } else {
                order.push((true, compressed.len()));
                compressed.push(*index - window_count);
            }
        }
        let device = self.device();
        let mut parts = Vec::with_capacity(2);
        if !window.is_empty() {
            parts.push(decode_packed_selection(self.window, &window, limits)?);
        }
        if !compressed.is_empty() {
            let source = self
                .compressed
                .ok_or_else(|| invalid("compressed attention rows have no bound cache"))?;
            parts.push(decode_packed_selection(source, &compressed, limits)?);
        }
        let joined = Tensor::cat(&parts, 0).map_err(tensor_error)?;
        let order: Vec<u32> = order
            .into_iter()
            .map(|(compressed, offset)| (offset + if compressed { window.len() } else { 0 }) as u32)
            .collect();
        let order =
            Tensor::from_slice(order.as_slice(), order.len(), device).map_err(tensor_error)?;
        joined
            .index_select(&order, 0)
            .and_then(|value| value.to_dtype(DType::F32))
            .map_err(tensor_error)
    }
}

fn decode_packed_selection(
    rows: &MainKvRowsData,
    selection: &[u32],
    limits: &InferenceLimits,
) -> Result<Tensor> {
    if selection.iter().copied().eq(0..selection.len() as u32) {
        return rows.decode_prefix(selection.len(), limits);
    }
    let indices =
        Tensor::from_slice(selection, selection.len(), rows.device()).map_err(tensor_error)?;
    rows.decode_selected(&indices, limits)
}

fn validate_geometry(heads: usize, head_dim: usize) -> Result<()> {
    if heads == 0 || heads > MAX_HEADS || head_dim == 0 || head_dim > MAX_HEAD_DIM {
        return Err(invalid(
            "MQA geometry requires 1..=64 heads and 1..=512 channels",
        ));
    }
    Ok(())
}

// Private CPU entry for independent small tests; serving constructors require
// CudaLayerDevice. This consumes already selected, causally validated row IDs.
fn attend_math<K: AttentionKeys>(
    query: &Tensor,
    kv: &K,
    sinks: &Tensor,
    plan: &SparseAttentionPlan,
    limits: &InferenceLimits,
    cancellation: &CancellationToken,
) -> Result<Tensor> {
    attend_math_typed(query, kv, sinks, plan, limits, DType::BF16, cancellation)
}

fn attend_math_typed<K: AttentionKeys>(
    query: &Tensor,
    kv: &K,
    sinks: &Tensor,
    plan: &SparseAttentionPlan,
    limits: &InferenceLimits,
    dtype: DType,
    cancellation: &CancellationToken,
) -> Result<Tensor> {
    check_cancel(cancellation)?;
    let (tokens, heads, head_dim) = query.dims3().map_err(tensor_error)?;
    let (keys, kv_dim) = kv.dimensions()?;
    validate_geometry(heads, head_dim)?;
    if !matches!(dtype, DType::BF16 | DType::F32)
        || query.dtype() != dtype
        || !kv.supported_dtype()
        || kv_dim != head_dim
        || !kv.device().same_device(query.device())
        || sinks.dtype() != DType::F32
        || sinks.dims() != [heads]
        || !sinks.device().same_device(query.device())
        || tokens != plan.selections.len()
        || keys != plan.kv_rows
    {
        return Err(invalid(
            "MQA requires its declared BF16/F32 query precision, typed KV, FP32 sinks and an exact candidate plan",
        ));
    }
    kv.validate_plan(plan)?;
    limits.checked_elements(query.dims(), "attention query")?;
    if keys > 0 {
        limits.checked_elements(&[keys, kv_dim], "attention KV")?;
    }
    limits.checked_state_bytes(
        attention_peak_bytes(tokens, heads, head_dim, keys, plan.max_selected)?,
        "unfused attention peak",
    )?;
    validate_finite(sinks, false, "attention sink")?;
    let mut output = Vec::with_capacity(tokens);
    for (row, selection) in plan.selections.iter().enumerate() {
        check_cancel(cancellation)?;
        let q = query
            .narrow(0, row, 1)
            .and_then(|query| query.squeeze(0))
            .and_then(|query| query.detach().to_dtype(DType::F32))
            // F32 RoPE can leave token-interleaved storage. A same-dtype
            // conversion aliases that layout; CUDA GEMM needs a packed row.
            .and_then(|query| query.contiguous())
            .map_err(tensor_error)?;
        validate_finite(&q, false, "attention query")?;
        if selection.is_empty() {
            output.push(
                Tensor::zeros((heads, head_dim), dtype, query.device()).map_err(tensor_error)?,
            );
            continue;
        }
        // Never convert or inspect unselected KV rows. Cached logical values
        // are only gathered for the selected causal/window positions.
        let selected = kv.gather(selection, limits)?;
        validate_finite(&selected, false, "selected attention KV")?;
        check_f32_matmul_precision(dtype, query.device())?;
        let logits = q
            .matmul(&selected.t().map_err(tensor_error)?)
            .and_then(|logits| logits.affine((head_dim as f64).powf(-0.5), 0.0))
            .map_err(tensor_error)?;
        validate_finite(&logits, false, "attention QK logits")?;
        check_f32_matmul_precision(dtype, query.device())?;
        let compute = || -> candle_core::Result<Tensor> {
            let logits = Tensor::cat(&[&logits, &sinks.unsqueeze(1)?], 1)?;
            let probabilities = candle_nn::ops::softmax_last_dim(&logits)?;
            probabilities
                .narrow(1, 0, selection.len())?
                .contiguous()?
                .matmul(&selected)?
                .to_dtype(dtype)
        };
        let result = compute().map_err(tensor_error)?.detach();
        validate_finite(&result, false, "attention output")?;
        output.push(result);
    }
    check_cancel(cancellation)?;
    Tensor::stack(&output, 0).map_err(tensor_error)
}

pub(crate) fn check_f32_matmul_precision(dtype: DType, device: &candle_core::Device) -> Result<()> {
    #[cfg(feature = "cuda")]
    if dtype == DType::F32
        && device.is_cuda()
        && candle_core::cuda_backend::gemm_reduced_precision_f32()
    {
        return Err(invalid(
            "XQ F32 control refuses Candle's global reduced-precision/TF32 GEMM mode",
        ));
    }
    let _ = (dtype, device);
    Ok(())
}

fn attention_peak_bytes(
    tokens: usize,
    heads: usize,
    dim: usize,
    keys: usize,
    selected: usize,
) -> Result<u64> {
    let elements = |values: &[usize]| -> Result<u64> {
        values
            .iter()
            .try_fold(1_u64, |count, value| count.checked_mul(*value as u64))
            .ok_or_else(|| invalid("attention byte arithmetic overflowed"))
    };
    // Static live-tensor estimate, not measured CUDA allocator/BLAS workspace
    // usage. The model-wide Power permit must separately reserve those backend
    // overheads before this control path is promoted for serving.
    // Include caller query/KV, retained row outputs plus the final stack, FP32
    // selected rows/query and conservative softmax/matmul working tensors.
    let query = elements(&[tokens, heads, dim, 6])?;
    let cache = elements(&[keys, dim, 2])?;
    let gathered = elements(&[selected, dim, 80])?;
    let query_work = elements(&[heads, dim, 12])?;
    let logits = elements(&[
        heads,
        selected
            .checked_add(1)
            .ok_or_else(|| invalid("attention selection width overflowed"))?,
        32,
    ])?;
    [query, cache, gathered, query_work, logits]
        .into_iter()
        .try_fold(0_u64, |sum, value| sum.checked_add(value))
        .ok_or_else(|| invalid("attention peak byte sum overflowed"))
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(aster_power::error::PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
