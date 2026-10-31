use super::*;
use candle_core::Device;

fn bf16(values: &[f32], shape: (usize, usize, usize)) -> Tensor {
    Tensor::from_slice(values, values.len(), &Device::Cpu)
        .unwrap()
        .reshape(shape)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap()
}

fn input() -> Tensor {
    bf16(&[1., 0., 0., 1., 1., 1., 0., 0.], (1, 4, 2))
}
fn kv() -> Tensor {
    bf16(&[1., 0., 0., -1., -1., -1., 0., 0., 2., -2.], (1, 5, 2))
}
fn weight() -> Tensor {
    Tensor::ones((4, 2), DType::BF16, &Device::Cpu).unwrap()
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
fn signed_root_gate_matches_independent_scalar_norm_and_keeps_zero_half_gate() {
    let residual = input();
    let before = values(&residual);
    let output = gate_math(&residual, &kv(), &weight(), &weight(), 1e-20, 1e-6, None).unwrap();
    let actual = values(&output);
    for head in 0..4 {
        let x = &before[head * 2..head * 2 + 2];
        let keys = values(&kv());
        let k = &keys[head * 2..head * 2 + 2];
        let dot = x
            .iter()
            .zip(k)
            .map(|(x, k)| f64::from(*x) * f64::from(*k))
            .sum::<f64>();
        let norm_x = (x.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / 2.0 + 1e-20).sqrt();
        let norm_k = (k.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / 2.0 + 1e-20).sqrt();
        let score = dot / norm_x / norm_k / 2_f64.sqrt();
        let signed = if score == 0.0 {
            0.0
        } else {
            score.abs().max(1e-6).sqrt() * score.signum()
        };
        let gate = 1.0 / (1.0 + (-signed).exp());
        for channel in 0..2 {
            let expected = f64::from(x[channel]) + gate * [2.0, -2.0][channel];
            assert!((f64::from(actual[head * 2 + channel]) - expected).abs() < 0.016);
        }
    }
    assert_eq!(&actual[6..], &[1., -1.]);
    assert_eq!(values(&residual), before);
}

#[test]
fn image_mask_preserves_original_streams_and_other_rows_keep_normal_gate() {
    let x = input();
    let packed = kv();
    let batched = Tensor::cat(&[&x, &x], 0).unwrap();
    let keys = Tensor::cat(&[&packed, &packed], 0).unwrap();
    let mask = Tensor::new(&[0_u8, 1], &Device::Cpu).unwrap();
    let output = gate_math(
        &batched,
        &keys,
        &weight(),
        &weight(),
        1e-20,
        1e-6,
        Some(&mask),
    )
    .unwrap();
    let expected = gate_math(&x, &packed, &weight(), &weight(), 1e-20, 1e-6, None).unwrap();
    assert_eq!(&values(&output)[..8], values(&expected));
    assert_eq!(&values(&output)[8..], values(&x));
}

#[test]
fn batch_and_single_token_gate_outputs_agree() {
    let x = input();
    let packed = kv();
    let batched = Tensor::cat(&[&x, &x, &x], 0).unwrap();
    let keys = Tensor::cat(&[&packed, &packed, &packed], 0).unwrap();
    let output = gate_math(&batched, &keys, &weight(), &weight(), 1e-20, 1e-6, None).unwrap();
    let single = gate_math(&x, &packed, &weight(), &weight(), 1e-20, 1e-6, None).unwrap();
    assert_eq!(values(&output), values(&single).repeat(3));
}

#[test]
fn norm_weights_change_the_dot_without_changing_rms_denominators() {
    let zero = Tensor::zeros((4, 2), DType::BF16, &Device::Cpu).unwrap();
    let half_gate = gate_math(&input(), &kv(), &zero, &weight(), 1e-20, 1e-6, None).unwrap();
    assert_eq!(values(&half_gate), vec![2., -1., 1., 0., 2., 0., 1., -1.]);
    let wh = Tensor::full(2_f32, (4, 2), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let wk = Tensor::full(3_f32, (4, 2), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let scaled = gate_math(&input(), &kv(), &wh, &wk, 1e-20, 1e-6, None).unwrap();
    let gate = 1.0 / (1.0 + (-(6.0 * 2_f64.sqrt()).sqrt()).exp());
    assert!((f64::from(values(&scaled)[0]) - (1.0 + 2.0 * gate)).abs() < 0.016);
}

#[test]
fn gate_refuses_wrong_shapes_dtypes_masks_and_nonfinite_numerics() {
    let x = input();
    let packed = kv();
    let w = weight();
    assert!(gate_math(
        &x,
        &packed.to_dtype(DType::F32).unwrap(),
        &w,
        &w,
        1e-20,
        1e-6,
        None
    )
    .is_err());
    assert!(gate_math(
        &x,
        &packed.narrow(1, 0, 4).unwrap(),
        &w,
        &w,
        1e-20,
        1e-6,
        None
    )
    .is_err());
    assert!(gate_math(
        &x,
        &packed,
        &w.to_dtype(DType::F32).unwrap(),
        &w,
        1e-20,
        1e-6,
        None
    )
    .is_err());
    for mask in [
        Tensor::zeros(2, DType::U8, &Device::Cpu).unwrap(),
        Tensor::zeros(1, DType::F32, &Device::Cpu).unwrap(),
    ] {
        assert!(gate_math(&x, &packed, &w, &w, 1e-20, 1e-6, Some(&mask)).is_err());
    }
    for invalid in [0., -1., f64::NAN, f64::INFINITY, f64::MAX, 1e-300] {
        assert!(gate_math(&x, &packed, &w, &w, invalid, 1e-6, None).is_err());
        assert!(gate_math(&x, &packed, &w, &w, 1e-20, invalid, None).is_err());
    }
}

#[test]
fn public_gate_snapshots_weights_and_rejects_nonfinite_parameters_inputs_and_masks() {
    use aster_power::inference::InferenceLimits;
    use candle_core::Var;
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let mutable = Var::ones((4, 2), DType::BF16, &Device::Cpu).unwrap();
    let gate = EngramGate::new(
        device.clone(),
        mutable.as_tensor().clone(),
        weight(),
        1e-20,
        1e-6,
    )
    .unwrap();
    let before = values(&gate.forward(&input(), &kv(), None).unwrap());
    mutable
        .set(&Tensor::zeros((4, 2), DType::BF16, &Device::Cpu).unwrap())
        .unwrap();
    assert_eq!(
        values(&gate.forward(&input(), &kv(), None).unwrap()),
        before
    );
    for code in [f32::NAN, f32::INFINITY] {
        let bad = Tensor::full(code, (4, 2), &Device::Cpu)
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap();
        assert!(EngramGate::new(device.clone(), bad, weight(), 1e-20, 1e-6).is_err());
        let bad = Tensor::full(code, (1, 4, 2), &Device::Cpu)
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap();
        assert!(gate.forward(&bad, &kv(), None).is_err());
        let bad = Tensor::full(code, (1, 5, 2), &Device::Cpu)
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap();
        assert!(gate.forward(&input(), &bad, None).is_err());
    }
    assert!(gate
        .forward(
            &input(),
            &kv(),
            Some(&Tensor::new(&[2_u8], &Device::Cpu).unwrap())
        )
        .is_err());
}

#[test]
fn public_gate_refuses_resident_and_live_tensor_budget_overflow() {
    use aster_power::inference::InferenceLimits;
    let resident = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 31,
        ..InferenceLimits::default()
    });
    assert!(EngramGate::new(resident, weight(), weight(), 1e-20, 1e-6).is_err());
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 5000,
        ..InferenceLimits::default()
    });
    let gate = EngramGate::new(device, weight(), weight(), 1e-20, 1e-6).unwrap();
    assert!(gate.forward(&input(), &kv(), None).is_err());
}
