use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::Device;

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}

pub(crate) fn for_pipeline(
    device: &CudaLayerDevice,
    layer: usize,
) -> (XqIndexQueryProjection, Option<XqIndexKeyProjection>) {
    use crate::layers::xq::attention::sublayer::tests::{cache, f16, norm};
    let query = XqIndexQueryProjection::from_parts(
        device.clone(),
        layer,
        f16(device, 256, 32, 11),
        f16(device, 2, 256, 12),
        cache(device, 8),
    )
    .unwrap();
    let key = [2, 8, 14, 20].contains(&layer).then(|| {
        XqIndexKeyProjection::from_parts(
            device.clone(),
            layer,
            f16(device, 128, 512, 13),
            norm(device, 128),
            cache(device, 8),
        )
        .unwrap()
    });
    (query, key)
}
fn matrix(
    device: &CudaLayerDevice,
    kind: XqTensorType,
    rows: usize,
    columns: usize,
) -> XqDeviceMatrix {
    XqDeviceMatrix::from_bytes_for_tests(
        device,
        kind,
        rows,
        columns,
        &vec![0; (kind.row_bytes(columns as u64).unwrap() * rows as u64) as usize],
    )
    .unwrap()
}
fn norm(device: &CudaLayerDevice) -> XqRmsNorm {
    XqRmsNorm::new(
        device.clone(),
        Tensor::ones(128, DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap()
}
fn cache(device: &CudaLayerDevice) -> Tensor {
    Tensor::cat(
        &[
            Tensor::ones((8, 32), DType::F32, device.tensor_device()).unwrap(),
            Tensor::zeros((8, 32), DType::F32, device.tensor_device()).unwrap(),
        ],
        1,
    )
    .unwrap()
}
fn query(device: &CudaLayerDevice, layer: usize) -> Result<XqIndexQueryProjection> {
    XqIndexQueryProjection::from_parts(
        device.clone(),
        layer,
        matrix(device, XqTensorType::F16, 256, 4),
        matrix(device, XqTensorType::F16, 2, 4),
        cache(device),
    )
}
fn key(device: &CudaLayerDevice, layer: usize) -> Result<XqIndexKeyProjection> {
    XqIndexKeyProjection::from_parts(
        device.clone(),
        layer,
        matrix(device, XqTensorType::F16, 128, 512),
        norm(device),
        cache(device),
    )
}

#[test]
fn exact_query_key_sources_original_types_and_aggregate_residency_are_required() {
    let device = device();
    for layer in [2, 8, 14, 20, 24, 28, 32, 36] {
        let projection = query(&device, layer).unwrap();
        assert_eq!(projection.layer(), layer);
        assert_eq!(projection.heads(), 2);
        assert_eq!(
            projection.retained_bytes(),
            (256 * 4 * 2 + 2 * 4 * 2 + 8 * 64 * 4) as u64
        );
    }
    for layer in [2, 8, 14, 20] {
        let projection = key(&device, layer).unwrap();
        assert_eq!(projection.layer(), layer);
        assert_eq!(
            projection.retained_bytes(),
            (128 * 512 * 2 + 128 * 4 + 8 * 64 * 4) as u64
        );
    }
    for layer in [0, 1, 3, 21, 39, 40, usize::MAX] {
        assert!(query(&device, layer).is_err());
        assert!(key(&device, layer).is_err());
    }
    assert!(key(&device, 24).is_err());
    assert!(XqIndexQueryProjection::from_parts(
        device.clone(),
        2,
        matrix(&device, XqTensorType::F32, 256, 4),
        matrix(&device, XqTensorType::F16, 2, 4),
        cache(&device)
    )
    .is_err());
    assert!(XqIndexKeyProjection::from_parts(
        device.clone(),
        2,
        matrix(&device, XqTensorType::F16, 128, 256),
        norm(&device),
        cache(&device)
    )
    .is_err());
    let other = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 1 << 30,
        ..InferenceLimits::default()
    });
    assert!(XqIndexKeyProjection::from_parts(
        device.clone(),
        2,
        matrix(&device, XqTensorType::F16, 128, 512),
        norm(&other),
        cache(&device)
    )
    .is_err());
    let good = key(&device, 2).unwrap();
    let small = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: good.retained_bytes() - 1,
        ..InferenceLimits::default()
    });
    assert!(key(&small, 2).is_err());
}

#[test]
fn explicit_head_scaling_is_f32_and_applied_exactly_once() {
    for heads in [1, 2, 32] {
        let values: Vec<f32> = (0..heads).map(|i| 1.0001 + i as f32 * 0.0317).collect();
        let tensor = Tensor::from_vec(values.clone(), (1, heads), &Device::Cpu).unwrap();
        let scaled = scale_head_weights(&tensor, heads).unwrap();
        assert_eq!(scaled.dtype(), DType::F32);
        let actual = scaled.to_vec2::<f32>().unwrap();
        for col in 0..heads {
            let expected = f64::from(values[col]) / (128_f64 * heads as f64).sqrt();
            assert!((f64::from(actual[0][col]) - expected).abs() < 1e-7 * expected.abs().max(1.));
        }
        assert_ne!(
            scaled
                .to_dtype(DType::BF16)
                .unwrap()
                .to_dtype(DType::F32)
                .unwrap()
                .to_vec2::<f32>()
                .unwrap(),
            actual
        );
    }
    assert!(
        scale_head_weights(&Tensor::ones((1, 2), DType::BF16, &Device::Cpu).unwrap(), 2).is_err()
    );
}

#[test]
fn key_empty_emissions_are_not_fabricated_and_projection_refusals_precede_cpu_compute() {
    let device = device();
    let key = key(&device, 2).unwrap();
    let query = query(&device, 2).unwrap();
    let cancel = CancellationToken::new();
    let empty = Tensor::zeros((0, 512), DType::F32, &Device::Cpu).unwrap();
    assert_eq!(key.forward(&empty, &[], &cancel).unwrap().row_count(), 0);
    assert!(key.forward(&empty, &[0], &cancel).is_err());
    assert!(key
        .forward(&empty.to_dtype(DType::BF16).unwrap(), &[], &cancel)
        .is_err());
    let x = Tensor::ones((1, 4), DType::F32, &Device::Cpu).unwrap();
    assert!(query.forward(&x, &x, &[8], &cancel).is_err());
    assert!(query
        .forward(&x.to_dtype(DType::BF16).unwrap(), &x, &[0], &cancel)
        .is_err());
    assert!(query.forward(&x, &x, &[0], &cancel).is_err()); // no CPU projection fallback
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(key.forward(&empty, &[], &cancelled).is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
