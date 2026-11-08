use super::super::{InputParts, OutputParts};
use super::*;
use crate::layers::linear::xq::XqDeviceMatrix;
use aster_power::inference::InferenceLimits;
use candle_core::Device;

pub(crate) fn cache(device: &CudaLayerDevice, context: usize) -> Tensor {
    Tensor::cat(
        &[
            Tensor::ones((context, 32), DType::F32, device.tensor_device()).unwrap(),
            Tensor::zeros((context, 32), DType::F32, device.tensor_device()).unwrap(),
        ],
        1,
    )
    .unwrap()
}
pub(crate) fn q8(
    device: &CudaLayerDevice,
    rows: usize,
    columns: usize,
    seed: usize,
) -> XqDeviceMatrix {
    let mut raw = vec![0_u8; rows * columns / 32 * 34];
    for row in 0..rows {
        let begin = row * columns / 32 * 34;
        for block in raw[begin..begin + columns / 32 * 34].chunks_exact_mut(34) {
            block[..2].copy_from_slice(&0x2800_u16.to_le_bytes());
        }
        let col = (row * 7 + seed * 13) % columns;
        raw[begin + col / 32 * 34 + 2 + col % 32] = if (row + seed) % 2 == 0 {
            3
        } else {
            (-2_i8) as u8
        };
    }
    XqDeviceMatrix::from_bytes_for_tests(device, XqTensorType::Q8_0, rows, columns, &raw).unwrap()
}

pub(crate) fn f16(
    device: &CudaLayerDevice,
    rows: usize,
    columns: usize,
    seed: usize,
) -> XqDeviceMatrix {
    let mut raw = vec![0_u8; rows * columns * 2];
    for row in 0..rows {
        let col = (row * 7 + seed * 13) % columns;
        let bits = if (row + seed) % 2 == 0 {
            0x2400_u16
        } else {
            0xa000_u16
        }; // 1/64, -1/128
        raw[(row * columns + col) * 2..(row * columns + col) * 2 + 2]
            .copy_from_slice(&bits.to_le_bytes());
    }
    XqDeviceMatrix::from_bytes_for_tests(device, XqTensorType::F16, rows, columns, &raw).unwrap()
}
pub(crate) fn norm(device: &CudaLayerDevice, width: usize) -> XqRmsNorm {
    XqRmsNorm::new(
        device.clone(),
        Tensor::ones(width, DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap()
}
pub(crate) fn mhc(device: &CudaLayerDevice, hidden: usize) -> XqMhcBlock {
    XqMhcBlock::new(
        device.clone(),
        XqDeviceMatrix::from_bytes_for_tests(
            device,
            XqTensorType::F16,
            24,
            4 * hidden,
            &vec![0; 24 * 4 * hidden * 2],
        )
        .unwrap(),
        Tensor::ones(3, DType::F32, device.tensor_device()).unwrap(),
        Tensor::from_vec(
            (0..24).map(|i| i as f32 / 31. - 0.3).collect::<Vec<_>>(),
            24,
            device.tensor_device(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn projections(
    device: &CudaLayerDevice,
    hidden: usize,
    context: usize,
) -> (XqAttentionInputProjection, XqAttentionOutputProjection) {
    let cache = cache(device, context);
    let input = XqAttentionInputProjection::from_parts(
        device.clone(),
        InputParts {
            query_a: q8(device, 32, hidden, 1),
            query_b: q8(device, 512, 32, 2),
            window: q8(device, 512, hidden, 3),
            query_norm: norm(device, 32),
            window_norm: norm(device, 512),
            cos_sin: cache.clone(),
        },
    )
    .unwrap();
    let output = XqAttentionOutputProjection::from_parts(
        device.clone(),
        OutputParts {
            group_a: q8(device, 64, 256, 4),
            output_b: q8(device, hidden, 64, 5),
            heads: 1,
            groups: 2,
            rank: 32,
            cos_sin: cache,
        },
    )
    .unwrap();
    (input, output)
}
pub(crate) fn sublayer(
    device: &CudaLayerDevice,
    hidden: usize,
    context: usize,
) -> XqAttentionSublayer {
    let (input, output) = projections(device, hidden, context);
    XqAttentionSublayer::new(
        device.clone(),
        mhc(device, hidden),
        norm(device, hidden),
        input,
        output,
        Tensor::full(-0.25_f32, 1, device.tensor_device()).unwrap(),
    )
    .unwrap()
}

#[test]
fn shifted_attention_half_counts_all_weights_and_both_rotary_snapshots() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (input, output) = projections(&device, 32, 4);
    let mhc = mhc(&device, 32);
    let norm = norm(&device, 32);
    let expected = input.retained_bytes()
        + output.retained_bytes()
        + mhc.retained_bytes()
        + norm.retained_bytes()
        + 4;
    let layer = XqAttentionSublayer::new(
        device.clone(),
        mhc,
        norm,
        input,
        output,
        Tensor::zeros(1, DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    assert_eq!(layer.retained_bytes(), expected);
    assert_eq!(layer.hidden_size(), 32);
    let trunk = XqMhcContinuation::from_embedding(
        &device,
        &Tensor::ones((1, 32), DType::F32, &Device::Cpu).unwrap(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(layer
        .prepare(trunk, &[0], &CancellationToken::new())
        .is_err()); // no CPU fallback
}

#[test]
fn different_rotary_geometry_device_limits_sinks_and_aggregate_budget_fail_closed() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (input, output) = projections(&device, 32, 4);
    assert!(XqAttentionSublayer::new(
        device.clone(),
        mhc(&device, 32),
        norm(&device, 64),
        input,
        output,
        Tensor::zeros(1, DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
    let (input, mut output) = projections(&device, 32, 4);
    output.rope = super::super::main_rope(
        &device,
        &Tensor::zeros((4, 64), DType::F32, &Device::Cpu).unwrap(),
    )
    .unwrap();
    assert!(XqAttentionSublayer::new(
        device.clone(),
        mhc(&device, 32),
        norm(&device, 32),
        input,
        output,
        Tensor::zeros(1, DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
    let (input, output) = projections(&device, 32, 4);
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_context_tokens: 8,
        ..InferenceLimits::default()
    });
    assert!(XqAttentionSublayer::new(
        strict,
        mhc(&device, 32),
        norm(&device, 32),
        input,
        output,
        Tensor::zeros(1, DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
    for sinks in [
        Tensor::zeros(2, DType::F32, &Device::Cpu).unwrap(),
        Tensor::zeros(1, DType::BF16, &Device::Cpu).unwrap(),
        Tensor::full(f32::NAN, 1, &Device::Cpu).unwrap(),
    ] {
        let (input, output) = projections(&device, 32, 4);
        assert!(XqAttentionSublayer::new(
            device.clone(),
            mhc(&device, 32),
            norm(&device, 32),
            input,
            output,
            sinks
        )
        .is_err());
    }
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 40000,
        ..InferenceLimits::default()
    });
    let (input, output) = projections(&strict, 32, 4);
    assert!(XqAttentionSublayer::new(
        strict.clone(),
        mhc(&strict, 32),
        norm(&strict, 32),
        input,
        output,
        Tensor::zeros(1, DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
}

#[test]
fn attention_prepare_refuses_empty_out_of_cache_positions_and_cancel_before_compute() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let layer = sublayer(&device, 32, 4);
    let trunk = || {
        XqMhcContinuation::from_embedding(
            &device,
            &Tensor::ones((1, 32), DType::F32, &Device::Cpu).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap()
    };
    for positions in [vec![], vec![4], vec![0, 1]] {
        assert!(layer
            .prepare(trunk(), &positions, &CancellationToken::new())
            .is_err());
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(layer.prepare(trunk(), &[0], &cancel).is_err());
}
