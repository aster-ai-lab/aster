//! Stable greedy token selection. Candle's CUDA fast_argmax keeps the winning
//! reduction lane on equal values, which need not be the smallest global ID.
//! Keep the float maximum on the device, then reduce equal-score U32 IDs by
//! minimum. Only the final one-word token ID crosses to the host.
use crate::layers::{
    tensor_error, validate_finite,
    xq::{check_cancel, invalid, same_device},
    CudaLayerDevice,
};
use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

pub(super) struct XqGreedySelector {
    device: CudaLayerDevice,
    indices: Tensor,
    sentinel: Tensor,
    vocabulary: usize,
}
impl XqGreedySelector {
    pub(super) fn new(
        device: &CudaLayerDevice,
        vocabulary: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        if vocabulary == 0 || vocabulary > crate::tokenizer::RAW_VOCABULARY {
            return Err(invalid(
                "XQ greedy selection vocabulary is outside 1..129280",
            ));
        }
        device
            .limits()
            .checked_elements(&[vocabulary], "XQ greedy token indices")?;
        device.limits().checked_state_bytes(
            (vocabulary as u64) * 24,
            "XQ greedy permanent indices and construction",
        )?;
        if vocabulary
            .checked_mul(8)
            .is_none_or(|n| n > device.limits().max_input_bytes)
        {
            return Err(invalid(
                "XQ greedy token index construction exceeds host input budget",
            ));
        }
        // Construct once with a fixed bound, rather than rebuilding a host
        // vocabulary-sized arange for every generated token.
        let indices = Tensor::from_vec(
            (0..vocabulary as u32).collect::<Vec<_>>(),
            vocabulary,
            device.tensor_device(),
        )
        .map_err(tensor_error)?;
        let sentinel =
            Tensor::full(u32::MAX, vocabulary, device.tensor_device()).map_err(tensor_error)?;
        check_cancel(cancellation)?;
        Ok(Self {
            device: device.clone(),
            indices,
            sentinel,
            vocabulary,
        })
    }
    pub(super) fn select_last(
        &self,
        logits: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<u32> {
        check_cancel(cancellation)?;
        same_device(&self.device, logits)?;
        let (tokens, vocabulary) = logits.dims2().map_err(tensor_error)?;
        if tokens == 0 || vocabulary != self.vocabulary || logits.dtype() != DType::F32 {
            return Err(invalid(
                "XQ greedy selector requires its bound nonempty F32[token,vocabulary] logits",
            ));
        }
        self.device
            .limits()
            .checked_elements(logits.dims(), "XQ greedy logits view")?;
        self.device.limits().checked_state_bytes(
            (vocabulary as u64) * 32,
            "XQ greedy last-row float maximum, predicate and U32 minimum",
        )?;
        let row = logits.get(tokens - 1).map_err(tensor_error)?;
        validate_finite(&row, false, "XQ greedy last logits")?;
        let max = row
            .max(0)
            .and_then(|x| x.broadcast_as(vocabulary))
            .map_err(tensor_error)?;
        let selected = row
            .eq(&max)
            .and_then(|mask| mask.where_cond(&self.indices, &self.sentinel))
            .and_then(|ids| ids.min(0))
            .and_then(|id| id.to_scalar::<u32>())
            .map_err(tensor_error)?;
        check_cancel(cancellation)?;
        if selected as usize >= vocabulary {
            return Err(invalid(
                "XQ greedy reduction produced no valid finite token ID",
            ));
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests;
