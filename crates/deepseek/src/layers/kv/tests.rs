use super::*;
use crate::reference;
use candle_core::Var;

fn bf16(data: Vec<f32>, rows: usize) -> Tensor {
    Tensor::from_vec(data, (rows, MAIN_KV_DIM), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap()
}
fn values(data: &Tensor) -> Vec<f32> {
    data.to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}
fn raw(records: Vec<u8>, rows: usize, format: MainKvFormat) -> Tensor {
    Tensor::from_vec(records, (rows, format.bytes_per_token()), &Device::Cpu).unwrap()
}

fn nearest(value: f32, palette: &[f64]) -> u8 {
    // Saturate before distance comparisons: for enormous finite inputs,
    // subtracting adjacent small palette values can lose the difference even
    // in F64. The mathematical nearest representable value is the endpoint.
    let absolute = f64::from(value.abs()).min(palette[palette.len() - 1]);
    let mut best = 0;
    for code in 1..palette.len() {
        let error = (absolute - palette[code]).abs();
        let previous = (absolute - palette[best]).abs();
        if error < previous || (error == previous && code % 2 == 0) {
            best = code;
        }
    }
    best as u8
}

// Source-style scalar oracle, independent of production table binary search.
// Keep normalized reciprocal multiplication in FP32, including E4M3 scales.
fn scalar_encode(format: MainKvFormat, row: &[f32]) -> Vec<u8> {
    let e4: Vec<f64> = (0..127)
        .map(|code| reference::decode_e4m3(code).unwrap())
        .collect();
    let e2: Vec<f64> = (0..8)
        .map(|code| reference::decode_e2m1(code).unwrap())
        .collect();
    let mut data = Vec::new();
    let mut scales = Vec::new();
    for group in row.chunks_exact(format.group_size()) {
        let amax = group.iter().map(|v| v.abs()).fold(0_f32, f32::max);
        let (scale_code, scale) = match format {
            MainKvFormat::Fp8Ue8m0 => {
                let target = amax.max(1e-4_f32) * (1_f32 / 448.);
                let code = (((target.to_bits() - 1) >> 23) + 1) as u8;
                (code, f32::from_bits(u32::from(code) << 23))
            }
            MainKvFormat::Fp4E4m3 => {
                let target = (amax.max(6_f32 * 2_f32.powi(-9)) * (1_f32 / 6.)).min(448.);
                let code = nearest(target, &e4);
                (code, e4[usize::from(code)] as f32)
            }
        };
        scales.push(scale_code);
        let inverse = 1_f32 / scale;
        let codes: Vec<_> = group
            .iter()
            .map(|value| {
                let normalized = *value * inverse;
                let code = nearest(
                    normalized,
                    if format == MainKvFormat::Fp8Ue8m0 {
                        &e4
                    } else {
                        &e2
                    },
                );
                code + if normalized.is_sign_negative() {
                    if format == MainKvFormat::Fp8Ue8m0 {
                        128
                    } else {
                        8
                    }
                } else {
                    0
                }
            })
            .collect();
        if format == MainKvFormat::Fp8Ue8m0 {
            data.extend(codes);
        } else {
            data.extend(codes.chunks_exact(2).map(|pair| pair[0] | pair[1] << 4));
        }
    }
    data.extend(scales);
    data
}

#[test]
fn formats_roundtrip_have_exact_token_widths_and_reject_unknown_profiles() {
    for (format, width, group) in [
        (MainKvFormat::Fp8Ue8m0, 528, 32),
        (MainKvFormat::Fp4E4m3, 288, 16),
    ] {
        assert_eq!(MainKvFormat::parse(format.as_str()).unwrap(), format);
        assert_eq!(format.bytes_per_token(), width);
        assert_eq!(format.group_size(), group);
    }
    for bad in ["", "bf16", "fp4", "fp8", "auto"] {
        assert!(MainKvFormat::parse(bad).is_err());
    }
}

#[test]
fn zero_groups_have_reference_minimum_scales_and_token_interleaved_records() {
    let limits = InferenceLimits::default();
    for (format, scale) in [(MainKvFormat::Fp8Ue8m0, 105), (MainKvFormat::Fp4E4m3, 1)] {
        let input = bf16(vec![0.; 2 * MAIN_KV_DIM], 2);
        let encoded = encode_rows(format, &input, &limits, &CancellationToken::new()).unwrap();
        let records = encoded.records.to_vec2::<u8>().unwrap();
        for row in records {
            assert_eq!(&row[..format.data_bytes()], vec![0; format.data_bytes()]);
            assert_eq!(
                &row[format.data_bytes()..],
                vec![scale; format.scale_bytes()]
            );
        }
        assert_eq!(
            values(&encoded.decode_prefix(2, &limits).unwrap()),
            vec![0.; 2 * MAIN_KV_DIM]
        );
        assert_eq!(
            encoded.retained_bytes(),
            (2 * format.bytes_per_token()) as u64
        );
    }
}

#[test]
fn every_finite_bf16_input_matches_independent_main_format_encoder() {
    let limits = InferenceLimits::default();
    let mut source: Vec<f32> = (0..=u16::MAX)
        .filter(|bits| bits & 0x7f80 != 0x7f80)
        .map(|bits| f32::from_bits(u32::from(bits) << 16))
        .collect();
    assert_eq!(source.len(), 65_280);
    source.resize(source.len().div_ceil(MAIN_KV_DIM) * MAIN_KV_DIM, 0.);
    let count = source.len() / MAIN_KV_DIM;
    let input = bf16(source.clone(), count);
    for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
        let encoded = encode_rows(format, &input, &limits, &CancellationToken::new()).unwrap();
        let actual = encoded.records.to_vec2::<u8>().unwrap();
        for (row, data) in source.chunks_exact(MAIN_KV_DIM).enumerate() {
            assert_eq!(
                actual[row],
                scalar_encode(format, data),
                "{} row {row}",
                format.as_str()
            );
        }
    }
}

#[test]
fn fp8_midpoint_rne_covers_all_finite_positive_bins_and_signed_zero() {
    let limits = InferenceLimits::default();
    let format = MainKvFormat::Fp8Ue8m0;
    let palette: Vec<_> = (0..127)
        .map(|code| reference::decode_e4m3(code).unwrap() as f32)
        .collect();
    let mut data = vec![0_f32; 8 * MAIN_KV_DIM];
    for lower in 0..126 {
        let start = lower * 32;
        let midpoint = (palette[lower] + palette[lower + 1]) * 0.5;
        data[start] = midpoint;
        data[start + 1] = -midpoint;
        data[start + 2] = -0.;
        data[start + 31] = 448.;
    }
    let encoded = encode_rows(format, &bf16(data, 8), &limits, &CancellationToken::new()).unwrap();
    let actual = encoded.records.to_vec2::<u8>().unwrap();
    for lower in 0..126 {
        let row = lower / 16;
        let group = lower % 16;
        let expected = (lower + lower % 2) as u8;
        assert_eq!(actual[row][group * 32], expected);
        assert_eq!(actual[row][group * 32 + 1], expected + 128);
        assert_eq!(actual[row][group * 32 + 2], 128);
        assert_eq!(actual[row][512 + group], 127);
    }
}

#[test]
fn fp4_non_power_scale_uses_fp32_reciprocal_multiply_before_rne() {
    let limits = InferenceLimits::default();
    let format = MainKvFormat::Fp4E4m3;
    let mut data = vec![0.; MAIN_KV_DIM];
    for group in 0..32 {
        data[group * 16] = 2.34375;
        data[group * 16 + 15] = 11.25;
    }
    assert!(2.34375_f32 * (1_f32 / 1.875) > 1.25);
    let encoded = encode_rows(format, &bf16(data, 1), &limits, &CancellationToken::new()).unwrap();
    let actual = encoded.records.to_vec2::<u8>().unwrap();
    assert_eq!(actual[0][0] & 15, 3); // ideal division would incorrectly select code2
    assert_eq!(&actual[0][256..], vec![63; 32]);
}

#[test]
fn raw_fp8_and_fp4_decode_all_value_codes_with_independent_scale_oracle() {
    let limits = InferenceLimits::default();
    for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
        let data: Vec<u8> = if format == MainKvFormat::Fp8Ue8m0 {
            (0..=u8::MAX)
                .filter(|v| v & 127 != 127)
                .cycle()
                .take(512)
                .collect()
        } else {
            (0..256)
                .map(|i| ((i * 2) % 16) as u8 | ((((i * 2 + 1) % 16) as u8) << 4))
                .collect()
        };
        let scales: Vec<u8> = if format == MainKvFormat::Fp8Ue8m0 {
            [127, 128, 126, 125].into_iter().cycle().take(16).collect()
        } else {
            [1, 10, 42, 80, 126].into_iter().cycle().take(32).collect()
        };
        let mut records = data.clone();
        records.extend(&scales);
        let stored = MainKvRowsData::new(format, &raw(records, 1, format), &limits).unwrap();
        let decoded = values(&stored.decode_prefix(1, &limits).unwrap());
        for channel in 0..MAIN_KV_DIM {
            let value = if format == MainKvFormat::Fp8Ue8m0 {
                reference::decode_e4m3(data[channel]).unwrap()
            } else {
                let byte = data[channel / 2];
                reference::decode_e2m1(if channel % 2 == 0 {
                    byte & 15
                } else {
                    byte >> 4
                })
                .unwrap()
            };
            let scale = if format == MainKvFormat::Fp8Ue8m0 {
                reference::decode_ue8m0(scales[channel / 32]).unwrap()
            } else {
                reference::decode_e4m3(scales[channel / 16]).unwrap()
            };
            assert_eq!(
                decoded[channel].to_bits(),
                ((value * scale) as f32).to_bits()
            );
        }
    }
}

#[test]
fn encode_batch_and_strided_rows_equal_each_token_and_real_quantization_changes_values() {
    let limits = InferenceLimits::default();
    let source: Vec<_> = (0..4 * MAIN_KV_DIM)
        .map(|i| (i as f32 % 137. - 68.) / 16.)
        .collect();
    let input = bf16(source, 4)
        .reshape((2, 2, MAIN_KV_DIM))
        .unwrap()
        .transpose(0, 1)
        .unwrap()
        .flatten(0, 1)
        .unwrap();
    for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
        let batch = encode_rows(format, &input, &limits, &CancellationToken::new()).unwrap();
        for row in 0..4 {
            let single = encode_rows(
                format,
                &input.narrow(0, row, 1).unwrap(),
                &limits,
                &CancellationToken::new(),
            )
            .unwrap();
            assert_eq!(
                batch
                    .records
                    .narrow(0, row, 1)
                    .unwrap()
                    .to_vec2::<u8>()
                    .unwrap(),
                single.records.to_vec2::<u8>().unwrap()
            );
        }
        assert_ne!(
            values(&batch.decode_prefix(4, &limits).unwrap()),
            values(&input)
        );
    }
}

#[test]
fn immutable_records_and_selected_order_survive_mutation_and_duplicate_gathers() {
    let limits = InferenceLimits::default();
    let input = bf16([vec![1.; MAIN_KV_DIM], vec![3.; MAIN_KV_DIM]].concat(), 2);
    for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
        let encoded = encode_rows(format, &input, &limits, &CancellationToken::new()).unwrap();
        let parameter = Var::from_tensor(&encoded.records).unwrap();
        let stored = MainKvRowsData::new(format, parameter.as_tensor(), &limits).unwrap();
        parameter
            .set(&Tensor::zeros((2, format.bytes_per_token()), DType::U8, &Device::Cpu).unwrap())
            .unwrap();
        let expected = stored
            .decode_prefix(2, &limits)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        let gathered = stored
            .decode_selected(&Tensor::new(&[1_u32, 0, 1], &Device::Cpu).unwrap(), &limits)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        assert_eq!(
            gathered,
            vec![
                expected[1].clone(),
                expected[0].clone(),
                expected[1].clone()
            ]
        );
    }
}

#[test]
fn selected_decode_never_expands_other_fp8_rows_with_overflowing_logical_values() {
    let limits = InferenceLimits::default();
    let format = MainKvFormat::Fp8Ue8m0;
    let mut records = vec![56_u8; 512];
    records.extend(vec![127; 16]);
    records.extend(vec![126; 512]);
    records.extend(vec![254; 16]);
    let stored = MainKvRowsData::new(format, &raw(records, 2, format), &limits).unwrap();
    assert_eq!(
        values(&stored.decode_prefix(1, &limits).unwrap()),
        vec![1.; MAIN_KV_DIM]
    );
    assert!(stored.decode_prefix(2, &limits).is_err());
    assert!(stored
        .decode_selected(&Tensor::new(&[1_u32], &Device::Cpu).unwrap(), &limits)
        .is_err());
    assert_eq!(
        values(
            &stored
                .decode_selected(&Tensor::new(&[0_u32], &Device::Cpu).unwrap(), &limits)
                .unwrap()
        ),
        vec![1.; MAIN_KV_DIM]
    );
}

#[test]
fn invalid_records_scales_data_and_budgets_refuse_before_decode() {
    let limits = InferenceLimits::default();
    for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
        let good = encode_rows(
            format,
            &bf16(vec![1.; MAIN_KV_DIM], 1),
            &limits,
            &CancellationToken::new(),
        )
        .unwrap();
        let good_bytes = good.records.to_vec2::<u8>().unwrap().remove(0);
        for scale in [0, 127, 128, 255] {
            if format == MainKvFormat::Fp8Ue8m0 && (1..=254).contains(&scale) {
                continue;
            }
            let mut bad = good_bytes.clone();
            bad[format.data_bytes()] = scale;
            assert!(MainKvRowsData::new(format, &raw(bad, 1, format), &limits).is_err());
        }
        if format == MainKvFormat::Fp8Ue8m0 {
            for nan in [127, 255] {
                let mut bad = good_bytes.clone();
                bad[0] = nan;
                assert!(MainKvRowsData::new(format, &raw(bad, 1, format), &limits).is_err());
            }
        }
        assert!(
            MainKvRowsData::new(format, &good.records.to_dtype(DType::F32).unwrap(), &limits)
                .is_err()
        );
        let short =
            Tensor::zeros((1, format.bytes_per_token() - 1), DType::U8, &Device::Cpu).unwrap();
        assert!(MainKvRowsData::new(format, &short, &limits).is_err());
        let tiny = InferenceLimits {
            max_state_bytes: 1,
            ..limits.clone()
        };
        assert!(MainKvRowsData::new(format, &good.records, &tiny).is_err());
        assert!(good.decode_prefix(1, &tiny).is_err());
        assert!(good.decode_prefix(2, &limits).is_err());
        for index in [1_u32, u32::MAX] {
            assert!(good
                .decode_selected(&Tensor::new(&[index], &Device::Cpu).unwrap(), &limits)
                .is_err());
        }
    }
}

#[test]
fn bad_encoder_inputs_cancellation_and_empty_rows_have_explicit_results() {
    let limits = InferenceLimits::default();
    for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
        for bad in [
            Tensor::ones((1, 512), DType::F32, &Device::Cpu).unwrap(),
            Tensor::ones((1, 511), DType::BF16, &Device::Cpu).unwrap(),
            bf16(vec![f32::NAN; MAIN_KV_DIM], 1),
        ] {
            assert!(encode_rows(format, &bad, &limits, &CancellationToken::new()).is_err());
        }
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(encode_rows(
            format,
            &bf16(vec![1.; MAIN_KV_DIM], 1),
            &limits,
            &cancellation
        )
        .is_err());
        let empty =
            encode_rows(format, &bf16(vec![], 0), &limits, &CancellationToken::new()).unwrap();
        assert_eq!(empty.records.dims(), &[0, format.bytes_per_token()]);
        assert_eq!(
            empty.decode_prefix(0, &limits).unwrap().dims(),
            &[0, MAIN_KV_DIM]
        );
    }
}
