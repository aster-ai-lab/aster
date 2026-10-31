//! Owned-H200, stage-local qualification of the actual XQ projection/codec/
//! sparse-attention chain. Independent F64 equations check each stage's
//! executed inputs; this is not a complete-model teacher-forced comparison.
use super::*;
use crate::layers::attention::{KvCandidate, KvSource, SparseAttentionPlan, SparseMqaAttention};
use crate::layers::xq::quant::tests::scalar;
use aster_power::inference::{DevicePreference, RuntimeDevice};

struct HostMatrix {
    rows: Vec<Vec<f64>>,
    raw: Vec<u8>,
}
impl HostMatrix {
    fn new(rows: usize, columns: usize, seed: usize) -> Self {
        let mut raw = Vec::new();
        let mut values = Vec::new();
        for row in 0..rows {
            let mut bytes = vec![0; columns / 32 * 34];
            for block in bytes.chunks_exact_mut(34) {
                block[..2].copy_from_slice(&0x2800_u16.to_le_bytes());
            } // 1/32
            let col = (row * 7 + seed * 11) % columns;
            let code = if (row + seed) % 2 == 0 { 3_i8 } else { -2_i8 };
            bytes[col / 32 * 34 + 2 + col % 32] = code as u8;
            let mut logical = vec![0_f64; columns];
            logical[col] = f64::from(code) / 32.;
            raw.extend(bytes);
            values.push(logical);
        }
        Self { rows: values, raw }
    }
    fn upload(&self, device: &CudaLayerDevice) -> XqDeviceMatrix {
        XqDeviceMatrix::from_bytes_for_tests(
            device,
            XqTensorType::Q8_0,
            self.rows.len(),
            self.rows[0].len(),
            &self.raw,
        )
        .unwrap()
    }
    fn dot(&self, first: usize, count: usize, input: &[f64]) -> Vec<f64> {
        self.rows[first..first + count]
            .iter()
            .map(|row| row.iter().zip(input).map(|(w, x)| w * x).sum())
            .collect()
    }
}
fn normalize(row: &[f64], weight: &[f32]) -> Vec<f64> {
    let rms = (row.iter().map(|x| x * x).sum::<f64>() / row.len() as f64 + 1e-20).sqrt();
    row.iter()
        .zip(weight)
        .map(|(x, w)| x / rms * f64::from(*w))
        .collect()
}
fn rotate(row: &[f64], cache: &[f32], conjugate: bool) -> Vec<f64> {
    let mut out = row.to_vec();
    for pair in 0..32 {
        let col = 448 + pair * 2;
        let x = row[col];
        let y = row[col + 1];
        let c = f64::from(cache[pair]);
        let s = f64::from(cache[32 + pair]) * if conjugate { -1. } else { 1. };
        out[col] = x * c - y * s;
        out[col + 1] = y * c + x * s;
    }
    out
}
fn close(actual: &[f32], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    // Frozen before hardware execution. These are stage-local F32 math
    // bounds; quantization itself is compared by exact encoded bytes.
    for (col, (a, e)) in actual.iter().zip(expected).enumerate() {
        let budget = 2e-5 + 2e-5 * e.abs();
        assert!(
            (f64::from(*a) - e).abs() <= budget,
            "column {col}: actual={a} expected={e} budget={budget}"
        );
    }
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_f32_projections_codecs_rope_and_sparse_attention_match_stage_oracles() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let mut midpoint = vec![0_f32; 512];
    midpoint[0] = f32::from_bits(0x3cc3ffff);
    midpoint[15] = 6. * 0.013671875;
    let midpoint = XqQuantizedActivation::encode(
        &device,
        XqActivationFormat::CompressedFp4,
        &Tensor::from_vec(midpoint, (1, 512), device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    assert_eq!(midpoint.scale_bytes().to_vec2::<u8>().unwrap()[0][0], 7);
    assert_eq!(midpoint.data_bytes().to_vec2::<u8>().unwrap()[0][0] & 15, 3);
    let mut subnormal = vec![0_f32; 128];
    subnormal[0] = -0.;
    subnormal[1] = -f32::MIN_POSITIVE * 0.5;
    subnormal[2] = f32::MIN_POSITIVE * 0.5;
    let subnormal = XqQuantizedActivation::encode(
        &device,
        XqActivationFormat::IndexFp4,
        &Tensor::from_vec(subnormal, (1, 128), device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    assert_eq!(
        subnormal.scale_bytes().to_vec2::<u8>().unwrap(),
        [vec![1_u8; 4]]
    );
    assert_eq!(
        &subnormal.data_bytes().to_vec2::<u8>().unwrap()[0][..2],
        &[0x98_u8, 0x01]
    );
    let qa = HostMatrix::new(32, 32, 1);
    let qb = HostMatrix::new(512, 32, 2);
    let kv = HostMatrix::new(512, 32, 3);
    let ga = HostMatrix::new(64, 256, 4);
    let ob = HostMatrix::new(32, 64, 5);
    let norm_q: Vec<f32> = (0..32).map(|i| 0.9 + (i % 7) as f32 * 0.03).collect();
    let norm_k: Vec<f32> = (0..512).map(|i| 1. + (i % 5) as f32 * 0.01).collect();
    let cache_rows: Vec<Vec<f32>> = (0..3)
        .map(|position| {
            let phases: Vec<f32> = (0..32)
                .map(|pair| position as f32 * (pair + 1) as f32 * 0.003)
                .collect();
            phases
                .iter()
                .map(|x| x.cos())
                .chain(phases.iter().map(|x| x.sin()))
                .collect()
        })
        .collect();
    let cache_tensor = Tensor::new(cache_rows.clone(), device.tensor_device()).unwrap();
    let input = XqAttentionInputProjection::from_parts(
        device.clone(),
        InputParts {
            query_a: qa.upload(&device),
            query_b: qb.upload(&device),
            window: kv.upload(&device),
            query_norm: XqRmsNorm::new(
                device.clone(),
                Tensor::new(norm_q.as_slice(), device.tensor_device()).unwrap(),
            )
            .unwrap(),
            window_norm: XqRmsNorm::new(
                device.clone(),
                Tensor::new(norm_k.as_slice(), device.tensor_device()).unwrap(),
            )
            .unwrap(),
            cos_sin: cache_tensor.clone(),
        },
    )
    .unwrap();
    let output = XqAttentionOutputProjection::from_parts(
        device.clone(),
        OutputParts {
            group_a: ga.upload(&device),
            output_b: ob.upload(&device),
            cos_sin: cache_tensor,
            heads: 1,
            groups: 2,
            rank: 32,
        },
    )
    .unwrap();
    let rows: Vec<Vec<f32>> = (0..3)
        .map(|token| {
            (0..32)
                .map(|col| ((col * 7 + token * 3) % 23) as f32 * 0.071 - 0.75 + 0.0001)
                .collect()
        })
        .collect();
    let hidden = Tensor::new(rows.clone(), device.tensor_device()).unwrap();
    let projected = input.forward(&hidden, &[0, 1, 2], &cancel).unwrap();
    assert_eq!(projected.query().dtype(), DType::F32);
    assert_eq!(projected.normalized_query().dtype(), DType::F32);
    assert_eq!(projected.positions(), [0, 1, 2]);
    let query = projected.query().to_vec3::<f32>().unwrap();
    let latent = projected.normalized_query().to_vec2::<f32>().unwrap();
    // Inspect the actual pre-quant F32 stage independently of the encoder.
    let raw_window = input
        .window
        .forward_f32_control(&hidden, &cancel)
        .unwrap()
        .0;
    let norm_window = input
        .window_norm
        .forward(&raw_window, &cancel)
        .unwrap()
        .unsqueeze(1)
        .unwrap();
    let post_rope = input
        .rope
        .forward_f32_control(&norm_window, &[0, 1, 2], RopeDirection::Forward, &cancel)
        .unwrap()
        .squeeze(1)
        .unwrap();
    let post_rows = post_rope.to_vec2::<f32>().unwrap();
    let encoded = projected.window_rows().records().to_vec2::<u8>().unwrap();
    let mut decoded_rows = Vec::new();
    for token in 0..3 {
        let source: Vec<_> = rows[token].iter().copied().map(f64::from).collect();
        let qref = normalize(&qa.dot(0, 32, &source), &norm_q);
        close(&latent[token], &qref);
        let qref = rotate(
            &qb.dot(
                0,
                512,
                &latent[token]
                    .iter()
                    .copied()
                    .map(f64::from)
                    .collect::<Vec<_>>(),
            ),
            &cache_rows[token],
            false,
        );
        close(&query[token][0], &qref);
        let kref = rotate(
            &normalize(&kv.dot(0, 512, &source), &norm_k),
            &cache_rows[token],
            false,
        );
        close(&post_rows[token], &kref);
        let (data, scales, decoded) = scalar(XqActivationFormat::WindowFp8, &post_rows[token]);
        assert_eq!(encoded[token], [data, scales].concat());
        decoded_rows.push(decoded);
    }
    // Both remaining activation formats also execute on the actual device.
    for format in [
        XqActivationFormat::CompressedFp4,
        XqActivationFormat::IndexFp4,
    ] {
        let values = if format == XqActivationFormat::IndexFp4 {
            post_rope.narrow(1, 0, 128).unwrap()
        } else {
            post_rope.clone()
        };
        let packed = XqQuantizedActivation::encode(&device, format, &values, &cancel).unwrap();
        let data = packed.data_bytes().to_vec2::<u8>().unwrap();
        let scales = packed.scale_bytes().to_vec2::<u8>().unwrap();
        for (token, row) in values.to_vec2::<f32>().unwrap().iter().enumerate() {
            let (expected_data, expected_scales, _) = scalar(format, row);
            assert_eq!(data[token], expected_data);
            assert_eq!(scales[token], expected_scales);
        }
    }
    let half_rows = Tensor::new(decoded_rows, &Device::Cpu)
        .unwrap()
        .to_dtype(DType::F16)
        .unwrap()
        .to_dtype(DType::F32)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    let candidates: Vec<_> = (0..3)
        .map(|position| KvCandidate::new(KvSource::Window, position))
        .collect();
    let plan = SparseAttentionPlan::new(
        device.limits(),
        128,
        &candidates,
        &[0, 1, 2],
        &[vec![0], vec![0, 1], vec![0, 1, 2]],
    )
    .unwrap();
    let sink = 0.137_f32;
    let attention = SparseMqaAttention::new(
        device.clone(),
        1,
        512,
        Tensor::new(&[sink], device.tensor_device()).unwrap(),
    )
    .unwrap();
    assert!(!candle_core::cuda_backend::gemm_reduced_precision_f32());
    {
        struct RestoreF32;
        impl Drop for RestoreF32 {
            fn drop(&mut self) {
                candle_core::cuda_backend::set_gemm_reduced_precision_f32(false);
            }
        }
        let _restore = RestoreF32;
        candle_core::cuda_backend::set_gemm_reduced_precision_f32(true);
        assert!(attention
            .forward_xq_f32_packed(
                projected.query(),
                projected.window_rows(),
                None,
                &plan,
                &cancel
            )
            .is_err());
    }
    let attended = attention
        .forward_xq_f32_packed(
            projected.query(),
            projected.window_rows(),
            None,
            &plan,
            &cancel,
        )
        .unwrap();
    let attended_rows = attended.to_vec3::<f32>().unwrap();
    for token in 0..3 {
        let logits: Vec<f64> = half_rows[..=token]
            .iter()
            .map(|key| {
                query[token][0]
                    .iter()
                    .zip(key)
                    .map(|(q, k)| f64::from(*q) * f64::from(*k))
                    .sum::<f64>()
                    / (512_f64).sqrt()
            })
            .collect();
        let maximum = logits.iter().copied().fold(f64::from(sink), f64::max);
        let denominator = logits.iter().map(|x| (x - maximum).exp()).sum::<f64>()
            + (f64::from(sink) - maximum).exp();
        let expected: Vec<f64> = (0..512)
            .map(|col| {
                logits
                    .iter()
                    .enumerate()
                    .map(|(row, x)| {
                        (x - maximum).exp() / denominator * f64::from(half_rows[row][col])
                    })
                    .sum()
            })
            .collect();
        close(&attended_rows[token][0], &expected);
    }
    let result = output.forward(&attended, &[0, 1, 2], &cancel).unwrap();
    assert_eq!(result.dtype(), DType::F32);
    let actual = result.to_vec2::<f32>().unwrap();
    for token in 0..3 {
        let rotated = rotate(
            &attended_rows[token][0]
                .iter()
                .copied()
                .map(f64::from)
                .collect::<Vec<_>>(),
            &cache_rows[token],
            true,
        );
        let mut grouped = ga.dot(0, 32, &rotated[..256]);
        grouped.extend(ga.dot(32, 32, &rotated[256..]));
        close(&actual[token], &ob.dot(0, 32, &grouped));
    }
    println!("qualified stage-local XQ Q8 projections, F32 norms/RoPE, exact three activation codecs, F16-KV sparse MQA and grouped output; full model/state qualification remains open");
}
