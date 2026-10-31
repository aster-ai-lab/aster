use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};
use candle_core::{DType, Tensor};
use sha2::{Digest, Sha256};
use std::time::Instant;

fn scalar(input: &[f32]) -> Vec<f64> {
    let mut matrix: Vec<f64> = input.iter().map(|&x| f64::from(x)).collect();
    for iteration in 0..20 {
        if iteration > 0 {
            for row in 0..4 {
                let denominator = (0..4).map(|col| matrix[row * 4 + col]).sum::<f64>() + 1e-6;
                for col in 0..4 {
                    matrix[row * 4 + col] /= denominator;
                }
            }
        }
        for col in 0..4 {
            let denominator = (0..4).map(|row| matrix[row * 4 + col]).sum::<f64>() + 1e-6;
            for row in 0..4 {
                matrix[row * 4 + col] /= denominator;
            }
        }
    }
    matrix
}

#[test]
#[ignore = "requires an owned H200; frozen numeric budget before candidate execution"]
fn h200_xq_sinkhorn_fused_matches_f64_tensor_control_and_reports_alternating_cost() {
    let runtime = RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap();
    let device = runtime.tensor_device();
    for tokens in [1, 3, 8, 129] {
        let values: Vec<f32> = (0..tokens * 16)
            .map(|index| match index % 11 {
                0 => 0.,
                1 => 1e-30,
                2 => 1e-6,
                3 => 2.,
                _ => ((index * 13 % 107) as f32 + 1.) / 107.,
            })
            .collect();
        let input = Tensor::from_slice(&values, (tokens, 4, 4), device).unwrap();
        let output = normalize(&input, 16 * 1024 * 1024).unwrap();
        let actual = output.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        let control = crate::layers::xq::mhc::tensor_sinkhorn(&input)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        for token in 0..tokens {
            let expected = scalar(&values[token * 16..(token + 1) * 16]);
            for col in 0..16 {
                let index = token * 16 + col;
                assert!(
                    (f64::from(actual[index]) - expected[col]).abs() <= 2e-6,
                    "F64 difference tokens={tokens} at={index}"
                );
                assert!(
                    (actual[index] - control[index]).abs() <= 5e-7,
                    "tensor difference tokens={tokens} at={index}"
                );
            }
        }
        // A genuinely strided input must retain the same admitted semantics.
        let transposed = input.transpose(1, 2).unwrap();
        let strided = normalize(&transposed, 16 * 1024 * 1024).unwrap();
        let reference = crate::layers::xq::mhc::tensor_sinkhorn(&transposed).unwrap();
        let difference = strided
            .sub(&reference)
            .unwrap()
            .abs()
            .unwrap()
            .max_all()
            .unwrap()
            .to_scalar::<f32>()
            .unwrap();
        assert!(difference <= 5e-7);
    }
    for value in [f32::NAN, f32::INFINITY, -0.01, 2.01] {
        let invalid = Tensor::full(value, (1, 4, 4), device).unwrap();
        assert!(normalize(&invalid, 65536).is_err());
    }
    let input = Tensor::full(0.25_f32, (8, 4, 4), device).unwrap();
    assert!(normalize(&input.to_dtype(DType::BF16).unwrap(), 65536).is_err());
    assert!(normalize(&input, 1).is_err());
    let mut measurements = Vec::new();
    for repeat in 0..3 {
        let order = if repeat % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        };
        for fused in order {
            device.synchronize().unwrap();
            let started = Instant::now();
            for _ in 0..20 {
                let _output = if fused {
                    normalize(&input, 65536).unwrap()
                } else {
                    crate::layers::xq::mhc::tensor_sinkhorn(&input).unwrap()
                };
            }
            device.synchronize().unwrap();
            measurements.push(serde_json::json!({"repeat":repeat+1,"fused":fused,"calls":20,"seconds":started.elapsed().as_secs_f64()}));
        }
    }
    println!(
        "{}",
        serde_json::json!({"abi":ABI,"source_sha256":format!("{:x}",Sha256::digest(SOURCE)),"numeric_f64_absolute_budget":2e-6,"tensor_absolute_budget":5e-7,"measurements":measurements,"full_model_promotion_claimed":false})
    );
}
