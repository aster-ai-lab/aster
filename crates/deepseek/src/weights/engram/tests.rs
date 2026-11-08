use super::*;
use aster_power::inference::{InferenceLimits, WeightFileDescriptor};
use sha2::{Digest, Sha256};
use std::io::{Seek, SeekFrom, Write};

fn source(bytes: &[u8], budget: u64) -> (tempfile::TempDir, VerifiedRawWeightFile) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("canonical-blob"), bytes).unwrap();
    let descriptor = WeightFileDescriptor {
        relative_path: "canonical-blob".into(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    };
    let file = VerifiedRawWeightFile::open(
        root.path(),
        descriptor,
        &InferenceLimits::default(),
        budget,
        &CancellationToken::new(),
    )
    .unwrap();
    (root, file)
}

#[test]
fn raw_table_ranges_keep_exact_value_scale_bytes_and_authenticate_later_reads() {
    let mut bytes: Vec<_> = (0..4 * 256).map(|index| (index % 251) as u8).collect();
    bytes.extend((0..32).map(|index| index as u8 + 100));
    let (root, file) = source(&bytes, 528);
    let table = RawEngramFile {
        source: file,
        layer: EngramLayer::Layer1,
        rows: 4,
        weight_start: 0,
        scale_start: 1024,
    };
    let matrix = RawEngramMatrix(&table);
    let read = matrix.read_rows(1, 2, &CancellationToken::new()).unwrap();
    assert_eq!(read.weight.bytes(), &bytes[256..768]);
    assert_eq!(read.scale.bytes(), &bytes[1032..1048]);
    assert_eq!(read.rows, 2);
    assert!(matrix.read_rows(0, 3, &CancellationToken::new()).is_err());
    assert!(matrix.read_rows(3, 2, &CancellationToken::new()).is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matrix.read_rows(0, 1, &cancelled).is_err());
    let mut writer = std::fs::OpenOptions::new()
        .write(true)
        .open(root.path().join("canonical-blob"))
        .unwrap();
    writer.seek(SeekFrom::Start(260)).unwrap();
    writer.write_all(&[42]).unwrap();
    writer.sync_all().unwrap();
    assert!(matrix.read_rows(1, 1, &CancellationToken::new()).is_err());
}

fn header(layer: EngramLayer, rows: usize) -> (serde_json::Value, u64) {
    let mut tensors = serde_json::Map::new();
    // Independently enumerate actual file order and byte extents. No table
    // payload is materialized by this metadata fixture.
    let layouts = [
        (
            "embed.weight",
            "F8_E4M3",
            vec![rows, 256],
            rows as u64 * 256,
        ),
        ("embed.scale", "F8_E8M0", vec![rows, 8], rows as u64 * 8),
        ("q_weight", "BF16", vec![4, 5120], 40960),
        ("k_weight", "BF16", vec![4, 5120], 40960),
        ("wkv.weight", "F8_E4M3", vec![25600, 6144], 157286400),
        ("wkv.scale", "F8_E8M0", vec![800, 192], 153600),
    ];
    let mut offset = 0;
    for (suffix, dtype, shape, bytes) in layouts {
        tensors.insert(
            format!("layers.{}.engram.{suffix}", layer.as_str()),
            serde_json::json!({"dtype":dtype,"shape":shape,"data_offsets":[offset,offset+bytes]}),
        );
        offset += bytes;
    }
    (serde_json::Value::Object(tensors), offset)
}

#[test]
fn exact_six_tensor_catalog_refuses_wrong_names_layout_dtypes_and_payload_extents() {
    for layer in [EngramLayer::Layer1, EngramLayer::Layer14] {
        let rows = layer.table_rows() as usize;
        let (value, bytes) = header(layer, rows);
        let encoded = serde_json::to_vec(&value).unwrap();
        let length = encoded.len() as u64;
        assert_eq!(
            validate_catalog(&encoded, layer, rows, 8 + length + bytes).unwrap(),
            (8 + length, 8 + length + rows as u64 * 256)
        );
        for mutation in ["dtype", "shape", "data_offsets"] {
            let mut bad = value.clone();
            bad[format!("layers.{}.engram.embed.weight", layer.as_str())][mutation] = match mutation
            {
                "dtype" => serde_json::json!("I8"),
                "shape" => serde_json::json!([rows, 128]),
                _ => serde_json::json!([1, rows as u64 * 256 + 1]),
            };
            let encoded = serde_json::to_vec(&bad).unwrap();
            assert!(
                validate_catalog(&encoded, layer, rows, 8 + encoded.len() as u64 + bytes).is_err()
            );
        }
        let mut missing = value.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove(&format!("layers.{}.engram.embed.scale", layer.as_str()));
        assert!(validate_catalog(
            &serde_json::to_vec(&missing).unwrap(),
            layer,
            rows,
            8 + length + bytes
        )
        .is_err());
        assert!(validate_catalog(&encoded, layer, rows, 8 + length + bytes + 1).is_err());
    }
}

#[test]
fn public_table_constructor_refuses_unpinned_files_before_metadata_binding() {
    let (_one, first) = source(b"not an official Engram file", 4096);
    let (_two, second) = source(b"not an official Engram file", 4096);
    assert!(
        VerifiedEngramTables::from_verified_files([first, second], &CancellationToken::new())
            .is_err()
    );
}
