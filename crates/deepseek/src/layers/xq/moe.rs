//! Resident mixed XQ experts with F32 routing/activations and lane reduction.
//! Pinned ds41_dense.cpp/ds41_moe.cpp: route weights follow the down projection;
//! routed lanes sum before the shared branch. No official FP8 activation cast
//! or CPU numerical fallback is inserted into this separate XQ control.

use std::collections::{BTreeMap, BTreeSet};

use aster_power::error::Result;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

use super::mhc::{XqMhcBlock, XqMhcContinuation, XqMhcSite};
use super::{check_cancel, invalid, same_device, XqRmsNorm};
use crate::layers::{
    linear::xq::XqDeviceMatrix,
    moe::{block::group_routes, router::score_math, MoeRouteSelection},
    tensor_error, validate_finite, CudaLayerDevice,
};
use crate::weights::xq::{
    experts::{XqExpertLayout, XqExpertRole},
    storage::XqWeights,
    XqTensorType,
};

const EXPERTS: usize = 384;
const HIDDEN: usize = 5120;
const INTERMEDIATE: usize = 2304;
const TOPK: usize = 6;

#[derive(Clone, Debug)]
struct MatrixRows {
    stack: String,
    kind: XqTensorType,
    first: usize,
    rows: usize,
    columns: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StackSpec {
    kind: XqTensorType,
    rows: usize,
    columns: usize,
}

struct MoePlan {
    prefix: String,
    experts: Vec<[MatrixRows; 3]>,
    stacks: BTreeMap<String, StackSpec>,
    resident_bytes: u64,
}

/// Each typed stack has one allocated owner. Expert ranges borrow that owner
/// only during execution, so neither persistent weight copies nor misleading
/// per-view retained-byte counts are needed.
pub struct XqMoeBlock {
    device: CudaLayerDevice,
    router: XqDeviceMatrix,
    selection: MoeRouteSelection,
    shared: [XqDeviceMatrix; 3],
    stacks: BTreeMap<String, XqDeviceMatrix>,
    experts: Vec<[MatrixRows; 3]>,
    hidden: usize,
    intermediate: usize,
    topk: usize,
    resident_bytes: u64,
}

/// Complete F32 shifted feed-forward half, with immutable carried mHC state.
pub struct XqFfnSublayer {
    device: CudaLayerDevice,
    layer: usize,
    mhc: XqMhcBlock,
    norm: XqRmsNorm,
    moe: XqMoeBlock,
    resident_bytes: u64,
}

impl XqFfnSublayer {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let plan = MoePlan::bind(weights, layer, device, cancellation)?;
        let mut bytes = plan.resident_bytes;
        for (suffix, kind, shape) in [
            ("hc_ffn_fn.weight", XqTensorType::F16, vec![20480, 24]),
            ("hc_ffn_scale.weight", XqTensorType::F32, vec![3]),
            ("hc_ffn_base.weight", XqTensorType::F32, vec![24]),
            ("ffn_norm.weight", XqTensorType::F32, vec![5120]),
        ] {
            let tensor = weights.tensor(&format!("{}.{suffix}", plan.prefix))?;
            if tensor.kind != kind || tensor.shape != shape {
                return Err(invalid(
                    "XQ FFN norm/mHC parameters differ from the pinned schema",
                ));
            }
            bytes = add(bytes, tensor.bytes)?;
        }
        if bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "complete XQ FFN half exceeds its pre-upload resident budget",
            ));
        }
        let mhc = XqMhcBlock::load(device, weights, layer, XqMhcSite::FeedForward, cancellation)?;
        let norm = XqRmsNorm::load(
            device,
            weights,
            &format!("{}.ffn_norm.weight", plan.prefix),
            HIDDEN,
            cancellation,
        )?;
        let moe = XqMoeBlock::load_plan(device, weights, plan, cancellation)?;
        let output = Self::from_parts(device.clone(), layer, mhc, norm, moe)?;
        if output.retained_bytes() != bytes {
            return Err(invalid(
                "XQ FFN retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(output)
    }

    fn from_parts(
        device: CudaLayerDevice,
        layer: usize,
        mhc: XqMhcBlock,
        norm: XqRmsNorm,
        moe: XqMoeBlock,
    ) -> Result<Self> {
        if layer >= 40 || mhc.hidden_size() != norm.width() || norm.width() != moe.hidden_size() {
            return Err(invalid("XQ FFN components differ in layer/hidden geometry"));
        }
        for owner in [mhc.device(), norm.device(), moe.device()] {
            if !owner.tensor_device().same_device(device.tensor_device())
                || owner.limits() != device.limits()
            {
                return Err(invalid(
                    "XQ FFN components differ from the owning device/limits",
                ));
            }
        }
        let resident_bytes = add(
            add(mhc.retained_bytes(), norm.retained_bytes())?,
            moe.retained_bytes(),
        )?;
        if resident_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "XQ FFN components exceed their aggregate resident budget",
            ));
        }
        Ok(Self {
            device,
            layer,
            mhc,
            norm,
            moe,
            resident_bytes,
        })
    }

    pub fn layer(&self) -> usize {
        self.layer
    }
    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }

    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }
    pub(crate) fn hidden_size(&self) -> usize {
        self.moe.hidden_size()
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let norm_live = (tokens as u64)
            .checked_mul(self.hidden_size() as u64)
            .and_then(|n| n.checked_mul(32))
            .ok_or_else(|| invalid("XQ FFN norm live bytes overflowed"))?;
        add(
            add(
                self.moe.live_tensor_bytes(tokens)?,
                self.mhc.live_tensor_bytes(tokens)?,
            )?,
            add(norm_live, self.resident_bytes)?,
        )
    }

    pub fn forward(
        &self,
        continuation: XqMhcContinuation,
        image_mask: Option<&Tensor>,
        cancellation: &CancellationToken,
    ) -> Result<XqMhcContinuation> {
        check_cancel(cancellation)?;
        let (tokens, _, hidden) = continuation.residual().dims3().map_err(tensor_error)?;
        if hidden != self.moe.hidden_size() {
            return Err(invalid("XQ FFN trunk differs from its hidden width"));
        }
        let live = self.live_tensor_bytes(tokens)?;
        self.device
            .limits()
            .checked_state_bytes(live, "XQ complete shifted FFN live tensors")?;
        let prepared = self.mhc.prepare(continuation, cancellation)?;
        let normalized = self.norm.forward(prepared.input(), cancellation)?;
        let branch = self.moe.forward(&normalized, image_mask, cancellation)?;
        prepared.finish(&branch, cancellation)
    }
}

impl MoePlan {
    fn bind(
        weights: &XqWeights,
        layer: usize,
        device: &CudaLayerDevice,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        check_cancel(cancellation)?;
        if layer >= 40 {
            return Err(invalid("XQ MoE layer requires a backbone index 0..39"));
        }
        if EXPERTS * 3 * 1024 > device.limits().max_input_bytes {
            return Err(invalid(
                "XQ expert binding metadata exceeds its host-byte budget",
            ));
        }
        let layout = XqExpertLayout::load(weights, layer, cancellation)?;
        let prefix = format!("blk.{layer}");
        let mut experts = Vec::with_capacity(EXPERTS);
        let mut stacks = BTreeMap::new();
        for expert in 0..EXPERTS {
            let row = |role| -> Result<MatrixRows> {
                let p = layout.projection(expert, role)?;
                Ok(MatrixRows {
                    stack: p.tensor_name().into(),
                    kind: p.kind(),
                    first: usize::try_from(p.first_row())
                        .map_err(|_| invalid("XQ expert slot offset exceeds usize"))?,
                    rows: usize::try_from(p.rows())
                        .map_err(|_| invalid("XQ expert rows exceed usize"))?,
                    columns: usize::try_from(p.columns())
                        .map_err(|_| invalid("XQ expert columns exceed usize"))?,
                })
            };
            let projections = [
                row(XqExpertRole::Gate)?,
                row(XqExpertRole::Up)?,
                row(XqExpertRole::Down)?,
            ];
            for projection in &projections {
                let descriptor = weights.tensor(&projection.stack)?;
                let row_bytes = descriptor.kind.row_bytes(descriptor.shape[0])?;
                let spec = StackSpec {
                    kind: descriptor.kind,
                    rows: usize::try_from(descriptor.bytes / row_bytes)
                        .map_err(|_| invalid("XQ stack row count exceeds usize"))?,
                    columns: projection.columns,
                };
                if let Some(old) = stacks.insert(projection.stack.clone(), spec) {
                    if old != spec {
                        return Err(invalid("XQ expert stack has inconsistent bindings"));
                    }
                }
            }
            experts.push(projections);
        }
        let mut resident_bytes = 0;
        for name in stacks.keys() {
            resident_bytes = add(resident_bytes, weights.tensor(name)?.bytes)?;
        }
        let expected = [
            (
                "ffn_gate_inp.weight",
                XqTensorType::F32,
                vec![HIDDEN as u64, EXPERTS as u64],
            ),
            ("exp_probs_b.bias", XqTensorType::F32, vec![EXPERTS as u64]),
            (
                "exp_probs_b_vl.bias",
                XqTensorType::F32,
                vec![EXPERTS as u64],
            ),
            (
                "ffn_gate_shexp.weight",
                XqTensorType::Q8_0,
                vec![HIDDEN as u64, INTERMEDIATE as u64],
            ),
            (
                "ffn_up_shexp.weight",
                XqTensorType::Q8_0,
                vec![HIDDEN as u64, INTERMEDIATE as u64],
            ),
            (
                "ffn_down_shexp.weight",
                XqTensorType::Q8_0,
                vec![INTERMEDIATE as u64, HIDDEN as u64],
            ),
        ];
        for (suffix, kind, shape) in expected {
            let tensor = weights.tensor(&format!("{prefix}.{suffix}"))?;
            if tensor.kind != kind || tensor.shape != shape {
                return Err(invalid(
                    "XQ router/shared parameter differs from its exact exported schema",
                ));
            }
            resident_bytes = add(resident_bytes, tensor.bytes)?;
        }
        if resident_bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "complete XQ MoE exceeds its pre-upload resident byte budget",
            ));
        }
        Ok(Self {
            prefix,
            experts,
            stacks,
            resident_bytes,
        })
    }
}

impl XqMoeBlock {
    pub fn load(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        layer: usize,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let plan = MoePlan::bind(weights, layer, device, cancellation)?;
        Self::load_plan(device, weights, plan, cancellation)
    }

    fn load_plan(
        device: &CudaLayerDevice,
        weights: &XqWeights,
        plan: MoePlan,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let load = |suffix: &str, rows| {
            XqDeviceMatrix::load(
                device,
                weights,
                &format!("{}.{suffix}", plan.prefix),
                0,
                rows,
                cancellation,
            )
        };
        let router = load("ffn_gate_inp.weight", EXPERTS)?;
        let bias = |suffix: &str| {
            weights.load_f32_parameter(
                device,
                &format!("{}.{suffix}", plan.prefix),
                XqTensorType::F32,
                &[EXPERTS as u64],
                cancellation,
            )
        };
        let text_bias = bias("exp_probs_b.bias")?;
        let image_bias = bias("exp_probs_b_vl.bias")?;
        let shared = [
            load("ffn_gate_shexp.weight", INTERMEDIATE)?,
            load("ffn_up_shexp.weight", INTERMEDIATE)?,
            load("ffn_down_shexp.weight", HIDDEN)?,
        ];
        let mut stacks = BTreeMap::new();
        for (name, spec) in &plan.stacks {
            check_cancel(cancellation)?;
            let matrix = XqDeviceMatrix::load(device, weights, name, 0, spec.rows, cancellation)?;
            if matrix.kind() != spec.kind || matrix.logical_shape() != [spec.rows, spec.columns] {
                return Err(invalid("loaded XQ stack differs from its admitted plan"));
            }
            stacks.insert(name.clone(), matrix);
        }
        let block = Self::from_parts(
            device.clone(),
            router,
            text_bias,
            image_bias,
            shared,
            stacks,
            plan.experts,
            TOPK,
        )?;
        if block.resident_bytes != plan.resident_bytes {
            return Err(invalid(
                "XQ MoE retained bytes differ from pre-upload admission",
            ));
        }
        check_cancel(cancellation)?;
        Ok(block)
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        device: CudaLayerDevice,
        router: XqDeviceMatrix,
        text_bias: Tensor,
        image_bias: Tensor,
        shared: [XqDeviceMatrix; 3],
        stacks: BTreeMap<String, XqDeviceMatrix>,
        experts: Vec<[MatrixRows; 3]>,
        topk: usize,
    ) -> Result<Self> {
        let [count, hidden] = router.logical_shape();
        let [intermediate, shared_hidden] = shared[0].logical_shape();
        if count == 0
            || count > EXPERTS
            || hidden == 0
            || hidden > HIDDEN
            || intermediate == 0
            || intermediate > INTERMEDIATE
            || router.kind() != XqTensorType::F32
            || experts.len() != count
            || hidden != shared_hidden
            || shared[1].logical_shape() != [intermediate, hidden]
            || shared[2].logical_shape() != [hidden, intermediate]
            || shared.iter().any(|m| m.kind() != XqTensorType::Q8_0)
        {
            return Err(invalid("XQ MoE requires F32 router, Q8 shared projections and complete mixed-expert hidden geometry"));
        }
        validate_ranges(&experts, &stacks, hidden, intermediate)?;
        let mut bytes = (count as u64) * 8; // two exact F32 bias vectors
        for matrix in std::iter::once(&router)
            .chain(shared.iter())
            .chain(stacks.values())
        {
            if !matrix
                .device()
                .tensor_device()
                .same_device(device.tensor_device())
                || matrix.device().limits() != device.limits()
            {
                return Err(invalid(
                    "XQ MoE projections differ from the owning device/limits",
                ));
            }
            bytes = add(bytes, matrix.retained_weight_bytes())?;
        }
        if bytes > device.limits().max_resident_weight_bytes {
            return Err(invalid(
                "XQ MoE exceeds its aggregate resident weight budget",
            ));
        }
        if text_bias.dims() != [count] || image_bias.dims() != [count] {
            return Err(invalid("XQ MoE biases differ from router expert count"));
        }
        let selection = MoeRouteSelection::new(device.clone(), text_bias, image_bias, topk, 1.5)?;
        Ok(Self {
            device,
            router,
            selection,
            shared,
            stacks,
            experts,
            hidden,
            intermediate,
            topk,
            resident_bytes: bytes,
        })
    }

    pub fn retained_bytes(&self) -> u64 {
        self.resident_bytes
    }
    pub fn hidden_size(&self) -> usize {
        self.hidden
    }
    pub fn expert_count(&self) -> usize {
        self.experts.len()
    }
    pub(crate) fn device(&self) -> &CudaLayerDevice {
        &self.device
    }

    pub fn forward(
        &self,
        input: &Tensor,
        image_mask: Option<&Tensor>,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        check_cancel(cancellation)?;
        let (tokens, hidden) = input.dims2().map_err(tensor_error)?;
        if input.dtype() != DType::F32 || hidden != self.hidden || tokens == 0 {
            return Err(invalid("XQ MoE requires matching nonempty F32 hidden rows"));
        }
        same_device(&self.device, input)?;
        self.live_tensor_bytes(tokens)?;
        let (logits, _) = self.router.forward_f32_control(input, cancellation)?;
        let scores = score_math(&logits)?;
        let routes = self.selection.select_scores(&scores, image_mask)?;
        check_cancel(cancellation)?;
        // Only bounded discrete routing metadata crosses to the host. All
        // activations, original weights, dot products and sums stay on CUDA.
        let logical = routes
            .logical_experts()
            .flatten_all()
            .and_then(|x| x.to_vec1::<u32>())
            .map_err(tensor_error)?;
        let groups = group_routes(&logical, tokens, self.topk, self.experts.len())?;
        let weights = routes.weights().flatten_all().map_err(tensor_error)?;
        let mut slots = Tensor::zeros((tokens * self.topk, hidden), DType::F32, input.device())
            .map_err(tensor_error)?;
        for (expert, entries) in self.experts.iter().zip(groups) {
            if entries.is_empty() {
                continue;
            }
            check_cancel(cancellation)?;
            let ids = Tensor::from_vec(
                entries.iter().map(|x| x.0).collect::<Vec<_>>(),
                entries.len(),
                input.device(),
            )
            .map_err(tensor_error)?;
            let slot_ids = Tensor::from_vec(
                entries.iter().map(|x| x.1).collect::<Vec<_>>(),
                entries.len(),
                input.device(),
            )
            .map_err(tensor_error)?;
            let rows = input.index_select(&ids, 0).map_err(tensor_error)?;
            let output = execute_expert(
                &rows,
                |role, value| self.project_range(&expert[role], value, cancellation),
                cancellation,
            )?;
            let route_weights = weights
                .index_select(&slot_ids, 0)
                .and_then(|x| x.unsqueeze(1))
                .map_err(tensor_error)?;
            // XQ weights are applied to down's F32 result, not its input.
            let weighted = output.broadcast_mul(&route_weights).map_err(tensor_error)?;
            validate_finite(&weighted, false, "XQ weighted routed output")?;
            slots = slots
                .index_add(&slot_ids, &weighted, 0)
                .map_err(tensor_error)?;
        }
        let slots = slots
            .reshape((tokens, self.topk, hidden))
            .map_err(tensor_error)?;
        let shared = execute_expert(
            input,
            |role, value| {
                self.shared[role]
                    .forward_f32_control(value, cancellation)
                    .map(|x| x.0)
            },
            cancellation,
        )?;
        let output = combine(&slots, &shared, cancellation)?;
        check_cancel(cancellation)?;
        Ok(output)
    }

    fn project_range(
        &self,
        rows: &MatrixRows,
        input: &Tensor,
        cancellation: &CancellationToken,
    ) -> Result<Tensor> {
        let matrix = self
            .stacks
            .get(&rows.stack)
            .ok_or_else(|| invalid("XQ expert references an absent resident stack"))?;
        matrix
            .forward_rows_f32_control(input, rows.first, rows.rows, cancellation)
            .map(|x| x.0)
    }

    pub(crate) fn live_tensor_bytes(&self, tokens: usize) -> Result<u64> {
        let limits = self.device.limits();
        let input = limits.checked_elements(&[tokens, self.hidden], "XQ MoE hidden")? as u64;
        let inner =
            limits.checked_elements(&[tokens, self.intermediate], "XQ MoE intermediate")? as u64;
        let slots = limits
            .checked_elements(&[tokens, self.topk, self.hidden], "XQ routed F32 slots")?
            as u64;
        let route_count =
            limits.checked_elements(&[tokens, self.topk], "XQ logical routes")? as u64;
        let scores =
            limits.checked_elements(&[tokens, self.experts.len()], "XQ router scores")? as u64;
        if route_count > u32::MAX as u64 {
            return Err(invalid("XQ route slot IDs exceed U32"));
        }
        let metadata = route_count
            .checked_mul(48)
            .and_then(|bytes| bytes.checked_add((self.experts.len() * 3 * 256) as u64))
            .ok_or_else(|| invalid("XQ route metadata bytes overflowed"))?;
        if metadata > limits.max_input_bytes as u64 {
            return Err(invalid("XQ route scheduling metadata exceeds host budget"));
        }
        let largest = self
            .experts
            .iter()
            .flatten()
            .map(|p| {
                p.kind.row_bytes(p.columns as u64).and_then(|b| {
                    b.checked_mul(p.rows as u64)
                        .ok_or_else(|| invalid("XQ expert projection bytes overflowed"))
                })
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .chain(
                self.shared
                    .iter()
                    .map(XqDeviceMatrix::retained_weight_bytes),
            )
            .chain(std::iter::once(self.router.retained_weight_bytes()))
            .max()
            .unwrap_or(0);
        let mut bytes = add(
            metadata,
            largest
                .checked_mul(2)
                .ok_or_else(|| invalid("XQ projection live bytes overflowed"))?,
        )?;
        for (count, factor) in [(input, 64), (inner, 128), (slots, 16), (scores, 224)] {
            bytes = add(
                bytes,
                count
                    .checked_mul(factor)
                    .ok_or_else(|| invalid("XQ MoE live tensor bytes overflowed"))?,
            )?;
        }
        limits.checked_state_bytes(bytes, "XQ F32 MoE composite live tensors")?;
        Ok(bytes)
    }
}

fn validate_ranges(
    experts: &[[MatrixRows; 3]],
    stacks: &BTreeMap<String, XqDeviceMatrix>,
    hidden: usize,
    intermediate: usize,
) -> Result<()> {
    if hidden == 0 || intermediate == 0 {
        return Err(invalid(
            "XQ expert ranges require positive hidden/intermediate widths",
        ));
    }
    let mut coverage: BTreeMap<&str, (usize, usize, BTreeSet<usize>)> = BTreeMap::new();
    for expert in experts {
        if expert[0].kind != expert[1].kind {
            return Err(invalid(
                "XQ gate/up expert formats must share the authenticated type map",
            ));
        }
        for (role, projection) in expert.iter().enumerate() {
            if !matches!(
                projection.kind,
                XqTensorType::Q2_0 | XqTensorType::Q3K | XqTensorType::Ptq1_0
            ) {
                return Err(invalid(
                    "XQ routed expert format is outside Q2_0/Q3_K/PTQ1_0",
                ));
            }
            let (rows, columns) = if role == 2 {
                (hidden, intermediate)
            } else {
                (intermediate, hidden)
            };
            let matrix = stacks
                .get(&projection.stack)
                .ok_or_else(|| invalid("XQ expert type partition has no resident owner"))?;
            let end = projection
                .first
                .checked_add(rows)
                .ok_or_else(|| invalid("XQ expert row boundary overflowed"))?;
            if projection.rows != rows
                || projection.columns != columns
                || projection.kind != matrix.kind()
                || matrix.logical_shape()[1] != columns
                || !projection.first.is_multiple_of(rows)
                || end > matrix.logical_shape()[0]
            {
                return Err(invalid(
                    "XQ expert rows differ from their exact typed stack geometry",
                ));
            }
            let covered = coverage
                .entry(&projection.stack)
                .or_insert_with(|| (role, rows, BTreeSet::new()));
            if covered.0 != role || !covered.2.insert(projection.first) {
                return Err(invalid(
                    "XQ physical expert rows are duplicated or cross projection roles",
                ));
            }
        }
    }
    if coverage.len() != stacks.len() {
        return Err(invalid("XQ resident stacks include an unbound allocation"));
    }
    for (name, (_, rows, starts)) in coverage {
        let matrix = stacks
            .get(name)
            .ok_or_else(|| invalid("XQ bound stack disappeared"))?;
        if starts
            .iter()
            .enumerate()
            .any(|(slot, first)| *first != slot * rows)
            || starts.len().checked_mul(rows) != Some(matrix.logical_shape()[0])
        {
            return Err(invalid(
                "XQ expert slots do not cover their resident stack exactly",
            ));
        }
    }
    Ok(())
}

fn execute_expert<F>(
    input: &Tensor,
    mut project: F,
    cancellation: &CancellationToken,
) -> Result<Tensor>
where
    F: FnMut(usize, &Tensor) -> Result<Tensor>,
{
    check_cancel(cancellation)?;
    let gate = project(0, input)?;
    let up = project(1, input)?;
    let activated = swiglu(&gate, &up)?;
    check_cancel(cancellation)?;
    project(2, &activated)
}

fn swiglu(gate: &Tensor, up: &Tensor) -> Result<Tensor> {
    if gate.dtype() != DType::F32
        || up.dtype() != DType::F32
        || gate.shape() != up.shape()
        || gate.rank() != 2
        || gate.elem_count() == 0
        || !gate.device().same_device(up.device())
    {
        return Err(invalid(
            "XQ SwiGLU requires matching nonempty F32 gate/up rows",
        ));
    }
    validate_finite(gate, false, "XQ gate projection")?;
    validate_finite(up, false, "XQ up projection")?;
    let gate = gate
        .clamp(f32::NEG_INFINITY, 10_f32)
        .map_err(tensor_error)?;
    let up = up.clamp(-10_f32, 10_f32).map_err(tensor_error)?;
    let output = gate
        .mul(&candle_nn::ops::sigmoid(&gate).map_err(tensor_error)?)
        .and_then(|x| x.mul(&up))
        .map_err(tensor_error)?
        .detach();
    validate_finite(&output, false, "XQ F32 SwiGLU")?;
    Ok(output)
}

fn combine(slots: &Tensor, shared: &Tensor, cancellation: &CancellationToken) -> Result<Tensor> {
    let (tokens, lanes, hidden) = slots.dims3().map_err(tensor_error)?;
    if tokens == 0
        || !(1..=TOPK).contains(&lanes)
        || slots.dtype() != DType::F32
        || shared.dtype() != DType::F32
        || shared.dims() != [tokens, hidden]
        || !shared.device().same_device(slots.device())
    {
        return Err(invalid(
            "XQ combine requires F32 routed lanes and matching shared output",
        ));
    }
    validate_finite(slots, false, "XQ routed output lanes")?;
    validate_finite(shared, false, "XQ shared output")?;
    let mut routed =
        Tensor::zeros((tokens, hidden), DType::F32, slots.device()).map_err(tensor_error)?;
    for lane in 0..lanes {
        check_cancel(cancellation)?;
        routed = routed
            .add(
                &slots
                    .narrow(1, lane, 1)
                    .and_then(|x| x.squeeze(1))
                    .map_err(tensor_error)?,
            )
            .map_err(tensor_error)?;
    }
    let output = shared.add(&routed).map_err(tensor_error)?.detach();
    validate_finite(&output, false, "XQ complete MoE output")?;
    check_cancel(cancellation)?;
    Ok(output)
}

fn add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| invalid("XQ MoE byte accounting overflowed"))
}

#[cfg(test)]
pub(crate) mod tests;
