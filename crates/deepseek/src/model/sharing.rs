//! One model step's sequential shared-index/candidate control. Only bounded
//! selected integer IDs cross from device to host; Q/K/hidden values stay on
//! their devices. Shared selections never outlive this request step. Actual
//! KV publication remains the complete BackboneState transaction's job.
//! Topology follows pinned DeepInfer cross_layer.py/index_attn.py (Apache-2.0).

use aster_power::{
    error::{PowerError, Result},
    inference::InferenceLimits,
};
use candle_core::DType;
use tokio_util::sync::CancellationToken;

use super::topology::LayerBinding;
use crate::layers::{
    attention::{KvCandidate, KvSource, SparseAttentionPlan},
    indexer::{SparseIndexPlan, SparseIndexSelection, INVALID_INDEX_ROW},
    tensor_error,
};

struct SharedTopk {
    source: usize,
    kv_source: usize,
    ready: Vec<usize>,
    rows: Vec<Vec<usize>>,
}
struct SharedCandidates {
    ready: Vec<usize>,
    blocks: Vec<Vec<usize>>,
}
pub struct StepIndexSharing {
    limits: InferenceLimits,
    positions: Vec<usize>,
    next_layer: usize,
    poisoned: bool,
    topk: Option<SharedTopk>,
    candidates: Option<SharedCandidates>,
}

impl StepIndexSharing {
    pub(crate) fn validate_next_layer(&self, binding: &LayerBinding) -> Result<()> {
        self.check_layer(binding)
    }
    pub(crate) fn poison(&mut self) {
        self.poisoned = true;
    }
    pub(crate) fn positions(&self) -> &[usize] {
        &self.positions
    }

    pub fn new(limits: &InferenceLimits, positions: &[usize]) -> Result<Self> {
        limits.validate()?;
        if limits.max_context_tokens > super::MAX_CONTEXT_TOKENS
            || positions.is_empty()
            || positions.len() > limits.max_context_tokens
            || positions.iter().any(|&p| p >= limits.max_context_tokens)
            || positions
                .windows(2)
                .any(|p| p[0].checked_add(1) != Some(p[1]))
        {
            return Err(invalid(
                "shared-index step requires bounded contiguous query positions",
            ));
        }
        metadata_bound(limits, positions.len(), 0)?;
        Ok(Self {
            limits: limits.clone(),
            positions: positions.to_vec(),
            next_layer: 0,
            poisoned: false,
            topk: None,
            candidates: None,
        })
    }

    /// Build the current source's causal search plan before executing its Q/K
    /// indexer. Later C1 sources restrict to the candidates published at layer20.
    pub fn index_plan(&self, binding: &LayerBinding, ready: &[usize]) -> Result<SparseIndexPlan> {
        self.check_layer(binding)?;
        if !binding.is_index_topk_source() {
            return Err(invalid("only a declared index source computes a new top-k"));
        }
        self.check_compressed(binding, ready)?;
        let restrictions = if binding.sources().and_then(|s| s.candidate()).is_some()
            && !binding.is_candidate_source()
        {
            let candidates = self
                .candidates
                .as_ref()
                .ok_or_else(|| invalid("candidate-source layer20 has not published this step"))?;
            if candidates.ready != ready {
                return Err(invalid(
                    "candidate consumers must use the same compressed row identities",
                ));
            }
            let mut rows = Vec::with_capacity(self.positions.len());
            for (blocks, position) in candidates.blocks.iter().zip(&self.positions) {
                let mut row = Vec::new();
                for &block in blocks {
                    let first = block
                        .checked_mul(8)
                        .ok_or_else(|| invalid("candidate block offset overflowed"))?;
                    let end = first
                        .checked_add(8)
                        .ok_or_else(|| invalid("candidate block end overflowed"))?
                        .min(ready.len());
                    for (offset, &p) in ready[first..end].iter().enumerate() {
                        if p <= *position {
                            row.push(first + offset);
                        }
                    }
                }
                rows.push(row);
            }
            Some(rows)
        } else {
            None
        };
        SparseIndexPlan::new(
            &self.limits,
            ready,
            &self.positions,
            restrictions.as_deref(),
        )
    }

    /// Advance exactly one layer. A refused advance poisons this step, so an
    /// ignored error cannot later construct apparently complete model results.
    pub fn attention_plan(
        &mut self,
        binding: &LayerBinding,
        window_ready: &[usize],
        compressed_ready: Option<&[usize]>,
        computed: Option<&SparseIndexSelection>,
        cancellation: &CancellationToken,
    ) -> Result<SparseAttentionPlan> {
        let checked = self.check_layer(binding);
        self.poisoned = true;
        checked?;
        check_cancel(cancellation)?;
        if window_ready.windows(2).any(|p| p[0] >= p[1])
            || window_ready
                .iter()
                .any(|&p| p >= self.limits.max_context_tokens)
            || binding.compression().is_some() != compressed_ready.is_some()
            || binding.is_index_topk_source() != computed.is_some()
        {
            return Err(invalid(
                "shared attention rows or index production differ from this layer's topology",
            ));
        }
        let main = compressed_ready.unwrap_or(&[]);
        if binding.compression().is_some() {
            self.check_compressed(binding, main)?;
        }
        metadata_bound(
            &self.limits,
            self.positions.len(),
            window_ready
                .len()
                .checked_add(main.len())
                .ok_or_else(|| invalid("shared KV metadata count overflowed"))?,
        )?;
        let staged = if let Some(selection) = computed {
            Some(self.capture_topk(binding, main, selection, cancellation)?)
        } else {
            None
        };
        let cached = staged.as_ref().map(|s| &s.0).or(self.topk.as_ref());
        let topk = if let Some(sources) = binding.sources() {
            let topk = cached
                .ok_or_else(|| invalid("this step's required index source has not published"))?;
            if topk.source != sources.index_topk()
                || topk.kv_source != sources.kv()
                || topk.ready != main
            {
                return Err(invalid(
                    "shared top-k source or exact compressed row identities differ",
                ));
            }
            Some(topk)
        } else {
            None
        };
        let candidates = window_ready
            .iter()
            .map(|&p| KvCandidate::new(KvSource::Window, p))
            .chain(
                main.iter()
                    .map(|&p| KvCandidate::new(KvSource::Compressed, p)),
            )
            .collect::<Vec<_>>();
        let mut selections = Vec::with_capacity(self.positions.len());
        for (row, &position) in self.positions.iter().enumerate() {
            check_cancel(cancellation)?;
            let mut selected = window_ready
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| (p <= position && position - p < 128).then_some(i))
                .collect::<Vec<_>>();
            if let Some(topk) = topk {
                selected.extend(topk.rows[row].iter().map(|i| window_ready.len() + i));
            }
            selections.push(selected);
        }
        let plan =
            SparseAttentionPlan::new(&self.limits, 128, &candidates, &self.positions, &selections)?;
        check_cancel(cancellation)?;
        if let Some((topk, candidates)) = staged {
            self.topk = Some(topk);
            if let Some(candidates) = candidates {
                self.candidates = Some(candidates);
            }
        }
        self.next_layer += 1;
        self.poisoned = false;
        Ok(plan)
    }

    pub fn is_complete(&self) -> bool {
        self.next_layer == 40 && !self.poisoned
    }
    fn check_layer(&self, binding: &LayerBinding) -> Result<()> {
        if self.poisoned || self.next_layer >= 40 || binding.layer() != self.next_layer {
            return Err(invalid(
                "shared-index step is poisoned, complete or received a nonsequential layer",
            ));
        }
        Ok(())
    }
    fn check_compressed(&self, binding: &LayerBinding, ready: &[usize]) -> Result<()> {
        let ratio = binding
            .compression()
            .ok_or_else(|| invalid("compressed rows require a declared compression kind"))?
            .ratio();
        metadata_bound(&self.limits, self.positions.len(), ready.len())?;
        if ready.iter().enumerate().any(|(r, &p)| {
            r.checked_add(1)
                .and_then(|r| r.checked_mul(ratio))
                .and_then(|r| r.checked_sub(1))
                != Some(p)
        }) || ready.len()
            != self
                .positions
                .last()
                .copied()
                .map_or(0, |p| (p + 1) / ratio)
        {
            return Err(invalid(
                "compressed row identities must cover the exact prepared causal step",
            ));
        }
        Ok(())
    }
    fn capture_topk(
        &self,
        binding: &LayerBinding,
        ready: &[usize],
        selection: &SparseIndexSelection,
        cancellation: &CancellationToken,
    ) -> Result<(SharedTopk, Option<SharedCandidates>)> {
        if selection.rows().dtype() != DType::U32
            || selection.rows().dims() != [self.positions.len(), 512]
            || selection.valid_lengths().len() != self.positions.len()
            || selection.candidate_blocks().is_some() != binding.is_candidate_source()
        {
            return Err(invalid(
                "source index output shape/type or candidate ownership differs",
            ));
        }
        check_cancel(cancellation)?;
        let raw = selection.rows().to_vec2::<u32>().map_err(tensor_error)?;
        let mut rows = Vec::with_capacity(raw.len());
        for ((row, &len), &position) in raw
            .iter()
            .zip(selection.valid_lengths())
            .zip(&self.positions)
        {
            if len > 512
                || row[..len]
                    .iter()
                    .any(|&i| i as usize >= ready.len() || ready[i as usize] > position)
                || row[..len].windows(2).any(|i| i[0] >= i[1])
                || row[len..].iter().any(|&i| i != INVALID_INDEX_ROW)
            {
                return Err(invalid(
                    "shared top-k contains future, duplicate, invalid or unsorted rows/padding",
                ));
            }
            rows.push(row[..len].iter().map(|&i| i as usize).collect());
        }
        let candidates=selection.candidate_blocks().map(|blocks| {
            let shape=blocks.dims2().map_err(tensor_error)?;
            if blocks.dtype()!=DType::U32 || shape.0!=self.positions.len() || shape.1==0 || shape.1>2048 {return Err(invalid("candidate source has invalid bounded block metadata"));}
            let raw=blocks.to_vec2::<u32>().map_err(tensor_error)?;
            let mut owned=Vec::with_capacity(raw.len());
            for (row,position) in raw.iter().zip(&self.positions) {
                let valid=row.iter().take_while(|&&i|i!=INVALID_INDEX_ROW).count();
                let eligible=ready.partition_point(|p|p<=position).div_ceil(8);
                if row[..valid].iter().any(|&i|i as usize>=eligible)
                    || row[..valid].windows(2).any(|i|i[0]>=i[1])
                    || row[valid..].iter().any(|&i|i!=INVALID_INDEX_ROW)
                {return Err(invalid("candidate blocks contain future, duplicate, unsorted or invalid padded rows"));}
                owned.push(row[..valid].iter().map(|&i|i as usize).collect());
            }
            Ok(SharedCandidates {ready:ready.to_vec(),blocks:owned})
        }).transpose()?;
        check_cancel(cancellation)?;
        let source = binding
            .sources()
            .ok_or_else(|| invalid("index source has no exact shared topology"))?;
        Ok((
            SharedTopk {
                source: binding.layer(),
                kv_source: source.kv(),
                ready: ready.to_vec(),
                rows,
            },
            candidates,
        ))
    }
}

fn metadata_bound(limits: &InferenceLimits, tokens: usize, keys: usize) -> Result<()> {
    // Includes old/new snapshots, selected IDs and the largest candidate
    // expansion (2048 blocks * eight rows), before host tensor readback.
    let bytes = (keys as u64)
        .checked_mul(64)
        .and_then(|n| {
            (tokens as u64)
                .checked_mul(512 * 32 + 2048 * 8 * 16 + 4096)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| invalid("shared-index metadata byte count overflowed"))?;
    if keys > limits.max_context_tokens * 2 + 128 || bytes > limits.max_graph_plan_bytes as u64 {
        return Err(invalid(
            "shared-index control exceeds its metadata byte budget",
        ));
    }
    Ok(())
}
fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        return Err(PowerError::InferenceCancelled);
    }
    Ok(())
}
fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}

#[cfg(test)]
mod tests;
