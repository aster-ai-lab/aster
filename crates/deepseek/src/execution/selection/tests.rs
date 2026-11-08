use super::*;
use aster_power::inference::InferenceLimits;

fn tied_view(device: &CudaLayerDevice) -> Tensor {
    let mut raw = vec![-20_f32; 2 * 2600];
    raw[5 + 17] = 30.; // earlier row's winner must be ignored
    for id in [1, 256, 1024] {
        raw[2600 + 5 + id] = 7.;
    }
    Tensor::from_vec(raw, (2, 2600), device.tensor_device())
        .unwrap()
        .narrow(1, 5, 1300)
        .unwrap()
}
#[test]
fn smallest_global_id_wins_ties_across_reduction_lanes_and_only_last_strided_row_counts() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let selector = XqGreedySelector::new(&device, 1300, &cancel).unwrap();
    let logits = tied_view(&device);
    assert!(!logits.is_contiguous());
    assert_eq!(selector.select_last(&logits, &cancel).unwrap(), 1);
    let selector = XqGreedySelector::new(&device, 4, &cancel).unwrap();
    let zero =
        Tensor::from_vec(vec![-1_f32, -0., 0., -4.], (1, 4), device.tensor_device()).unwrap();
    assert_eq!(selector.select_last(&zero, &cancel).unwrap(), 1);
    let all_negative =
        Tensor::from_vec(vec![-10_f32, -1., -1., -8.], (1, 4), device.tensor_device()).unwrap();
    assert_eq!(selector.select_last(&all_negative, &cancel).unwrap(), 1);
}
#[test]
fn selector_refuses_nonfinite_wrong_dtype_width_budget_or_cancelled_work() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let selector = XqGreedySelector::new(&device, 4, &cancel).unwrap();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let input =
            Tensor::from_vec(vec![0_f32, value, 1., 2.], (1, 4), device.tensor_device()).unwrap();
        assert!(selector.select_last(&input, &cancel).is_err());
    }
    assert!(selector
        .select_last(
            &Tensor::zeros((1, 4), DType::F64, device.tensor_device()).unwrap(),
            &cancel
        )
        .is_err());
    assert!(selector
        .select_last(
            &Tensor::zeros((1, 3), DType::F32, device.tensor_device()).unwrap(),
            &cancel
        )
        .is_err());
    let limited = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_input_bytes: 31,
        ..InferenceLimits::default()
    });
    assert!(XqGreedySelector::new(&limited, 4, &cancel).is_err());
    cancel.cancel();
    assert!(selector
        .select_last(
            &Tensor::zeros((1, 4), DType::F32, device.tensor_device()).unwrap(),
            &cancel
        )
        .is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
