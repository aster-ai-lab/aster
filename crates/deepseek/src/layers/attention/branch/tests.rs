use super::*;
use crate::layers::{
    attention::{test_support::*, KvCandidate, KvSource},
    norm::EffectiveRmsNorm,
};
use crate::state::kv::{KvCacheKind, KvState};
use aster_power::inference::InferenceLimits;
use candle_core::Device;

fn identity_weights(rows: usize, cols: usize) -> Vec<u8> {
    let mut data = vec![0_u8; rows * cols];
    for r in 0..rows {
        data[r * cols + r % cols] = 56;
    }
    data
}
fn unit_norm(device: &CudaLayerDevice, width: usize) -> EffectiveRmsNorm {
    EffectiveRmsNorm::new(
        device.clone(),
        width,
        1e-20,
        Some(bf16(&vec![1.; width], &[width])),
    )
    .unwrap()
}
fn identity_cache() -> Tensor {
    Tensor::from_vec(
        [vec![1_f32; 32], vec![0_f32; 32]].concat().repeat(4),
        (4, 64),
        &Device::Cpu,
    )
    .unwrap()
}
fn components(
    device: &CudaLayerDevice,
) -> (AttentionInputProjection, AttentionOutputProjection, Tensor) {
    let input = AttentionInputProjection::new(
        device.clone(),
        matrix(device, 32, 32, &identity_weights(32, 32), &[127]),
        matrix(device, 1024, 32, &identity_weights(1024, 32), &[127; 32]),
        matrix(device, 512, 32, &identity_weights(512, 32), &[127; 16]),
        unit_norm(device, 32),
        unit_norm(device, 512),
        identity_cache(),
    )
    .unwrap();
    let mut output = vec![0_u8; 32 * 64];
    for r in 0..32 {
        output[r * 64 + r] = 56;
        output[r * 64 + 32 + r] = 64;
    }
    let output = AttentionOutputProjection::new(
        device.clone(),
        2,
        matrix(
            device,
            64,
            512,
            &vec![56; 64 * 512],
            &[vec![127; 16], vec![126; 16]].concat(),
        ),
        matrix(device, 32, 64, &output, &[127; 2]),
        identity_cache(),
    )
    .unwrap();
    let logit = (512_f64 * (512_f64).powf(-0.5)) as f32;
    (
        input,
        output,
        Tensor::new(&[logit, logit], &Device::Cpu).unwrap(),
    )
}
pub(crate) fn build(device: &CudaLayerDevice) -> NativeAttention {
    let (i, o, s) = components(device);
    NativeAttention::new(i, o, s).unwrap()
}
pub(crate) fn plan(
    device: &CudaLayerDevice,
    positions: &[usize],
    keys: &[usize],
) -> SparseAttentionPlan {
    let candidates: Vec<_> = keys
        .iter()
        .map(|p| KvCandidate::new(KvSource::Window, *p))
        .collect();
    let selections: Vec<Vec<_>> = positions
        .iter()
        .map(|p| {
            keys.iter()
                .enumerate()
                .filter_map(|(i, k)| (*k <= *p).then_some(i))
                .collect()
        })
        .collect();
    SparseAttentionPlan::new(device.limits(), 128, &candidates, positions, &selections).unwrap()
}

#[test]
fn full_projected_attention_matches_independent_uniform_sink_golden_in_prefill_and_decode() {
    let device = device();
    let owner = build(&device);
    let cancel = CancellationToken::new();
    let input = bf16(&[1.; 96], &[3, 32]);
    let projected = owner.project(&input, &[0, 1, 2], &cancel).unwrap();
    let window = MainKvRows::from_records(
        &device,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        projected.window_rows().records(),
    )
    .unwrap();
    let output = owner
        .attend(
            projected,
            &window,
            None,
            &plan(&device, &[0, 1, 2], &[0, 1, 2]),
            &cancel,
        )
        .unwrap();
    // Each key logit equals its FP32 sink. Context is n/(n+1), then BF16
    // rounds and FP8 casts. Two Wv groups use factors 1 and 1/2; Wo adds
    // group0 + 2*group1. Expected rows are 512,704,768 across all features.
    let expected = [vec![512.; 32], vec![704.; 32], vec![768.; 32]].concat();
    assert_eq!(floats(&output), expected);
    let mut state = KvState::new(
        &device,
        KvCacheKind::Window,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        4,
        4,
    )
    .unwrap();
    let mut decoded = Vec::new();
    for row in 0..3 {
        let projected = owner
            .project(&input.narrow(0, row, 1).unwrap(), &[row], &cancel)
            .unwrap();
        let proposal = state
            .prepare(row, 1, &[row], projected.window_rows().records(), &cancel)
            .unwrap();
        let keys: Vec<_> = proposal.ready_positions().collect();
        let view = proposal.attention_rows(&device).unwrap();
        decoded.extend(floats(
            &owner
                .attend(
                    projected,
                    &view,
                    None,
                    &plan(&device, &[row], &keys),
                    &cancel,
                )
                .unwrap(),
        ));
        state.commit_prefix(proposal, 1, &cancel).unwrap();
    }
    assert_eq!(decoded, expected);
    assert_eq!(state.position(), 3);
}

#[test]
fn foreign_projection_changed_positions_different_rotary_cache_and_cancel_refuse() {
    let device = device();
    let owner = build(&device);
    let other = build(&device);
    let cancel = CancellationToken::new();
    let input = bf16(&[1.; 32], &[1, 32]);
    let projected = owner.project(&input, &[0], &cancel).unwrap();
    let window = MainKvRows::from_records(
        &device,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        projected.window_rows().records(),
    )
    .unwrap();
    assert!(other
        .attend(
            projected,
            &window,
            None,
            &plan(&device, &[0], &[0]),
            &cancel
        )
        .is_err());
    let projected = owner.project(&input, &[0], &cancel).unwrap();
    assert!(owner
        .attend(
            projected,
            &window,
            None,
            &plan(&device, &[1], &[0]),
            &cancel
        )
        .is_err());
    let projected = owner.project(&input, &[0], &cancel).unwrap();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(owner
        .attend(
            projected,
            &window,
            None,
            &plan(&device, &[0], &[0]),
            &cancelled
        )
        .is_err());
    let (input, mut output, sink) = components(&device);
    output.rope = crate::layers::attention::projection::main_rope(&device, cache()).unwrap();
    assert!(NativeAttention::new(input, output, sink).is_err());
}

#[test]
fn aggregate_parameters_and_full_attention_working_set_have_joint_bounds() {
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 90_000,
        ..InferenceLimits::default()
    });
    let (i, o, s) = components(&low);
    assert!(NativeAttention::new(i, o, s).is_err());
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 2_000_000,
        ..InferenceLimits::default()
    });
    let owner = build(&low);
    let cancel = CancellationToken::new();
    let input = bf16(&[1.; 96], &[3, 32]);
    let projected = owner.project(&input, &[0, 1, 2], &cancel).unwrap();
    let window = MainKvRows::from_records(
        &low,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        projected.window_rows().records(),
    )
    .unwrap();
    let error = owner
        .attend(
            projected,
            &window,
            None,
            &plan(&low, &[0, 1, 2], &[0, 1, 2]),
            &cancel,
        )
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("native attention composite live tensors"),
        "{error}"
    );
}
