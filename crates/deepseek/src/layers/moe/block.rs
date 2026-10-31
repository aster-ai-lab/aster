//! Complete single-device MoE control: BF16 router, packed input dispatch,
//! MXFP4 routed experts, one FP8 shared expert and FP32 lane-ordered reduction.
//! Only bounded top-k logical IDs cross to the host for control scheduling;
//! activations, weights and numerical outputs stay on their bound device.
//! This supplies neither TP/EP collectives nor a grouped/fused SM90 kernel.
//! Reduction ordering follows DeepEP's MIT refs.py at revision
//! f6aa38fe06cd4a0489e7730365d4e0114970a2f3 with local/scale-up reductions off:
//! initialize FP32 with the shared bias, then add top-k lanes in order.

use aster_power::error::{PowerError, Result};
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::{invalid, router::MoeRouter, tensor_error};
use crate::layers::{
    linear::{activation::Fp8Activations, expert::QuantizedExpert},
    validate_finite, CudaLayerDevice,
};
use crate::weights::MatrixLayout;

pub struct MoeBlock {
    device: CudaLayerDevice,
    router: MoeRouter,
    routed: Vec<QuantizedExpert>,
    shared: QuantizedExpert,
    resident_bytes: u64,
}

impl MoeBlock {
    pub fn new(
        device: CudaLayerDevice,
        router: MoeRouter,
        routed: Vec<QuantizedExpert>,
        shared: QuantizedExpert,
    ) -> Result<Self> {
        if routed.len() != router.expert_count()
            || shared.layout() != MatrixLayout::Fp8Block32
            || !router
                .device()
                .tensor_device()
                .same_device(device.tensor_device())
            || router.device().limits() != device.limits()
        {
            return Err(invalid("MoE block requires one FP8 shared expert and the router's exact routed-expert inventory/device"));
        }
        let mut resident_bytes = router.retained_parameter_bytes();
        for (index, expert) in routed.iter().chain(std::iter::once(&shared)).enumerate() {
            if expert.hidden_size() != router.hidden_size()
                || expert.intermediate_size() != shared.intermediate_size()
                || !expert
                    .device()
                    .tensor_device()
                    .same_device(device.tensor_device())
                || expert.device().limits() != device.limits()
                || (index < routed.len() && expert.layout() != MatrixLayout::Mxfp4Block32)
            {
                return Err(invalid("MoE expert format, hidden geometry, intermediate size or owning device/limits differ"));
            }
            resident_bytes = resident_bytes
                .checked_add(expert.retained_weight_bytes())
                .ok_or_else(|| invalid("MoE resident weight bytes overflowed"))?;
        }
        if resident_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "MoE block exceeds its aggregate resident-weight budget",
            ));
        }
        Ok(Self {
            device,
            router,
            routed,
            shared,
            resident_bytes,
        })
    }

    pub fn retained_weight_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn hidden_size(&self) -> usize {
        self.router.hidden_size()
    }

    pub fn forward(
        &self,
        hidden: &Tensor,
        image_mask: Option<&Tensor>,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        let (tokens, width) = hidden.dims2().map_err(tensor_error)?;
        if tokens == 0
            || width != self.router.hidden_size()
            || hidden.dtype() != DType::BF16
            || !hidden.device().same_device(self.device.tensor_device())
        {
            return Err(invalid(
                "MoE block requires matching nonempty same-device BF16 hidden rows",
            ));
        }
        self.live_tensor_bytes(tokens)?;
        let routes = self.router.forward(hidden, image_mask, cancellation)?;
        check_cancel(cancellation)?;
        let topk = self.router.top_k();
        // This bounded metadata copy is an explicit synchronization in the
        // correctness control, not CPU copies of numerical tensor payloads.
        let logical = routes
            .logical_experts()
            .flatten_all()
            .and_then(|t| t.to_vec1::<u32>())
            .map_err(tensor_error)?;
        let groups = group_routes(&logical, tokens, topk, self.routed.len())?;
        check_cancel(cancellation)?;
        let input = Fp8Activations::encode(&self.device, hidden, cancellation)?;
        let flat_weights = routes.weights().flatten_all().map_err(tensor_error)?;
        let mut slots = Tensor::zeros((tokens * topk, width), DType::F32, hidden.device())
            .map_err(tensor_error)?;
        for (expert, entries) in self.routed.iter().zip(groups) {
            if entries.is_empty() {
                continue;
            }
            check_cancel(cancellation)?;
            let rows = Tensor::from_vec(
                entries.iter().map(|entry| entry.0).collect::<Vec<_>>(),
                entries.len(),
                hidden.device(),
            )
            .map_err(tensor_error)?;
            let slot_ids = Tensor::from_vec(
                entries.iter().map(|entry| entry.1).collect::<Vec<_>>(),
                entries.len(),
                hidden.device(),
            )
            .map_err(tensor_error)?;
            let packed = input.select_rows(&self.device, &rows, cancellation)?;
            let weights = flat_weights
                .index_select(&slot_ids, 0)
                .map_err(tensor_error)?;
            let output = expert.forward(&packed, Some(&weights), cancellation)?;
            slots = slots
                .index_add(
                    &slot_ids,
                    &output.to_dtype(DType::F32).map_err(tensor_error)?,
                    0,
                )
                .map_err(tensor_error)?;
        }
        let slots = slots.reshape((tokens, topk, width)).map_err(tensor_error)?;
        let shared = self.shared.forward(&input, None, cancellation)?;
        let mut combined = shared.to_dtype(DType::F32).map_err(tensor_error)?;
        for lane in 0..topk {
            check_cancel(cancellation)?;
            combined = combined
                .add(
                    &slots
                        .narrow(1, lane, 1)
                        .and_then(|t| t.squeeze(1))
                        .map_err(tensor_error)?,
                )
                .map_err(tensor_error)?;
        }
        validate_finite(&combined, false, "MoE combined accumulation")?;
        let output = combined
            .to_dtype(DType::BF16)
            .map_err(tensor_error)?
            .detach();
        validate_finite(&output, false, "MoE output")?;
        check_cancel(cancellation)?;
        Ok(output)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let limits = self.device.limits();
        let topk = self.router.top_k();
        let hidden = self.router.hidden_size();
        let intermediate = self.shared.intermediate_size();
        let input = limits.checked_elements(&[tokens, hidden], "MoE hidden rows")? as u64;
        let inner =
            limits.checked_elements(&[tokens, intermediate], "MoE intermediate rows")? as u64;
        let slots =
            limits.checked_elements(&[tokens, topk, hidden], "MoE lane output slots")? as u64;
        let routes = limits.checked_elements(&[tokens, topk], "MoE logical routes")? as u64;
        let scores =
            limits.checked_elements(&[tokens, self.routed.len()], "MoE router scores")? as u64;
        if routes > u32::MAX as u64 {
            return Err(invalid("MoE routed slot IDs exceed U32"));
        }
        let metadata = routes
            .checked_mul(48)
            .and_then(|n| n.checked_add((self.routed.len() * 64 + 4096) as u64))
            .ok_or_else(|| invalid("MoE host metadata budget overflowed"))?;
        if metadata > limits.max_input_bytes as u64 {
            return Err(invalid("MoE host route metadata exceeds its byte budget"));
        }
        let tile = limits.checked_elements(
            &[32.min(hidden.max(intermediate)), hidden.max(intermediate)],
            "MoE expanded weight tile",
        )? as u64;
        let mut bytes = 4096_u64;
        for (count, factor) in [
            (input, 192),
            (inner, 160),
            (slots, 24),
            (scores, 224),
            (tile, 48),
            (self.router.retained_parameter_bytes(), 6),
        ] {
            bytes = count
                .checked_mul(factor)
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(|| invalid("MoE composite tensor budget overflowed"))?;
        }
        limits.checked_state_bytes(bytes, "MoE composite live tensors")?;
        Ok(bytes)
    }
}

pub(crate) fn group_routes(
    logical: &[u32],
    tokens: usize,
    topk: usize,
    experts: usize,
) -> Result<Vec<Vec<(u32, u32)>>> {
    let count = tokens
        .checked_mul(topk)
        .filter(|count| *count <= u32::MAX as usize)
        .ok_or_else(|| invalid("MoE route metadata dimensions overflowed"))?;
    if tokens == 0
        || topk == 0
        || topk > 6
        || topk > experts
        || experts == 0
        || experts > 384
        || logical.len() != count
    {
        return Err(invalid(
            "MoE route metadata differs from bounded router geometry",
        ));
    }
    let mut groups = vec![Vec::new(); experts];
    for (slot, &expert) in logical.iter().enumerate() {
        let row = slot / topk;
        let lane = slot % topk;
        if expert as usize >= experts || logical[row * topk..row * topk + lane].contains(&expert) {
            return Err(invalid(
                "MoE route IDs contain an unknown or duplicate expert for one token",
            ));
        }
        groups[expert as usize].push((row as u32, slot as u32));
    }
    Ok(groups)
}
fn check_cancel(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(PowerError::InferenceCancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
