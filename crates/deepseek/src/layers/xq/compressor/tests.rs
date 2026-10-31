use super::*;
use crate::layers::xq::indexer::XqIndexKeyProjection;
use crate::layers::xq::quant::tests::scalar;
use aster_power::inference::InferenceLimits;
use candle_core::Device;

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}

pub(crate) fn for_pipeline(device: &CudaLayerDevice, layer: usize) -> XqKvCompressor {
    use crate::layers::xq::attention::sublayer::tests::{cache, f16, norm};
    XqKvCompressor::from_parts(
        device.clone(),
        layer,
        f16(device, 512, 256, 9),
        (source_kind(layer).unwrap() == CompressionKind::C2).then(|| f16(device, 512, 256, 10)),
        norm(device, 512),
        cache(device, 8),
    )
    .unwrap()
}
fn matrix(device: &CudaLayerDevice, rows: usize, columns: usize) -> XqDeviceMatrix {
    XqDeviceMatrix::from_bytes_for_tests(
        device,
        XqTensorType::F16,
        rows,
        columns,
        &vec![0; rows * columns * 2],
    )
    .unwrap()
}
fn norm(device: &CudaLayerDevice, width: usize) -> XqRmsNorm {
    XqRmsNorm::new(
        device.clone(),
        Tensor::ones(width, DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap()
}
fn cache(device: &CudaLayerDevice) -> Tensor {
    let mut values = Vec::new();
    for p in 0..8 {
        let (c, s) = [(1_f32, 0_f32), (0., 1.), (-1., 0.), (0., -1.)][p % 4];
        values.extend([c; 32]);
        values.extend([s; 32]);
    }
    Tensor::from_vec(values, (8, 64), device.tensor_device()).unwrap()
}
fn compressor(device: &CudaLayerDevice, layer: usize) -> XqKvCompressor {
    XqKvCompressor::from_parts(
        device.clone(),
        layer,
        matrix(device, 512, 4),
        (source_kind(layer).unwrap() == CompressionKind::C2).then(|| matrix(device, 512, 4)),
        norm(device, 512),
        cache(device),
    )
    .unwrap()
}
fn bytes(tensor: &Tensor) -> Vec<u8> {
    tensor.flatten_all().unwrap().to_vec1::<u8>().unwrap()
}
fn floats(tensor: &Tensor) -> Vec<f32> {
    tensor.flatten_all().unwrap().to_vec1::<f32>().unwrap()
}

#[test]
fn c2_rotary_uses_group_start_while_cache_visibility_uses_pair_end() {
    let device = device();
    let compressor = compressor(&device, 2);
    let state = compressor.new_state(8, 8).unwrap();
    let cancel = CancellationToken::new();
    let latent = Tensor::ones((2, 512), DType::F32, &Device::Cpu).unwrap();
    let scores = Tensor::zeros((2, 512), DType::F32, &Device::Cpu).unwrap();
    let prepared = compressor
        .prepare_projected(&state, 0, Some(&scores), &latent, &cancel)
        .unwrap();
    assert_eq!(prepared.layer(), 2);
    assert_eq!(prepared.emitted_positions(), [1]);
    assert_eq!(prepared.rotary_positions(), [0]);
    assert_eq!(prepared.pre_rope_kv().dtype(), DType::F32);
    let (data, scales, _) = scalar(XqActivationFormat::CompressedFp4, &vec![1_f32; 512]);
    assert_eq!(bytes(prepared.rows().records()), [data, scales].concat());
    assert_eq!(state.position(), 0);
    drop(prepared);
    for bad in [vec![0], vec![1, 1], vec![3, 1], vec![1, 5]] {
        assert!(compressed_rotary_positions(CompressionKind::C2, &bad).is_err());
    }
}

#[test]
fn all_c1_c2_chunk_partitions_preserve_pre_rope_f32_and_main_quantized_bytes() {
    let device = device();
    let cancel = CancellationToken::new();
    let latent = Tensor::from_vec(
        (0..6 * 512)
            .map(|i| ((i * 7 + i / 512 * 3) % 23) as f32 * 0.137 - 1.5 + 0.0001)
            .collect::<Vec<_>>(),
        (6, 512),
        &Device::Cpu,
    )
    .unwrap();
    let scores = Tensor::from_vec(
        (0..6 * 512)
            .map(|i| ((i + i / 512 * 11) % 13) as f32 * 0.11 - 0.5)
            .collect::<Vec<_>>(),
        (6, 512),
        &Device::Cpu,
    )
    .unwrap();
    for layer in [2, 20] {
        let compressor = compressor(&device, layer);
        let mut control = None;
        for partition in 0..32 {
            let mut state = compressor.new_state(8, 8).unwrap();
            let mut start = 0;
            let mut all_bytes = Vec::new();
            let mut all_pre = Vec::new();
            let mut all_ready = Vec::new();
            let mut all_rotary = Vec::new();
            for end in 1..=6 {
                if end != 6 && partition & (1 << (end - 1)) == 0 {
                    continue;
                }
                let n = end - start;
                let gate = (compressor.kind() == CompressionKind::C2)
                    .then(|| scores.narrow(0, start, n).unwrap());
                let prepared = compressor
                    .prepare_projected(
                        &state,
                        start,
                        gate.as_ref(),
                        &latent.narrow(0, start, n).unwrap(),
                        &cancel,
                    )
                    .unwrap();
                assert_eq!(state.position(), start);
                all_ready.extend_from_slice(prepared.emitted_positions());
                all_rotary.extend_from_slice(prepared.rotary_positions());
                all_bytes.extend(bytes(prepared.rows().records()));
                all_pre.extend(floats(prepared.pre_rope_kv()).iter().map(|x| x.to_bits()));
                let (proposal, _, _, _) = prepared.into_parts();
                state.commit_prefix(proposal, n, &cancel).unwrap();
                start = end;
            }
            let actual = (all_bytes, all_pre, all_ready, all_rotary);
            if let Some(expected) = &control {
                assert_eq!(&actual, expected);
            } else {
                control = Some(actual);
            }
            assert_eq!(state.position(), 6);
            assert_eq!(state.pending_bytes(), 0);
        }
    }
}

#[test]
fn source_profile_empty_pair_and_late_cancel_fail_without_committed_changes() {
    let device = device();
    let compressor = compressor(&device, 2);
    let mut state = compressor.new_state(8, 8).unwrap();
    let cancel = CancellationToken::new();
    let latent = Tensor::ones((1, 512), DType::F32, &Device::Cpu).unwrap();
    let scores = Tensor::zeros((1, 512), DType::F32, &Device::Cpu).unwrap();
    let prepared = compressor
        .prepare_projected(&state, 0, Some(&scores), &latent, &cancel)
        .unwrap();
    assert_eq!(prepared.pre_rope_kv().dims(), [0, 512]);
    assert_eq!(prepared.rows().row_count(), 0);
    assert!(prepared.rotary_positions().is_empty());
    let same = XqIndexKeyProjection::from_parts(
        device.clone(),
        2,
        matrix(&device, 128, 512),
        norm(&device, 128),
        cache(&device),
    )
    .unwrap();
    assert_eq!(
        same.forward_compressed(&prepared, &cancel)
            .unwrap()
            .row_count(),
        0
    );
    let foreign = XqIndexKeyProjection::from_parts(
        device.clone(),
        8,
        matrix(&device, 128, 512),
        norm(&device, 128),
        cache(&device),
    )
    .unwrap();
    assert!(foreign.forward_compressed(&prepared, &cancel).is_err());
    let (proposal, _, _, _) = prepared.into_parts();
    let late = CancellationToken::new();
    late.cancel();
    assert!(state.commit_prefix(proposal, 1, &late).is_err());
    assert_eq!(state.position(), 0);
    assert_eq!(state.pending_bytes(), 0);
    let retry = compressor
        .prepare_projected(&state, 0, Some(&scores), &latent, &cancel)
        .unwrap();
    drop(retry);
    let official = CompressionState::new(&device, CompressionKind::C2, 512, 8, 8).unwrap();
    assert!(matches!(
        compressor.prepare(
            &official,
            0,
            &Tensor::ones((1, 4), DType::F32, &Device::Cpu).unwrap(),
            &cancel
        ),
        Err(aster_power::error::PowerError::InvalidRequest(_))
    ));
}

#[test]
fn constructor_refuses_wrong_sources_gate_presence_weights_norms_limits_and_residency() {
    let device = device();
    for layer in [0, 1, 3, 21, 40, usize::MAX] {
        assert!(source_kind(layer).is_err());
    }
    assert!(XqKvCompressor::from_parts(
        device.clone(),
        2,
        matrix(&device, 512, 4),
        None,
        norm(&device, 512),
        cache(&device)
    )
    .is_err());
    assert!(XqKvCompressor::from_parts(
        device.clone(),
        20,
        matrix(&device, 512, 4),
        Some(matrix(&device, 512, 4)),
        norm(&device, 512),
        cache(&device)
    )
    .is_err());
    let bad = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F32,
        512,
        4,
        &vec![0; 512 * 4 * 4],
    )
    .unwrap();
    assert!(XqKvCompressor::from_parts(
        device.clone(),
        20,
        bad,
        None,
        norm(&device, 512),
        cache(&device)
    )
    .is_err());
    let other = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 1 << 30,
        ..InferenceLimits::default()
    });
    assert!(XqKvCompressor::from_parts(
        device.clone(),
        20,
        matrix(&device, 512, 4),
        None,
        norm(&other, 512),
        cache(&device)
    )
    .is_err());
    let good = compressor(&device, 2);
    let small = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: good.retained_bytes() - 1,
        ..InferenceLimits::default()
    });
    assert!(XqKvCompressor::from_parts(
        small.clone(),
        2,
        matrix(&small, 512, 4),
        Some(matrix(&small, 512, 4)),
        norm(&small, 512),
        cache(&small)
    )
    .is_err());
    assert!(good.new_state(9, 1).is_err());
}
