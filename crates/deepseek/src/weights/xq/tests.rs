use super::*;
use sha2::{Digest, Sha256};

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../fixtures/xq-codec-golden.json")).unwrap()
}

fn decode_hex(raw: &str) -> Vec<u8> {
    raw.as_bytes()
        .chunks_exact(2)
        .map(|b| {
            (char::from(b[0]).to_digit(16).unwrap() * 16 + char::from(b[1]).to_digit(16).unwrap())
                as u8
        })
        .collect()
}

fn digest_bits(values: &[f32]) -> String {
    let mut hash = Sha256::new();
    for value in values {
        hash.update(value.to_bits().to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}

#[test]
fn pinned_numpy_vectors_match_every_decoded_f32_bit() {
    let fixture = fixture();
    assert_eq!(fixture["schema"], "aster.xq-codec-golden.v1");
    assert_eq!(fixture["numpy"], "1.26.4");
    assert_eq!(
        fixture["revision"],
        "b3452666e07a6374d06bb8c2185dffa271e8f65f"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        let kind = XqTensorType::from_gguf_id(case["type_id"].as_u64().unwrap() as u32).unwrap();
        let raw = decode_hex(case["raw_hex"].as_str().unwrap());
        let columns = case["columns"].as_u64().unwrap() as usize;
        let decoded = kind
            .decode_f32_row(&raw, columns, columns as u64 * 4)
            .unwrap();
        let expected = case["expected_bits"].as_array().unwrap();
        assert_eq!(decoded.len(), expected.len());
        for (index, (actual, expected)) in decoded.iter().zip(expected).enumerate() {
            assert_eq!(
                u64::from(actual.to_bits()),
                expected.as_u64().unwrap(),
                "{} at logical element {index}",
                case["name"]
            );
        }
    }
}

#[test]
fn all_ptq_and_q2_packed_bytes_match_the_independent_reference_digest() {
    let fixture = fixture();
    for (kind, bytes, offset) in [(XqTensorType::Ptq1_0, 28, 26), (XqTensorType::Q2_0, 18, 0)] {
        let mut raw = Vec::new();
        for byte in 0..=255_u8 {
            let mut block = vec![byte; bytes];
            block[offset..offset + 2].copy_from_slice(&0x3c00_u16.to_le_bytes());
            raw.extend(block);
        }
        let columns = 256 * kind.block_layout().0 as usize;
        let values = kind
            .decode_f32_row(&raw, columns, columns as u64 * 4)
            .unwrap();
        assert_eq!(
            digest_bits(&values),
            fixture["exhaustive_hashes"][kind.gguf_id().to_string()]
        );
    }
}

#[test]
fn all_63488_finite_half_encodings_preserve_bits_against_numpy() {
    let raw = (0..=u16::MAX)
        .filter(|h| h & 0x7c00 != 0x7c00)
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let columns = raw.len() / 2;
    assert_eq!(columns, 63488);
    let values = XqTensorType::F16
        .decode_f32_row(&raw, columns, columns as u64 * 4)
        .unwrap();
    assert_eq!(
        digest_bits(&values),
        fixture()["exhaustive_hashes"]["all_finite_f16"]
    );
}

#[test]
fn type_names_and_ids_are_a_closed_roundtripping_set() {
    for (name, id) in [
        ("F32", 0),
        ("F16", 1),
        ("Q8_0", 8),
        ("Q3_K", 11),
        ("I8", 24),
        ("I32", 26),
        ("Q2_0", 42),
        ("PTQ1_0", 143),
    ] {
        let kind = XqTensorType::parse(name).unwrap();
        assert_eq!(kind.as_str(), name);
        assert_eq!(kind.gguf_id(), id);
        assert_eq!(XqTensorType::from_gguf_id(id).unwrap(), kind);
    }
    for id in [2, 10, 34, 43, 144, u32::MAX] {
        assert!(XqTensorType::from_gguf_id(id).is_err());
    }
    for name in ["", "ptq1_0", "TQ1_0", "Q2_K", "F32 "] {
        assert!(XqTensorType::parse(name).is_err());
    }
}

#[test]
fn tensor_sizes_require_complete_blocks_per_row_and_checked_products() {
    assert_eq!(
        XqTensorType::Ptq1_0
            .tensor_bytes(&[5120, 2304, 384])
            .unwrap(),
        990904320
    );
    assert_eq!(
        XqTensorType::Q3K.tensor_bytes(&[2304, 5120, 17]).unwrap(),
        86169600
    );
    assert!(XqTensorType::Q3K.tensor_bytes(&[128, 2]).is_err());
    assert!(XqTensorType::Q2_0.tensor_bytes(&[63, 64]).is_err());
    for shape in [
        vec![],
        vec![0],
        vec![32, 0],
        vec![1; 5],
        vec![u64::MAX],
        vec![1, u64::MAX, 2],
    ] {
        assert!(XqTensorType::F32.tensor_bytes(&shape).is_err());
    }
    assert!(XqTensorType::F32.row_bytes(u64::MAX).is_err());
}

#[test]
fn row_decoding_refuses_truncation_trailing_data_partial_blocks_and_output_overrun() {
    let kind = XqTensorType::Ptq1_0;
    assert!(kind.decode_f32_row(&[0; 27], 128, 512).is_err());
    assert!(kind.decode_f32_row(&[0; 29], 128, 512).is_err());
    assert!(kind.decode_f32_row(&[0; 28], 128, 511).is_err());
    assert!(kind.decode_f32_row(&[0; 28], 127, 512).is_err());
    assert!(kind.decode_f32_row(&[], 0, 512).is_err());
    assert!(XqTensorType::F32
        .decode_f32_row(&[], usize::MAX, u64::MAX)
        .is_err());
}

#[test]
fn nonfinite_weights_and_scales_are_refused_in_each_block_format() {
    for raw in [0x7c00_u16, 0xfc00, 0x7e00, 0x7c01, 0xffff] {
        assert!(XqTensorType::F16
            .decode_f32_row(&raw.to_le_bytes(), 1, 4)
            .is_err());
        for (kind, bytes, offset, columns) in [
            (XqTensorType::Q8_0, 34, 0, 32),
            (XqTensorType::Q2_0, 18, 0, 64),
            (XqTensorType::Ptq1_0, 28, 26, 128),
            (XqTensorType::Q3K, 110, 108, 256),
        ] {
            let mut block = vec![0; bytes * 2];
            block[bytes + offset..bytes + offset + 2].copy_from_slice(&raw.to_le_bytes());
            assert!(kind
                .decode_f32_row(&block, columns * 2, columns as u64 * 8)
                .is_err());
        }
    }
    for raw in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        assert!(XqTensorType::F32
            .decode_f32_row(&raw.to_le_bytes(), 1, 4)
            .is_err());
    }
}

#[test]
fn integer_metadata_keeps_all_bits_and_never_silently_converts_to_float() {
    let input = [i32::MIN, -1, 0, 11, 42, 143, 16_777_217, i32::MAX];
    let raw = input
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(
        XqTensorType::I32
            .decode_i32_row(&raw, input.len(), raw.len() as u64)
            .unwrap(),
        input
    );
    assert!(XqTensorType::I32
        .decode_f32_row(&raw, input.len(), 64)
        .is_err());
    assert!(XqTensorType::I8.decode_f32_row(&[0], 1, 4).is_err());
    assert!(XqTensorType::F32
        .decode_i32_row(&raw, input.len(), 64)
        .is_err());
    assert!(XqTensorType::I32
        .decode_i32_row(&raw, input.len(), raw.len() as u64 - 1)
        .is_err());
}
