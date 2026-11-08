use super::*;
use crate::{layers::linear::DeviceMatrix, weights::tests::fixture};
use aster_power::inference::InferenceLimits;

#[test]
fn multichunk_fp8_upload_preserves_global_scale_rows_and_partial_checkpoint_tail() {
    let rows = 97;
    let data: Vec<_> = (0..rows)
        .flat_map(|r| vec![if r % 2 == 0 { 56 } else { 184 }; 64])
        .collect();
    let scales = vec![126, 127, 128, 129, 130, 131, 132, 133];
    let (_dir, checkpoint) = fixture(
        &[
            ("w", "F8_E4M3", vec![rows, 64], data.clone()),
            ("s", "F8_E8M0", vec![4, 2], scales.clone()),
        ],
        2050,
    );
    let binding = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [rows, 64])
        .unwrap();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancellation = CancellationToken::new();
    let loaded = DeviceMatrix::load(&device, &binding, 0, rows, &cancellation).unwrap();
    assert_eq!(
        loaded.data.flatten_all().unwrap().to_vec1::<u8>().unwrap(),
        data
    );
    assert_eq!(
        loaded
            .scales
            .as_ref()
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<u8>()
            .unwrap(),
        scales
    );
    let partial = DeviceMatrix::load(&device, &binding, 32, 65, &cancellation).unwrap();
    assert_eq!(partial.source_first_row(), 32);
    assert_eq!(partial.source_total_rows(), 97);
    assert_eq!(
        partial
            .scales
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<u8>()
            .unwrap(),
        scales[2..]
    );
    let ids = Tensor::new(&[0_u32, 31, 32, 95, 96], device.tensor_device()).unwrap();
    let actual = loaded
        .gather_rows(&ids, DType::F32, &cancellation)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    for (row, source) in actual.iter().zip([0, 31, 32, 95, 96]) {
        let sign = if source % 2 == 0 { 1. } else { -1. };
        assert_eq!(
            row[0],
            sign * 2_f32.powi(scales[(source / 32) * 2] as i32 - 127)
        );
        assert_eq!(
            row[32],
            sign * 2_f32.powi(scales[(source / 32) * 2 + 1] as i32 - 127)
        );
    }
}

#[test]
fn chunked_packed_per_row_formats_and_bf16_f32_keep_every_source_byte() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancellation = CancellationToken::new();
    for (layout, dtype, cols, budget) in [
        (MatrixLayout::Mxfp4Block32, "I8", 16, 17),
        (MatrixLayout::Fp8Row32, "F8_E4M3", 32, 33),
    ] {
        let bytes: Vec<_> = (0..5)
            .flat_map(|row| {
                vec![
                    if layout == MatrixLayout::Mxfp4Block32 {
                        0x98 + row
                    } else {
                        56 + row
                    };
                    cols
                ]
            })
            .collect();
        let (_dir, checkpoint) = fixture(
            &[
                ("w", dtype, vec![5, cols], bytes.clone()),
                ("s", "F8_E8M0", vec![5, 1], vec![125, 126, 127, 128, 129]),
            ],
            budget,
        );
        let binding = checkpoint.matrix("w", Some("s"), layout, [5, 32]).unwrap();
        let loaded = DeviceMatrix::load(&device, &binding, 0, 5, &cancellation).unwrap();
        assert_eq!(
            loaded.data.flatten_all().unwrap().to_vec1::<u8>().unwrap(),
            bytes
        );
        assert_eq!(
            loaded
                .scales
                .unwrap()
                .flatten_all()
                .unwrap()
                .to_vec1::<u8>()
                .unwrap(),
            [125, 126, 127, 128, 129]
        );
    }
    for (layout, dtype, bytes_per_value) in [
        (MatrixLayout::Bf16, "BF16", 2),
        (MatrixLayout::F32, "F32", 4),
    ] {
        let raw = if bytes_per_value == 2 {
            [0x8000_u16, 1, 0x3f80, 0xbf80]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        } else {
            [-0_f32, f32::from_bits(1), 1., -1.]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>()
        };
        let (_dir, checkpoint) = fixture(&[("w", dtype, vec![4, 1], raw)], bytes_per_value);
        let binding = checkpoint.matrix("w", None, layout, [4, 1]).unwrap();
        let loaded = DeviceMatrix::load(&device, &binding, 0, 4, &cancellation).unwrap();
        let actual = loaded
            .data
            .to_dtype(DType::F32)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        assert_eq!(actual[0].to_bits(), (-0_f32).to_bits());
        assert_eq!(
            actual[1].to_bits(),
            if bytes_per_value == 2 { 0x10000 } else { 1 }
        );
        assert_eq!(&actual[2..], &[1., -1.]);
    }
}

#[test]
fn upload_refuses_inadequate_block_budget_misalignment_invalid_range_and_late_bad_scale() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancellation = CancellationToken::new();
    let tensors = [
        ("w", "F8_E4M3", vec![65, 32], vec![56; 65 * 32]),
        ("s", "F8_E8M0", vec![3, 1], vec![127; 3]),
    ];
    let (_dir, checkpoint) = fixture(&tensors, 1024);
    let binding = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [65, 32])
        .unwrap();
    assert!(DeviceMatrix::load(&device, &binding, 0, 65, &cancellation).is_err());
    // A final short checkpoint tail needs its actual bytes, not 32 full rows.
    DeviceMatrix::load(&device, &binding, 64, 1, &cancellation).unwrap();
    let (_dir, checkpoint) = fixture(&tensors, 1025);
    let binding = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [65, 32])
        .unwrap();
    for (first, rows) in [(1, 32), (0, 33), (64, 2), (usize::MAX, 2), (0, 0)] {
        assert!(DeviceMatrix::load(&device, &binding, first, rows, &cancellation).is_err());
    }
    let (_dir, checkpoint) = fixture(
        &[
            ("w", "F8_E4M3", vec![65, 32], vec![56; 65 * 32]),
            ("s", "F8_E8M0", vec![3, 1], vec![127, 127, 255]),
        ],
        1025,
    );
    let binding = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [65, 32])
        .unwrap();
    assert!(DeviceMatrix::load(&device, &binding, 0, 65, &cancellation).is_err());
    cancellation.cancel();
    assert!(DeviceMatrix::load(&device, &binding, 0, 65, &cancellation).is_err());
}

#[test]
fn upload_checks_chunk_metadata_before_allocating_tensor_lists() {
    let (_dir, checkpoint) = fixture(&[("w", "F32", vec![16, 1], vec![0; 64])], 4);
    let binding = checkpoint
        .matrix("w", None, MatrixLayout::F32, [16, 1])
        .unwrap();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 2048,
        ..InferenceLimits::default()
    });
    assert!(DeviceMatrix::load(&device, &binding, 0, 16, &CancellationToken::new()).is_err());
}
