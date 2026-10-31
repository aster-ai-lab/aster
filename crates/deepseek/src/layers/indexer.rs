//! Unfused sparse-index control over explicitly decoded quantized logical Q/K.
//!
//! Q/K must already reflect the actual FP4+UE8M0 norm/RoPE/cast path. Ordinary
//! unquantized BF16 inputs are not a substitute. Effective BF16 head weights
//! are used as supplied, without an inferred softmax/head-count scale. Scores
//! are sum_h(weight_h * relu(dot(q_h, k))), rounded to BF16 before selection.
//! This is not an SM90 packed-input kernel or a cache implementation.
//!
//! Candidate/source semantics follow Apache-2.0 DeepInfer revision
//! 9bd9cf243fc53ef9afbf8f04d391d989117d6ab9. DeepSelect does not promise a
//! particular tied-index subset; this control chooses the lowest row on ties
//! and sorts the selected IDs. GPU/model qualification remains mandatory.

use aster_power::error::{PowerError, Result};
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::quant::{Fp4IndexRows, Fp4IndexRowsData, INDEX_HEAD_DIM};
use super::{invalid, tensor_error, validate_finite, CudaLayerDevice};
use crate::model::MAX_CONTEXT_TOKENS;

pub mod projection;

/// U32 device padding; native attention must preserve it as an invalid slot.
pub const INVALID_INDEX_ROW: u32 = u32::MAX;

pub struct SparseIndexPlan {
    keys: usize,
    prefix_lengths: Vec<usize>,
    restrictions: Option<Vec<Vec<u32>>>,
    max_candidates: usize,
}

impl SparseIndexPlan {
    /// One request's ordered compressed rows and ordered query positions.
    /// A restricted search supplies increasing physical row IDs per query;
    /// the plan refuses future rows before any tensor data is inspected.
    pub fn new(
        limits: &InferenceLimits,
        ready_positions: &[usize],
        query_positions: &[usize],
        restrictions: Option<&[Vec<usize>]>,
    ) -> Result<Self> {
        limits.validate()?;
        if limits.max_context_tokens > MAX_CONTEXT_TOKENS
            || ready_positions.len() > limits.max_context_tokens
            || query_positions.is_empty()
            || query_positions.len() > limits.max_context_tokens
            || ready_positions
                .iter()
                .chain(query_positions)
                .any(|p| *p >= limits.max_context_tokens)
            || ready_positions.windows(2).any(|pair| pair[0] >= pair[1])
            || query_positions.windows(2).any(|pair| pair[0] >= pair[1])
            || restrictions.is_some_and(|rows| rows.len() != query_positions.len())
        {
            return Err(invalid(
                "sparse index plan exceeds context/order/query-row bounds",
            ));
        }
        let entries = restrictions
            .map_or(Some(0), |rows| {
                rows.iter()
                    .try_fold(0_usize, |sum, row| sum.checked_add(row.len()))
            })
            .ok_or_else(|| invalid("sparse index metadata count overflowed"))?;
        let bytes = query_positions
            .len()
            .checked_mul(40)
            .and_then(|bytes| {
                entries
                    .checked_mul(12)
                    .and_then(|entries| bytes.checked_add(entries))
            })
            .and_then(|bytes| {
                ready_positions
                    .len()
                    .checked_mul(8)
                    .and_then(|keys| bytes.checked_add(keys))
            })
            .ok_or_else(|| invalid("sparse index metadata bytes overflowed"))?;
        if bytes > limits.max_graph_plan_bytes {
            return Err(invalid("sparse index plan exceeds its metadata budget"));
        }
        let prefix_lengths: Vec<_> = query_positions
            .iter()
            .map(|position| ready_positions.partition_point(|ready| ready <= position))
            .collect();
        if let Some(rows) = restrictions {
            for (row, prefix) in rows.iter().zip(&prefix_lengths) {
                if row.iter().any(|index| index >= prefix)
                    || row.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return Err(invalid("sparse index restrictions contain future, duplicate, unordered or out-of-range rows"));
                }
            }
        }
        let max_candidates = restrictions.map_or_else(
            || prefix_lengths.iter().copied().max().unwrap_or(0),
            |rows| rows.iter().map(Vec::len).max().unwrap_or(0),
        );
        let restrictions = restrictions
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        row.iter()
                            .map(|index| {
                                u32::try_from(*index).map_err(|_| invalid("index row exceeds u32"))
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        Ok(Self {
            keys: ready_positions.len(),
            prefix_lengths,
            restrictions,
            max_candidates,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateBlockWidth {
    Eight,
    Sixteen,
}

impl CandidateBlockWidth {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "8" => Ok(Self::Eight),
            "16" => Ok(Self::Sixteen),
            _ => Err(invalid(
                "unsupported candidate block width; supported set: 8, 16",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eight => "8",
            Self::Sixteen => "16",
        }
    }
    pub fn tokens(self) -> usize {
        match self {
            Self::Eight => 8,
            Self::Sixteen => 16,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CandidateBlockPolicy {
    width: CandidateBlockWidth,
    topk: usize,
}

impl CandidateBlockPolicy {
    pub fn new(width: CandidateBlockWidth, topk: usize) -> Result<Self> {
        if topk == 0 || topk > 4096 {
            return Err(invalid("candidate block top-k requires 1..=4096 blocks"));
        }
        Ok(Self { width, topk })
    }
}

pub struct SparseIndexSelection {
    rows: Tensor,
    valid_lengths: Vec<usize>,
    candidate_blocks: Option<Tensor>,
}

impl SparseIndexSelection {
    pub fn rows(&self) -> &Tensor {
        &self.rows
    }
    pub fn valid_lengths(&self) -> &[usize] {
        &self.valid_lengths
    }
    pub fn candidate_blocks(&self) -> Option<&Tensor> {
        self.candidate_blocks.as_ref()
    }
}

pub struct SparseIndexer {
    device: CudaLayerDevice,
    heads: usize,
    dim: usize,
    topk: usize,
    block_policy: Option<CandidateBlockPolicy>,
}

impl SparseIndexer {
    pub fn new(
        device: CudaLayerDevice,
        heads: usize,
        dim: usize,
        topk: usize,
        block_policy: Option<CandidateBlockPolicy>,
    ) -> Result<Self> {
        validate_geometry(heads, dim, topk)?;
        Ok(Self {
            device,
            heads,
            dim,
            topk,
            block_policy,
        })
    }

    pub fn select(
        &self,
        decoded_query: &Tensor,
        decoded_keys: &Tensor,
        effective_head_weights: &Tensor,
        plan: &SparseIndexPlan,
        cancellation: &CancellationToken,
    ) -> Result<SparseIndexSelection> {
        if !decoded_query
            .device()
            .same_device(self.device.tensor_device())
            || decoded_query.dims().get(1..) != Some(&[self.heads, self.dim][..])
        {
            return Err(invalid(
                "index query differs from its bound CUDA geometry/device",
            ));
        }
        select_math(
            decoded_query,
            decoded_keys,
            effective_head_weights,
            plan,
            self.topk,
            self.block_policy,
            self.device.limits(),
            cancellation,
        )
    }

    /// Packed Q rows are flattened [tokens*heads,128]. Key rows are private to
    /// this request; only the plan's eligible/restricted rows are decoded.
    pub fn select_packed(
        &self,
        query: &Fp4IndexRows,
        keys: &Fp4IndexRows,
        effective_head_weights: &Tensor,
        plan: &SparseIndexPlan,
        cancellation: &CancellationToken,
    ) -> Result<SparseIndexSelection> {
        self.select_packed_typed(
            query,
            keys,
            effective_head_weights,
            plan,
            DType::BF16,
            cancellation,
        )
    }

    /// XQ supplies already scaled F32 head weights and keeps F32 scores for
    /// top-k. Packed Q/K still represent the exact FP4/pow2 grid; no BF16
    /// activation or score rounding is inferred from the original API.
    pub fn select_xq_f32_packed(
        &self,
        query: &Fp4IndexRows,
        keys: &Fp4IndexRows,
        scaled_head_weights: &Tensor,
        plan: &SparseIndexPlan,
        cancellation: &CancellationToken,
    ) -> Result<SparseIndexSelection> {
        if self
            .block_policy
            .is_some_and(|policy| policy.width != CandidateBlockWidth::Eight)
        {
            return Err(invalid(
                "XQ candidate source requires the pinned block width 8",
            ));
        }
        self.select_packed_typed(
            query,
            keys,
            scaled_head_weights,
            plan,
            DType::F32,
            cancellation,
        )
    }

    fn select_packed_typed(
        &self,
        query: &Fp4IndexRows,
        keys: &Fp4IndexRows,
        effective_head_weights: &Tensor,
        plan: &SparseIndexPlan,
        dtype: DType,
        cancellation: &CancellationToken,
    ) -> Result<SparseIndexSelection> {
        check_cancel(cancellation)?;
        let tokens = plan.prefix_lengths.len();
        if self.dim != INDEX_HEAD_DIM
            || tokens.checked_mul(self.heads) != Some(query.row_count())
            || keys.row_count() != plan.keys
            || !query
                .rows_data()
                .device()
                .same_device(self.device.tensor_device())
            || !keys
                .rows_data()
                .device()
                .same_device(self.device.tensor_device())
            || !matches!(dtype, DType::BF16 | DType::F32)
            || effective_head_weights.dtype() != dtype
            || effective_head_weights.dims() != [tokens, self.heads]
            || !effective_head_weights
                .device()
                .same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "packed index Q/K differ from the bound CUDA geometry/causal plan",
            ));
        }
        self.device.limits().checked_state_bytes(
            peak_bytes(
                tokens,
                self.heads,
                self.dim,
                plan.keys,
                plan.max_candidates,
                self.topk + self.block_policy.map_or(0, |policy| policy.topk),
            )?,
            "packed indexer live tensors",
        )?;
        let query = query
            .rows_data()
            .decode_prefix(query.row_count(), self.device.limits())?
            .reshape((tokens, self.heads, self.dim))
            .map_err(tensor_error)?;
        select_math_typed(
            &query,
            keys.rows_data(),
            effective_head_weights,
            plan,
            self.topk,
            self.block_policy,
            self.device.limits(),
            dtype,
            cancellation,
        )
    }
}

trait IndexKeys {
    fn dimensions(&self) -> Result<(usize, usize)>;
    fn device(&self) -> &candle_core::Device;
    fn supported_dtype(&self) -> bool;
    fn gather_logical(
        &self,
        indices: &Tensor,
        dense_prefix: Option<usize>,
        limits: &InferenceLimits,
    ) -> Result<Tensor>;
}

impl IndexKeys for Tensor {
    fn dimensions(&self) -> Result<(usize, usize)> {
        self.dims2().map_err(tensor_error)
    }
    fn device(&self) -> &candle_core::Device {
        Tensor::device(self)
    }
    fn supported_dtype(&self) -> bool {
        self.dtype() == DType::BF16
    }
    fn gather_logical(
        &self,
        indices: &Tensor,
        dense_prefix: Option<usize>,
        _limits: &InferenceLimits,
    ) -> Result<Tensor> {
        match dense_prefix {
            Some(count) => self.narrow(0, 0, count),
            None => self.index_select(indices, 0),
        }
        .and_then(|value| value.detach().to_dtype(DType::F32))
        .map_err(tensor_error)
    }
}

impl IndexKeys for Fp4IndexRowsData {
    fn dimensions(&self) -> Result<(usize, usize)> {
        Ok((self.row_count(), INDEX_HEAD_DIM))
    }
    fn device(&self) -> &candle_core::Device {
        Fp4IndexRowsData::device(self)
    }
    fn supported_dtype(&self) -> bool {
        true
    }
    fn gather_logical(
        &self,
        indices: &Tensor,
        dense_prefix: Option<usize>,
        limits: &InferenceLimits,
    ) -> Result<Tensor> {
        match dense_prefix {
            Some(count) => self.decode_prefix(count, limits),
            None => self.decode_selected(indices, limits),
        }?
        .to_dtype(DType::F32)
        .map_err(tensor_error)
    }
}

fn validate_geometry(heads: usize, dim: usize, topk: usize) -> Result<()> {
    if heads == 0 || heads > 32 || dim == 0 || dim > 128 || topk == 0 || topk > 512 {
        return Err(invalid(
            "indexer requires 1..=32 heads, 1..=128 channels and 1..=512 selected rows",
        ));
    }
    Ok(())
}

// Private CPU entry only for small independent tests. The public constructor
// requires actual CUDA. Only explicit selected/causal key rows are converted.
#[allow(clippy::too_many_arguments)]
fn select_math<K: IndexKeys>(
    query: &Tensor,
    keys: &K,
    weights: &Tensor,
    plan: &SparseIndexPlan,
    topk: usize,
    block_policy: Option<CandidateBlockPolicy>,
    limits: &InferenceLimits,
    cancellation: &CancellationToken,
) -> Result<SparseIndexSelection> {
    select_math_typed(
        query,
        keys,
        weights,
        plan,
        topk,
        block_policy,
        limits,
        DType::BF16,
        cancellation,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_math_typed<K: IndexKeys>(
    query: &Tensor,
    keys: &K,
    weights: &Tensor,
    plan: &SparseIndexPlan,
    topk: usize,
    block_policy: Option<CandidateBlockPolicy>,
    limits: &InferenceLimits,
    dtype: DType,
    cancellation: &CancellationToken,
) -> Result<SparseIndexSelection> {
    check_cancel(cancellation)?;
    let (tokens, heads, dim) = query.dims3().map_err(tensor_error)?;
    let (key_rows, key_dim) = keys.dimensions()?;
    validate_geometry(heads, dim, topk)?;
    if query.dtype() != DType::BF16
        || !keys.supported_dtype()
        || !matches!(dtype, DType::BF16 | DType::F32)
        || weights.dtype() != dtype
        || !keys.device().same_device(query.device())
        || !weights.device().same_device(query.device())
        || key_rows != plan.keys
        || dim != key_dim
        || weights.dims() != [tokens, heads]
        || tokens != plan.prefix_lengths.len()
        || (block_policy.is_some() && plan.restrictions.is_some())
    {
        return Err(invalid("indexer requires BF16 decoded Q/K, declared BF16/F32 weights and an exact plan; candidate source requires dense causal rows"));
    }
    limits.checked_elements(query.dims(), "index query")?;
    limits.checked_elements(weights.dims(), "index head weights")?;
    if key_rows > 0 {
        limits.checked_elements(&[key_rows, key_dim], "index keys")?;
    }
    let blocks = block_policy.map_or(0, |policy| policy.topk);
    let result_width = topk
        .checked_add(blocks)
        .ok_or_else(|| invalid("index output width overflowed"))?;
    limits.checked_elements(&[tokens, result_width], "index output")?;
    limits.checked_state_bytes(
        peak_bytes(
            tokens,
            heads,
            dim,
            key_rows,
            plan.max_candidates,
            result_width,
        )?,
        "indexer live tensors",
    )?;
    validate_finite(query, false, "index query")?;
    validate_finite(weights, false, "index head weights")?;
    let mut rows = Vec::with_capacity(tokens);
    let mut candidate_blocks = Vec::with_capacity(tokens);
    let mut valid_lengths = Vec::with_capacity(tokens);
    for row in 0..tokens {
        check_cancel(cancellation)?;
        let count = plan
            .restrictions
            .as_ref()
            .map_or(plan.prefix_lengths[row], |rows| rows[row].len());
        valid_lengths.push(count.min(topk));
        if count == 0 {
            rows.push(Tensor::full(INVALID_INDEX_ROW, topk, query.device()).map_err(tensor_error)?);
            if let Some(policy) = block_policy {
                candidate_blocks.push(
                    Tensor::full(INVALID_INDEX_ROW, policy.topk, query.device())
                        .map_err(tensor_error)?,
                );
            }
            continue;
        }
        let indices = match &plan.restrictions {
            Some(restrictions) => {
                Tensor::from_slice(restrictions[row].as_slice(), count, query.device())
            }
            None => Tensor::arange(0_u32, count as u32, query.device()),
        }
        .map_err(tensor_error)?;
        let k = keys.gather_logical(
            &indices,
            plan.restrictions.is_none().then_some(count),
            limits,
        )?;
        validate_finite(&k, false, "eligible index keys")?;
        let q = query
            .narrow(0, row, 1)
            .and_then(|value| value.squeeze(0))
            .and_then(|value| value.detach().to_dtype(DType::F32))
            // F32 projection/RoPE views are not packed by to_dtype(F32).
            .and_then(|value| value.contiguous())
            .map_err(tensor_error)?;
        let w = weights
            .narrow(0, row, 1)
            .and_then(|value| value.reshape((heads, 1)))
            .and_then(|value| value.detach().to_dtype(DType::F32))
            .map_err(tensor_error)?;
        super::attention::check_f32_matmul_precision(dtype, query.device())?;
        let dots = q
            .matmul(&k.t().map_err(tensor_error)?)
            .map_err(tensor_error)?;
        validate_finite(&dots, false, "index dot products")?;
        let scores = dots
            .relu()
            .and_then(|value| value.broadcast_mul(&w))
            .and_then(|value| value.sum(0))
            .map_err(tensor_error)?;
        validate_finite(&scores, false, "index head reduction")?;
        let scores = scores.to_dtype(dtype).map_err(tensor_error)?;
        validate_finite(&scores, false, "declared-precision index scores")?;
        rows.push(select_scores(&scores, &indices, topk, cancellation)?);
        if let Some(policy) = block_policy {
            candidate_blocks.push(if dtype == DType::BF16 {
                select_blocks(&scores, policy, cancellation)?
            } else {
                select_blocks_typed(&scores, policy, dtype, cancellation)?
            });
        }
    }
    let selection = SparseIndexSelection {
        rows: Tensor::stack(&rows, 0).map_err(tensor_error)?.detach(),
        valid_lengths,
        candidate_blocks: if block_policy.is_some() {
            Some(
                Tensor::stack(&candidate_blocks, 0)
                    .map_err(tensor_error)?
                    .detach(),
            )
        } else {
            None
        },
    };
    check_cancel(cancellation)?;
    Ok(selection)
}

fn select_scores(
    scores: &Tensor,
    row_ids: &Tensor,
    topk: usize,
    cancellation: &CancellationToken,
) -> Result<Tensor> {
    let count = scores.dims1().map_err(tensor_error)?;
    let keep = count.min(topk);
    let select = || -> Result<Tensor> {
        let sorted = if keep == count {
            row_ids.clone()
        } else {
            let ids = row_ids.to_dtype(DType::F32).map_err(tensor_error)?;
            let sentinel = Tensor::full((MAX_CONTEXT_TOKENS + 1) as f32, count, scores.device())
                .map_err(tensor_error)?;
            let excluded =
                Tensor::full(f32::NEG_INFINITY, count, scores.device()).map_err(tensor_error)?;
            let mut rankings = scores.to_dtype(DType::F32).map_err(tensor_error)?;
            let mut selected = Vec::with_capacity(keep);
            for _ in 0..keep {
                check_cancel(cancellation)?;
                let math = || -> candle_core::Result<(Tensor, Tensor)> {
                    let choice = rankings
                        .broadcast_eq(&rankings.max_keepdim(0)?)?
                        .where_cond(&ids, &sentinel)?
                        .min_keepdim(0)?;
                    let remaining = ids
                        .broadcast_eq(&choice)?
                        .where_cond(&excluded, &rankings)?;
                    Ok((choice, remaining))
                };
                let (choice, remaining) = math().map_err(tensor_error)?;
                selected.push(choice);
                rankings = remaining;
            }
            let choices = Tensor::cat(&selected, 0).map_err(tensor_error)?;
            // Only <=4096 selected IDs are sorted, never a million-key row.
            // Distinct exact IDs make this sort independent of score ties.
            let order = choices.arg_sort_last_dim(true).map_err(tensor_error)?;
            choices
                .index_select(&order, 0)
                .and_then(|value| value.to_dtype(DType::U32))
                .map_err(tensor_error)?
        };
        if keep == topk {
            return Ok(sorted);
        }
        let padding =
            Tensor::full(INVALID_INDEX_ROW, topk - keep, scores.device()).map_err(tensor_error)?;
        Tensor::cat(&[sorted, padding], 0).map_err(tensor_error)
    };
    select()
}

fn select_blocks(
    scores: &Tensor,
    policy: CandidateBlockPolicy,
    cancellation: &CancellationToken,
) -> Result<Tensor> {
    select_blocks_typed(scores, policy, DType::BF16, cancellation)
}

fn select_blocks_typed(
    scores: &Tensor,
    policy: CandidateBlockPolicy,
    dtype: DType,
    cancellation: &CancellationToken,
) -> Result<Tensor> {
    let count = scores.dims1().map_err(tensor_error)?;
    let width = policy.width.tokens();
    let blocks = count.div_ceil(width);
    let compute = || -> candle_core::Result<(Tensor, Tensor)> {
        let mut maxima = Vec::with_capacity(2);
        if blocks > 1 {
            maxima.push(
                scores
                    .narrow(0, 0, (blocks - 1) * width)?
                    .reshape((blocks - 1, width))?
                    .to_dtype(DType::F32)?
                    .max(1)?,
            );
        }
        if dtype == DType::F32 {
            // Pinned XQ runtime: add a finite 1e30 to the actual last-block
            // maximum. It is not an unconditional +inf replacement.
            let first = (blocks - 1) * width;
            maxima.push(
                scores
                    .narrow(0, first, count - first)?
                    .to_dtype(DType::F32)?
                    .max_keepdim(0)?
                    .affine(1., 1e30)?,
            );
        } else {
            // Original official profile unconditionally pins the last block.
            maxima.push(Tensor::full(f32::INFINITY, 1, scores.device())?);
        }
        Ok((
            Tensor::cat(&maxima, 0)?,
            Tensor::arange(0_u32, blocks as u32, scores.device())?,
        ))
    };
    let (maxima, ids) = compute().map_err(tensor_error)?;
    select_scores(&maxima, &ids, policy.topk, cancellation)
}

fn peak_bytes(
    tokens: usize,
    heads: usize,
    dim: usize,
    keys: usize,
    candidates: usize,
    output: usize,
) -> Result<u64> {
    let product = |shape: &[usize]| {
        shape
            .iter()
            .try_fold(1_u64, |n, d| n.checked_mul(*d as u64))
            .ok_or_else(|| invalid("index tensor-byte estimate overflowed"))
    };
    // Named tensors and finite-status/sort work only. Backend allocator/BLAS
    // scratch and model-wide reservations require separate GPU qualification.
    [
        product(&[tokens, heads, dim, 4])?,
        product(&[keys, dim, 2])?,
        product(&[candidates, dim, 80])?,
        product(&[heads, candidates, 32])?,
        product(&[candidates, 96])?,
        product(&[tokens, output, 16])?,
    ]
    .into_iter()
    .try_fold(0_u64, |sum, n| sum.checked_add(n))
    .ok_or_else(|| invalid("index tensor-byte sum overflowed"))
}

fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(PowerError::InferenceCancelled);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod xq_tests;
