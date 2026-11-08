use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::{Device, Var};

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn values(tensor: &Tensor) -> Vec<f32> {
    tensor.flatten_all().unwrap().to_vec1().unwrap()
}

// Independent scalar F64 oracle; the scale is explicitly the reference F32
// 1/sqrt(H). Stage-local budget fixed before any owned-H200 execution.
pub(crate) fn oracle(
    residual: &[f32],
    projected: &[f32],
    query: &[f32],
    key: &[f32],
    images: &[u8],
    hidden: usize,
) -> Vec<f64> {
    let mut output = Vec::new();
    let scale = f64::from(1_f32 / (hidden as f32).sqrt());
    for (token, image) in images.iter().enumerate() {
        for stream in 0..4 {
            let offset = (token * 4 + stream) * hidden;
            let kv_offset = (token * 5 + stream) * hidden;
            let x = &residual[offset..offset + hidden];
            let k = &projected[kv_offset..kv_offset + hidden];
            let xrms = (x.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / hidden as f64
                + 1e-20)
                .sqrt();
            let krms = (k.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / hidden as f64
                + 1e-20)
                .sqrt();
            let dot = (0..hidden)
                .map(|col| {
                    let index = stream * hidden + col;
                    f64::from(x[col]) / xrms
                        * f64::from(query[index] * key[index])
                        * (f64::from(k[col]) / krms)
                })
                .sum::<f64>()
                * scale;
            let signed_root = if dot == 0. {
                0.
            } else {
                dot.signum() * dot.abs().max(1e-6).sqrt()
            };
            let gate = 1. / (1. + (-signed_root).exp());
            for col in 0..hidden {
                output.push(if *image == 1 {
                    f64::from(x[col])
                } else {
                    f64::from(x[col]) + gate * f64::from(projected[(token * 5 + 4) * hidden + col])
                });
            }
        }
    }
    output
}
pub(crate) fn close(actual: &[f32], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (f64::from(actual) - expected).abs() <= 2e-5 + 2e-5 * expected.abs(),
            "{index}: {actual} != {expected}"
        );
    }
}

#[test]
fn normalize_first_f32_gate_matches_independent_math_with_shared_values_and_image_mask() {
    let device = device();
    let hidden = 8;
    let x: Vec<_> = (0..3 * 4 * hidden)
        .map(|i| (i as f32 - 42.03125) / 17.)
        .collect();
    let kv: Vec<_> = (0..3 * 5 * hidden)
        .map(|i| ((i * 7 % 23) as f32 - 10.5) / 5.)
        .collect();
    let qw: Vec<_> = (0..4 * hidden)
        .map(|i| {
            if i / hidden == 0 {
                0.
            } else {
                0.2 + i as f32 / 31.
            }
        })
        .collect();
    let kw: Vec<_> = (0..4 * hidden)
        .map(|i| {
            if i / hidden == 2 {
                -0.8
            } else {
                1.0001 + i as f32 / 27.
            }
        })
        .collect();
    let images = [0_u8, 1, 0];
    let gate = XqEngramGate::new(
        device.clone(),
        Tensor::from_slice(&qw, (4, hidden), &Device::Cpu).unwrap(),
        Tensor::from_slice(&kw, (4, hidden), &Device::Cpu).unwrap(),
    )
    .unwrap();
    let actual = gate
        .forward(
            &Tensor::from_slice(&x, (3, 4, hidden), &Device::Cpu).unwrap(),
            &Tensor::from_slice(&kv, (3, 5, hidden), &Device::Cpu).unwrap(),
            &Tensor::new(&images, &Device::Cpu).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(actual.dtype(), DType::F32);
    close(
        &values(&actual),
        &oracle(&x, &kv, &qw, &kw, &images, hidden),
    );
    for (actual, expected) in values(&actual)[4 * hidden..8 * hidden]
        .iter()
        .zip(&x[4 * hidden..8 * hidden])
    {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(gate.retained_bytes(), hidden as u64 * 32);
}

#[test]
fn zero_signed_root_is_exact_half_and_image_negative_zero_is_preserved() {
    let device = device();
    let gate = XqEngramGate::new(
        device.clone(),
        Tensor::zeros((4, 4), DType::F32, &Device::Cpu).unwrap(),
        Tensor::ones((4, 4), DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    let x = Tensor::full(-0_f32, (2, 4, 4), &Device::Cpu).unwrap();
    let kv = Tensor::full(2_f32, (2, 5, 4), &Device::Cpu).unwrap();
    let result = gate
        .forward(
            &x,
            &kv,
            &Tensor::new(&[0_u8, 1], &Device::Cpu).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(&values(&result)[..16], &[1_f32; 16]);
    assert!(values(&result)[16..]
        .iter()
        .all(|v| v.to_bits() == (-0_f32).to_bits()));
}

#[test]
fn normalize_before_weighted_dot_avoids_raw_product_overflow_and_does_not_round_bf16() {
    let device = device();
    let q = Tensor::full(1e10_f32, (4, 4), &Device::Cpu).unwrap();
    let gate = XqEngramGate::new(device.clone(), q.clone(), q).unwrap();
    let x = Tensor::full(1e18_f32, (1, 4, 4), &Device::Cpu).unwrap();
    let kv = Tensor::full(1e18_f32, (1, 5, 4), &Device::Cpu).unwrap();
    let mask = Tensor::zeros(1, DType::U8, &Device::Cpu).unwrap();
    let result = gate
        .forward(&x, &kv, &mask, &CancellationToken::new())
        .unwrap();
    assert!(values(&result).iter().all(|v| v.is_finite() && *v > 1e18));
    let gate = XqEngramGate::new(
        device,
        Tensor::zeros((4, 4), DType::F32, &Device::Cpu).unwrap(),
        Tensor::ones((4, 4), DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    let x = Tensor::full(1.0001_f32, (1, 4, 4), &Device::Cpu).unwrap();
    let kv = Tensor::zeros((1, 5, 4), DType::F32, &Device::Cpu).unwrap();
    let result = gate
        .forward(&x, &kv, &mask, &CancellationToken::new())
        .unwrap();
    assert!(values(&result)
        .iter()
        .all(|v| v.to_bits() == 1.0001_f32.to_bits()));
}

#[test]
fn gate_owns_parameter_snapshots_and_rejects_dtype_nonfinite_mask_budget_and_cancel() {
    let device = device();
    let query = Var::ones((4, 4), DType::F32, &Device::Cpu).unwrap();
    let gate = XqEngramGate::new(
        device.clone(),
        query.as_tensor().clone(),
        query.as_tensor().clone(),
    )
    .unwrap();
    query
        .set(&Tensor::full(f32::NAN, (4, 4), &Device::Cpu).unwrap())
        .unwrap();
    let x = Tensor::ones((1, 4, 4), DType::F32, &Device::Cpu).unwrap();
    let kv = Tensor::ones((1, 5, 4), DType::F32, &Device::Cpu).unwrap();
    let mask = Tensor::zeros(1, DType::U8, &Device::Cpu).unwrap();
    assert!(gate
        .forward(&x, &kv, &mask, &CancellationToken::new())
        .is_ok());
    for bad in [
        x.to_dtype(DType::BF16).unwrap(),
        Tensor::full(f32::INFINITY, (1, 4, 4), &Device::Cpu).unwrap(),
        Tensor::zeros((1, 3, 4), DType::F32, &Device::Cpu).unwrap(),
    ] {
        assert!(gate
            .forward(&bad, &kv, &mask, &CancellationToken::new())
            .is_err());
    }
    for bad in [
        Tensor::new(&[2_u8], &Device::Cpu).unwrap(),
        Tensor::zeros(2, DType::U8, &Device::Cpu).unwrap(),
        mask.to_dtype(DType::F32).unwrap(),
    ] {
        assert!(gate
            .forward(&x, &kv, &bad, &CancellationToken::new())
            .is_err());
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(gate.forward(&x, &kv, &mask, &cancel).is_err());
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 8191,
        ..InferenceLimits::default()
    });
    let strict_gate = XqEngramGate::new(
        strict,
        Tensor::ones((4, 4), DType::F32, &Device::Cpu).unwrap(),
        Tensor::ones((4, 4), DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    assert!(strict_gate
        .forward(&x, &kv, &mask, &CancellationToken::new())
        .is_err());
    assert!(XqEngramGate::new(
        device.clone(),
        Tensor::ones((4, 4), DType::BF16, &Device::Cpu).unwrap(),
        Tensor::ones((4, 4), DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
    assert!(XqEngramGate::new(
        device,
        Tensor::full(f32::NAN, (4, 4), &Device::Cpu).unwrap(),
        Tensor::ones((4, 4), DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
}

fn parts(device: &CudaLayerDevice, kind: XqTensorType) -> (XqDeviceMatrix, XqEngramGate) {
    let rows = 5 * 4;
    let raw =
        vec![0_u8; (kind.row_bytes(PROJECTED_INPUT_WIDTH as u64).unwrap() * rows as u64) as usize];
    let matrix =
        XqDeviceMatrix::from_bytes_for_tests(device, kind, rows, PROJECTED_INPUT_WIDTH, &raw)
            .unwrap();
    let norm = Tensor::ones((4, 4), DType::F32, device.tensor_device()).unwrap();
    (
        matrix,
        XqEngramGate::new(device.clone(), norm.clone(), norm).unwrap(),
    )
}

#[test]
fn projection_requires_exported_f16_aggregate_residency_and_identical_owner_limits() {
    let device = device();
    let (matrix, gate) = parts(&device, XqTensorType::F32);
    assert!(XqEngramProjection::new(device.clone(), EngramLayer::Layer1, matrix, gate).is_err());
    let (matrix, gate) = parts(&device, XqTensorType::F16);
    let projection =
        XqEngramProjection::new(device.clone(), EngramLayer::Layer14, matrix, gate).unwrap();
    assert_eq!(projection.layer(), EngramLayer::Layer14);
    assert_eq!(projection.hidden_size(), 4);
    assert_eq!(projection.retained_bytes(), (20 * 6144 * 2 + 128) as u64);
    let (matrix, gate) = parts(&device, XqTensorType::F16);
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_context_tokens: 8,
        ..InferenceLimits::default()
    });
    assert!(XqEngramProjection::new(strict, EngramLayer::Layer1, matrix, gate).is_err());
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 20 * 6144 * 2,
        ..InferenceLimits::default()
    });
    let (matrix, gate) = parts(&strict, XqTensorType::F16);
    assert!(XqEngramProjection::new(strict, EngramLayer::Layer1, matrix, gate).is_err());
}

#[test]
fn packed_lookup_projection_refuses_cpu_fallback_and_preserves_cancel_boundary() {
    use crate::layers::engram::lookup::tests::{checkpoint, table};
    let device = device();
    let (matrix, gate) = parts(&device, XqTensorType::F16);
    let projection =
        XqEngramProjection::new(device.clone(), EngramLayer::Layer1, matrix, gate).unwrap();
    let (_dir, checkpoint) = checkpoint(vec![56; 256], vec![127; 8], 1, 264);
    let embeddings = table(&device, &checkpoint, 1)
        .fetch(&[[0; 24]], &CancellationToken::new())
        .unwrap();
    let continuation = || {
        XqMhcContinuation::from_embedding(
            &device,
            &Tensor::ones((1, 4), DType::F32, &Device::Cpu).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap()
    };
    assert!(projection
        .forward(continuation(), &embeddings, &CancellationToken::new())
        .is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(projection
        .forward(continuation(), &embeddings, &cancel)
        .is_err());
    assert_eq!(
        values(
            embeddings
                .activations()
                .data_bytes()
                .to_dtype(DType::F32)
                .as_ref()
                .unwrap()
        )[0],
        56.
    );
}

#[test]
fn sparse_lookup_host_metadata_is_bounded_before_allocating_a_plan() {
    use crate::layers::engram::lookup::tests::{checkpoint, table};
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_input_bytes: 1024,
        ..InferenceLimits::default()
    });
    let (_dir, checkpoint) = checkpoint(vec![56; 256], vec![127; 8], 1, 264);
    assert!(table(&strict, &checkpoint, 1)
        .fetch(&[[0; 24]], &CancellationToken::new())
        .is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
