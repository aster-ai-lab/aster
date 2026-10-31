use super::*;
use crate::{
    layers::indexer::{SparseIndexPlan, SparseIndexer, INVALID_INDEX_ROW},
    reference,
};
use aster_power::inference::InferenceLimits;
use candle_core::Device;

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn bf16(data: &[f32], rows: usize, width: usize) -> Tensor {
    Tensor::from_vec(data.to_vec(), (rows, width), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap()
}
fn floats(t: &Tensor) -> Vec<f32> {
    t.to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}
fn bytes(t: &Tensor) -> Vec<u8> {
    t.flatten_all().unwrap().to_vec1().unwrap()
}
fn round_bf16(x: f32) -> f32 {
    let bits = x.to_bits();
    f32::from_bits(bits.wrapping_add(0x7fff + ((bits >> 16) & 1)) & 0xffff0000)
}
fn cache() -> Tensor {
    let mut values: Vec<f32> = Vec::new();
    for (cos, sin) in [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)] {
        values.extend([cos; 32]);
        values.extend([sin; 32]);
    }
    Tensor::from_vec(values, (4, 64), &Device::Cpu).unwrap()
}
fn matrix(device: &CudaLayerDevice, data: &[f32], rows: usize, width: usize) -> DeviceMatrix {
    DeviceMatrix::from_tensors_for_tests(
        device,
        MatrixLayout::Bf16,
        [rows, width],
        &bf16(data, rows, width),
        None,
    )
    .unwrap()
}
fn query_matrix(device: &CudaLayerDevice, rank: usize, heads: usize) -> DeviceMatrix {
    let mut data = vec![0_u8; heads * 128 * rank];
    for out in 0..heads * 128 {
        data[out * rank + out % rank] = if out / 128 == 0 { 56 } else { 184 };
    }
    DeviceMatrix::from_tensors_for_tests(
        device,
        MatrixLayout::Fp8Block32,
        [heads * 128, rank],
        &Tensor::from_vec(data, (heads * 128, rank), &Device::Cpu).unwrap(),
        Some(
            &Tensor::full(
                127_u8,
                ((heads * 128).div_ceil(32), rank.div_ceil(32)),
                &Device::Cpu,
            )
            .unwrap(),
        ),
    )
    .unwrap()
}
fn query(device: &CudaLayerDevice) -> IndexQueryProjection {
    IndexQueryProjection::new(
        device.clone(),
        query_matrix(device, 32, 2),
        matrix(device, &[1., 2., 0., -0.5, -1., 0., 2., 1.], 2, 4),
        cache(),
    )
    .unwrap()
}
fn key_weights() -> Vec<f32> {
    let mut weights = vec![0.; 128 * 4];
    for out in 0..128 {
        weights[out * 4 + out % 4] = [1., 2., 0.5, -1.][out % 4];
    }
    weights
}
fn norm(device: &CudaLayerDevice, weight: bool) -> EffectiveRmsNorm {
    let weights: Vec<_> = (0..128).map(|i| [1., 2., 0.5, -1.][i % 4]).collect();
    EffectiveRmsNorm::new(
        device.clone(),
        128,
        1e-20,
        weight.then(|| bf16(&weights, 1, 128).squeeze(0).unwrap()),
    )
    .unwrap()
}
fn key(device: &CudaLayerDevice) -> IndexKeyProjection {
    IndexKeyProjection::new(
        device.clone(),
        matrix(device, &key_weights(), 128, 4),
        norm(device, true),
        cache(),
    )
    .unwrap()
}
fn latent() -> Tensor {
    let mut values: Vec<_> = (0..3 * 32)
        .map(|i| ((i * 7 + 3) % 37) as f32 / 16. - 1.)
        .collect();
    // With a group max of 6, FP8 changes 0.2578125 to 0.25. Subsequent FP4
    // ties-to-even changes it to zero; omitting FP8 would produce 0.5.
    values[0] = 0.2578125;
    values[1] = 6.;
    bf16(&values, 3, 32)
}
fn hidden() -> Tensor {
    bf16(
        &[1., 0.5, -2., 0.25, -1., 2., 0.5, 1., 1., 0.5, 2., 1.],
        3,
        4,
    )
}
fn nearest(value: f32, palette: &[f32]) -> u8 {
    (0..palette.len())
        .min_by(|a, b| {
            let da = (f64::from(value.abs()) - f64::from(palette[*a])).abs();
            let db = (f64::from(value.abs()) - f64::from(palette[*b])).abs();
            da.total_cmp(&db).then_with(|| (a % 2).cmp(&(b % 2)))
        })
        .unwrap() as u8
}
fn scalar_fp8(row: &[f32]) -> Vec<f32> {
    let palette: Vec<_> = (0..127)
        .map(|code| reference::decode_e4m3(code).unwrap() as f32)
        .collect();
    let max = row.iter().copied().map(f32::abs).fold(1e-4_f32, f32::max);
    let sf = 2_f32.powi((max * (1_f32 / 448.)).log2().ceil() as i32);
    row.iter()
        .map(|x| {
            let code = nearest(*x / sf, &palette);
            palette[code as usize] * sf * if x.is_sign_negative() { -1. } else { 1. }
        })
        .collect()
}
fn scalar_rope(row: &mut [f32], position: usize) {
    let (cos, sin) = [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)][position];
    for pair in 0..32 {
        let first = 64 + pair * 2;
        let a = row[first];
        let b = row[first + 1];
        row[first] = round_bf16(a * cos - b * sin);
        row[first + 1] = round_bf16(b * cos + a * sin);
    }
}
fn scalar_pack(rows: &[Vec<f32>]) -> (Vec<u8>, Vec<u8>, Vec<Vec<f32>>) {
    let palette = [0., 0.5, 1., 1.5, 2., 3., 4., 6.];
    let mut data = Vec::new();
    let mut scales = Vec::new();
    let mut decoded = Vec::new();
    for row in rows {
        let mut codes = Vec::new();
        let mut logical = Vec::new();
        for group in row.chunks_exact(32) {
            let max = group
                .iter()
                .copied()
                .map(f32::abs)
                .fold(6. * f32::MIN_POSITIVE, f32::max);
            let exp = (max * (1_f32 / 6.)).log2().ceil() as i32;
            let sf = 2_f32.powi(exp);
            scales.push((exp + 127) as u8);
            for x in group {
                let code = nearest(*x / sf, &palette);
                let negative = x.is_sign_negative();
                codes.push(code | if negative { 8 } else { 0 });
                logical.push(palette[code as usize] * sf * if negative { -1. } else { 1. });
            }
        }
        data.extend(codes.chunks_exact(2).map(|pair| pair[0] | (pair[1] << 4)));
        decoded.push(logical);
    }
    (data, scales, decoded)
}
fn scalar_queries(input: &Tensor, positions: &[usize]) -> Vec<Vec<f32>> {
    let values = floats(input);
    let mut result = Vec::new();
    for (token, position) in positions.iter().enumerate() {
        let cast = scalar_fp8(&values[token * 32..(token + 1) * 32]);
        for head in 0..2 {
            let mut row: Vec<_> = (0..128)
                .map(|i| {
                    // GEMM accumulates its products into +0. A sparse diagonal
                    // multiply alone would retain -0 for the negative head.
                    let dot = (0..32).fold(0_f32, |sum, column| {
                        let weight = if column == i % 32 {
                            if head == 0 {
                                1.
                            } else {
                                -1.
                            }
                        } else {
                            0.
                        };
                        sum + cast[column] * weight
                    });
                    round_bf16(dot)
                })
                .collect();
            scalar_rope(&mut row, *position);
            result.push(row);
        }
    }
    result
}
fn scalar_keys(input: &Tensor, positions: &[usize]) -> Vec<Vec<f32>> {
    let input = floats(input);
    let mut result = Vec::new();
    for (token, position) in positions.iter().enumerate() {
        let mut row: Vec<_> = (0..128)
            .map(|i| round_bf16(input[token * 4 + i % 4] * [1., 2., 0.5, -1.][i % 4]))
            .collect();
        let mean = (row.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / 128.) as f32;
        let divisor = (mean + 1e-20_f32).sqrt();
        for (i, x) in row.iter_mut().enumerate() {
            *x = round_bf16((*x / divisor) * [1., 2., 0.5, -1.][i % 4]);
        }
        scalar_rope(&mut row, *position);
        result.push(row);
    }
    result
}

#[test]
fn queries_preserve_fp8_input_bf16_projection_trailing_rope_and_unscaled_weights() {
    let device = device();
    let projection = query(&device);
    let input = latent();
    let before = floats(&input);
    let hidden = hidden();
    let output = projection
        .forward(&hidden, &input, &[0, 2, 3], &CancellationToken::new())
        .unwrap();
    let (data, scales, _) = scalar_pack(&scalar_queries(&input, &[0, 2, 3]));
    assert_eq!(bytes(output.rows().data_bytes()), data);
    assert_eq!(bytes(output.rows().scale_bytes()), scales);
    assert_eq!(output.rows().row_count(), 6);
    assert_eq!(data[0] & 15, 0, "FP8 cast must precede FP4 cast");
    assert_eq!(data[0] >> 4, 7, "no extra per-head Q norm is applied");
    let weight_rows = [[1., 2., 0., -0.5], [-1., 0., 2., 1.]];
    let expected: Vec<_> = floats(&hidden)
        .chunks_exact(4)
        .flat_map(|x| {
            weight_rows
                .iter()
                .map(|w| round_bf16(x.iter().zip(w).map(|(x, w)| x * w).sum()))
        })
        .collect();
    assert_eq!(floats(output.head_weights()), expected);
    assert!(expected.iter().any(|x| *x < 0.));
    assert_eq!(floats(&input), before);
    assert_eq!(projection.retained_bytes(), 9224 + 16 + 1024);
}

#[test]
fn keys_match_independent_projection_learned_norm_and_explicit_compressed_positions() {
    let device = device();
    let projection = key(&device);
    let input = bf16(&[1., 2., -1., 0.5, -1., 1., 2., -0.5], 2, 4);
    let before = floats(&input);
    let output = projection
        .forward(&input, &[1, 3], &CancellationToken::new())
        .unwrap();
    let (data, scales, _) = scalar_pack(&scalar_keys(&input, &[1, 3]));
    assert_eq!(bytes(output.data_bytes()), data);
    assert_eq!(bytes(output.scale_bytes()), scales);
    let unrotated = scalar_pack(&scalar_keys(&input, &[0, 0]));
    assert_ne!(
        data, unrotated.0,
        "positions_c cannot be replaced by row numbers"
    );
    assert_eq!(floats(&input), before);
    assert_eq!(projection.retained_bytes(), 1024 + 512 + 1024);
}

#[test]
fn chunking_and_zero_completed_c2_pairs_keep_exact_packed_rows() {
    let device = device();
    let q = query(&device);
    let k = key(&device);
    let cancel = CancellationToken::new();
    let latent = latent();
    let hidden = hidden();
    let all = q.forward(&hidden, &latent, &[0, 2, 3], &cancel).unwrap();
    let mut data = Vec::new();
    let mut scales = Vec::new();
    let mut weights = Vec::new();
    for (row, pos) in [0, 2, 3].into_iter().enumerate() {
        let one = q
            .forward(
                &hidden.narrow(0, row, 1).unwrap(),
                &latent.narrow(0, row, 1).unwrap(),
                &[pos],
                &cancel,
            )
            .unwrap();
        data.extend(bytes(one.rows().data_bytes()));
        scales.extend(bytes(one.rows().scale_bytes()));
        weights.extend(floats(one.head_weights()));
    }
    assert_eq!(data, bytes(all.rows().data_bytes()));
    assert_eq!(scales, bytes(all.rows().scale_bytes()));
    assert_eq!(weights, floats(all.head_weights()));
    let empty = k.forward(&bf16(&[], 0, 4), &[], &cancel).unwrap();
    assert_eq!(empty.row_count(), 0);
    assert_eq!(empty.data_bytes().dims(), [0, 64]);
    let input = bf16(&[1., 2., -1., 0.5, -1., 1., 2., -0.5], 2, 4);
    let all = k.forward(&input, &[1, 3], &cancel).unwrap();
    let mut data = Vec::new();
    let mut scales = Vec::new();
    for (row, pos) in [1, 3].into_iter().enumerate() {
        let one = k
            .forward(&input.narrow(0, row, 1).unwrap(), &[pos], &cancel)
            .unwrap();
        data.extend(bytes(one.data_bytes()));
        scales.extend(bytes(one.scale_bytes()));
    }
    assert_eq!(data, bytes(all.data_bytes()));
    assert_eq!(scales, bytes(all.scale_bytes()));
}

#[test]
fn projected_packed_queries_and_keys_drive_causal_topk_without_future_rows() {
    let device = device();
    let cancel = CancellationToken::new();
    let latent = latent();
    let q = query(&device)
        .forward(&hidden(), &latent, &[0, 2, 3], &cancel)
        .unwrap();
    let input = bf16(&[1., 2., -1., 0.5, -1., 1., 2., -0.5], 2, 4);
    let k = key(&device).forward(&input, &[1, 3], &cancel).unwrap();
    let plan = SparseIndexPlan::new(device.limits(), &[1, 3], &[0, 2, 3], None).unwrap();
    let indexer = SparseIndexer::new(device.clone(), 2, 128, 1, None).unwrap();
    let selection = indexer
        .select_packed(q.rows(), &k, q.head_weights(), &plan, &cancel)
        .unwrap();
    let logical_q = scalar_pack(&scalar_queries(&latent, &[0, 2, 3])).2;
    let logical_k = scalar_pack(&scalar_keys(&input, &[1, 3])).2;
    let weights = floats(q.head_weights());
    let mut scores = Vec::new();
    for key in &logical_k {
        let score: f64 = (0..2)
            .map(|h| {
                let dot: f64 = logical_q[4 + h]
                    .iter()
                    .zip(key)
                    .map(|(q, k)| f64::from(*q) * f64::from(*k))
                    .sum();
                dot.max(0.) * f64::from(weights[4 + h])
            })
            .sum();
        scores.push(round_bf16(score as f32));
    }
    assert!(
        (scores[0] - scores[1]).abs() > 0.1,
        "selection must be non-vacuous"
    );
    let winner = if scores[0] >= scores[1] { 0 } else { 1 };
    assert_eq!(
        selection.rows().to_vec2::<u32>().unwrap(),
        vec![vec![INVALID_INDEX_ROW], vec![0], vec![winner]]
    );
    assert_eq!(selection.valid_lengths(), [0, 1, 1]);
}

#[test]
fn layouts_geometry_missing_learned_norm_and_foreign_limits_refuse() {
    let device = device();
    assert!(IndexQueryProjection::new(
        device.clone(),
        query_matrix(&device, 31, 2),
        matrix(&device, &[0.; 8], 2, 4),
        cache()
    )
    .is_err());
    assert!(IndexQueryProjection::new(
        device.clone(),
        query_matrix(&device, 32, 1),
        matrix(&device, &[0.; 8], 2, 4),
        cache()
    )
    .is_err());
    assert!(IndexQueryProjection::new(
        device.clone(),
        query_matrix(&device, 32, 2),
        matrix(&device, &[0.; 8], 2, 4),
        Tensor::zeros((4, 32), DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
    assert!(IndexKeyProjection::new(
        device.clone(),
        matrix(&device, &key_weights(), 128, 4),
        norm(&device, false),
        cache()
    )
    .is_err());
    let other = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_context_tokens: 32,
        ..InferenceLimits::default()
    });
    assert!(IndexKeyProjection::new(
        device.clone(),
        matrix(&device, &key_weights(), 128, 4),
        norm(&other, true),
        cache()
    )
    .is_err());
    let f32_matrix = DeviceMatrix::from_tensors_for_tests(
        &device,
        MatrixLayout::F32,
        [128, 4],
        &Tensor::zeros((128, 4), DType::F32, &Device::Cpu).unwrap(),
        None,
    )
    .unwrap();
    assert!(
        IndexKeyProjection::new(device.clone(), f32_matrix, norm(&device, true), cache()).is_err()
    );
}

#[test]
fn runtime_shape_nonfinite_positions_and_cancellation_refuse_without_output() {
    let device = device();
    let q = query(&device);
    let k = key(&device);
    let cancel = CancellationToken::new();
    assert!(q.forward(&hidden(), &latent(), &[0, 1], &cancel).is_err());
    assert!(q
        .forward(&hidden(), &latent(), &[0, 1, 4], &cancel)
        .is_err());
    assert!(q
        .forward(
            &hidden().narrow(0, 0, 2).unwrap(),
            &latent(),
            &[0, 1, 2],
            &cancel
        )
        .is_err());
    assert!(q
        .forward(
            &hidden(),
            &latent().to_dtype(DType::F32).unwrap(),
            &[0, 1, 2],
            &cancel
        )
        .is_err());
    let bad = bf16(&[f32::NAN; 12], 3, 4);
    assert!(q.forward(&bad, &latent(), &[0, 1, 2], &cancel).is_err());
    assert!(k.forward(&bad, &[0, 1, 2], &cancel).is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        q.forward(&hidden(), &latent(), &[0, 1, 2], &cancelled),
        Err(PowerError::InferenceCancelled)
    ));
    assert!(matches!(
        k.forward(&bf16(&[], 0, 4), &[], &cancelled),
        Err(PowerError::InferenceCancelled)
    ));
}

#[test]
fn aggregate_resident_and_composite_live_budgets_refuse() {
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 10_000,
        ..InferenceLimits::default()
    });
    assert!(IndexQueryProjection::new(
        low.clone(),
        query_matrix(&low, 32, 2),
        matrix(&low, &[0.; 8], 2, 4),
        cache()
    )
    .is_err());
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 300_000,
        ..InferenceLimits::default()
    });
    let projection = query(&low);
    let error = projection
        .forward(
            &hidden().narrow(0, 0, 1).unwrap(),
            &latent().narrow(0, 0, 1).unwrap(),
            &[0],
            &CancellationToken::new(),
        )
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("index projection composite live tensors"),
        "{error}"
    );
}
