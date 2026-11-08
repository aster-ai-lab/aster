use super::*;

type TensorSpec<'a> = (&'a str, Vec<u64>, u32, u64);

fn string(value: &[u8]) -> Vec<u8> {
    let mut raw = (value.len() as u64).to_le_bytes().to_vec();
    raw.extend(value);
    raw
}

fn metadata(name: &str, kind: u32, value: &[u8]) -> Vec<u8> {
    let mut raw = string(name.as_bytes());
    raw.extend(kind.to_le_bytes());
    raw.extend(value);
    raw
}

fn alignment(value: u32) -> Vec<u8> {
    metadata("general.alignment", 4, &value.to_le_bytes())
}

fn header(meta: &[Vec<u8>], tensors: &[TensorSpec<'_>], align: usize) -> (Vec<u8>, usize) {
    let mut raw = b"GGUF".to_vec();
    raw.extend(3_u32.to_le_bytes());
    raw.extend((tensors.len() as u64).to_le_bytes());
    raw.extend((meta.len() as u64).to_le_bytes());
    for record in meta {
        raw.extend(record);
    }
    for (name, shape, kind, offset) in tensors {
        raw.extend(string(name.as_bytes()));
        raw.extend((shape.len() as u32).to_le_bytes());
        for dim in shape {
            raw.extend(dim.to_le_bytes());
        }
        raw.extend(kind.to_le_bytes());
        raw.extend(offset.to_le_bytes());
    }
    let end = raw.len();
    raw.resize(end.div_ceil(align) * align, 0);
    (raw, end)
}

fn parse_single(meta: &[Vec<u8>], shape: Vec<u64>, kind: u32, offset: u64) -> Result<XqGgufHeader> {
    let (raw, _) = header(meta, &[("tensor", shape, kind, offset)], 32);
    XqGgufHeader::parse(&raw, 1024 * 1024)
}

#[test]
fn mixed_quantized_shapes_keep_gguf_order_and_absolute_ranges() {
    let (raw, _) = header(
        &[alignment(32)],
        &[
            ("ptq", vec![128, 2, 3], 143, 0),
            ("q2", vec![64, 2], 42, 192),
            ("q3", vec![256], 11, 256),
            ("q8", vec![32], 8, 384),
            ("map", vec![4], 26, 448),
        ],
        32,
    );
    let parsed = XqGgufHeader::parse(&raw, raw.len() as u64 + 464).unwrap();
    assert_eq!(parsed.data_offset(), raw.len() as u64);
    assert_eq!(parsed.alignment(), 32);
    assert_eq!(parsed.tensors()["ptq"].bytes, 168);
    assert_eq!(parsed.tensors()["q2"].bytes, 36);
    assert_eq!(parsed.tensors()["q3"].bytes, 110);
    assert_eq!(parsed.tensors()["q8"].bytes, 34);
    assert_eq!(
        parsed.tensors()["map"].absolute_offset,
        raw.len() as u64 + 448
    );
}

#[test]
fn metadata_types_remain_exact_and_arrays_reference_bounded_header_ranges() {
    let mut array = 4_u32.to_le_bytes().to_vec();
    array.extend(3_u64.to_le_bytes());
    for value in [3_u32, 4, 5] {
        array.extend(value.to_le_bytes());
    }
    let records = [
        alignment(32),
        metadata("signed", 11, &(-1_i64).to_le_bytes()),
        metadata("float", 6, &0.5_f32.to_le_bytes()),
        metadata("enabled", 7, &[1]),
        metadata("array", 9, &array),
        metadata("name", 8, &string(b"deepseek41")),
    ];
    let (raw, _) = header(&records, &[("tensor", vec![1], 0, 0)], 32);
    let parsed = XqGgufHeader::parse(&raw, raw.len() as u64 + 4).unwrap();
    assert_eq!(parsed.metadata()["signed"], XqMetadataValue::Signed(-1));
    assert_eq!(parsed.metadata()["float"], XqMetadataValue::Float(0.5));
    assert_eq!(parsed.metadata()["enabled"], XqMetadataValue::Bool(true));
    assert_eq!(
        parsed.metadata()["name"],
        XqMetadataValue::String("deepseek41".into())
    );
    let XqMetadataValue::Array {
        element_type,
        elements,
        range,
    } = &parsed.metadata()["array"]
    else {
        panic!("array descriptor required")
    };
    assert_eq!((*element_type, *elements), (4, 3));
    assert_eq!(&raw[range.clone()], &array[12..]);
}

#[test]
fn all_supported_metadata_scalars_are_read_with_their_signedness() {
    for (kind, bytes, expected) in [
        (0, vec![255], XqMetadataValue::Unsigned(255)),
        (1, vec![255], XqMetadataValue::Signed(-1)),
        (
            2,
            u16::MAX.to_le_bytes().to_vec(),
            XqMetadataValue::Unsigned(65535),
        ),
        (
            3,
            i16::MIN.to_le_bytes().to_vec(),
            XqMetadataValue::Signed(-32768),
        ),
        (
            4,
            u32::MAX.to_le_bytes().to_vec(),
            XqMetadataValue::Unsigned(u64::from(u32::MAX)),
        ),
        (
            5,
            i32::MIN.to_le_bytes().to_vec(),
            XqMetadataValue::Signed(i64::from(i32::MIN)),
        ),
        (
            10,
            u64::MAX.to_le_bytes().to_vec(),
            XqMetadataValue::Unsigned(u64::MAX),
        ),
        (
            12,
            0.25_f64.to_le_bytes().to_vec(),
            XqMetadataValue::Float(0.25),
        ),
    ] {
        let parsed = parse_single(&[metadata("scalar", kind, &bytes)], vec![1], 0, 0).unwrap();
        assert_eq!(parsed.metadata()["scalar"], expected);
    }
}

#[test]
fn duplicate_metadata_tensor_names_and_nul_names_are_refused() {
    assert!(parse_single(&[alignment(32), alignment(32)], vec![1], 0, 0).is_err());
    assert!(parse_single(
        &[metadata("bad\0key", 4, &1_u32.to_le_bytes())],
        vec![1],
        0,
        0
    )
    .is_err());
    for name in ["same", "", "nul\0name"] {
        let (raw, _) = header(
            &[alignment(32)],
            &[(name, vec![1], 0, 0), (name, vec![1], 0, 32)],
            32,
        );
        assert!(XqGgufHeader::parse(&raw, 1024).is_err());
    }
}

#[test]
fn unknown_quantization_metadata_types_and_partial_quantized_rows_fail_closed() {
    for kind in [2, 10, 43, u32::MAX] {
        assert!(parse_single(&[alignment(32)], vec![256], kind, 0).is_err());
    }
    for kind in [13, u32::MAX] {
        assert!(parse_single(&[metadata("unknown", kind, &[])], vec![1], 0, 0).is_err());
    }
    assert!(parse_single(&[alignment(32)], vec![128, 2], 11, 0).is_err());
    for shape in [
        vec![],
        vec![0],
        vec![1; 5],
        vec![u64::MAX],
        vec![1, u64::MAX, 2],
    ] {
        assert!(parse_single(&[alignment(32)], shape, 0, 0).is_err());
    }
}

#[test]
fn tensor_overlap_alignment_and_out_of_file_ranges_are_rejected() {
    let (raw, _) = header(
        &[alignment(32)],
        &[("a", vec![16], 0, 0), ("b", vec![1], 0, 32)],
        32,
    );
    assert!(XqGgufHeader::parse(&raw, 1024).is_err());
    assert!(parse_single(&[alignment(32)], vec![1], 0, 1).is_err());
    assert!(parse_single(&[alignment(32)], vec![1], 0, u64::MAX - 31).is_err());
    let (raw, _) = header(&[alignment(32)], &[("a", vec![16], 0, 0)], 32);
    assert!(XqGgufHeader::parse(&raw, raw.len() as u64 + 63).is_err());
    for value in [0, 3, 32768] {
        assert!(parse_single(&[alignment(value)], vec![1], 0, 0).is_err());
    }
    assert!(parse_single(
        &[metadata("general.alignment", 5, &32_i32.to_le_bytes())],
        vec![1],
        0,
        0
    )
    .is_err());
}

#[test]
fn truncation_padding_trailing_bytes_and_invalid_versions_are_refused() {
    let (raw, end) = header(&[alignment(32)], &[("tensor", vec![1], 0, 0)], 32);
    for length in 0..raw.len() {
        assert!(XqGgufHeader::parse(&raw[..length], 1024).is_err());
    }
    assert!(end < raw.len());
    let mut bad = raw.clone();
    bad[end] = 1;
    assert!(XqGgufHeader::parse(&bad, 1024).is_err());
    let mut bad = raw.clone();
    bad.push(0);
    assert!(XqGgufHeader::parse(&bad, 1024).is_err());
    for version in [0_u32, 2, 4] {
        let mut bad = raw.clone();
        bad[4..8].copy_from_slice(&version.to_le_bytes());
        assert!(XqGgufHeader::parse(&bad, 1024).is_err());
    }
    for (offset, count) in [(8, u64::MAX), (8, 0), (16, 4097), (16, 0)] {
        let mut bad = raw.clone();
        bad[offset..offset + 8].copy_from_slice(&count.to_le_bytes());
        assert!(XqGgufHeader::parse(&bad, 1024).is_err());
    }
}

#[test]
fn nested_arrays_large_lengths_invalid_utf8_and_nonfinite_metadata_are_refused() {
    for (kind, value) in [
        (7, vec![2]),
        (8, string(&[255])),
        (8, (MAX_STRING_BYTES as u64 + 1).to_le_bytes().to_vec()),
        (6, f32::NAN.to_le_bytes().to_vec()),
        (12, f64::INFINITY.to_le_bytes().to_vec()),
    ] {
        assert!(parse_single(&[metadata("value", kind, &value)], vec![1], 0, 0).is_err());
    }
    for (kind, elements) in [(9_u32, 0), (13, 0), (0, MAX_ARRAY_ELEMENTS + 1), (8, 1)] {
        let mut value = kind.to_le_bytes().to_vec();
        value.extend(elements.to_le_bytes());
        assert!(parse_single(&[metadata("array", 9, &value)], vec![1], 0, 0).is_err());
    }
}

#[test]
fn unpinned_small_or_wrong_hash_headers_cannot_construct_a_public_handle() {
    let (raw, _) = header(&[alignment(32)], &[("tensor", vec![1], 0, 0)], 32);
    assert!(XqGgufHeader::from_pinned_bytes(&raw).is_err());
    assert!(XqGgufHeader::from_pinned_bytes(&vec![0; XQ_HEADER_BYTES]).is_err());
}
