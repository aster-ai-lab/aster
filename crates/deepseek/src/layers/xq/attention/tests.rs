use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::{Device, Var};

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
fn cache(device: &CudaLayerDevice) -> Tensor {
    Tensor::cat(
        &[
            Tensor::ones((3, 32), DType::F32, device.tensor_device()).unwrap(),
            Tensor::zeros((3, 32), DType::F32, device.tensor_device()).unwrap(),
        ],
        1,
    )
    .unwrap()
}
fn norm(device: &CudaLayerDevice, width: usize) -> XqRmsNorm {
    XqRmsNorm::new(
        device.clone(),
        Tensor::ones(width, DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap()
}
fn input_parts(device: &CudaLayerDevice) -> InputParts {
    InputParts {
        query_a: matrix(device, XqTensorType::Q8_0, 32, 32),
        query_b: matrix(device, XqTensorType::Q8_0, 512, 32),
        window: matrix(device, XqTensorType::Q8_0, 512, 32),
        query_norm: norm(device, 32),
        window_norm: norm(device, 512),
        cos_sin: cache(device),
    }
}
fn output_parts(device: &CudaLayerDevice) -> OutputParts {
    OutputParts {
        group_a: matrix(device, XqTensorType::Q8_0, 64, 256),
        output_b: matrix(device, XqTensorType::Q8_0, 32, 64),
        cos_sin: cache(device),
        heads: 1,
        groups: 2,
        rank: 32,
    }
}

#[test]
fn original_q8_geometry_and_single_rotary_snapshot_have_exact_resident_counts() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let parts = input_parts(&device);
    let expected = parts.query_a.retained_weight_bytes()
        + parts.query_b.retained_weight_bytes()
        + parts.window.retained_weight_bytes()
        + parts.query_norm.retained_bytes()
        + parts.window_norm.retained_bytes()
        + 3 * 64 * 4;
    let input = XqAttentionInputProjection::from_parts(device.clone(), parts).unwrap();
    assert_eq!(input.retained_bytes(), expected);
    assert_eq!(input.heads(), 1);
    assert_eq!(input.hidden_size(), 32);
    let parts = output_parts(&device);
    let expected =
        parts.group_a.retained_weight_bytes() + parts.output_b.retained_weight_bytes() + 3 * 64 * 4;
    let output = XqAttentionOutputProjection::from_parts(device, parts).unwrap();
    assert_eq!(output.retained_bytes(), expected);
    assert_eq!(output.hidden_size(), 32);
}

#[test]
fn mismatched_kind_norm_group_geometry_device_limits_and_aggregate_budget_refuse() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    for change in 0..4 {
        let mut parts = input_parts(&device);
        match change {
            0 => parts.query_a = matrix(&device, XqTensorType::F16, 32, 32),
            1 => parts.query_norm = norm(&device, 64),
            2 => parts.cos_sin = Tensor::ones((3, 32), DType::F32, &Device::Cpu).unwrap(),
            3 => parts.query_b = matrix(&device, XqTensorType::Q8_0, 513, 32),
            _ => unreachable!(),
        }
        assert!(XqAttentionInputProjection::from_parts(device.clone(), parts).is_err());
    }
    for (heads, groups, rank) in [
        (0, 2, 32),
        (1, 0, 32),
        (1, 9, 32),
        (1, 2, 0),
        (1, 2, 33),
        (usize::MAX, 2, 32),
        (1, usize::MAX, 32),
        (1, 2, usize::MAX),
    ] {
        let mut parts = output_parts(&device);
        parts.heads = heads;
        parts.groups = groups;
        parts.rank = rank;
        assert!(XqAttentionOutputProjection::from_parts(device.clone(), parts).is_err());
    }
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 1 << 30,
        ..InferenceLimits::default()
    });
    assert!(XqAttentionInputProjection::from_parts(strict.clone(), input_parts(&device)).is_err());
    assert!(XqAttentionOutputProjection::from_parts(strict, output_parts(&device)).is_err());
    let admitted = XqAttentionInputProjection::from_parts(device.clone(), input_parts(&device))
        .unwrap()
        .retained_bytes();
    let small = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: admitted - 1,
        ..InferenceLimits::default()
    });
    assert!(XqAttentionInputProjection::from_parts(small.clone(), input_parts(&small)).is_err());
}

#[test]
fn rotary_snapshot_does_not_alias_the_supplied_cache() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let mut parts = input_parts(&device);
    let variable = Var::from_tensor(&parts.cos_sin).unwrap();
    parts.cos_sin = variable.as_tensor().clone();
    let input = XqAttentionInputProjection::from_parts(device.clone(), parts).unwrap();
    variable
        .set(&Tensor::zeros((3, 64), DType::F32, &Device::Cpu).unwrap())
        .unwrap();
    let values: Vec<f32> = (0..512).map(|i| i as f32 * 0.003 + 0.0001).collect();
    let q = Tensor::from_vec(values.clone(), (1, 1, 512), &Device::Cpu).unwrap();
    let result = input
        .rope
        .forward_f32_control(&q, &[2], RopeDirection::Forward, &CancellationToken::new())
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1::<f32>()
        .unwrap();
    assert_eq!(result, values);
}

#[test]
fn projections_refuse_wrong_precision_positions_cancellation_and_cpu_numerical_fallback() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let input =
        XqAttentionInputProjection::from_parts(device.clone(), input_parts(&device)).unwrap();
    let output =
        XqAttentionOutputProjection::from_parts(device.clone(), output_parts(&device)).unwrap();
    let x = Tensor::ones((1, 32), DType::F32, &Device::Cpu).unwrap();
    let q = Tensor::ones((1, 1, 512), DType::F32, &Device::Cpu).unwrap();
    for positions in [vec![], vec![3], vec![0, 1]] {
        assert!(input
            .forward(&x, &positions, &CancellationToken::new())
            .is_err());
        assert!(output
            .forward(&q, &positions, &CancellationToken::new())
            .is_err());
    }
    assert!(input
        .forward(
            &x.to_dtype(DType::BF16).unwrap(),
            &[0],
            &CancellationToken::new()
        )
        .is_err());
    assert!(output
        .forward(
            &q.to_dtype(DType::BF16).unwrap(),
            &[0],
            &CancellationToken::new()
        )
        .is_err());
    assert!(input.forward(&x, &[0], &CancellationToken::new()).is_err());
    assert!(output.forward(&q, &[0], &CancellationToken::new()).is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(input.forward(&x, &[0], &cancel).is_err());
    assert!(output.forward(&q, &[0], &cancel).is_err());
    let (_root, weights, _) = crate::weights::xq::storage::tests::fixture(56);
    assert!(XqAttentionInputProjection::load(
        &device,
        &weights,
        40,
        cache(&device),
        &CancellationToken::new()
    )
    .is_err());
    assert!(XqAttentionOutputProjection::load(
        &device,
        &weights,
        40,
        cache(&device),
        &CancellationToken::new()
    )
    .is_err());
    assert!(XqAttentionInputProjection::load(
        &device,
        &weights,
        0,
        cache(&device),
        &CancellationToken::new()
    )
    .is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
