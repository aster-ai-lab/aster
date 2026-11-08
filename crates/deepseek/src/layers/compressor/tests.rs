use super::*;
use crate::{
    layers::indexer::projection::IndexKeyProjection,
    state::{
        index::IndexKvState,
        kv::{KvCacheKind, KvState},
        transaction::StateCommitBatch,
    },
};
use aster_power::inference::InferenceLimits;
use candle_core::Device;

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn bf16(values: &[f32], rows: usize, width: usize) -> Tensor {
    Tensor::from_vec(values.to_vec(), (rows, width), &Device::Cpu)
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
fn round(x: f32) -> f32 {
    let bits = x.to_bits();
    f32::from_bits(bits.wrapping_add(0x7fff + ((bits >> 16) & 1)) & 0xffff0000)
}
fn cache() -> Tensor {
    let mut values: Vec<f32> = Vec::new();
    for (c, s) in [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)] {
        values.extend([c; 32]);
        values.extend([s; 32]);
    }
    Tensor::from_vec(values, (4, 64), &Device::Cpu).unwrap()
}
fn projection_weights(gate: bool) -> Vec<f32> {
    let latent = [
        [1., 1. / 256., 0., 0.],
        [0., 0., 1., 1. / 128.],
        [-1., 0., 0.5, 0.],
        [0.25, 0.5, 0., -1.],
    ];
    let scores = [
        [0., 0., 0., 0.],
        [0., 0., 1. / 256., 0.],
        [0.125, 0., 0., 0.],
        [0., 0.25, 0., 0.],
    ];
    (0..512)
        .flat_map(|i| if gate { scores[i % 4] } else { latent[i % 4] })
        .collect()
}
fn matrix(device: &CudaLayerDevice, gate: bool) -> DeviceMatrix {
    DeviceMatrix::from_tensors_for_tests(
        device,
        MatrixLayout::Bf16,
        [512, 4],
        &bf16(&projection_weights(gate), 512, 4),
        None,
    )
    .unwrap()
}
fn norm(device: &CudaLayerDevice, width: usize) -> EffectiveRmsNorm {
    let values: Vec<_> = (0..width).map(|i| [1., 0.5, 2., -1.][i % 4]).collect();
    EffectiveRmsNorm::new(
        device.clone(),
        width,
        1e-20,
        Some(bf16(&values, 1, width).squeeze(0).unwrap()),
    )
    .unwrap()
}
fn compressor(
    device: &CudaLayerDevice,
    kind: CompressionKind,
    format: MainKvFormat,
) -> KvCompressor {
    KvCompressor::new(
        device.clone(),
        kind,
        matrix(device, false),
        (kind == CompressionKind::C2).then(|| matrix(device, true)),
        norm(device, 512),
        cache(),
        format,
    )
    .unwrap()
}
fn hidden() -> Tensor {
    bf16(
        &[
            1., 1., 1., 1., 1., 2., 2., 0.25, -1., 0.5, -0.5, 2., 2., -1., 1., -0.5,
        ],
        4,
        4,
    )
}
fn index(device: &CudaLayerDevice) -> IndexKeyProjection {
    let mut weights = vec![0_f32; 128 * 512];
    for i in 0..128 {
        weights[i * 512 + i] = 1.;
    }
    let matrix = DeviceMatrix::from_tensors_for_tests(
        device,
        MatrixLayout::Bf16,
        [128, 512],
        &bf16(&weights, 128, 512),
        None,
    )
    .unwrap();
    IndexKeyProjection::new(device.clone(), matrix, norm(device, 128), cache()).unwrap()
}
fn scalar_emissions(kind: CompressionKind, hidden: &Tensor) -> Vec<Vec<f32>> {
    let hidden = floats(hidden);
    let latent = projection_weights(false);
    let gate = projection_weights(true);
    let project = |input: &[f32], weight: &[f32]| -> Vec<f32> {
        weight
            .chunks_exact(4)
            .map(|row| row.iter().zip(input).fold(0_f32, |s, (w, x)| s + w * x))
            .collect()
    };
    let values: Vec<_> = hidden
        .chunks_exact(4)
        .map(|x| project(x, &latent))
        .collect();
    if kind == CompressionKind::C1 {
        return values
            .into_iter()
            .map(|row| row.into_iter().map(round).collect())
            .collect();
    }
    let scores: Vec<_> = hidden.chunks_exact(4).map(|x| project(x, &gate)).collect();
    values
        .chunks_exact(2)
        .zip(scores.chunks_exact(2))
        .map(|(v, s)| {
            (0..512)
                .map(|i| {
                    let a = f64::from(s[0][i]);
                    let b = f64::from(s[1][i]);
                    let max = a.max(b);
                    let x = (a - max).exp();
                    let y = (b - max).exp();
                    round(((x * f64::from(v[0][i]) + y * f64::from(v[1][i])) / (x + y)) as f32)
                })
                .collect()
        })
        .collect()
}
fn scalar_norm(rows: &[Vec<f32>]) -> Vec<f32> {
    rows.iter()
        .flat_map(|row| {
            let mean = (row.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / 512.) as f32;
            row.iter().enumerate().map(move |(i, x)| {
                round((*x / (mean + 1e-20_f32).sqrt()) * [1., 0.5, 2., -1.][i % 4])
            })
        })
        .collect()
}
fn scalar_rope(pre: &[f32], positions: &[usize]) -> Tensor {
    let mut post = pre.to_vec();
    for (row, pos) in positions.iter().enumerate() {
        let (c, s) = [(1., 0.), (0., 1.), (-1., 0.), (0., -1.)][*pos];
        for pair in 0..32 {
            let i = row * 512 + 448 + pair * 2;
            let a = pre[i];
            let b = pre[i + 1];
            post[i] = round(a * c - b * s);
            post[i + 1] = round(b * c + a * s);
        }
    }
    bf16(&post, positions.len(), 512)
}

#[test]
fn both_compressors_match_scalar_precision_norm_rope_and_main_formats() {
    let device = device();
    let hidden = hidden();
    let cancel = CancellationToken::new();
    for kind in [CompressionKind::C1, CompressionKind::C2] {
        for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
            let compressor = compressor(&device, kind, format);
            let state = CompressionState::new(&device, kind, 512, 4, 4).unwrap();
            let proposal = compressor.prepare(&state, 0, &hidden, &cancel).unwrap();
            let emissions = scalar_emissions(kind, &hidden);
            assert_eq!(
                floats(proposal.compression.emissions().unwrap()),
                emissions.iter().flatten().copied().collect::<Vec<_>>()
            );
            if kind == CompressionKind::C2 {
                assert_eq!(emissions[0][0], 1.0078125);
                assert_eq!(
                    round((round(1.00390625) + round(1.0078125)) / 2.),
                    1.,
                    "premature BF16 projection changes the pair"
                );
            }
            let expected = scalar_norm(&emissions);
            assert_eq!(floats(proposal.pre_rope_kv()), expected);
            let positions: Vec<_> = if kind == CompressionKind::C1 {
                vec![0, 1, 2, 3]
            } else {
                vec![1, 3]
            };
            assert_eq!(proposal.emitted_positions(), positions);
            let post = scalar_rope(&expected, &positions);
            assert_ne!(
                floats(&post),
                expected,
                "index-K must receive the distinct pre-RoPE view"
            );
            let expected_rows = MainKvRows::encode(&device, format, &post, &cancel).unwrap();
            assert_eq!(
                bytes(proposal.rows().records()),
                bytes(expected_rows.records())
            );
            assert_eq!(state.position(), 0);
            assert_eq!(state.pending_bytes(), 0);
            assert_eq!(compressor.kind(), kind);
        }
    }
}

fn commit(
    device: &CudaLayerDevice,
    output: PreparedCompressedKv,
    indexer: &IndexKeyProjection,
    compression: &mut CompressionState,
    main: &mut KvState,
    index: &mut IndexKvState,
    start: usize,
    tokens: usize,
    accepted: usize,
    cancel_after_staging: bool,
) -> Result<()> {
    let cancel = CancellationToken::new();
    let positions = output.emitted_positions();
    let k = indexer.forward(output.pre_rope_kv(), positions, &cancel)?;
    let main_proposal = main.prepare(start, tokens, positions, output.rows().records(), &cancel)?;
    let index_proposal = index.prepare(
        start,
        tokens,
        positions,
        k.data_bytes(),
        k.scale_bytes(),
        &cancel,
    )?;
    let (proposal, _, _) = output.into_parts();
    let mut batch =
        StateCommitBatch::new(start, tokens, accepted, device.limits().max_state_bytes)?;
    batch.add_compression(compression, proposal, &cancel)?;
    batch.add_main_kv(main, main_proposal, &cancel)?;
    batch.add_index_kv(index, index_proposal, &cancel)?;
    if cancel_after_staging {
        cancel.cancel();
    }
    batch.commit(&cancel)?;
    Ok(())
}
fn states(
    device: &CudaLayerDevice,
    kind: CompressionKind,
    format: MainKvFormat,
) -> (CompressionState, KvState, IndexKvState) {
    (
        CompressionState::new(device, kind, 512, 4, 4).unwrap(),
        KvState::new(
            device,
            if kind == CompressionKind::C1 {
                KvCacheKind::C1
            } else {
                KvCacheKind::C2
            },
            format,
            4,
            4,
        )
        .unwrap(),
        IndexKvState::new(device, kind, 4, 4).unwrap(),
    )
}

#[test]
fn all_chunk_partitions_preserve_compressed_main_and_index_rows_after_atomic_commit() {
    let device = device();
    let hidden = hidden();
    let cancel = CancellationToken::new();
    let indexer = index(&device);
    for kind in [CompressionKind::C1, CompressionKind::C2] {
        for format in [MainKvFormat::Fp8Ue8m0, MainKvFormat::Fp4E4m3] {
            let compressor = compressor(&device, kind, format);
            let (state, _, _) = states(&device, kind, format);
            let all = compressor.prepare(&state, 0, &hidden, &cancel).unwrap();
            let expected_main = bytes(all.rows().records());
            let k = indexer
                .forward(all.pre_rope_kv(), all.emitted_positions(), &cancel)
                .unwrap();
            let expected_data = bytes(k.data_bytes());
            let expected_scales = bytes(k.scale_bytes());
            drop(all);
            for mask in 0..8 {
                let (mut state, mut main, mut index) = states(&device, kind, format);
                let mut start = 0;
                for end in 1..=4 {
                    if end < 4 && mask & (1 << (end - 1)) == 0 {
                        continue;
                    }
                    let output = compressor
                        .prepare(
                            &state,
                            start,
                            &hidden.narrow(0, start, end - start).unwrap(),
                            &cancel,
                        )
                        .unwrap();
                    commit(
                        &device,
                        output,
                        &indexer,
                        &mut state,
                        &mut main,
                        &mut index,
                        start,
                        end - start,
                        end - start,
                        false,
                    )
                    .unwrap();
                    assert_eq!(state.position(), end);
                    assert_eq!(main.position(), end);
                    assert_eq!(index.position(), end);
                    start = end;
                }
                assert_eq!(
                    bytes(main.attention_rows(&device).unwrap().records()),
                    expected_main
                );
                let rows = index.index_rows(&device).unwrap();
                assert_eq!(bytes(rows.data_bytes()), expected_data);
                assert_eq!(bytes(rows.scale_bytes()), expected_scales);
                assert_eq!(state.pending_bytes(), 0);
            }
        }
    }
}

#[test]
fn partial_accept_cancel_and_drop_keep_all_three_owners_consistent() {
    let device = device();
    let hidden = hidden();
    let cancel = CancellationToken::new();
    let indexer = index(&device);
    let kind = CompressionKind::C2;
    let format = MainKvFormat::Fp4E4m3;
    let compressor = compressor(&device, kind, format);
    let (mut state, mut main, mut index) = states(&device, kind, format);
    let output = compressor.prepare(&state, 0, &hidden, &cancel).unwrap();
    let expected = bytes(output.rows().records());
    commit(
        &device, output, &indexer, &mut state, &mut main, &mut index, 0, 4, 3, true,
    )
    .unwrap_err();
    assert_eq!(
        (state.position(), main.position(), index.position()),
        (0, 0, 0)
    );
    assert_eq!(
        (state.pending_bytes(), main.row_count(), index.row_count()),
        (0, 0, 0)
    );
    let output = compressor.prepare(&state, 0, &hidden, &cancel).unwrap();
    drop(output);
    let output = compressor.prepare(&state, 0, &hidden, &cancel).unwrap();
    commit(
        &device, output, &indexer, &mut state, &mut main, &mut index, 0, 4, 3, false,
    )
    .unwrap();
    assert_eq!(
        (state.position(), main.position(), index.position()),
        (3, 3, 3)
    );
    assert_eq!(state.pending_bytes(), 512 * 8);
    assert_eq!(main.row_count(), 1);
    assert_eq!(index.row_count(), 1);
    assert_eq!(
        bytes(main.attention_rows(&device).unwrap().records()),
        expected[..format.bytes_per_token()]
    );
    let output = compressor
        .prepare(&state, 3, &hidden.narrow(0, 3, 1).unwrap(), &cancel)
        .unwrap();
    assert_eq!(output.emitted_positions(), [3]);
    commit(
        &device, output, &indexer, &mut state, &mut main, &mut index, 3, 1, 1, false,
    )
    .unwrap();
    assert_eq!(
        bytes(main.attention_rows(&device).unwrap().records()),
        expected
    );
    assert_eq!(
        (state.position(), main.position(), index.position()),
        (4, 4, 4)
    );
    assert_eq!(state.pending_bytes(), 0);
}

#[test]
fn invalid_gate_norm_state_shape_positions_nonfinite_cancel_and_joint_budgets_refuse() {
    let device = device();
    let cancel = CancellationToken::new();
    let kind = CompressionKind::C1;
    let format = MainKvFormat::Fp8Ue8m0;
    assert!(KvCompressor::new(
        device.clone(),
        kind,
        matrix(&device, false),
        Some(matrix(&device, true)),
        norm(&device, 512),
        cache(),
        format
    )
    .is_err());
    assert!(KvCompressor::new(
        device.clone(),
        CompressionKind::C2,
        matrix(&device, false),
        None,
        norm(&device, 512),
        cache(),
        format
    )
    .is_err());
    assert!(KvCompressor::new(
        device.clone(),
        kind,
        matrix(&device, false),
        None,
        norm(&device, 128),
        cache(),
        format
    )
    .is_err());
    let compressor = compressor(&device, kind, format);
    let wrong = CompressionState::new(&device, CompressionKind::C2, 512, 4, 4).unwrap();
    assert!(compressor.prepare(&wrong, 0, &hidden(), &cancel).is_err());
    let (state, _, _) = states(&device, kind, format);
    assert!(compressor.prepare(&state, 1, &hidden(), &cancel).is_err());
    assert!(compressor
        .prepare(&state, 0, &hidden().to_dtype(DType::F32).unwrap(), &cancel)
        .is_err());
    assert!(compressor
        .prepare(&state, 0, &bf16(&[f32::NAN; 16], 4, 4), &cancel)
        .is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        compressor.prepare(&state, 0, &hidden(), &cancelled),
        Err(PowerError::InferenceCancelled)
    ));
    assert_eq!(state.position(), 0);
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 6_000,
        ..InferenceLimits::default()
    });
    assert!(KvCompressor::new(
        low.clone(),
        kind,
        matrix(&low, false),
        None,
        norm(&low, 512),
        cache(),
        format
    )
    .is_err());
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 500_000,
        ..InferenceLimits::default()
    });
    let compressor = super::tests::compressor(&low, kind, format);
    let state = CompressionState::new(&low, kind, 512, 4, 4).unwrap();
    let error = compressor
        .prepare(&state, 0, &hidden(), &cancel)
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("compressor composite live tensors"),
        "{error}"
    );
}
