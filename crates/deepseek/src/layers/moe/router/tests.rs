use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::{Device, Var};

fn bf16(rows: &[[f32; 2]]) -> Tensor {
    let flat: Vec<_> = rows.iter().flatten().copied().collect();
    Tensor::from_vec(flat, (rows.len(), 2), &Device::Cpu)
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
        .to_vec1()
        .unwrap()
}
// Test-only CPU arithmetic. Production cannot construct this CUDA device.
fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn router(weight: Tensor) -> MoeRouter {
    let experts = weight.dims()[0];
    MoeRouter::new(
        device(),
        weight,
        Tensor::zeros(experts, DType::F32, &Device::Cpu).unwrap(),
        Tensor::zeros(experts, DType::F32, &Device::Cpu).unwrap(),
        1,
        1.5,
    )
    .unwrap()
}
fn ids(routes: &MoeRoutes) -> Vec<u32> {
    routes
        .logical_experts()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}

#[test]
fn bf16_operands_produce_fp32_logits_without_an_extra_bf16_cast() {
    let weight = bf16(&[[1., 0.], [1., 1. / 256.]]);
    let hidden = bf16(&[[1., 1.]]);
    let logits = project_math(&hidden, &weight).unwrap();
    assert_eq!(logits.dtype(), DType::F32);
    assert_eq!(values(&logits), [1., 1.00390625]);
    assert_eq!(values(&logits.to_dtype(DType::BF16).unwrap()), [1., 1.]);
    let routes = router(weight)
        .forward(&hidden, None, &CancellationToken::new())
        .unwrap();
    assert_eq!(ids(&routes), [1]);
    assert_eq!(values(routes.weights()), [1.5]);
}

#[test]
fn projection_matches_independent_dot_and_chunked_routing_matches_prefill() {
    let weight = bf16(&[[1., 0.5], [-0.25, 2.], [3., -1.]]);
    let hidden = bf16(&[[1., 2.], [-1., 0.5], [0.25, -0.5]]);
    let actual = values(&project_math(&hidden, &weight).unwrap());
    assert_eq!(actual, [2., 3.75, 1., -0.75, 1.25, -3.5, 0., -1.0625, 1.25]);
    let router = router(weight);
    let full = router
        .forward(&hidden, None, &CancellationToken::new())
        .unwrap();
    for row in 0..3 {
        let one = router
            .forward(
                &hidden.narrow(0, row, 1).unwrap(),
                None,
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(ids(&one), ids(&full)[row..row + 1]);
        assert_eq!(values(one.weights()), values(full.weights())[row..row + 1]);
    }
}

#[test]
fn compensated_softplus_preserves_tiny_scores_and_full_fp32_range() {
    let logits = [
        -100_f32,
        -90.,
        -81.,
        -80.,
        -30.,
        -20.,
        -1.,
        0.,
        1.,
        20.,
        21.,
        80.,
        f32::MAX,
        -f32::MAX,
    ];
    let input = Tensor::new(&logits, &Device::Cpu)
        .unwrap()
        .reshape((1, logits.len()))
        .unwrap();
    let actual = values(&score_math(&input).unwrap());
    for (&logit, &actual) in logits.iter().zip(&actual) {
        let expected = if logit > 20. {
            logit.sqrt()
        } else {
            logit.exp().ln_1p().sqrt()
        };
        assert!(
            (actual - expected).abs() <= expected.abs() * 8e-7 + f32::MIN_POSITIVE,
            "logit={logit},actual={actual},expected={expected}"
        );
    }
    assert!(actual[0] > 0.);
    assert!(actual[3] > 0.);
    assert_eq!(actual[13], 0.);
}

#[test]
fn every_finite_bf16_logit_matches_scalar_fp32_softplus_within_four_ulps() {
    let logits: Vec<_> = (0..=u16::MAX)
        .map(|bits| f32::from_bits(u32::from(bits) << 16))
        .filter(|value| value.is_finite())
        .collect();
    assert_eq!(logits.len(), 65_280);
    let input = Tensor::from_vec(logits.clone(), (510, 128), &Device::Cpu).unwrap();
    for (&logit, actual) in logits.iter().zip(values(&score_math(&input).unwrap())) {
        let expected = if logit > 20. {
            logit.sqrt()
        } else {
            logit.exp().ln_1p().sqrt()
        };
        assert!(
            actual.to_bits().abs_diff(expected.to_bits()) <= 4,
            "logit={logit},actual={actual},expected={expected}"
        );
    }
}

#[test]
fn frozen_router_weights_and_biases_ignore_later_variable_mutations() {
    let weight = Var::from_tensor(&bf16(&[[1., 0.], [0., 1.]])).unwrap();
    let bias = Var::from_tensor(&Tensor::new(&[0_f32, 0.], &Device::Cpu).unwrap()).unwrap();
    let router = MoeRouter::new(
        device(),
        weight.as_tensor().clone(),
        bias.as_tensor().clone(),
        bias.as_tensor().clone(),
        1,
        1.5,
    )
    .unwrap();
    weight.set(&bf16(&[[0., 0.], [0., 4.]])).unwrap();
    bias.set(&Tensor::new(&[0_f32, 100.], &Device::Cpu).unwrap())
        .unwrap();
    let routes = router
        .forward(&bf16(&[[2., 1.]]), None, &CancellationToken::new())
        .unwrap();
    assert_eq!(ids(&routes), [0]);
}

#[test]
fn invalid_formats_nonfinite_projection_overflow_mask_and_budget_refuse() {
    let weight = bf16(&[[1., 0.], [0., 1.]]);
    assert!(project_math(&bf16(&[[1., 1.]]).to_dtype(DType::F32).unwrap(), &weight).is_err());
    assert!(project_math(&bf16(&[[f32::NAN, 1.]]), &weight).is_err());
    assert!(project_math(&bf16(&[[3e38, 3e38]]), &bf16(&[[2., 2.]])).is_err());
    assert!(score_math(&Tensor::new(&[[f32::NAN]], &Device::Cpu).unwrap()).is_err());
    let router = router(weight);
    assert!(router
        .forward(
            &bf16(&[[1., 1.]]),
            Some(&Tensor::new(&[2_u8], &Device::Cpu).unwrap()),
            &CancellationToken::new()
        )
        .is_err());
    let mut small_device = device();
    small_device.limits.max_state_bytes = 1;
    assert!(MoeRouter::new(
        small_device,
        bf16(&[[1., 0.], [0., 1.]]),
        Tensor::zeros(2, DType::F32, &Device::Cpu).unwrap(),
        Tensor::zeros(2, DType::F32, &Device::Cpu).unwrap(),
        1,
        1.5
    )
    .is_err());
}

#[test]
fn cancelled_router_refuses_before_projection_with_the_canonical_error() {
    let router = router(bf16(&[[1., 0.], [0., 1.]]));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        router.forward(&bf16(&[[1., 1.]]), None, &cancellation),
        Err(PowerError::InferenceCancelled)
    ));
}
