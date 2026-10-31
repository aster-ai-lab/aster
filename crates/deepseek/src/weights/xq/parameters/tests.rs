use super::*;
use crate::weights::xq::storage::tests::fixture_from_tensors;
use aster_power::inference::InferenceLimits;

#[test]
fn small_f16_and_f32_parameters_keep_gguf_dimension_order_and_exact_values() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (_root, weights) = fixture_from_tensors(
        &[
            (
                "half",
                vec![3, 2],
                XqTensorType::F16,
                [0_u16, 0x8000, 0x0001, 0x3c00, 0xbc00, 0x7bff]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            ),
            (
                "float",
                vec![4],
                XqTensorType::F32,
                [1.0001_f32, -2.01, 0., -0.]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            ),
        ],
        128,
    );
    let half = weights
        .load_f32_parameter(
            &device,
            "half",
            XqTensorType::F16,
            &[3, 2],
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(half.dtype(), DType::F32);
    assert_eq!(half.dims(), [2, 3]);
    let values = half.flatten_all().unwrap().to_vec1::<f32>().unwrap();
    assert_eq!(
        values.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        [0., -0., 2_f32.powi(-24), 1., -1., 65504.]
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
    );
    let float = weights
        .load_f32_parameter(
            &device,
            "float",
            XqTensorType::F32,
            &[4],
            &CancellationToken::new(),
        )
        .unwrap()
        .to_vec1::<f32>()
        .unwrap();
    assert_eq!(
        float.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        [1.0001_f32, -2.01, 0., -0.]
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
    );
}

#[test]
fn parameter_loader_refuses_wrong_types_shapes_nonfinite_values_and_budget() {
    let (_root, weights) = fixture_from_tensors(
        &[(
            "float",
            vec![2],
            XqTensorType::F32,
            [1_f32, f32::INFINITY]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )],
        8,
    );
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    for (kind, shape) in [
        (XqTensorType::I32, vec![2]),
        (XqTensorType::F16, vec![2]),
        (XqTensorType::F32, vec![]),
        (XqTensorType::F32, vec![1, 2]),
        (XqTensorType::F32, vec![2]),
    ] {
        assert!(weights
            .load_f32_parameter(&device, "float", kind, &shape, &CancellationToken::new())
            .is_err());
    }
    let (_root, weights) = fixture_from_tensors(
        &[(
            "float",
            vec![2],
            XqTensorType::F32,
            [1_f32, 2.].into_iter().flat_map(f32::to_le_bytes).collect(),
        )],
        4,
    );
    assert!(weights
        .load_f32_parameter(
            &device,
            "float",
            XqTensorType::F32,
            &[2],
            &CancellationToken::new()
        )
        .is_err());
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(weights
        .load_f32_parameter(&device, "float", XqTensorType::F32, &[2], &cancellation)
        .is_err());
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 8,
        ..InferenceLimits::default()
    });
    assert!(weights
        .load_f32_parameter(
            &device,
            "float",
            XqTensorType::F32,
            &[2],
            &CancellationToken::new()
        )
        .is_err());
}
