use super::*;
use aster_power::inference::{InferenceLimits, WeightReadStrategy, WeightStoreConfig};
use tempfile::TempDir;

type FixtureTensor<'a> = (&'a str, &'a str, Vec<usize>, Vec<u8>);

fn write_shard(path: &std::path::Path, tensors: &[FixtureTensor<'_>]) {
    let mut header = serde_json::Map::new();
    let mut data = Vec::new();
    for (name, dtype, shape, bytes) in tensors {
        let start = data.len();
        data.extend_from_slice(bytes);
        header.insert(
            (*name).into(),
            serde_json::json!({"dtype":dtype,"shape":shape,"data_offsets":[start,data.len()]}),
        );
    }
    let mut raw = serde_json::to_vec(&header).unwrap();
    raw.resize(raw.len().div_ceil(8) * 8, b' ');
    let mut file = (raw.len() as u64).to_le_bytes().to_vec();
    file.extend(raw);
    file.extend(data);
    std::fs::write(path, file).unwrap();
}

fn open_store(directory: &std::path::Path) -> Result<WeightStore> {
    WeightStore::open_config(
        &WeightStoreConfig::new(directory)
            .with_primary_read_strategy(WeightReadStrategy::PositionalBuffered),
        &InferenceLimits::default(),
    )
}

pub(crate) fn fixture(tensors: &[FixtureTensor<'_>], budget: u64) -> (TempDir, CheckpointWeights) {
    let dir = tempfile::tempdir().unwrap();
    // A different scale shard verifies tensor-relative ranges across real files.
    for (index, tensor) in tensors.iter().enumerate() {
        write_shard(
            &dir.path().join(format!("part-{index}.safetensors")),
            std::slice::from_ref(tensor),
        );
    }
    let store = open_store(dir.path()).unwrap();
    let index = index_for(&store);
    let files = store.files().to_vec();
    let checkpoint = CheckpointWeights::bind(store, index, &files, budget).unwrap();
    (dir, checkpoint)
}

fn index_for(store: &WeightStore) -> CheckpointIndex {
    CheckpointIndex {
        metadata: IndexMetadata {
            total_size: store.inventory().map(|tensor| tensor.bytes).sum(),
        },
        weight_map: store
            .inventory()
            .map(|tensor| {
                let storage = store.storage_descriptor(&tensor.name).unwrap();
                (
                    tensor.name.clone(),
                    store.files()[storage.file_index].relative_path.clone(),
                )
            })
            .collect(),
    }
}

fn fp8() -> Vec<FixtureTensor<'static>> {
    vec![
        ("w", "F8_E4M3", vec![64, 64], [0, 1, 128, 126].repeat(1024)),
        ("s", "F8_E8M0", vec![2, 2], vec![127, 128, 126, 0]),
    ]
}

#[test]
fn verified_range_reads_upload_exact_fp8_scale_chunks_and_signed_fp4_container_bits() {
    use crate::layers::{linear::DeviceMatrix, CudaLayerDevice};
    use candle_core::{DType, Tensor};
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancellation = CancellationToken::new();
    let (_dir, checkpoint) = fixture(&fp8(), 4096);
    let weights = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [64, 64])
        .unwrap();
    let matrix = DeviceMatrix::load(&device, &weights, 32, 32, &cancellation).unwrap();
    assert_eq!(matrix.logical_shape(), [32, 64]);
    assert_eq!(matrix.source_first_row(), 32);
    assert_eq!(matrix.source_total_rows(), 64);
    assert_eq!(matrix.retained_weight_bytes(), 2048 + 2 + 128);
    let row = matrix
        .gather_rows(
            &Tensor::new(&[0_u32], device.tensor_device()).unwrap(),
            DType::F32,
            &cancellation,
        )
        .unwrap()
        .to_vec2::<f32>()
        .unwrap()
        .remove(0);
    assert_eq!(row[1], 2_f32.powi(-10));
    assert_eq!(row[2].to_bits(), (-0_f32).to_bits());
    assert_eq!(row[3], 224.);
    assert_eq!(row[33], f32::from_bits(1 << 13));
    let tensors = vec![
        ("w", "I8", vec![1, 16], vec![0xab; 16]),
        ("s", "F8_E8M0", vec![1, 1], vec![127]),
    ];
    let (_dir, checkpoint) = fixture(&tensors, 64);
    let weights = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Mxfp4Block32, [1, 32])
        .unwrap();
    let matrix = DeviceMatrix::load(&device, &weights, 0, 1, &cancellation).unwrap();
    let row = matrix
        .gather_rows(
            &Tensor::new(&[0_u32], device.tensor_device()).unwrap(),
            DType::F32,
            &cancellation,
        )
        .unwrap()
        .to_vec2::<f32>()
        .unwrap()
        .remove(0);
    assert_eq!(row, [-1.5, -1.].repeat(16));
    assert_eq!(matrix.retained_weight_bytes(), 17);
    cancellation.cancel();
    assert!(matches!(
        DeviceMatrix::load(&device, &weights, 0, 1, &cancellation),
        Err(PowerError::InferenceCancelled)
    ));
}

#[test]
fn layouts_roundtrip_and_never_infer_int8() {
    for layout in [
        MatrixLayout::Bf16,
        MatrixLayout::F32,
        MatrixLayout::Fp8Block32,
        MatrixLayout::Fp8Row32,
        MatrixLayout::Mxfp4Block32,
    ] {
        assert_eq!(MatrixLayout::parse(layout.as_str()).unwrap(), layout);
    }
    for invalid in ["i8", "fp4", "fp8", "auto", ""] {
        assert!(MatrixLayout::parse(invalid).is_err());
    }
}

#[test]
fn fp8_rows_preserve_original_bytes_and_scale_shard() {
    let tensors = fp8();
    let (_dir, checkpoint) = fixture(&tensors, 4096);
    let matrix = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [64, 64])
        .unwrap();
    let rows = matrix.read_rows(32, 32, &CancellationToken::new()).unwrap();
    assert_eq!(rows.weight.bytes(), &tensors[0].3[2048..4096]);
    assert_eq!(rows.weight.tensor_offset(), 2048);
    let scale = rows.scale.unwrap();
    assert_eq!(scale.bytes(), &[126, 0]);
    assert_eq!(scale.tensor_offset(), 2);
    assert_ne!(rows.weight.storage().file_index, scale.storage().file_index);
    assert_eq!(matrix.descriptor().logical_shape(), [64, 64]);
    assert_eq!(matrix.descriptor().layout(), MatrixLayout::Fp8Block32);
}

#[test]
fn fp8_partial_final_tiles_are_preserved_without_requantization() {
    let tensors = vec![
        ("w", "F8_E4M3", vec![35, 33], vec![42; 35 * 33]),
        ("s", "F8_E8M0", vec![2, 2], vec![127, 128, 129, 130]),
    ];
    let (_dir, checkpoint) = fixture(&tensors, 4096);
    let matrix = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [35, 33])
        .unwrap();
    let rows = matrix.read_rows(32, 3, &CancellationToken::new()).unwrap();
    assert_eq!(rows.weight.bytes(), vec![42; 99]);
    assert_eq!(rows.weight.tensor_offset(), 1056);
    assert_eq!(rows.scale.unwrap().bytes(), &[129, 130]);
}

#[test]
fn packed_fp4_has_two_values_per_byte_and_per_row_scales() {
    let weights = [0x10, 0xfe, 0x87, 0x23].repeat(24);
    let tensors = vec![
        ("w", "I8", vec![3, 32], weights.clone()),
        ("s", "F8_E8M0", vec![3, 2], vec![0, 127, 128, 129, 130, 131]),
    ];
    let (_dir, checkpoint) = fixture(&tensors, 1024);
    let matrix = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Mxfp4Block32, [3, 64])
        .unwrap();
    let rows = matrix.read_rows(1, 1, &CancellationToken::new()).unwrap();
    assert_eq!(rows.weight.bytes(), &weights[32..64]);
    assert_eq!(rows.weight.storage().dtype, "i8");
    assert_eq!(matrix.descriptor().weight().shape, [3, 32]);
    assert_eq!(matrix.descriptor().scale().unwrap().shape, [3, 2]);
    assert_eq!(rows.scale.unwrap().bytes(), &[128, 129]);
}

#[test]
fn engram_per_row_fp8_gathers_exact_rows_and_refuses_dense_scale_layout() {
    let tensors = vec![
        ("embed", "F8_E4M3", vec![3, 64], vec![42; 3 * 64]),
        (
            "sf",
            "F8_E8M0",
            vec![3, 2],
            vec![127, 128, 129, 130, 131, 132],
        ),
    ];
    let (_dir, checkpoint) = fixture(&tensors, 1024);
    let matrix = checkpoint
        .matrix("embed", Some("sf"), MatrixLayout::Fp8Row32, [3, 64])
        .unwrap();
    let row = matrix.read_rows(1, 1, &CancellationToken::new()).unwrap();
    assert_eq!(row.weight.tensor_offset(), 64);
    assert_eq!(row.weight.bytes(), vec![42; 64]);
    assert_eq!(row.scale.unwrap().bytes(), &[129, 130]);
    assert!(checkpoint
        .matrix("embed", Some("sf"), MatrixLayout::Fp8Block32, [3, 64])
        .is_err());
    assert!(checkpoint
        .matrix("embed", Some("sf"), MatrixLayout::Fp8Row32, [3, 63])
        .is_err());
}

#[test]
fn unquantized_rows_preserve_bf16_and_f32_bytes() {
    for (dtype, layout, width) in [
        ("BF16", MatrixLayout::Bf16, 2),
        ("F32", MatrixLayout::F32, 4),
    ] {
        let bytes = (0..6 * width).map(|value| value as u8).collect::<Vec<_>>();
        let (_dir, checkpoint) = fixture(&[("w", dtype, vec![2, 3], bytes.clone())], 1024);
        let matrix = checkpoint.matrix("w", None, layout, [2, 3]).unwrap();
        let rows = matrix.read_rows(1, 1, &CancellationToken::new()).unwrap();
        assert_eq!(rows.weight.bytes(), &bytes[3 * width..]);
        assert!(rows.scale.is_none());
    }
}

#[test]
fn missing_unexpected_and_wrong_scales_refuse() {
    let (_dir, checkpoint) = fixture(&fp8(), 8192);
    assert!(checkpoint
        .matrix("w", None, MatrixLayout::Fp8Block32, [64, 64])
        .is_err());
    assert!(checkpoint
        .matrix("w", Some("missing"), MatrixLayout::Fp8Block32, [64, 64])
        .is_err());
    assert!(checkpoint
        .matrix("w", Some("w"), MatrixLayout::Fp8Block32, [64, 64])
        .is_err());
    assert!(checkpoint
        .matrix("missing", Some("s"), MatrixLayout::Fp8Block32, [64, 64])
        .is_err());
    assert!(checkpoint
        .matrix("w", Some("s"), MatrixLayout::Bf16, [64, 64])
        .is_err());
    for (dtype, shape, bytes) in [
        ("U8", vec![2, 2], vec![127; 4]),
        ("F8_E8M0", vec![4, 1], vec![127; 4]),
        ("F8_E8M0", vec![1, 2], vec![127; 2]),
    ] {
        let mut tensors = fp8();
        tensors[1] = ("s", dtype, shape, bytes);
        let (_dir, checkpoint) = fixture(&tensors, 8192);
        assert!(checkpoint
            .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [64, 64])
            .is_err());
    }
}

#[test]
fn block_shape_dtype_offset_and_arithmetic_overflows_refuse() {
    let (_dir, checkpoint) = fixture(&fp8(), 8192);
    for shape in [[0, 64], [64, 0], [usize::MAX, 2], [64, 32], [64, 65]] {
        assert!(checkpoint
            .matrix("w", Some("s"), MatrixLayout::Fp8Block32, shape)
            .is_err());
    }
    assert!(checkpoint
        .matrix("w", Some("s"), MatrixLayout::Mxfp4Block32, [64, 63])
        .is_err());
    let storage = checkpoint.storage("w").unwrap();
    let scale = checkpoint.storage("s").unwrap();
    let mut broken = storage.clone();
    broken.absolute_offset = u64::MAX;
    assert!(MatrixDescriptor::validate(
        MatrixLayout::Fp8Block32,
        [64, 64],
        broken,
        Some(scale.clone())
    )
    .is_err());
    let mut broken = storage.clone();
    broken.bytes -= 1;
    assert!(MatrixDescriptor::validate(
        MatrixLayout::Fp8Block32,
        [64, 64],
        broken,
        Some(scale.clone())
    )
    .is_err());
    let mut broken = storage.clone();
    broken.dtype = "i8".into();
    assert!(
        MatrixDescriptor::validate(MatrixLayout::Fp8Block32, [64, 64], broken, Some(scale))
            .is_err()
    );
    assert!(MatrixDescriptor::validate(MatrixLayout::F32, [1, usize::MAX], storage, None).is_err());
}

#[test]
fn incomplete_fp8_blocks_and_invalid_row_ranges_refuse() {
    let (_dir, checkpoint) = fixture(&fp8(), 8192);
    let matrix = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [64, 64])
        .unwrap();
    for (start, count) in [(0, 0), (1, 32), (0, 31), (32, 33), (64, 1), (usize::MAX, 2)] {
        assert!(matrix
            .read_rows(start, count, &CancellationToken::new())
            .is_err());
    }
}

#[test]
fn combined_weight_scale_budget_and_cancellation_are_enforced() {
    let (_dir, checkpoint) = fixture(&fp8(), 2049);
    let matrix = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [64, 64])
        .unwrap();
    assert!(matrix.read_rows(0, 32, &CancellationToken::new()).is_err()); // 2048 + 2
    assert!(checkpoint
        .read_range("w", 0, 2050, &CancellationToken::new())
        .is_err());
    assert!(checkpoint
        .read_range("w", 0, 0, &CancellationToken::new())
        .is_err());
    let token = CancellationToken::new();
    token.cancel();
    assert!(checkpoint.read_range("w", 0, 1, &token).is_err());
    assert!(checkpoint
        .read_range("w", u64::MAX, 2, &CancellationToken::new())
        .is_err());
    assert!(checkpoint
        .read_range("w", 4095, 2, &CancellationToken::new())
        .is_err());
}

#[test]
fn ue8m0_reserved_nan_scale_refuses_without_returning_weight_bytes() {
    let mut tensors = fp8();
    tensors[1].3[2] = 255;
    let (_dir, checkpoint) = fixture(&tensors, 8192);
    let matrix = checkpoint
        .matrix("w", Some("s"), MatrixLayout::Fp8Block32, [64, 64])
        .unwrap();
    assert!(matrix.read_rows(32, 32, &CancellationToken::new()).is_err());
    assert!(matrix.read_rows(0, 32, &CancellationToken::new()).is_ok());
}

#[test]
fn truncated_or_malformed_container_refuses_in_power_storage() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("part.safetensors");
    write_shard(&path, &fp8());
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(open_store(dir.path()).is_err());
    std::fs::write(&path, u64::MAX.to_le_bytes()).unwrap();
    assert!(open_store(dir.path()).is_err());
}

#[test]
fn payload_hash_wrong_shard_and_index_total_refuse() {
    let (dir, checkpoint) = fixture(&fp8(), 8192);
    let expected = checkpoint.store.files().to_vec();
    let mut wrong_hash = expected.clone();
    wrong_hash[0].sha256 = "0".repeat(64);
    assert!(validate_files(&expected, &wrong_hash).is_err());
    let mut wrong_size = expected.clone();
    wrong_size[0].bytes += 1;
    assert!(validate_files(&expected, &wrong_size).is_err());
    let mut wrong_name = expected.clone();
    wrong_name[0].relative_path = "other.safetensors".into();
    assert!(validate_files(&expected, &wrong_name).is_err());
    let mut wrong_index = index_for(&checkpoint.store);
    wrong_index
        .weight_map
        .insert("w".into(), "part-1.safetensors".into());
    assert!(CheckpointWeights::bind(
        open_store(dir.path()).unwrap(),
        wrong_index,
        &expected,
        8192
    )
    .is_err());
    let mut wrong_total = index_for(&checkpoint.store);
    wrong_total.metadata.total_size += 1;
    assert!(CheckpointWeights::bind(
        open_store(dir.path()).unwrap(),
        wrong_total,
        &expected,
        8192
    )
    .is_err());
    let mut missing = index_for(&checkpoint.store);
    missing.weight_map.remove("s");
    assert!(
        CheckpointWeights::bind(open_store(dir.path()).unwrap(), missing, &expected, 8192).is_err()
    );
    // A same-length mutation produces a new Power file hash, refused against the pin.
    let path = dir.path().join(&expected[0].relative_path);
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    let changed = open_store(dir.path()).unwrap();
    assert!(
        CheckpointWeights::bind(changed, index_for(&checkpoint.store), &expected, 8192).is_err()
    );
}

#[test]
fn index_has_bounded_exact_hash_and_closed_artifact_schema() {
    let good = br#"{"metadata":{"total_size":4},"weight_map":{"w":"part.safetensors"}}"#;
    let digest = format!("{:x}", Sha256::digest(good));
    assert!(parse_index(good, &digest).is_ok());
    assert!(parse_index(good, OFFICIAL_INDEX_SHA256).is_err());
    for bytes in [
        b"{".as_slice(),
        br#"{"metadata":{"total_size":4},"weight_map":{},"unknown":true}"#,
        br#"{"metadata":{"total_size":4},"weight_map":{}}"#,
        br#"{"metadata":{"total_size":0},"weight_map":{"w":"x"}}"#,
    ] {
        assert!(parse_index(bytes, &format!("{:x}", Sha256::digest(bytes))).is_err());
    }
    assert!(parse_index(&vec![0; MAX_INDEX_BYTES + 1], &digest).is_err());
    assert!(parse_index(b"", &digest).is_err());
}

#[test]
fn public_official_constructor_refuses_tiny_unpinned_checkpoint() {
    let (_dir, checkpoint) = fixture(&fp8(), 8192);
    let config = crate::model::DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../fixtures/official-config.json"))
        .unwrap();
    assert!(CheckpointWeights::from_store(checkpoint.store, &config, b"{}", 8192).is_err());
}
