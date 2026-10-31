use super::*;
use crate::layers::{
    attention::{
        branch::tests::{build, plan},
        test_support::{bf16, floats},
    },
    kv::MainKvFormat,
    mhc::{MhcNumerics, MhcParameters},
};
use crate::state::kv::{KvCacheKind, KvState};
use aster_power::{error::PowerError, inference::InferenceLimits};
use candle_core::DType;

pub(crate) fn build_layer(limits: InferenceLimits) -> AttentionSublayer {
    try_build_layer(limits).unwrap()
}

fn try_build_layer(limits: InferenceLimits) -> Result<AttentionSublayer> {
    let device = CudaLayerDevice::cpu_for_tests(limits);
    let mhc = MhcBlock::new(
        device.clone(),
        MhcParameters {
            norm_fused_projection: Tensor::zeros((24, 128), DType::F32, device.tensor_device())
                .unwrap(),
            scale: Tensor::zeros(3, DType::F32, device.tensor_device()).unwrap(),
            base: Tensor::zeros(24, DType::F32, device.tensor_device()).unwrap(),
        },
        MhcNumerics {
            norm_epsilon: 1e-20,
            pre_epsilon: 1e-6,
            sinkhorn_epsilon: 1e-6,
            sinkhorn_iterations: 20,
            post_multiplier: 2.,
        },
    )
    .unwrap();
    let norm =
        EffectiveRmsNorm::new(device.clone(), 32, 1e-20, Some(bf16(&[1.; 32], &[32]))).unwrap();
    AttentionSublayer::new(device.clone(), mhc, norm, build(&device))
}

pub(crate) fn initial(owner: &AttentionSublayer, tokens: usize) -> MhcContinuation {
    let initial = owner
        .mhc
        .initial_shifted(&bf16(&vec![1.; tokens * 32], &[tokens, 32]))
        .unwrap();
    let trunk = [vec![1.; 32], vec![-1.; 32], vec![2.; 32], vec![-2.; 32]]
        .concat()
        .repeat(tokens);
    initial
        .with_residual(&bf16(&trunk, &[tokens, 4, 32]))
        .unwrap()
}

#[test]
fn complete_attention_half_uses_previous_mix_and_matches_prefill_decode_residual_golden() {
    let owner = build_layer(InferenceLimits::default());
    let device = &owner.device;
    let cancel = CancellationToken::new();
    let prepared = owner
        .prepare(initial(&owner, 3), &[0, 1, 2], &cancel)
        .unwrap();
    // Own pre mix is [0.5;4], which would reduce this trunk to zero. The
    // carried initial mix selects its first stream, producing ones instead.
    assert_eq!(floats(prepared.current_hidden()), vec![1.; 96]);
    let window = MainKvRows::from_records(
        device,
        MainKvFormat::Fp8Ue8m0,
        prepared.projected().window_rows().records(),
    )
    .unwrap();
    let output = owner
        .finish(
            prepared,
            &window,
            None,
            &plan(device, &[0, 1, 2], &[0, 1, 2]),
            &cancel,
        )
        .unwrap();
    let golden = [vec![512.; 128], vec![704.; 128], vec![768.; 128]].concat();
    assert_eq!(floats(output.residual()), golden);
    assert!(floats(output.next_pre_mix())
        .iter()
        .all(|v| (*v - 0.500001).abs() < 1e-7));
    let mut state =
        KvState::new(device, KvCacheKind::Window, MainKvFormat::Fp8Ue8m0, 4, 4).unwrap();
    let mut decoded = Vec::new();
    for position in 0..3 {
        let prepared = owner
            .prepare(initial(&owner, 1), &[position], &cancel)
            .unwrap();
        let proposal = state
            .prepare(
                position,
                1,
                &[position],
                prepared.projected().window_rows().records(),
                &cancel,
            )
            .unwrap();
        let keys: Vec<_> = proposal.ready_positions().collect();
        let window = proposal.attention_rows(device).unwrap();
        let out = owner
            .finish(
                prepared,
                &window,
                None,
                &plan(device, &[position], &keys),
                &cancel,
            )
            .unwrap();
        decoded.extend(floats(out.residual()));
        state.commit_prefix(proposal, 1, &cancel).unwrap();
    }
    assert_eq!(decoded, golden);
}

#[test]
fn foreign_owner_and_cancelled_half_cannot_publish_prepared_window_state() {
    let owner = build_layer(InferenceLimits::default());
    let other = build_layer(InferenceLimits::default());
    let device = &owner.device;
    let cancel = CancellationToken::new();
    let state = KvState::new(device, KvCacheKind::Window, MainKvFormat::Fp8Ue8m0, 4, 4).unwrap();
    for foreign in [false, true] {
        let prepared = owner.prepare(initial(&owner, 1), &[0], &cancel).unwrap();
        let proposal = state
            .prepare(
                0,
                1,
                &[0],
                prepared.projected().window_rows().records(),
                &cancel,
            )
            .unwrap();
        let window = proposal.attention_rows(device).unwrap();
        let stop = CancellationToken::new();
        if !foreign {
            stop.cancel();
        }
        let result = if foreign {
            other.finish(prepared, &window, None, &plan(device, &[0], &[0]), &stop)
        } else {
            owner.finish(prepared, &window, None, &plan(device, &[0], &[0]), &stop)
        };
        assert!(result.is_err());
        if !foreign {
            assert!(matches!(result, Err(PowerError::InferenceCancelled)));
        }
        drop(proposal);
        assert_eq!(state.position(), 0);
    }
    assert!(owner.prepare(initial(&owner, 2), &[0], &cancel).is_err());
}

#[test]
fn attention_half_enforces_composite_working_set_and_resident_parameter_budgets() {
    let unbounded = build_layer(InferenceLimits::default());
    let plan = plan(&unbounded.device, &[0, 1, 2], &[0, 1, 2]);
    let native_peak = unbounded.attention.live_tensor_bytes(3, &plan).unwrap();
    let owner = build_layer(InferenceLimits {
        max_state_bytes: native_peak,
        ..InferenceLimits::default()
    });
    let cancel = CancellationToken::new();
    let prepared = owner
        .prepare(initial(&owner, 3), &[0, 1, 2], &cancel)
        .unwrap();
    let window = MainKvRows::from_records(
        &owner.device,
        MainKvFormat::Fp8Ue8m0,
        prepared.projected().window_rows().records(),
    )
    .unwrap();
    assert!(owner
        .finish(prepared, &window, None, &plan, &cancel)
        .is_err());
    let owner = build_layer(InferenceLimits::default());
    // Every constituent fits, but the complete half exceeds this limit.
    assert!(try_build_layer(InferenceLimits {
        max_resident_weight_bytes: owner.retained_bytes() - 1,
        ..InferenceLimits::default()
    })
    .is_err());
}
