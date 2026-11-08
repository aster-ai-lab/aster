//! Callable layer -> uncommitted request state. A failed computation poisons
//! both the backbone transaction and per-step index sharing. No layer can
//! independently publish KV, compression or n-gram history.

use super::{invalid, LoadedTextLayer};
use crate::{
    engram::EngramLayer,
    layers::{mhc::MhcContinuation, tensor_error},
    model::sharing::StepIndexSharing,
    state::backbone::{BackboneState, PendingBackboneStep},
};
use aster_power::error::{PowerError, Result};
use candle_core::Tensor;
use tokio_util::sync::CancellationToken;

impl LoadedTextLayer<'_> {
    pub fn forward_pending(
        &self,
        input: MhcContinuation,
        state: &BackboneState,
        pending: &mut PendingBackboneStep,
        sharing: &mut StepIndexSharing,
        cancellation: &CancellationToken,
    ) -> Result<MhcContinuation> {
        let output = self.forward_step(input, state, pending, sharing, cancellation);
        if output.is_err() {
            pending.poison();
            sharing.poison();
        }
        output
    }

    fn forward_step(
        &self,
        input: MhcContinuation,
        state: &BackboneState,
        pending: &mut PendingBackboneStep,
        sharing: &mut StepIndexSharing,
        cancellation: &CancellationToken,
    ) -> Result<MhcContinuation> {
        if cancellation.is_cancelled() {
            return Err(PowerError::InferenceCancelled);
        }
        if state.compression_numerics() != crate::state::CompressionNumerics::OfficialBf16 {
            return Err(invalid(
                "official text execution cannot use XQ F32 backbone state",
            ));
        }
        if pending.is_poisoned() || pending.position() != state.position() {
            return Err(invalid(
                "loaded layer requires the current unpoisoned backbone proposal",
            ));
        }
        let end = pending
            .position()
            .checked_add(pending.token_count())
            .ok_or_else(|| invalid("model step position overflowed"))?;
        let positions = (pending.position()..end).collect::<Vec<_>>();
        if positions != sharing.positions()
            || input.residual().dims().first() != Some(&pending.token_count())
        {
            return Err(invalid(
                "layer input, shared-index and backbone step ranges differ",
            ));
        }
        let mask = Tensor::from_vec(
            pending
                .ngrams()
                .iter()
                .map(|r| u8::from(r.is_image()))
                .collect::<Vec<_>>(),
            pending.token_count(),
            self.device.tensor_device(),
        )
        .map_err(tensor_error)?;
        let embeddings = match (&self.engram_table, self.binding.engram()) {
            (Some(table), Some(binding)) => {
                let layer = match binding.table() {
                    0 => EngramLayer::Layer1,
                    1 => EngramLayer::Layer14,
                    _ => return Err(invalid("unknown bound Engram table index")),
                };
                Some(table.fetch_ngrams(layer, pending.ngrams(), cancellation)?)
            }
            (None, None) => None,
            _ => return Err(invalid("loaded Engram table differs from the bound layer")),
        };
        let prepared = self.decoder.prepare(
            input,
            &positions,
            embeddings.as_ref(),
            Some(&mask),
            cancellation,
        )?;
        let layer = self.binding.layer();
        let window = state.window(layer)?.prepare(
            pending.position(),
            pending.token_count(),
            &positions,
            prepared.attention().projected().window_rows().records(),
            cancellation,
        )?;
        pending.stage_window(layer, window, cancellation)?;
        if self.binding.is_kv_source() {
            let source = state.compressed_source(layer)?;
            let compressor = self
                .compressor
                .as_ref()
                .ok_or_else(|| invalid("compressed source is missing its loaded compressor"))?;
            let index_key = self
                .index_key
                .as_ref()
                .ok_or_else(|| invalid("index-K source is missing its loaded projection"))?;
            let compressed = compressor.prepare(
                source.compression(),
                pending.position(),
                prepared.attention().current_hidden(),
                cancellation,
            )?;
            let key = index_key.forward(
                compressed.pre_rope_kv(),
                compressed.emitted_positions(),
                cancellation,
            )?;
            let main = source.main().prepare(
                pending.position(),
                pending.token_count(),
                compressed.emitted_positions(),
                compressed.rows().records(),
                cancellation,
            )?;
            let index = source.index().prepare(
                pending.position(),
                pending.token_count(),
                compressed.emitted_positions(),
                key.rows_data().data_bytes(),
                key.rows_data().scale_bytes(),
                cancellation,
            )?;
            let (compression, _, _) = compressed.into_parts();
            pending.stage_source(layer, compression, main, index, cancellation)?;
        }
        let source = self
            .binding
            .sources()
            .map(|s| pending.compressed_source(s.kv()))
            .transpose()?;
        let main = source
            .map(|s| s.main().attention_rows(&self.device))
            .transpose()?;
        let main_ready = source.map(|s| s.main().ready_positions().collect::<Vec<_>>());
        let selected = if self.binding.is_index_topk_source() {
            let projection = self
                .index_query
                .as_ref()
                .ok_or_else(|| invalid("index source is missing its loaded query projection"))?;
            let indexer = self
                .indexer
                .as_ref()
                .ok_or_else(|| invalid("index source is missing its loaded selector"))?;
            let topology = self
                .binding
                .sources()
                .ok_or_else(|| invalid("index source has no compressed topology"))?;
            let index_source = pending.compressed_source(topology.index_k())?;
            let keys = index_source.index().index_rows(&self.device)?;
            let query = projection.forward_packed(
                prepared.attention().current_hidden(),
                prepared.attention().projected().normalized_query(),
                &positions,
                cancellation,
            )?;
            let ready = main_ready
                .as_deref()
                .ok_or_else(|| invalid("index source has no prepared main KV rows"))?;
            let index_ready = index_source.index().ready_positions().collect::<Vec<_>>();
            if index_ready != ready {
                return Err(invalid("prepared main and index-K row identities differ"));
            }
            Some(indexer.select_packed(
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
        self.decoder
            .finish(prepared, &window, main.as_ref(), &plan, cancellation)
    }
}

#[cfg(test)]
mod tests;
