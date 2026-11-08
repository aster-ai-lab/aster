use crate::{
    layers::{attention::projection::add, xq::invalid, CudaLayerDevice},
    model::{topology::TextArchitecture, MAX_CONTEXT_TOKENS},
    parallel::{XqDeviceMap, XqPlacement},
    weights::xq::{load::LoadedXqLayer, storage::XqWeights},
};
use aster_power::{
    error::{PowerError, Result},
    inference::{InferenceLimits, RuntimeDevice, RuntimeDeviceKind},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

// Exact pinned base-text special token, not a name/content inference.
pub(super) const IMAGE_TOKEN: u32 = 129264;
pub(super) const EOS_TOKEN: u32 = 1;

/// Rust execution plan; persisted product settings must enter through ACL.
/// State and peer budgets are explicit caller admissions, never auto-expanded.
#[derive(Clone, Debug)]
pub struct XqPipelinePlan {
    pub placement: XqPlacement,
    pub context_tokens: usize,
    pub max_step_tokens: usize,
    pub max_peer_bytes: u64,
    pub max_peer_copies: usize,
    pub max_staged_bytes: u64,
    pub driver_reserve_bytes: u64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XqDeviceAdmission {
    pub ordinal: usize,
    pub name: String,
    pub uuid: [u8; 16],
    pub capability: (i32, i32),
    pub free_bytes_at_load: u64,
    pub total_bytes: u64,
    pub resident_upper_bytes: u64,
    pub state_and_driver_reserve: u64,
}
impl XqPipelinePlan {
    pub fn validate(&self, limits: &InferenceLimits) -> Result<()> {
        limits.validate()?;
        if self.context_tokens == 0
            || self.context_tokens > MAX_CONTEXT_TOKENS
            || self.context_tokens > limits.max_context_tokens
            || self.max_step_tokens == 0
            || self.max_step_tokens > self.context_tokens
            || self.max_peer_bytes == 0
            || self.max_peer_bytes > limits.max_state_bytes
            || self.max_peer_copies == 0
            || self.max_peer_copies > 4096
            || self.max_staged_bytes == 0
            || self.max_staged_bytes > limits.max_state_bytes
            || self.driver_reserve_bytes == 0
        {
            return Err(invalid("XQ pipeline requires positive bounded context, step, peer/count, staged and driver-reserve budgets"));
        }
        if limits.max_concurrent_requests != 1 {
            return Err(invalid("XQ serial correctness pipeline admits exactly one live request; batching/parallel admission is a separate execution profile"));
        }
        if self
            .max_peer_copies
            .checked_mul(512)
            .is_none_or(|n| n > limits.max_input_bytes)
        {
            return Err(invalid(
                "XQ pipeline peer receipts and memoized views exceed host metadata budget",
            ));
        }
        Ok(())
    }

    pub(super) fn resident_by_device(
        &self,
        weights: &XqWeights,
        architecture: &TextArchitecture,
        limits: &InferenceLimits,
    ) -> Result<[u64; 8]> {
        let mut resident = [0_u64; 8];
        for (id, &ordinal) in self.placement.layer_ordinals().iter().enumerate() {
            resident[ordinal] = add(
                resident[ordinal],
                LoadedXqLayer::resident_bound(weights, architecture, id, self.context_tokens)?,
            )?;
        }
        for (name, ordinal) in [
            ("token_embd.weight", self.placement.embedding_ordinal()),
            ("output.weight", self.placement.head_ordinal()),
            ("output_norm.weight", self.placement.head_ordinal()),
        ] {
            resident[ordinal] = add(resident[ordinal], weights.tensor(name)?.bytes)?;
        }
        let head = self.placement.head_ordinal();
        resident[head] = add(
            resident[head],
            (crate::tokenizer::RAW_VOCABULARY * 8) as u64,
        )?;
        if resident[..self.placement.device_count()]
            .iter()
            .any(|&n| n == 0 || n > limits.max_resident_weight_bytes)
        {
            return Err(invalid(
                "XQ placement exceeds a per-device resident weight budget",
            ));
        }
        Ok(resident)
    }
}

pub(super) fn validate_token_step(
    tokens: &[u32],
    position: usize,
    accepted: usize,
    plan: &XqPipelinePlan,
    limits: &InferenceLimits,
) -> Result<()> {
    if tokens.is_empty()
        || tokens.len() > plan.max_step_tokens
        || accepted > tokens.len()
        || position
            .checked_add(tokens.len())
            .is_none_or(|end| end > plan.context_tokens)
    {
        return Err(invalid(
            "XQ token proposal exceeds its nonempty step/context or accepted-prefix bound",
        ));
    }
    validate_text_tokens(tokens, limits)?;
    if tokens
        .len()
        .checked_mul(64)
        .is_none_or(|n| n > limits.max_input_bytes)
    {
        return Err(invalid(
            "XQ token/position/Engram metadata exceeds input budget",
        ));
    }
    Ok(())
}

pub(super) fn validate_text_tokens(tokens: &[u32], limits: &InferenceLimits) -> Result<()> {
    if tokens
        .len()
        .checked_mul(4)
        .is_none_or(|n| n > limits.max_input_bytes)
    {
        return Err(invalid("XQ raw tokens exceed input byte budget"));
    }
    if tokens
        .iter()
        .any(|&id| id as usize >= crate::tokenizer::RAW_VOCABULARY)
    {
        return Err(invalid("XQ text token ID is outside the pinned vocabulary"));
    }
    if tokens.contains(&IMAGE_TOKEN) {
        return Err(invalid(
            "XQ base-text pipeline does not execute the image-token/vision path",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn admit_devices(
    runtimes: &[RuntimeDevice],
    resident: &[u64; 8],
    plan: &XqPipelinePlan,
    limits: &InferenceLimits,
) -> Result<(Vec<CudaLayerDevice>, Vec<XqDeviceAdmission>)> {
    admit_mapped_devices(
        runtimes,
        &XqDeviceMap::identity(plan.placement.replica_width()),
        resident,
        plan,
        limits,
    )
}

pub(super) fn admit_mapped_devices(
    runtimes: &[RuntimeDevice],
    mapping: &XqDeviceMap,
    resident: &[u64; 8],
    plan: &XqPipelinePlan,
    limits: &InferenceLimits,
) -> Result<(Vec<CudaLayerDevice>, Vec<XqDeviceAdmission>)> {
    let device_count = plan.placement.device_count();
    if runtimes.len() != device_count
        || mapping.ordinals().len() != device_count
        || resident[device_count..].iter().any(|&bytes| bytes != 0)
    {
        return Err(invalid(
            "XQ native pipeline requires its exact declared CUDA group and no weights on inactive ordinals",
        ));
    }
    let mut ordered = BTreeMap::new();
    for runtime in runtimes {
        if runtime.kind() != RuntimeDeviceKind::Cuda || !runtime.tensor_device().is_cuda() {
            return Err(PowerError::BackendNotAvailable(
                "XQ native pipeline refuses CPU/Metal fallback".into(),
            ));
        }
        let ordinal = runtime.ordinal().ok_or_else(|| {
            invalid("XQ CUDA ordinal is outside the declared replica-local group")
        })?;
        let local = mapping.local_rank(ordinal)?;
        if ordered.insert(local, runtime).is_some() {
            return Err(invalid("XQ native CUDA ordinals are duplicated"));
        }
    }
    let reserve = add(limits.max_state_bytes, plan.driver_reserve_bytes)?;
    let mut identities = BTreeSet::new();
    let mut devices = Vec::with_capacity(device_count);
    let mut admissions = Vec::with_capacity(device_count);
    for (local, runtime) in ordered {
        let device = CudaLayerDevice::new(runtime.clone(), limits.clone())?;
        let mut admission =
            admit_device(runtime, mapping.ordinals()[local], resident[local], reserve)?;
        if !identities.insert(admission.uuid) {
            return Err(invalid("XQ CUDA ordinals alias one physical device UUID"));
        }
        admission.resident_upper_bytes = resident[local];
        devices.push(device);
        admissions.push(admission);
    }
    Ok((devices, admissions))
}

#[cfg(feature = "cuda")]
fn admit_device(
    runtime: &RuntimeDevice,
    ordinal: usize,
    resident: u64,
    reserve: u64,
) -> Result<XqDeviceAdmission> {
    let error = |e| PowerError::BackendNotAvailable(format!("XQ CUDA admission query failed: {e}"));
    let device = runtime
        .tensor_device()
        .as_cuda_device()
        .map_err(crate::layers::tensor_error)?;
    let stream = device.cuda_stream();
    let ctx = stream.context();
    let capability = ctx.compute_capability().map_err(error)?;
    if capability != (9, 0) {
        return Err(PowerError::BackendNotAvailable(
            "XQ native packed kernels require actual SM90 capability 9.0".into(),
        ));
    }
    let (free, total) = ctx.mem_get_info().map_err(error)?;
    if add(resident, reserve)? > free as u64 {
        return Err(invalid("XQ resident weights plus admitted state/driver reserve exceed actual free device memory"));
    }
    Ok(XqDeviceAdmission {
        ordinal,
        name: ctx.name().map_err(error)?,
        uuid: ctx.uuid().map_err(error)?.bytes.map(|n| n as u8),
        capability,
        free_bytes_at_load: free as u64,
        total_bytes: total as u64,
        resident_upper_bytes: resident,
        state_and_driver_reserve: reserve,
    })
}
#[cfg(not(feature = "cuda"))]
fn admit_device(
    _runtime: &RuntimeDevice,
    _ordinal: usize,
    _resident: u64,
    _reserve: u64,
) -> Result<XqDeviceAdmission> {
    Err(PowerError::BackendNotAvailable(
        "XQ native pipeline requires the cuda feature".into(),
    ))
}
