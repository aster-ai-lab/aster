use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};
use std::time::Instant;

#[test]
#[ignore = "requires owned H200; immutable step validation cache oracle and A/B"]
fn h200_step_validation_cache_preserves_predicates_views_step_isolation_and_reports_cost() {
    let runtime = RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap();
    let device = runtime.tensor_device();
    let limits = InferenceLimits::default();
    let validate = |input: &Tensor, positive| {
        super::super::validate(input, positive, || {
            crate::cuda::finite::validate(input, positive, "cached GPU oracle")
        })
    };
    for dtype in [DType::F32, DType::BF16] {
        let input = Tensor::ones((3, 257), dtype, device).unwrap();
        let scope = Scope::begin(&limits, 0).unwrap();
        for _ in 0..20 {
            validate(&input, true).unwrap();
            validate(&input, false).unwrap();
        }
        assert_eq!(scope.counts(), (1, 39));
        let strided = input.transpose(0, 1).unwrap().narrow(0, 1, 255).unwrap();
        validate(&strided, true).unwrap();
        assert_eq!(scope.counts(), (2, 39));
        scope.validate_complete().unwrap();
        drop(scope);
        let scope = Scope::begin(&limits, 0).unwrap();
        validate(&input, false).unwrap();
        assert_eq!(scope.counts(), (1, 0));
        drop(scope);
        for value in [f32::NAN, f32::INFINITY, -1.] {
            let input = Tensor::full(value, (3, 257), device)
                .unwrap()
                .to_dtype(dtype)
                .unwrap();
            let scope = Scope::begin(&limits, 0).unwrap();
            assert!(validate(&input, true).is_err());
            assert!(scope.validate_complete().is_err());
        }
    }
    let input = Tensor::ones((8, 5120), DType::F32, device).unwrap();
    let mut measurements = Vec::new();
    for repeat in 0..3 {
        for cached in if repeat % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            device.synchronize().unwrap();
            let started = Instant::now();
            let scope = if cached {
                Some(Scope::begin(&limits, 0).unwrap())
            } else {
                None
            };
            for _ in 0..100 {
                validate(&input, true).unwrap();
            }
            device.synchronize().unwrap();
            let counts = scope.as_ref().map(Scope::counts);
            if let Some(scope) = &scope {
                scope.validate_complete().unwrap();
                assert_eq!(counts, Some((1, 99)));
            }
            measurements.push(serde_json::json!({"repeat":repeat+1,"cached":cached,"calls":100,"counts":counts,"seconds":started.elapsed().as_secs_f64()}));
        }
    }
    println!(
        "{}",
        serde_json::json!({"profile":"immutable-step-validation-cache-v1","measurements":measurements,"full_model_promotion_claimed":false})
    );
}
