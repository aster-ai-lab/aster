use super::*;
use crate::reference;
use candle_core::{Device, Var};

fn values(tensor: &Tensor) -> Vec<f32> {
    tensor
        .to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}

#[test]
fn effective_weights_match_f64_oracle_without_exponentiating_or_rescaling() {
    let input = Tensor::new(&[[3_f32, 4.], [-3., 4.]], &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let weight = Tensor::new(&[2_f32, -0.5], &Device::Cpu).unwrap();
    let output = rms_math(&input, Some(&weight), 1e-20).unwrap();
    let mut expected = Vec::new();
    for row in [[3_f64, 4.], [-3., 4.]] {
        expected.extend(reference::rms_norm(&row, &[2., -0.5], 1e-20).unwrap());
    }
    for (actual, expected) in values(&output).iter().zip(expected) {
        assert!((f64::from(*actual) - expected).abs() < 0.008);
    }
    assert_eq!(output.dtype(), DType::BF16);
}

#[test]
fn fp32_compression_inputs_and_weightless_zero_rows_are_supported() {
    let input = Tensor::new(&[[0_f32, 0.], [1., 2.]], &Device::Cpu).unwrap();
    let output = rms_math(&input, None, 1e-20).unwrap();
    assert_eq!(&values(&output)[..2], &[0., 0.]);
    let expected = reference::rms_norm(&[1., 2.], &[1., 1.], 1e-20).unwrap();
    for (actual, expected) in values(&output)[2..].iter().zip(expected) {
        assert!((f64::from(*actual) - expected).abs() < 0.004);
    }
}

#[test]
fn arbitrary_leading_dimensions_and_strided_rows_normalize_independently() {
    let source = Tensor::new(&[[1_f32, 2., 3.], [4., 5., 6.]], &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let transposed = source.t().unwrap();
    let batch = rms_math(&transposed, None, 1e-6).unwrap();
    let mut expected = Vec::new();
    for row in 0..3 {
        expected.extend(values(
            &rms_math(&transposed.narrow(0, row, 1).unwrap(), None, 1e-6).unwrap(),
        ));
    }
    assert_eq!(values(&batch), expected);
    let shaped = source.reshape((1, 2, 3)).unwrap();
    assert_eq!(
        values(&rms_math(&shaped, None, 1e-6).unwrap()),
        values(&rms_math(&source, None, 1e-6).unwrap())
    );
}

#[test]
fn frozen_effective_weight_does_not_follow_mutable_parameter_changes() {
    let raw = Var::from_tensor(
        &Tensor::new(&[1_f32, 2.], &Device::Cpu)
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap(),
    )
    .unwrap();
    let frozen = freeze_weight(raw.as_tensor(), 2, &Device::Cpu).unwrap();
    raw.set(&Tensor::zeros(2, DType::BF16, &Device::Cpu).unwrap())
        .unwrap();
    assert_eq!(values(&frozen), vec![1., 2.]);
}

#[test]
fn bad_shapes_nonfinite_values_overflow_and_invalid_epsilon_refuse() {
    let input = Tensor::new(&[1_f32, 2.], &Device::Cpu).unwrap();
    for epsilon in [0., -1., f64::NAN, f64::INFINITY, 1e-300] {
        assert!(rms_math(&input, None, epsilon).is_err());
    }
    assert!(rms_math(
        &input,
        Some(&Tensor::ones(3, DType::F32, &Device::Cpu).unwrap()),
        1e-6
    )
    .is_err());
    for value in [f32::NAN, f32::INFINITY, 1e30] {
        let bad = Tensor::full(value, 2, &Device::Cpu).unwrap();
        assert!(rms_math(&bad, None, 1e-6).is_err());
    }
    assert!(freeze_weight(
        &Tensor::new(&[f32::NAN, 1.], &Device::Cpu).unwrap(),
        2,
        &Device::Cpu
    )
    .is_err());
    assert!(freeze_weight(
        &Tensor::ones(2, DType::U32, &Device::Cpu).unwrap(),
        2,
        &Device::Cpu
    )
    .is_err());
}
