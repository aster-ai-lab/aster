use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_greedy_ties_choose_first_global_token_in_strided_last_row() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let stream = device
        .tensor_device()
        .as_cuda_device()
        .unwrap()
        .cuda_stream();
    assert_eq!(stream.context().compute_capability().unwrap(), (9, 0));
    let cancel = CancellationToken::new();
    let selector = XqGreedySelector::new(&device, 1300, &cancel).unwrap();
    let logits = tied_view(&device);
    assert_eq!(selector.select_last(&logits, &cancel).unwrap(), 1);
    let selector = XqGreedySelector::new(&device, 4, &cancel).unwrap();
    let zero =
        Tensor::from_vec(vec![-1_f32, -0., 0., -4.], (1, 4), device.tensor_device()).unwrap();
    assert_eq!(selector.select_last(&zero, &cancel).unwrap(), 1);
    let negative =
        Tensor::from_vec(vec![-10_f32, -1., -1., -8.], (1, 4), device.tensor_device()).unwrap();
    assert_eq!(selector.select_last(&negative, &cancel).unwrap(), 1);
    cancel.cancel();
    assert!(selector.select_last(&zero, &cancel).is_err());
}
