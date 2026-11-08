use super::*;
use crate::reference;
use candle_core::Var;

fn bytes(data: Vec<u8>, shape: (usize, usize)) -> Tensor {
    Tensor::from_vec(data, shape, &Device::Cpu).unwrap()
}
fn values(tensor: &Tensor) -> Vec<f32> {
    tensor
        .to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}

#[test]
fn every_nibble_and_group_scale_matches_independent_f64_format_decoder() {
    let limits = InferenceLimits::default();
    let data: Vec<u8> = (0..128)
        .map(|i| ((i * 2) % 16) as u8 | ((((i * 2 + 1) % 16) as u8) << 4))
        .collect();
    let scales = [127_u8, 128, 126, 125, 129, 127, 124, 130];
    let packed = Fp4IndexRowsData::new(
        &limits,
        &bytes(data.clone(), (2, DATA_BYTES)),
        &bytes(scales.to_vec(), (2, SCALE_BYTES)),
    )
    .unwrap();
    let decoded = values(&packed.decode_prefix(2, &limits).unwrap());
    for row in 0..2 {
        for channel in 0..INDEX_HEAD_DIM {
            let byte = data[row * DATA_BYTES + channel / 2];
            let nibble = if channel % 2 == 0 {
                byte & 15
            } else {
                byte >> 4
            };
            let expected = (reference::decode_e2m1(nibble).unwrap()
                * reference::decode_ue8m0(scales[row * SCALE_BYTES + channel / 32]).unwrap())
                as f32;
            assert_eq!(
                decoded[row * INDEX_HEAD_DIM + channel].to_bits(),
                expected.to_bits()
            );
        }
    }
    assert_eq!(packed.retained_bytes(), 136);
}

#[test]
fn admitted_packed_exponent_extremes_preserve_bf16_subnormals() {
    let limits = InferenceLimits::default();
    let packed = Fp4IndexRowsData::new(
        &limits,
        &bytes(vec![0x11; DATA_BYTES], (1, DATA_BYTES)),
        &bytes(vec![1, 2, 253, 254], (1, SCALE_BYTES)),
    )
    .unwrap();
    let decoded = values(&packed.decode_prefix(1, &limits).unwrap());
    for (group, code) in [1_u8, 2, 253, 254].iter().enumerate() {
        let expected = (0.5 * reference::decode_ue8m0(*code).unwrap()) as f32;
        assert!(expected > 0. && expected.is_finite());
        for channel in 0..32 {
            assert_eq!(decoded[group * 32 + channel].to_bits(), expected.to_bits());
        }
    }
}

#[test]
fn packed_snapshots_freeze_data_and_scales_before_mutable_parameters_change() {
    let limits = InferenceLimits::default();
    let data = Var::from_tensor(&bytes(vec![0x22; DATA_BYTES], (1, DATA_BYTES))).unwrap();
    let scales = Var::from_tensor(&bytes(vec![127; SCALE_BYTES], (1, SCALE_BYTES))).unwrap();
    let packed = Fp4IndexRowsData::new(&limits, data.as_tensor(), scales.as_tensor()).unwrap();
    data.set(&bytes(vec![0; DATA_BYTES], (1, DATA_BYTES)))
        .unwrap();
    scales
        .set(&bytes(vec![128; SCALE_BYTES], (1, SCALE_BYTES)))
        .unwrap();
    assert_eq!(
        values(&packed.decode_prefix(1, &limits).unwrap()),
        vec![1.; INDEX_HEAD_DIM]
    );
}

#[test]
fn selected_gather_and_prefix_never_decode_unselected_overflowing_values() {
    let limits = InferenceLimits::default();
    let mut data = vec![0x22; DATA_BYTES];
    data.extend(vec![0x77; DATA_BYTES]);
    let mut scales = vec![127; SCALE_BYTES];
    scales.extend(vec![254; SCALE_BYTES]);
    let packed = Fp4IndexRowsData::new(
        &limits,
        &bytes(data, (2, DATA_BYTES)),
        &bytes(scales, (2, SCALE_BYTES)),
    )
    .unwrap();
    assert_eq!(
        values(&packed.decode_prefix(1, &limits).unwrap()),
        vec![1.; INDEX_HEAD_DIM]
    );
    let good = Tensor::new(&[0_u32], &Device::Cpu).unwrap();
    assert_eq!(
        values(&packed.decode_selected(&good, &limits).unwrap()),
        vec![1.; INDEX_HEAD_DIM]
    );
    assert!(packed.decode_prefix(2, &limits).is_err());
    assert!(packed
        .decode_selected(&Tensor::new(&[1_u32], &Device::Cpu).unwrap(), &limits)
        .is_err());
    for row in [2_u32, u32::MAX] {
        assert!(packed
            .decode_selected(&Tensor::new(&[row], &Device::Cpu).unwrap(), &limits)
            .is_err());
    }
}

#[test]
fn gather_restores_arbitrary_order_and_duplicate_selected_rows() {
    let limits = InferenceLimits::default();
    let mut data = vec![0x22; DATA_BYTES];
    data.extend(vec![0x66; DATA_BYTES]);
    let packed = Fp4IndexRowsData::new(
        &limits,
        &bytes(data, (2, DATA_BYTES)),
        &bytes(vec![127; 2 * SCALE_BYTES], (2, SCALE_BYTES)),
    )
    .unwrap();
    let indices = Tensor::new(&[1_u32, 0, 1], &Device::Cpu).unwrap();
    let output = packed
        .decode_selected(&indices, &limits)
        .unwrap()
        .to_dtype(DType::F32)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(
        output,
        vec![
            vec![4.; INDEX_HEAD_DIM],
            vec![1.; INDEX_HEAD_DIM],
            vec![4.; INDEX_HEAD_DIM]
        ]
    );
}

#[test]
fn empty_packed_rows_and_empty_gathers_have_explicit_bf16_shapes() {
    let limits = InferenceLimits::default();
    let packed = Fp4IndexRowsData::new(
        &limits,
        &bytes(vec![], (0, DATA_BYTES)),
        &bytes(vec![], (0, SCALE_BYTES)),
    )
    .unwrap();
    let prefix = packed.decode_prefix(0, &limits).unwrap();
    assert_eq!(prefix.dims(), &[0, INDEX_HEAD_DIM]);
    assert_eq!(prefix.dtype(), DType::BF16);
    let indices = Tensor::from_vec(Vec::<u32>::new(), 0, &Device::Cpu).unwrap();
    assert_eq!(
        packed.decode_selected(&indices, &limits).unwrap().dims(),
        &[0, INDEX_HEAD_DIM]
    );
}

#[test]
fn bad_layout_dtype_reserved_scales_and_construction_budgets_refuse() {
    let limits = InferenceLimits::default();
    let data = bytes(vec![0; DATA_BYTES], (1, DATA_BYTES));
    let scales = bytes(vec![127; SCALE_BYTES], (1, SCALE_BYTES));
    for (bad_data, bad_scales) in [
        (data.to_dtype(DType::F32).unwrap(), scales.clone()),
        (data.clone(), scales.to_dtype(DType::F32).unwrap()),
        (
            bytes(vec![0; DATA_BYTES - 1], (1, DATA_BYTES - 1)),
            scales.clone(),
        ),
        (
            data.clone(),
            bytes(vec![127; SCALE_BYTES + 1], (1, SCALE_BYTES + 1)),
        ),
        (
            data.clone(),
            bytes(vec![255; SCALE_BYTES], (1, SCALE_BYTES)),
        ),
        (data.clone(), bytes(vec![0; SCALE_BYTES], (1, SCALE_BYTES))),
    ] {
        assert!(Fp4IndexRowsData::new(&limits, &bad_data, &bad_scales).is_err());
    }
    let tiny_bytes = InferenceLimits {
        max_state_bytes: 1,
        ..limits.clone()
    };
    assert!(Fp4IndexRowsData::new(&tiny_bytes, &data, &scales).is_err());
    let tiny_elements = InferenceLimits {
        max_tensor_elements: 127,
        ..limits
    };
    assert!(Fp4IndexRowsData::new(&tiny_elements, &data, &scales).is_err());
}

#[test]
fn decode_limits_invalid_counts_and_wrong_index_dtype_refuse_before_gather() {
    let limits = InferenceLimits::default();
    let packed = Fp4IndexRowsData::new(
        &limits,
        &bytes(vec![0x22; DATA_BYTES], (1, DATA_BYTES)),
        &bytes(vec![127; SCALE_BYTES], (1, SCALE_BYTES)),
    )
    .unwrap();
    assert!(packed.decode_prefix(2, &limits).is_err());
    let tiny_bytes = InferenceLimits {
        max_state_bytes: 128,
        ..limits.clone()
    };
    assert!(packed.decode_prefix(1, &tiny_bytes).is_err());
    let tiny_elements = InferenceLimits {
        max_tensor_elements: 127,
        ..limits.clone()
    };
    assert!(packed.decode_prefix(1, &tiny_elements).is_err());
    assert!(packed
        .decode_selected(&Tensor::new(&[0_f32], &Device::Cpu).unwrap(), &limits)
        .is_err());
}

fn scalar_encode(row: &[f32]) -> (Vec<u8>, Vec<u8>) {
    let palette = [0_f64, 0.5, 1., 1.5, 2., 3., 4., 6.];
    let mut codes = Vec::new();
    let mut scales = Vec::new();
    for group in row.chunks_exact(32) {
        let amax = group
            .iter()
            .map(|x| x.abs())
            .fold(0_f32, f32::max)
            .max(6_f32 * f32::MIN_POSITIVE);
        let target = amax * (1_f32 / 6.);
        // Independent source-style exponent-bit oracle; production uses an
        // exact table binary search rather than this bit extraction formula.
        let exponent_byte = (((target.to_bits() - 1) >> 23) + 1) as u8;
        scales.push(exponent_byte);
        let sf = f64::from(f32::from_bits(u32::from(exponent_byte) << 23));
        for value in group {
            let normalized = f64::from(*value).abs() / sf;
            let mut best = 0;
            for candidate in 1..8 {
                let error = (normalized - palette[candidate]).abs();
                let current = (normalized - palette[best]).abs();
                if error < current || (error == current && candidate % 2 == 0) {
                    best = candidate;
                }
            }
            codes.push(best as u8 + if value.is_sign_negative() { 8 } else { 0 });
        }
    }
    (
        codes
            .chunks_exact(2)
            .map(|pair| pair[0] | (pair[1] << 4))
            .collect(),
        scales,
    )
}

#[test]
fn encoder_midpoint_ties_are_rne_and_negative_zero_keeps_its_sign_bit() {
    let limits = InferenceLimits::default();
    let pattern = [
        0.25_f32, 0.75, 1.25, 1.75, 2.5, 3.5, 5., 6., -0., 0., -0.25, -0.75, -1.25, -1.75, -2.5,
        -6.,
    ];
    let values: Vec<f32> = pattern.into_iter().cycle().take(INDEX_HEAD_DIM).collect();
    let input = Tensor::from_vec(values, (1, INDEX_HEAD_DIM), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let encoded = encode_rows(&input, &limits, &CancellationToken::new()).unwrap();
    assert_eq!(encoded.scales.to_vec2::<u8>().unwrap(), vec![vec![127; 4]]);
    assert_eq!(
        &encoded.data.to_vec2::<u8>().unwrap()[0][..8],
        &[0x20, 0x42, 0x64, 0x76, 0x08, 0xa8, 0xca, 0xfc]
    );
}

#[test]
fn encoder_scale_exponents_and_bytes_match_scalar_oracle_across_bf16_range() {
    let limits = InferenceLimits::default();
    let mut source = Vec::new();
    for exponent in -133..=127 {
        let amax = 2_f32.powi(exponent);
        let pattern = [amax, -0.75 * amax, 0.5 * amax, -0.];
        source.extend(pattern.into_iter().cycle().take(INDEX_HEAD_DIM));
    }
    source.extend(
        [f32::from_bits(0x7f7f0000), -0., 0., 1.]
            .into_iter()
            .cycle()
            .take(INDEX_HEAD_DIM),
    );
    let count = source.len() / INDEX_HEAD_DIM;
    let input = Tensor::from_vec(source, (count, INDEX_HEAD_DIM), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let encoded = encode_rows(&input, &limits, &CancellationToken::new()).unwrap();
    let raw = input
        .to_dtype(DType::F32)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    let data = encoded.data.to_vec2::<u8>().unwrap();
    let scales = encoded.scales.to_vec2::<u8>().unwrap();
    for row in 0..count {
        let (expected_data, expected_scales) = scalar_encode(&raw[row]);
        assert_eq!(scales[row], expected_scales, "scale row {row}");
        assert_eq!(data[row], expected_data, "data row {row}");
    }
}

#[test]
fn zero_groups_use_positive_normal_scale_and_encode_decode_stays_zero() {
    let limits = InferenceLimits::default();
    let input = Tensor::zeros((1, INDEX_HEAD_DIM), DType::BF16, &Device::Cpu).unwrap();
    let encoded = encode_rows(&input, &limits, &CancellationToken::new()).unwrap();
    assert_eq!(encoded.scales.to_vec2::<u8>().unwrap(), vec![vec![1; 4]]);
    assert_eq!(
        encoded.data.to_vec2::<u8>().unwrap(),
        vec![vec![0; DATA_BYTES]]
    );
    assert_eq!(
        values(&encoded.decode_prefix(1, &limits).unwrap()),
        vec![0.; INDEX_HEAD_DIM]
    );
}

#[test]
fn batched_encoding_matches_each_row_and_keeps_actual_quantization_rounding() {
    let limits = InferenceLimits::default();
    let source: Vec<_> = (0..4 * INDEX_HEAD_DIM)
        .map(|i| (i as f32 % 71. - 35.) / 8.)
        .collect();
    let input = Tensor::from_vec(source, (4, INDEX_HEAD_DIM), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let batch = encode_rows(&input, &limits, &CancellationToken::new()).unwrap();
    for row in 0..4 {
        let single = encode_rows(
            &input.narrow(0, row, 1).unwrap(),
            &limits,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(
            batch
                .data
                .narrow(0, row, 1)
                .unwrap()
                .to_vec2::<u8>()
                .unwrap(),
            single.data.to_vec2::<u8>().unwrap()
        );
        assert_eq!(
            batch
                .scales
                .narrow(0, row, 1)
                .unwrap()
                .to_vec2::<u8>()
                .unwrap(),
            single.scales.to_vec2::<u8>().unwrap()
        );
    }
    assert_ne!(
        values(&batch.decode_prefix(4, &limits).unwrap()),
        values(&input)
    );
}

#[test]
fn encoder_refuses_bad_shapes_nonfinite_input_cancellation_and_budget() {
    let limits = InferenceLimits::default();
    for input in [
        Tensor::ones((1, INDEX_HEAD_DIM), DType::F32, &Device::Cpu).unwrap(),
        Tensor::ones((1, INDEX_HEAD_DIM - 1), DType::BF16, &Device::Cpu).unwrap(),
        Tensor::full(f32::NAN, (1, INDEX_HEAD_DIM), &Device::Cpu)
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap(),
    ] {
        assert!(encode_rows(&input, &limits, &CancellationToken::new()).is_err());
    }
    let input = Tensor::zeros((1, INDEX_HEAD_DIM), DType::BF16, &Device::Cpu).unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(encode_rows(&input, &limits, &cancellation).is_err());
    let tiny = InferenceLimits {
        max_state_bytes: 128,
        ..limits
    };
    assert!(encode_rows(&input, &tiny, &CancellationToken::new()).is_err());
}

#[test]
fn every_finite_bf16_encoding_matches_independent_packed_scalar_oracle() {
    let limits = InferenceLimits::default();
    let source: Vec<f32> = (0..=u16::MAX)
        .filter(|bits| bits & 0x7f80 != 0x7f80)
        .map(|bits| f32::from_bits(u32::from(bits) << 16))
        .collect();
    assert_eq!(source.len(), 65_280);
    let count = source.len() / INDEX_HEAD_DIM;
    let input = Tensor::from_vec(source.clone(), (count, INDEX_HEAD_DIM), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let encoded = encode_rows(&input, &limits, &CancellationToken::new()).unwrap();
    let data = encoded.data.to_vec2::<u8>().unwrap();
    let scales = encoded.scales.to_vec2::<u8>().unwrap();
    for (row, original) in source.chunks_exact(INDEX_HEAD_DIM).enumerate() {
        let (expected_data, expected_scales) = scalar_encode(original);
        assert_eq!(scales[row], expected_scales, "exhaustive scale row {row}");
        assert_eq!(data[row], expected_data, "exhaustive data row {row}");
    }
}
