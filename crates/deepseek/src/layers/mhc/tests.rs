use super::*;
use aster_power::inference::{DevicePreference, InferenceLimits, RuntimeDevice};
use candle_core::Device;

fn numerics() -> MhcNumerics {
    MhcNumerics {
        norm_epsilon: 1e-6,
        pre_epsilon: 1e-6,
        sinkhorn_epsilon: 1e-6,
        sinkhorn_iterations: 20,
        post_multiplier: 2.0,
    }
}

fn parameters() -> MhcParameters {
    let device = &Device::Cpu;
    let mut base = vec![0_f32; MIXES];
    base[..8].copy_from_slice(&[
        0.,
        3_f32.ln(),
        -3_f32.ln(),
        0.,
        0.,
        3_f32.ln(),
        -3_f32.ln(),
        0.,
    ]);
    base[8..].copy_from_slice(&[
        0.2, -0.5, 1., 0.3, 0.1, 0.4, -0.2, 1.3, 0.8, -0.1, 0.5, -0.7, 0.3, 0.9, -0.4, 0.2,
    ]);
    MhcParameters {
        norm_fused_projection: Tensor::zeros((MIXES, 8), DType::F32, device).unwrap(),
        scale: Tensor::new(&[0_f32, 0., 1.], device).unwrap(),
        base: Tensor::from_vec(base, MIXES, device).unwrap(),
    }
}

fn residual() -> Tensor {
    Tensor::from_vec(
        (1..=8).map(|value| value as f32).collect::<Vec<_>>(),
        (1, 4, 2),
        &Device::Cpu,
    )
    .unwrap()
    .to_dtype(DType::BF16)
    .unwrap()
}

fn values(tensor: &Tensor) -> Vec<f32> {
    tensor
        .to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1::<f32>()
        .unwrap()
}

#[test]
fn public_device_boundary_refuses_cpu_without_fallback() {
    let device = RuntimeDevice::resolve(DevicePreference::Cpu).unwrap();
    assert!(CudaLayerDevice::new(device, InferenceLimits::default()).is_err());
}

#[test]
fn mix_and_sinkhorn_match_independent_scalar_golden() {
    let step = pre_math(residual(), &parameters(), numerics()).unwrap();
    assert_eq!(values(step.input()), vec![7.5, 9.5]);
    let expected: Vec<f32> =
        serde_json::from_slice(include_bytes!("../../../fixtures/mhc-golden.json")).unwrap();
    let actual = values(&step.residual_mix);
    for (actual, expected) in actual.iter().zip(&expected) {
        assert!((actual - expected).abs() < 2e-6, "{actual} vs {expected}");
    }
    // An asymmetric mix proves source/destination orientation at residual update.
    let branch = Tensor::new(&[[0.25_f32, -0.5]], &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let output = values(&step.finish(&branch).unwrap());
    let input = [1_f32, 2., 3., 4., 5., 6., 7., 8.];
    let post = [1_f32, 1.5, 0.5, 1.];
    for destination in 0..4 {
        for channel in 0..2 {
            let scalar: f32 = (0..4)
                .map(|source| expected[source * 4 + destination] * input[source * 2 + channel])
                .sum::<f32>()
                + post[destination] * [0.25, -0.5][channel];
            assert!((output[destination * 2 + channel] - scalar).abs() < 0.032);
        }
    }
}

#[test]
fn prenorm_projection_uses_all_four_streams_before_mixing() {
    let mut parameters = parameters();
    parameters.norm_fused_projection = Tensor::ones((MIXES, 8), DType::F32, &Device::Cpu).unwrap();
    parameters.scale = Tensor::new(&[1_f32, 0., 0.], &Device::Cpu).unwrap();
    parameters.base = Tensor::zeros(MIXES, DType::F32, &Device::Cpu).unwrap();
    let step = pre_math(residual(), &parameters, numerics()).unwrap();
    let projected = 36_f64 / (25.5_f64 + 1e-6).sqrt();
    let mix = 1.0 / (1.0 + (-projected).exp()) + 1e-6;
    for (actual, scalar) in values(step.input()).iter().zip([16.0 * mix, 20.0 * mix]) {
        assert!((*actual as f64 - scalar).abs() < 0.07);
    }
}

#[test]
fn batched_tokens_equal_individual_decode_without_mutating_residual() {
    let one = residual();
    let before = values(&one);
    let batch = Tensor::cat(&[&one, &one, &one], 0).unwrap();
    let step = pre_math(batch, &parameters(), numerics()).unwrap();
    let expected = values(
        pre_math(one.clone(), &parameters(), numerics())
            .unwrap()
            .input(),
    );
    assert_eq!(values(step.input()), expected.repeat(3));
    assert_eq!(values(&one), before);
}

#[test]
fn invalid_shapes_dtypes_and_finish_shape_refuse() {
    let params = parameters();
    let wrong = residual().to_dtype(DType::F32).unwrap();
    assert!(pre_math(wrong, &params, numerics()).is_err());
    let wrong = Tensor::zeros((1, 2, 4), DType::BF16, &Device::Cpu).unwrap();
    assert!(pre_math(wrong, &params, numerics()).is_err());
    let wrong = Tensor::zeros((1, 4, 3), DType::BF16, &Device::Cpu).unwrap();
    assert!(pre_math(wrong, &params, numerics()).is_err());
    let mut params = parameters();
    params.scale = Tensor::zeros(4, DType::F32, &Device::Cpu).unwrap();
    assert!(pre_math(residual(), &params, numerics()).is_err());
    let step = pre_math(residual(), &parameters(), numerics()).unwrap();
    assert!(step
        .finish(&Tensor::zeros((1, 3), DType::BF16, &Device::Cpu).unwrap())
        .is_err());
    let step = pre_math(residual(), &parameters(), numerics()).unwrap();
    assert!(step
        .finish(&Tensor::zeros((1, 2), DType::F32, &Device::Cpu).unwrap())
        .is_err());
}

#[test]
fn numeric_parameters_refuse_nan_overflow_underflow_and_unbounded_iterations() {
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX, 1e-300] {
        let mut config = numerics();
        config.norm_epsilon = value;
        assert!(pre_math(residual(), &parameters(), config).is_err());
    }
    for iterations in [0, 65, usize::MAX] {
        let mut config = numerics();
        config.sinkhorn_iterations = iterations;
        assert!(pre_math(residual(), &parameters(), config).is_err());
    }
}

#[test]
fn shifted_initial_streams_replicate_embedding_and_apply_one_hot_first_mix() {
    let embedding = Tensor::new(&[[1_f32, 2.], [3., 4.]], &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let initial = initial_math(&embedding).unwrap();
    assert_eq!(initial.residual().dims(), &[2, 4, 2]);
    assert_eq!(
        values(initial.residual()),
        vec![1., 2., 1., 2., 1., 2., 1., 2., 3., 4., 3., 4., 3., 4., 3., 4.]
    );
    assert_eq!(
        values(initial.next_pre_mix()),
        vec![1., 0., 0., 0., 1., 0., 0., 0.]
    );
    assert_eq!(values(&initial.head_input().unwrap()), values(&embedding));
}

#[test]
fn shifted_input_uses_previous_mix_while_current_mix_is_carried_forward() {
    let previous = Tensor::new(&[0_f32, 1., 0., 0.], &Device::Cpu)
        .unwrap()
        .reshape((1, 4, 1))
        .unwrap();
    let first =
        pre_math_with_previous(residual(), &parameters(), numerics(), Some(&previous)).unwrap();
    assert_eq!(values(first.input()), vec![3., 4.]);
    let expected = [0.500001_f32, 0.750001, 0.250001, 0.500001];
    for (actual, expected) in values(first.next_pre_mix()).iter().zip(expected) {
        assert!((actual - expected).abs() < 1e-6);
    }
    let mut changed = parameters();
    changed.base = changed.base.affine(1., 2.).unwrap();
    let second = pre_math_with_previous(residual(), &changed, numerics(), Some(&previous)).unwrap();
    assert_eq!(values(second.input()), values(first.input()));
    assert_ne!(values(second.next_pre_mix()), values(first.next_pre_mix()));
}

#[test]
fn two_shifted_sublayers_and_final_head_apply_exactly_one_half_layer_delay() {
    let embedding = Tensor::new(&[[1_f32, 2.]], &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let initial = initial_math(&embedding).unwrap();
    let first = pre_math_with_previous(
        initial.residual,
        &parameters(),
        numerics(),
        Some(&initial.next_pre_mix),
    )
    .unwrap();
    assert_eq!(values(first.input()), vec![1., 2.]);
    let branch = Tensor::new(&[[0.25_f32, -0.5]], &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let after_first = first.finish_shifted(&branch).unwrap();
    let trunk = values(after_first.residual());
    let prior = values(after_first.next_pre_mix());
    let mut next_parameters = parameters();
    next_parameters.base = next_parameters.base.affine(1., 1.).unwrap();
    let second = pre_math_with_previous(
        after_first.residual,
        &next_parameters,
        numerics(),
        Some(&after_first.next_pre_mix),
    )
    .unwrap();
    for feature in 0..2 {
        let expected: f64 = (0..4)
            .map(|head| f64::from(trunk[head * 2 + feature]) * f64::from(prior[head]))
            .sum();
        assert!((f64::from(values(second.input())[feature]) - expected).abs() < 0.035);
    }
    let own_mix = values(second.next_pre_mix());
    assert_ne!(own_mix, prior);
    let after_second = second.finish_shifted(&branch).unwrap();
    let final_trunk = values(after_second.residual());
    let output = values(&after_second.head_input().unwrap());
    for feature in 0..2 {
        let expected: f64 = (0..4)
            .map(|head| f64::from(final_trunk[head * 2 + feature]) * f64::from(own_mix[head]))
            .sum();
        assert!((f64::from(output[feature]) - expected).abs() < 0.035);
    }
}

#[test]
fn mhc_parameter_and_residual_snapshots_do_not_follow_mutable_storage() {
    use candle_core::Var;
    let original = residual();
    let variable = Var::from_tensor(&original).unwrap();
    let step = pre_math(variable.as_tensor().clone(), &parameters(), numerics()).unwrap();
    variable
        .set(&Tensor::zeros((1, 4, 2), DType::BF16, &Device::Cpu).unwrap())
        .unwrap();
    let expected = pre_math(residual(), &parameters(), numerics()).unwrap();
    let branch = Tensor::zeros((1, 2), DType::BF16, &Device::Cpu).unwrap();
    assert_eq!(
        values(&step.finish(&branch).unwrap()),
        values(&expected.finish(&branch).unwrap())
    );
    let mut params = parameters();
    let base = Var::from_tensor(&params.base).unwrap();
    params.base = base.as_tensor().clone();
    let frozen = freeze_parameters(params).unwrap();
    let before = values(&frozen.base);
    base.set(&Tensor::zeros(MIXES, DType::F32, &Device::Cpu).unwrap())
        .unwrap();
    assert_eq!(values(&frozen.base), before);
}

#[test]
fn mhc_rejects_nonfinite_weights_prior_mixes_square_overflow_and_branch_overflow() {
    let params = parameters();
    for previous in [
        Tensor::zeros((1, 3, 1), DType::F32, &Device::Cpu).unwrap(),
        Tensor::full(-1_f32, (1, 4, 1), &Device::Cpu).unwrap(),
        Tensor::full(f32::NAN, (1, 4, 1), &Device::Cpu).unwrap(),
    ] {
        assert!(pre_math_with_previous(residual(), &params, numerics(), Some(&previous)).is_err());
    }
    let mut bad_params = parameters();
    bad_params.base = Tensor::full(f32::NAN, MIXES, &Device::Cpu).unwrap();
    assert!(pre_math(residual(), &bad_params, numerics()).is_err());
    let huge = Tensor::full(1e30_f32, (1, 4, 2), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    assert!(pre_math(huge, &params, numerics()).is_err());
    let bad_branch = Tensor::full(3e38_f32, (1, 2), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    assert!(pre_math(residual(), &params, numerics())
        .unwrap()
        .finish(&bad_branch)
        .is_err());
}

#[test]
fn engram_trunk_intervention_keeps_the_carried_shifted_mix() {
    let embedding = Tensor::new(&[[1_f32, 2.]], &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let initial = initial_math(&embedding).unwrap();
    let mix = values(initial.next_pre_mix());
    let replacement = initial.residual().affine(2., 0.).unwrap();
    let changed = initial.with_residual(&replacement).unwrap();
    assert_eq!(values(changed.next_pre_mix()), mix);
    assert_eq!(values(&changed.head_input().unwrap()), vec![2., 4.]);
    let initial = initial_math(&embedding).unwrap();
    assert!(initial
        .with_residual(&Tensor::zeros((1, 4, 3), DType::BF16, &Device::Cpu).unwrap())
        .is_err());
}
