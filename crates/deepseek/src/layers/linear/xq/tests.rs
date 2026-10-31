use super::*;
use aster_power::inference::InferenceLimits;

#[test]
fn expert_row_views_keep_parent_allocation_and_exact_packed_offsets() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let raw: Vec<u8> = (0..5 * 28).map(|i| i as u8).collect();
    let matrix =
        XqDeviceMatrix::from_bytes_for_tests(&device, XqTensorType::Ptq1_0, 5, 128, &raw).unwrap();
    let part = matrix.row_view(2, 2).unwrap();
    assert_eq!(
        part.flatten_all().unwrap().to_vec1::<u8>().unwrap(),
        raw[56..112]
    );
    assert_eq!(matrix.retained_weight_bytes(), 140);
    for (first, rows) in [(0, 0), (5, 1), (4, 2), (usize::MAX, 2)] {
        assert!(matrix
            .forward_rows_f32_control(
                &Tensor::zeros((1, 128), DType::F32, device.tensor_device()).unwrap(),
                first,
                rows,
                &CancellationToken::new()
            )
            .is_err());
    }
}

#[test]
fn chunked_verified_upload_retains_all_packed_bytes_and_selected_rows() {
    let (_root, weights, payload) = crate::weights::xq::storage::tests::fixture(56);
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let matrix = XqDeviceMatrix::load(
        &device,
        &weights,
        "stacked",
        1,
        5,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(matrix.kind(), XqTensorType::Ptq1_0);
    assert_eq!(matrix.logical_shape(), [5, 128]);
    assert_eq!(matrix.retained_weight_bytes(), 140);
    assert_eq!(
        matrix
            .packed
            .flatten_all()
            .unwrap()
            .to_vec1::<u8>()
            .unwrap(),
        payload[28..]
    );
}

#[test]
fn integer_empty_overflowing_and_under_budget_uploads_refuse() {
    let (_root, weights, _) = crate::weights::xq::storage::tests::fixture(27);
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    for (name, first, rows) in [
        ("map", 0, 1),
        ("stacked", 0, 0),
        ("stacked", u64::MAX, 2),
        ("stacked", 5, 2),
        ("stacked", 0, 1),
    ] {
        assert!(XqDeviceMatrix::load(
            &device,
            &weights,
            name,
            first,
            rows,
            &CancellationToken::new()
        )
        .is_err());
    }
    let (_root, weights, _) = crate::weights::xq::storage::tests::fixture(56);
    let limits = InferenceLimits {
        max_resident_weight_bytes: 27,
        ..InferenceLimits::default()
    };
    let device = CudaLayerDevice::cpu_for_tests(limits);
    assert!(XqDeviceMatrix::load(
        &device,
        &weights,
        "stacked",
        0,
        1,
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn pure_payload_validation_refuses_nonfinite_values_scales_and_integer_types() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/xq-codec-golden.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let kind = XqTensorType::from_gguf_id(case["type_id"].as_u64().unwrap() as u32).unwrap();
        let columns = case["columns"].as_u64().unwrap();
        let hex = case["raw_hex"].as_str().unwrap();
        let raw: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        kind.validate_floating_payload(&raw, columns, 1).unwrap();
        assert!(kind.validate_floating_payload(&raw, columns, 2).is_err());
        let mut bad = raw;
        let offset = match kind {
            XqTensorType::Q3K => 108,
            XqTensorType::Ptq1_0 => 26,
            _ => 0,
        };
        if kind == XqTensorType::F32 {
            bad[0..4].copy_from_slice(&f32::INFINITY.to_le_bytes());
        } else {
            bad[offset..offset + 2].copy_from_slice(&0x7c00_u16.to_le_bytes());
        }
        assert!(kind.validate_floating_payload(&bad, columns, 1).is_err());
    }
    for kind in [XqTensorType::I8, XqTensorType::I32] {
        assert!(kind.validate_floating_payload(&[0; 4], 1, 1).is_err());
    }
}

#[test]
fn upload_honors_cancellation_and_projection_never_uses_cpu_math() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (_root, weights, _) = crate::weights::xq::storage::tests::fixture(56);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(XqDeviceMatrix::load(&device, &weights, "stacked", 0, 1, &cancellation).is_err());
    let matrix = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F32,
        1,
        1,
        &1_f32.to_le_bytes(),
    )
    .unwrap();
    let input = Tensor::from_vec(vec![1_f32], (1, 1), device.tensor_device()).unwrap();
    assert!(matrix
        .forward_f32_control(&input, &CancellationToken::new())
        .is_err());
}
