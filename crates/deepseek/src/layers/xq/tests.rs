use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::{Device, Var};

#[test]
fn xq_norm_keeps_f32_mantissas_and_matches_independent_f64_math() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let weight = [1.0001_f32, -0.75003];
    let norm = XqRmsNorm::new(device, Tensor::new(&weight, &Device::Cpu).unwrap()).unwrap();
    let values = [3.0001_f32, -4.0002, 1.25, 2.125];
    let input = Tensor::from_slice(&values, (2, 2), &Device::Cpu).unwrap();
    let output = norm.forward(&input, &CancellationToken::new()).unwrap();
    assert_eq!(output.dtype(), DType::F32);
    let actual = output.to_vec2::<f32>().unwrap();
    for row in 0..2 {
        let denominator =
            ((f64::from(values[row * 2]).powi(2) + f64::from(values[row * 2 + 1]).powi(2)) / 2.
                + 1e-20)
                .sqrt();
        for col in 0..2 {
            assert!(
                (f64::from(actual[row][col])
                    - f64::from(values[row * 2 + col]) / denominator * f64::from(weight[col]))
                .abs()
                    < 2e-7
            );
        }
    }
    assert_ne!(
        output
            .to_dtype(DType::BF16)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap(),
        actual
    );
}

#[test]
fn xq_norm_freezes_parameters_and_refuses_precision_nonfinite_and_budget_errors() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let weight = Var::from_vec(vec![1_f32, 2.], 2, &Device::Cpu).unwrap();
    let norm = XqRmsNorm::new(device.clone(), weight.as_tensor().clone()).unwrap();
    weight
        .set(&Tensor::new(&[9_f32, 9.], &Device::Cpu).unwrap())
        .unwrap();
    assert_eq!(norm.weight.to_vec1::<f32>().unwrap(), [1., 2.]);
    for values in [[f32::NAN, 0.], [f32::MAX, f32::MAX]] {
        assert!(norm
            .forward(
                &Tensor::from_vec(values.to_vec(), (1, 2), &Device::Cpu).unwrap(),
                &CancellationToken::new()
            )
            .is_err());
    }
    let input = Tensor::zeros((1, 2), DType::BF16, &Device::Cpu).unwrap();
    assert!(norm.forward(&input, &CancellationToken::new()).is_err());
    assert!(XqRmsNorm::new(device, Tensor::zeros(2, DType::BF16, &Device::Cpu).unwrap()).is_err());
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 1,
        ..InferenceLimits::default()
    });
    assert!(XqRmsNorm::new(device, Tensor::ones(2, DType::F32, &Device::Cpu).unwrap()).is_err());
}
