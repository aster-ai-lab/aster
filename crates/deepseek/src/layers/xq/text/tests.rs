use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Device};

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}

#[test]
fn embedding_contract_refuses_empty_unknown_and_cancelled_tokens_before_compute() {
    let device = device();
    let matrix = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F16,
        3,
        32,
        &vec![0; 3 * 32 * 2],
    )
    .unwrap();
    let embedding = XqTokenEmbedding::new(device, matrix).unwrap();
    assert_eq!(
        (
            embedding.vocabulary_size(),
            embedding.hidden_size(),
            embedding.retained_bytes()
        ),
        (3, 32, 192)
    );
    for tokens in [vec![], vec![3], vec![u32::MAX]] {
        assert!(embedding
            .forward(&tokens, &CancellationToken::new())
            .is_err());
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(embedding.forward(&[0], &cancel).is_err());
    assert!(embedding.forward(&[0], &CancellationToken::new()).is_err()); // no CPU fallback
}

#[test]
fn text_endpoints_require_exact_stored_types_and_matching_hidden_norm() {
    let device = device();
    let matrix = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F32,
        3,
        32,
        &vec![0; 3 * 32 * 4],
    )
    .unwrap();
    assert!(XqTokenEmbedding::new(device.clone(), matrix).is_err());
    let norm = XqRmsNorm::new(
        device.clone(),
        Tensor::ones(32, DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    let head =
        XqDeviceMatrix::from_bytes_for_tests(&device, XqTensorType::Q8_0, 3, 32, &vec![0; 3 * 34])
            .unwrap();
    let head = XqTextHead::new(device.clone(), norm, head).unwrap();
    assert_eq!((head.vocabulary_size(), head.retained_bytes()), (3, 230));
    let trunk = XqMhcContinuation::from_embedding(
        &device,
        &Tensor::zeros((1, 32), DType::F32, &Device::Cpu).unwrap(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(head.forward(&trunk, &CancellationToken::new()).is_err()); // no CPU fallback
    let norm = XqRmsNorm::new(
        device.clone(),
        Tensor::ones(16, DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    let matrix =
        XqDeviceMatrix::from_bytes_for_tests(&device, XqTensorType::Q8_0, 3, 32, &vec![0; 3 * 34])
            .unwrap();
    assert!(XqTextHead::new(device, norm, matrix).is_err());
}

#[cfg(feature = "cuda")]
mod gpu;

#[test]
fn component_owners_refuse_matrices_with_different_resource_limits() {
    let device = device();
    let matrix = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F16,
        3,
        32,
        &vec![0; 3 * 32 * 2],
    )
    .unwrap();
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 128,
        ..InferenceLimits::default()
    });
    assert!(XqTokenEmbedding::new(strict.clone(), matrix).is_err());
    let norm = XqRmsNorm::new(
        device.clone(),
        Tensor::ones(32, DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    let output =
        XqDeviceMatrix::from_bytes_for_tests(&device, XqTensorType::Q8_0, 3, 32, &vec![0; 3 * 34])
            .unwrap();
    assert!(XqTextHead::new(strict, norm, output).is_err());
}
