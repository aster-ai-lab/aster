//! Owned-H200 stage-local F16/F32 compression, index projections/selection and
//! atomic compressed/main/index state qualification. Not a complete model.
use super::*;
use crate::{
    layers::{
        indexer::{SparseIndexPlan, SparseIndexer, INVALID_INDEX_ROW},
        kv::MainKvFormat,
        xq::{compressor::XqKvCompressor, quant::tests::scalar},
    },
    state::{
        index::IndexKvState,
        kv::{KvCacheKind, KvState},
        transaction::StateCommitBatch,
        CompressionKind, CompressionState,
    },
};
use aster_power::inference::{DevicePreference, RuntimeDevice};

struct HostMatrix {
    values: Vec<Vec<f64>>,
    raw: Vec<u8>,
}
impl HostMatrix {
    fn new(rows: usize, columns: usize, seed: usize) -> Self {
        let mut raw = Vec::new();
        let mut values = Vec::new();
        for row in 0..rows {
            let col = (row * 7 + row / 128 + seed * 3) % columns;
            let (bits, value) = if (row + seed + row / 64) % 2 == 0 {
                (0x3000_u16, 0.125)
            } else {
                (0xac00_u16, -0.0625)
            };
            let mut logical = vec![0.; columns];
            logical[col] = value;
            values.push(logical);
            for column in 0..columns {
                raw.extend(if column == col { bits } else { 0_u16 }.to_le_bytes());
            }
        }
        Self { values, raw }
    }
    fn upload(&self, device: &CudaLayerDevice) -> XqDeviceMatrix {
        XqDeviceMatrix::from_bytes_for_tests(
            device,
            XqTensorType::F16,
            self.values.len(),
            self.values[0].len(),
            &self.raw,
        )
        .unwrap()
    }
    fn dot(&self, input: &[f64]) -> Vec<f64> {
        self.values
            .iter()
            .map(|row| row.iter().zip(input).map(|(w, x)| w * x).sum())
            .collect()
    }
}
fn norm_reference(values: &[f64], weight: &[f32]) -> Vec<f64> {
    let rms = (values.iter().map(|x| x * x).sum::<f64>() / values.len() as f64 + 1e-20).sqrt();
    values
        .iter()
        .zip(weight)
        .map(|(x, w)| x / rms * f64::from(*w))
        .collect()
}
fn close(actual: &[f32], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    // Frozen before hardware execution; activation encoding has exact-byte
    // oracles separately, rather than a relaxed quantized-output tolerance.
    for (col, (a, e)) in actual.iter().zip(expected).enumerate() {
        let budget = 2e-5 + 2e-5 * e.abs();
        assert!(
            (f64::from(*a) - e).abs() <= budget,
            "column {col}: actual={a} expected={e} budget={budget}"
        );
    }
}
fn quarter_rotate(row: &[f32], position: usize) -> Vec<f32> {
    let mut output = row.to_vec();
    let first = row.len() - 64;
    for pair in 0..32 {
        let a = first + pair * 2;
        let x = row[a];
        let y = row[a + 1];
        let (real, imag) = match position % 4 {
            0 => (x, y),
            1 => (-y, x),
            2 => (-x, -y),
            3 => (y, -x),
            _ => unreachable!(),
        };
        output[a] = real;
        output[a + 1] = imag;
    }
    output
}
fn gpu_cache(device: &CudaLayerDevice) -> Tensor {
    let mut values = Vec::new();
    for position in 0..8 {
        let (c, s) = [(1_f32, 0_f32), (0., 1.), (-1., 0.), (0., -1.)][position % 4];
        values.extend([c; 32]);
        values.extend([s; 32]);
    }
    Tensor::from_vec(values, (8, 64), device.tensor_device()).unwrap()
}

struct Rig {
    device: CudaLayerDevice,
    compressor: XqKvCompressor,
    key: XqIndexKeyProjection,
    query: XqIndexQueryProjection,
    compression: CompressionState,
    main: KvState,
    index: IndexKvState,
    a: HostMatrix,
    gate: HostMatrix,
    k: HostMatrix,
    q: HostMatrix,
    w: HostMatrix,
    norm_main: Vec<f32>,
    norm_key: Vec<f32>,
    hidden: Vec<Vec<f32>>,
    latent: Vec<Vec<f32>>,
}
impl Rig {
    fn new(device: &CudaLayerDevice, layer: usize) -> Self {
        let a = HostMatrix::new(512, 4, 1);
        let gate = HostMatrix::new(512, 4, 2);
        let k = HostMatrix::new(128, 512, 3);
        let q = HostMatrix::new(256, 4, 4);
        let w = HostMatrix::new(2, 4, 5);
        let norm_main: Vec<f32> = (0..512).map(|col| 1. + (col % 3) as f32 * 0.03).collect();
        let norm_key: Vec<f32> = (0..128).map(|col| 0.9 + (col % 5) as f32 * 0.02).collect();
        let make_norm = |weight: &[f32]| {
            XqRmsNorm::new(
                device.clone(),
                Tensor::new(weight, device.tensor_device()).unwrap(),
            )
            .unwrap()
        };
        let kind = if layer == 20 {
            CompressionKind::C1
        } else {
            CompressionKind::C2
        };
        let compressor = XqKvCompressor::from_parts(
            device.clone(),
            layer,
            a.upload(device),
            (kind == CompressionKind::C2).then(|| gate.upload(device)),
            make_norm(&norm_main),
            gpu_cache(device),
        )
        .unwrap();
        let key = XqIndexKeyProjection::from_parts(
            device.clone(),
            layer,
            k.upload(device),
            make_norm(&norm_key),
            gpu_cache(device),
        )
        .unwrap();
        let query = XqIndexQueryProjection::from_parts(
            device.clone(),
            layer,
            q.upload(device),
            w.upload(device),
            gpu_cache(device),
        )
        .unwrap();
        let compression = compressor.new_state(8, 8).unwrap();
        let main = KvState::new(
            device,
            if kind == CompressionKind::C1 {
                KvCacheKind::C1
            } else {
                KvCacheKind::C2
            },
            MainKvFormat::Fp4E4m3,
            8,
            8,
        )
        .unwrap();
        let index = IndexKvState::new(device, kind, 8, 8).unwrap();
        let hidden: Vec<Vec<f32>> = (0..5)
            .map(|token| {
                vec![
                    1.0001 + token as f32 * 0.13,
                    -0.7503 - token as f32 * 0.07,
                    0.5001 + token as f32 * 0.11,
                    -1.5007 + token as f32 * 0.01,
                ]
            })
            .collect();
        let latent: Vec<Vec<f32>> = (0..5)
            .map(|token| {
                vec![
                    0.9001 + token as f32 * 0.09,
                    -1.1003 - token as f32 * 0.03,
                    1.7001 + token as f32 * 0.02,
                    -0.4007 - token as f32 * 0.12,
                ]
            })
            .collect();
        Self {
            device: device.clone(),
            compressor,
            key,
            query,
            compression,
            main,
            index,
            a,
            gate,
            k,
            q,
            w,
            norm_main,
            norm_key,
            hidden,
            latent,
        }
    }

    fn exercise(&mut self, start: usize, tokens: usize, accepted: usize, cancel_at_commit: bool) {
        let cancel = CancellationToken::new();
        let end = start + tokens;
        let positions: Vec<_> = (start..end).collect();
        let hidden = Tensor::new(
            self.hidden[start..end].to_vec(),
            self.device.tensor_device(),
        )
        .unwrap();
        let latent = Tensor::new(
            self.latent[start..end].to_vec(),
            self.device.tensor_device(),
        )
        .unwrap();
        let prepared = self
            .compressor
            .prepare(&self.compression, start, &hidden, &cancel)
            .unwrap();
        let ready = prepared.emitted_positions().to_vec();
        let rotation = prepared.rotary_positions().to_vec();
        let pre = prepared.pre_rope_kv().to_vec2::<f32>().unwrap();
        let ratio = self.compression.kind().ratio();
        assert_eq!(
            ready,
            ((start / ratio)..(end / ratio))
                .map(|row| (row + 1) * ratio - 1)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            rotation,
            ready
                .iter()
                .map(|ready| ready - (ratio - 1))
                .collect::<Vec<_>>()
        );
        let records = prepared.rows().records().to_vec2::<u8>().unwrap();
        for (row, position) in rotation.iter().enumerate() {
            let first = *position;
            let raw = if ratio == 1 {
                self.a.dot(
                    &self.hidden[first]
                        .iter()
                        .copied()
                        .map(f64::from)
                        .collect::<Vec<_>>(),
                )
            } else {
                let x0: Vec<_> = self.hidden[first].iter().copied().map(f64::from).collect();
                let x1: Vec<_> = self.hidden[first + 1]
                    .iter()
                    .copied()
                    .map(f64::from)
                    .collect();
                let a = self.a.dot(&x0);
                let b = self.a.dot(&x1);
                let ga = self.gate.dot(&x0);
                let gb = self.gate.dot(&x1);
                (0..512)
                    .map(|col| {
                        let max = ga[col].max(gb[col]);
                        let wa = (ga[col] - max).exp();
                        let wb = (gb[col] - max).exp();
                        (a[col] * wa + b[col] * wb) / (wa + wb)
                    })
                    .collect()
            };
            close(&pre[row], &norm_reference(&raw, &self.norm_main));
            let rotated = quarter_rotate(&pre[row], *position);
            let (data, scales, _) = scalar(XqActivationFormat::CompressedFp4, &rotated);
            assert_eq!(records[row], [data, scales].concat());
        }
        let packed_keys = self.key.forward_compressed(&prepared, &cancel).unwrap();
        if !pre.is_empty() {
            let raw = self
                .key
                .key
                .forward_f32_control(prepared.pre_rope_kv(), &cancel)
                .unwrap()
                .0;
            let normalized = self.key.norm.forward(&raw, &cancel).unwrap();
            let actual = normalized.to_vec2::<f32>().unwrap();
            for row in 0..pre.len() {
                close(
                    &actual[row],
                    &norm_reference(
                        &self
                            .k
                            .dot(&pre[row].iter().copied().map(f64::from).collect::<Vec<_>>()),
                        &self.norm_key,
                    ),
                );
                let rotated = quarter_rotate(&actual[row], rotation[row]);
                let (data, scales, _) = scalar(XqActivationFormat::IndexFp4, &rotated);
                assert_eq!(packed_keys.data_bytes().to_vec2::<u8>().unwrap()[row], data);
                assert_eq!(
                    packed_keys.scale_bytes().to_vec2::<u8>().unwrap()[row],
                    scales
                );
            }
        }
        let projected_query = self
            .query
            .forward(&hidden, &latent, &positions, &cancel)
            .unwrap();
        let raw_query = self
            .query
            .query
            .forward_f32_control(&latent, &cancel)
            .unwrap()
            .0
            .to_vec2::<f32>()
            .unwrap();
        let weights = projected_query.head_weights().to_vec2::<f32>().unwrap();
        for token in 0..tokens {
            close(
                &raw_query[token],
                &self.q.dot(
                    &self.latent[start + token]
                        .iter()
                        .copied()
                        .map(f64::from)
                        .collect::<Vec<_>>(),
                ),
            );
            let expected_w: Vec<_> = self
                .w
                .dot(
                    &self.hidden[start + token]
                        .iter()
                        .copied()
                        .map(f64::from)
                        .collect::<Vec<_>>(),
                )
                .iter()
                .map(|x| x / 16.)
                .collect();
            close(&weights[token], &expected_w);
            for head in 0..2 {
                let rotated = quarter_rotate(
                    &raw_query[token][head * 128..(head + 1) * 128],
                    positions[token],
                );
                let (data, scales, _) = scalar(XqActivationFormat::IndexFp4, &rotated);
                assert_eq!(
                    projected_query.rows().data_bytes().to_vec2::<u8>().unwrap()[token * 2 + head],
                    data
                );
                assert_eq!(
                    projected_query
                        .rows()
                        .scale_bytes()
                        .to_vec2::<u8>()
                        .unwrap()[token * 2 + head],
                    scales
                );
            }
        }
        let (compression, _, main_rows, _) = prepared.into_parts();
        let main = self
            .main
            .prepare(start, tokens, &ready, main_rows.records(), &cancel)
            .unwrap();
        let index = self
            .index
            .prepare(
                start,
                tokens,
                &ready,
                packed_keys.data_bytes(),
                packed_keys.scale_bytes(),
                &cancel,
            )
            .unwrap();
        let all_keys = index.index_rows(&self.device).unwrap();
        let all_ready: Vec<_> = index.ready_positions().collect();
        let plan =
            SparseIndexPlan::new(self.device.limits(), &all_ready, &positions, None).unwrap();
        let selector = SparseIndexer::new(self.device.clone(), 2, 128, 1, None).unwrap();
        let selected = selector
            .select_xq_f32_packed(
                projected_query.rows(),
                &all_keys,
                projected_query.head_weights(),
                &plan,
                &cancel,
            )
            .unwrap()
            .rows()
            .to_vec2::<u32>()
            .unwrap();
        let q = projected_query
            .rows()
            .rows_data()
            .decode_prefix(tokens * 2, self.device.limits())
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        let k = all_keys
            .rows_data()
            .decode_prefix(all_keys.row_count(), self.device.limits())
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        for token in 0..tokens {
            let mut ranking: Vec<_> = all_ready
                .iter()
                .enumerate()
                .filter(|(_, ready)| **ready <= positions[token])
                .map(|(row, _)| {
                    let products: Vec<f32> = (0..2)
                        .map(|head| {
                            let dot: f64 = q[token * 2 + head]
                                .iter()
                                .zip(&k[row])
                                .map(|(q, k)| f64::from(*q) * f64::from(*k))
                                .sum();
                            (dot as f32).max(0.) * weights[token][head]
                        })
                        .collect();
                    let score = products[0] + products[1];
                    (row as u32, f64::from(score))
                })
                .collect();
            ranking.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            assert_eq!(
                selected[token],
                [ranking.first().map_or(INVALID_INDEX_ROW, |x| x.0)]
            );
        }
        let old = self.compression.position();
        let old_main = self.main.row_count();
        let old_index = self.index.row_count();
        let mut batch = StateCommitBatch::new(start, tokens, accepted, 4 * 1024 * 1024).unwrap();
        batch
            .add_compression(&mut self.compression, compression, &cancel)
            .unwrap();
        batch.add_main_kv(&mut self.main, main, &cancel).unwrap();
        batch.add_index_kv(&mut self.index, index, &cancel).unwrap();
        let commit_cancel = CancellationToken::new();
        if cancel_at_commit {
            commit_cancel.cancel();
        }
        let committed = batch.commit(&commit_cancel);
        if cancel_at_commit {
            assert!(committed.is_err());
            assert_eq!(self.compression.position(), old);
            assert_eq!(self.main.row_count(), old_main);
            assert_eq!(self.index.row_count(), old_index);
        } else {
            let committed = committed.unwrap();
            for result in committed {
                if let Some(emissions) = result.emissions() {
                    assert_eq!(emissions.dtype(), DType::F32);
                }
            }
            assert_eq!(self.compression.position(), start + accepted);
            assert_eq!(self.main.position(), start + accepted);
            assert_eq!(self.index.position(), start + accepted);
        }
    }
    fn cache_bytes(&self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>) {
        let main = self
            .main
            .attention_rows(&self.device)
            .unwrap()
            .records()
            .to_vec2::<u8>()
            .unwrap();
        let index = self.index.index_rows(&self.device).unwrap();
        (
            main,
            index.data_bytes().to_vec2::<u8>().unwrap(),
            index.scale_bytes().to_vec2::<u8>().unwrap(),
        )
    }
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_f32_compression_index_projections_and_atomic_cache_match_stage_oracles() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    for layer in [2, 20] {
        let mut candidate = Rig::new(&device, layer);
        candidate.exercise(0, 3, 3, true);
        candidate.exercise(0, 3, 1, false);
        candidate.exercise(1, 4, 4, false);
        let mut control = Rig::new(&device, layer);
        control.exercise(0, 5, 5, false);
        assert_eq!(candidate.cache_bytes(), control.cache_bytes());
        assert_eq!(
            candidate.compression.pending_bytes(),
            control.compression.pending_bytes()
        );
    }
    println!("qualified small XQ F16/F32 C1/C2, group-start RoPE, exact main/index grids, scaled F32 index scoring and cancel/partial-prefix atomic caches; full backbone/model/8-card qualification remains open");
}
