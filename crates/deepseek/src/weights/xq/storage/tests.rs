use aster_power::inference::{InferenceLimits, WeightFileDescriptor};
use sha2::{Digest, Sha256};

use super::*;
use crate::weights::xq::XqTensorType;

pub(crate) type SmallTensor<'a> = (&'a str, Vec<u64>, XqTensorType, Vec<u8>);

#[test]
fn model_metadata_is_bound_to_xq_identity_and_missing_or_altered_config_refuses() {
    let config = include_bytes!("../../../../fixtures/xq-model-config.json");
    let tensors = [("dummy", vec![1], XqTensorType::F32, vec![0; 4])];
    let string_value = |bytes: &[u8]| {
        let mut value = (bytes.len() as u64).to_le_bytes().to_vec();
        value.extend(bytes);
        value
    };
    let (_dir, weights) = fixture_with_metadata(
        &tensors,
        &[("deepseek41.config", 8, string_value(config))],
        8192,
    );
    let bound = weights.model_config().unwrap();
    assert_eq!(bound.model(), crate::model::DeepSeekModel::V41Flash1017Xq);
    assert_eq!(bound.sha256(), crate::model::XQ_CONFIG_SHA256);
    let (_dir, missing) = fixture_from_tensors(&tensors, 8192);
    assert!(missing.model_config().is_err());
    let changed = std::str::from_utf8(config).unwrap().replace("5120", "4096");
    let (_dir, changed) = fixture_with_metadata(
        &tensors,
        &[("deepseek41.config", 8, string_value(changed.as_bytes()))],
        8192,
    );
    assert!(changed.model_config().is_err());
}

pub(crate) fn fixture_from_tensors(
    tensors: &[SmallTensor<'_>],
    budget: u64,
) -> (tempfile::TempDir, XqWeights) {
    fixture_with_metadata(tensors, &[], budget)
}

pub(crate) fn fixture_with_metadata(
    tensors: &[SmallTensor<'_>],
    metadata: &[(&str, u32, Vec<u8>)],
    budget: u64,
) -> (tempfile::TempDir, XqWeights) {
    let mut header = b"GGUF".to_vec();
    header.extend(3_u32.to_le_bytes());
    header.extend((tensors.len() as u64).to_le_bytes());
    header.extend((1 + metadata.len() as u64).to_le_bytes());
    string("general.alignment", &mut header);
    header.extend(4_u32.to_le_bytes());
    header.extend(32_u32.to_le_bytes());
    for (name, kind, raw) in metadata {
        string(name, &mut header);
        header.extend(kind.to_le_bytes());
        header.extend(raw);
    }
    let mut payload = Vec::new();
    for (name, shape, kind, raw) in tensors {
        assert_eq!(kind.tensor_bytes(shape).unwrap(), raw.len() as u64);
        payload.resize(payload.len().div_ceil(32) * 32, 0);
        string(name, &mut header);
        header.extend((shape.len() as u32).to_le_bytes());
        for value in shape {
            header.extend(value.to_le_bytes());
        }
        header.extend(kind.gguf_id().to_le_bytes());
        header.extend((payload.len() as u64).to_le_bytes());
        payload.extend(raw);
    }
    assert!(payload.len() <= 8 * 1024 * 1024);
    header.resize(header.len().div_ceil(32) * 32, 0);
    let mut bytes = header.clone();
    bytes.extend(payload);
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("small.gguf"), &bytes).unwrap();
    let source = VerifiedRawWeightFile::open(
        root.path(),
        WeightFileDescriptor {
            relative_path: "small.gguf".into(),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        },
        &InferenceLimits::default(),
        budget,
        &CancellationToken::new(),
    )
    .unwrap();
    let header = XqGgufHeader::parse(&header, bytes.len() as u64).unwrap();
    (root, XqWeights { source, header })
}

fn string(value: &str, output: &mut Vec<u8>) {
    output.extend((value.len() as u64).to_le_bytes());
    output.extend(value.as_bytes());
}

pub(crate) fn fixture(budget: u64) -> (tempfile::TempDir, XqWeights, Vec<u8>) {
    let mut raw = b"GGUF".to_vec();
    raw.extend(3_u32.to_le_bytes());
    raw.extend(2_u64.to_le_bytes());
    raw.extend(1_u64.to_le_bytes());
    string("general.alignment", &mut raw);
    raw.extend(4_u32.to_le_bytes());
    raw.extend(32_u32.to_le_bytes());
    for (name, shape, kind, offset) in [
        ("stacked", vec![128_u64, 2, 3], 143_u32, 0_u64),
        ("map", vec![4], 26, 192),
    ] {
        string(name, &mut raw);
        raw.extend((shape.len() as u32).to_le_bytes());
        for value in shape {
            raw.extend(value.to_le_bytes());
        }
        raw.extend(kind.to_le_bytes());
        raw.extend(offset.to_le_bytes());
    }
    raw.resize(raw.len().div_ceil(32) * 32, 0);
    let header = raw.clone();
    // Six PTQ1_0 rows, preserving all packed bytes and in-block scale bits.
    let payload: Vec<_> = (0..168).map(|value| (value % 251) as u8).collect();
    raw.extend(&payload);
    raw.resize(header.len() + 192, 0);
    for value in [11_i32, 42, 143, 16_777_217] {
        raw.extend(value.to_le_bytes());
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("weights.gguf"), &raw).unwrap();
    let source = VerifiedRawWeightFile::open(
        root.path(),
        WeightFileDescriptor {
            relative_path: "weights.gguf".into(),
            bytes: raw.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&raw)),
        },
        &InferenceLimits::default(),
        budget,
        &CancellationToken::new(),
    )
    .unwrap();
    let header = XqGgufHeader::parse(&header, raw.len() as u64).unwrap();
    // Tiny format fixture: no public constructor accepts this as the real XQ model.
    (root, XqWeights { source, header }, payload)
}

#[test]
fn stacked_expert_rows_keep_their_physical_slot_order_and_packed_bytes() {
    let (_root, weights, payload) = fixture(168);
    let read = weights
        .read_rows("stacked", 2, 2, &CancellationToken::new())
        .unwrap();
    assert_eq!(read.bytes(), &payload[56..112]);
    assert_eq!(read.first_row(), 2);
    assert_eq!(read.rows(), 2);
    assert_eq!(read.tensor().shape, [128, 2, 3]);
    assert_eq!(read.tensor().kind, XqTensorType::Ptq1_0);
    assert_eq!(read.absolute_offset(), weights.header.data_offset() + 56);
    assert_eq!(
        weights
            .read_tensor("stacked", &CancellationToken::new())
            .unwrap()
            .bytes(),
        payload
    );
}

#[test]
fn integer_metadata_uses_the_typed_exact_reader() {
    let (_root, weights, _) = fixture(168);
    let read = weights
        .read_tensor("map", &CancellationToken::new())
        .unwrap();
    assert_eq!(read.tensor().kind, XqTensorType::I32);
    assert_eq!(
        XqTensorType::I32
            .decode_i32_row(read.bytes(), 4, 16)
            .unwrap(),
        [11, 42, 143, 16_777_217]
    );
}

#[test]
fn out_of_range_unknown_cancelled_and_over_budget_reads_fail_closed() {
    let (_root, weights, _) = fixture(56);
    for (first, rows) in [(0, 0), (6, 1), (5, 2), (u64::MAX, 2)] {
        assert!(weights
            .read_rows("stacked", first, rows, &CancellationToken::new())
            .is_err());
    }
    assert!(weights
        .read_tensor("unknown", &CancellationToken::new())
        .is_err());
    assert!(weights
        .read_tensor("stacked", &CancellationToken::new())
        .is_err());
    assert!(weights
        .read_rows("stacked", 0, 2, &CancellationToken::new())
        .is_ok());
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(weights.read_rows("stacked", 0, 1, &cancellation).is_err());
}

#[test]
fn public_binding_refuses_a_valid_but_unpinned_file() {
    let (_root, weights, _) = fixture(168);
    assert!(XqWeights::from_verified_file(weights.source, &CancellationToken::new()).is_err());
}

#[test]
fn absolute_offset_and_row_geometry_overflow_are_refused() {
    let tensor = XqGgufTensor {
        shape: vec![128, 2],
        kind: XqTensorType::Ptq1_0,
        absolute_offset: u64::MAX - 7,
        bytes: 56,
    };
    assert!(row_range(&tensor, 0, 1).is_err());
    assert!(row_range(&tensor, 1, 1).is_err());
    let tensor = XqGgufTensor {
        shape: vec![0],
        ..tensor
    };
    assert!(row_range(&tensor, 0, 1).is_err());
}
