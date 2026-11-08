use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::Tensor;

fn matrix(device: &CudaLayerDevice, layout: MatrixLayout) -> DeviceMatrix {
    let (columns, code) = if layout == MatrixLayout::Mxfp4Block32 {
        (16, 0x22_u8)
    } else {
        (32, 56_u8)
    };
    DeviceMatrix::from_tensors(
        device,
        layout,
        [32, 32],
        &Tensor::full(code, (32, columns), device.tensor_device()).unwrap(),
        Some(
            &Tensor::full(
                127_u8,
                (
                    if layout == MatrixLayout::Mxfp4Block32 {
                        32
                    } else {
                        1
                    },
                    1,
                ),
                device.tensor_device(),
            )
            .unwrap(),
        ),
    )
    .unwrap()
}
fn expert(device: &CudaLayerDevice, layout: MatrixLayout) -> QuantizedExpert {
    QuantizedExpert::new(
        device.clone(),
        matrix(device, layout),
        matrix(device, layout),
        matrix(device, layout),
        10.,
    )
    .unwrap()
}

#[test]
fn complete_expert_weights_before_down_quantization_and_shared_omits_weight() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let input = Tensor::full(1_f32 / 32., (1, 32), device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let input = Fp8Activations::encode(&device, &input, &cancel).unwrap();
    let weight = Tensor::new(&[0.3_f32], device.tensor_device()).unwrap();
    let routed = expert(&device, MatrixLayout::Mxfp4Block32);
    let shared = expert(&device, MatrixLayout::Fp8Block32);
    let actual = routed
        .forward(&input, Some(&weight), &cancel)
        .unwrap()
        .to_dtype(DType::F32)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(actual, vec![vec![7.; 32]]);
    let shared = shared
        .forward(&input, None, &cancel)
        .unwrap()
        .to_dtype(DType::F32)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(shared, vec![vec![24.; 32]]);
    // Weighting the already quantized down output would instead give 7.2.
    assert!((shared[0][0] * 0.3 - actual[0][0]).abs() > 0.19);
}

#[test]
fn batched_expert_routes_preserve_per_row_weights() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let input = Tensor::full(1_f32 / 32., (2, 32), device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let input = Fp8Activations::encode(&device, &input, &cancel).unwrap();
    let weights = Tensor::new(&[0.3_f32, 1.], device.tensor_device()).unwrap();
    let actual = expert(&device, MatrixLayout::Mxfp4Block32)
        .forward(&input, Some(&weights), &cancel)
        .unwrap()
        .to_dtype(DType::F32)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(actual, vec![vec![7.; 32], vec![24.; 32]]);
}

#[test]
fn distinct_gate_and_up_projections_keep_the_asymmetric_swiglu_order() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let up = DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::Mxfp4Block32,
        [32, 32],
        &Tensor::full(0x44_u8, (32, 16), device.tensor_device()).unwrap(),
        Some(&Tensor::full(127_u8, (32, 1), device.tensor_device()).unwrap()),
    )
    .unwrap();
    let expert = QuantizedExpert::new(
        device.clone(),
        matrix(&device, MatrixLayout::Mxfp4Block32),
        up,
        matrix(&device, MatrixLayout::Mxfp4Block32),
        10.,
    )
    .unwrap();
    let input = Fp8Activations::encode(
        &device,
        &Tensor::full(1_f32 / 32., (1, 32), device.tensor_device())
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap(),
        &cancel,
    )
    .unwrap();
    let weight = Tensor::new(&[0.3_f32], device.tensor_device()).unwrap();
    let output = expert
        .forward(&input, Some(&weight), &cancel)
        .unwrap()
        .to_dtype(DType::F32)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(output, vec![vec![14.; 32]]);
}

#[test]
fn mixed_weight_layouts_and_cancelled_expert_execution_refuse() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    assert!(QuantizedExpert::new(
        device.clone(),
        matrix(&device, MatrixLayout::Fp8Block32),
        matrix(&device, MatrixLayout::Mxfp4Block32),
        matrix(&device, MatrixLayout::Fp8Block32),
        10.
    )
    .is_err());
    let input = Fp8Activations::encode(
        &device,
        &Tensor::ones((1, 32), DType::BF16, device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    let expert = expert(&device, MatrixLayout::Mxfp4Block32);
    cancel.cancel();
    assert!(matches!(
        expert.forward(&input, None, &cancel),
        Err(aster_power::error::PowerError::InferenceCancelled)
    ));
}
