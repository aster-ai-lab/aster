use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};
use candle_core::{DType, Tensor};
use std::time::Instant;

#[test]
#[ignore = "requires owned H200; finite predicate qualification and A/B before promotion"]
fn h200_fused_finite_matches_tensor_control_for_strides_tail_bf16_and_invalid_values() {
    let runtime = RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap();
    let device = runtime.tensor_device();
    let valid = [
        0_f32,
        -0.,
        f32::from_bits(1),
        -f32::from_bits(1),
        f32::MAX,
        -f32::MAX,
        1.,
        -1.,
    ];
    for dtype in [DType::F32, DType::BF16] {
        for nonnegative in [false, true] {
            for value in valid
                .into_iter()
                .chain([f32::NAN, f32::INFINITY, f32::NEG_INFINITY])
            {
                let input = Tensor::full(value, (3, 257), device)
                    .unwrap()
                    .to_dtype(dtype)
                    .unwrap();
                let stored = input
                    .to_dtype(DType::F32)
                    .unwrap()
                    .flatten_all()
                    .unwrap()
                    .to_vec1::<f32>()
                    .unwrap()[0];
                let expected = stored.is_finite() && (!nonnegative || stored >= 0.);
                assert_eq!(
                    validate(&input, nonnegative, "case").is_ok(),
                    expected,
                    "{dtype:?} {value} {nonnegative}"
                );
                assert_eq!(
                    crate::layers::validate_finite_tensor(&input, nonnegative, "control").is_ok(),
                    expected
                );
                let strided = input.transpose(0, 1).unwrap().narrow(0, 1, 255).unwrap();
                assert_eq!(validate(&strided, nonnegative, "strided").is_ok(), expected);
            }
        }
    }
    // One invalid element among many exercises all warp/tail positions.
    for position in [0, 31, 32, 255, 256, 770] {
        let mut values = vec![1_f32; 771];
        values[position] = f32::NAN;
        let input = Tensor::from_vec(values, (3, 257), device).unwrap();
        assert!(validate(&input, false, "single invalid").is_err());
    }
    // Contiguous views can retain an offset into a larger storage. Invalid
    // guards outside the view must never affect validation of its actual slice.
    let mut guarded = vec![1_f32; 773];
    guarded[0] = f32::NAN;
    guarded[772] = f32::INFINITY;
    for dtype in [DType::F32, DType::BF16] {
        let source = Tensor::from_vec(guarded.clone(), 773, device)
            .unwrap()
            .to_dtype(dtype)
            .unwrap();
        let view = source.narrow(0, 1, 771).unwrap();
        for policy in [
            crate::cuda::input::Policy::FrozenCopy,
            crate::cuda::input::Policy::BorrowContiguous,
        ] {
            super::super::execution::validate_with_input_policy(
                &view,
                true,
                "guarded offset",
                policy,
            )
            .unwrap();
        }
    }
    let input = Tensor::ones((8, 5120), DType::F32, device).unwrap();
    assert!(validate(&input.to_dtype(DType::U32).unwrap(), false, "invalid dtype").is_err());
    let mut measurements = Vec::new();
    for repeat in 0..3 {
        for fused in if repeat % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            device.synchronize().unwrap();
            let started = Instant::now();
            for _ in 0..100 {
                if fused {
                    validate(&input, true, "fused").unwrap();
                } else {
                    crate::layers::validate_finite_tensor(&input, true, "control").unwrap();
                }
            }
            device.synchronize().unwrap();
            measurements.push(serde_json::json!({"repeat":repeat+1,"fused":fused,"calls":100,"seconds":started.elapsed().as_secs_f64()}));
        }
    }
    let mut input_policy_measurements = Vec::new();
    for repeat in 0..3 {
        let policies = if repeat % 2 == 0 {
            [
                crate::cuda::input::Policy::FrozenCopy,
                crate::cuda::input::Policy::BorrowContiguous,
            ]
        } else {
            [
                crate::cuda::input::Policy::BorrowContiguous,
                crate::cuda::input::Policy::FrozenCopy,
            ]
        };
        for policy in policies {
            device.synchronize().unwrap();
            let started = Instant::now();
            for _ in 0..100 {
                super::super::execution::validate_with_input_policy(
                    &input,
                    true,
                    "input-policy",
                    policy,
                )
                .unwrap();
            }
            device.synchronize().unwrap();
            input_policy_measurements.push(serde_json::json!({"repeat":repeat+1,"policy":format!("{policy:?}"),"calls":100,"seconds":started.elapsed().as_secs_f64()}));
        }
    }
    println!(
        "{}",
        serde_json::json!({"abi":ABI,"measurements":measurements,"input_policy_measurements":input_policy_measurements,"full_model_promotion_claimed":false})
    );
}
