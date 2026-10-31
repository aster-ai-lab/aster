//! Stage-local owned-H200 oracle; no whole-model or speed claim.
use super::*;
use crate::layers::{
    engram::lookup::tests::{checkpoint, table},
    xq::mhc::XqMhcBlock,
};
use aster_power::inference::{DevicePreference, RuntimeDevice};

fn fp8(byte: u8) -> f32 {
    let exponent = (byte >> 3) & 15;
    let mantissa = byte & 7;
    let magnitude = if exponent == 0 {
        f32::from(mantissa) * 2_f32.powi(-9)
    } else {
        (1. + f32::from(mantissa) / 8.) * 2_f32.powi(i32::from(exponent) - 7)
    };
    if byte & 128 != 0 {
        -magnitude
    } else {
        magnitude
    }
}
fn decode(data: &[u8], scales: &[u8], row: usize, col: usize) -> f32 {
    let code = scales[row * 8 + col / 32];
    let scale = if code == 0 {
        f32::from_bits(1 << 22)
    } else {
        f32::from_bits(u32::from(code) << 23)
    };
    fp8(data[row * 256 + col]) * scale
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_engram_packed_lookup_f16_projection_f32_gate_and_carried_mix_match_oracles() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let hidden = 32;
    let data: Vec<_> = (0..4 * 256)
        .map(|i| [56_u8, 184, 1, 0, 128, 57][i % 6])
        .collect();
    let scales: Vec<_> = (0..4 * 8).map(|i| [127_u8, 128, 126, 0][i % 4]).collect();
    let (_dir, checkpoint) = checkpoint(data.clone(), scales.clone(), 4, 264);
    let hashes = [
        std::array::from_fn(|head| [3, 0, 3, 2, 1][head % 5]),
        [-1; 24],
        std::array::from_fn(|head| [1, 2, 0, 1][head % 4]),
    ];
    let embeddings = table(&device, &checkpoint, 4)
        .fetch(&hashes, &cancel)
        .unwrap();
    let decoded = values(&embeddings.activations().decode().unwrap());
    let mut expected_input = Vec::new();
    for row in &hashes {
        for &id in row {
            for col in 0..256 {
                expected_input.push(if id < 0 {
                    0.
                } else {
                    decode(&data, &scales, id as usize, col)
                });
            }
        }
    }
    assert_eq!(
        decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        expected_input
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
    );
    let mut raw = Vec::new();
    let mut weights = Vec::new();
    for row in 0..5 * hidden {
        for col in 0..PROJECTED_INPUT_WIDTH {
            let (weight, bits) = [
                (0.000244140625_f32, 0x0c00_u16),
                (-0.0001220703125, 0x8800),
                (0., 0),
            ][(row * 3 + col + row / hidden) % 3];
            raw.extend(bits.to_le_bytes());
            weights.push(weight);
        }
    }
    let matrix = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F16,
        5 * hidden,
        PROJECTED_INPUT_WIDTH,
        &raw,
    )
    .unwrap();
    let query: Vec<_> = (0..4 * hidden).map(|i| 0.51 + i as f32 / 117.).collect();
    let key: Vec<_> = (0..4 * hidden)
        .map(|i| {
            if i / hidden % 2 == 0 {
                -0.87
            } else {
                1.0003 + i as f32 / 233.
            }
        })
        .collect();
    let gate = XqEngramGate::new(
        device.clone(),
        Tensor::from_slice(&query, (4, hidden), device.tensor_device()).unwrap(),
        Tensor::from_slice(&key, (4, hidden), device.tensor_device()).unwrap(),
    )
    .unwrap();
    let projection =
        XqEngramProjection::new(device.clone(), EngramLayer::Layer14, matrix, gate).unwrap();
    let actual_projected = projection
        .projection
        .forward_f32_control(&embeddings.activations().decode().unwrap(), &cancel)
        .unwrap()
        .0;
    let mut projected = Vec::new();
    for token in 0..hashes.len() {
        for row in 0..5 * hidden {
            projected.push(
                (0..PROJECTED_INPUT_WIDTH)
                    .map(|col| {
                        f64::from(expected_input[token * PROJECTED_INPUT_WIDTH + col])
                            * f64::from(weights[row * PROJECTED_INPUT_WIDTH + col])
                    })
                    .sum::<f64>(),
            );
        }
    }
    close(&values(&actual_projected), &projected);
    let embedding: Vec<_> = (0..3 * hidden)
        .map(|i| ((i * 7 % 41) as f32 - 20.) / 17.)
        .collect();
    let initial = XqMhcContinuation::from_embedding(
        &device,
        &Tensor::from_slice(&embedding, (3, hidden), device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    // Prepare a non-initial prior pre-mix, then replace only its trunk.
    let mix = XqMhcBlock::new(
        device.clone(),
        XqDeviceMatrix::from_bytes_for_tests(
            &device,
            XqTensorType::F16,
            24,
            4 * hidden,
            &vec![0; 24 * 4 * hidden * 2],
        )
        .unwrap(),
        Tensor::ones(3, DType::F32, device.tensor_device()).unwrap(),
        Tensor::zeros(24, DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap();
    let continuation = mix
        .prepare(initial, &cancel)
        .unwrap()
        .finish(
            &Tensor::zeros((3, hidden), DType::F32, device.tensor_device()).unwrap(),
            &cancel,
        )
        .unwrap();
    let trunk = values(continuation.residual());
    let carried_mix = values(continuation.next_pre_mix());
    assert!(carried_mix.iter().all(|v| *v > 0.5 && *v < 0.501));
    // Compare the gate using independent F64 projection outputs rounded once
    // at its declared F32 boundary, plus a separate projection-stage check.
    let projected_f32: Vec<_> = projected.iter().map(|v| *v as f32).collect();
    let expected = oracle(&trunk, &projected_f32, &query, &key, &[0, 1, 0], hidden);
    let output = projection
        .forward(continuation, &embeddings, &cancel)
        .unwrap();
    close(&values(output.residual()), &expected);
    assert_eq!(values(output.next_pre_mix()), carried_mix);
    let actual = values(output.residual());
    for index in 4 * hidden..8 * hidden {
        assert_eq!(actual[index].to_bits(), trunk[index].to_bits());
    }
    let gate = XqEngramGate::new(
        device.clone(),
        Tensor::zeros((4, hidden), DType::F32, device.tensor_device()).unwrap(),
        Tensor::ones((4, hidden), DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap();
    let zeros = Tensor::full(-0_f32, (2, 4, hidden), device.tensor_device()).unwrap();
    let result = gate
        .forward(
            &zeros,
            &Tensor::full(2_f32, (2, 5, hidden), device.tensor_device()).unwrap(),
            &Tensor::new(&[0_u8, 1], device.tensor_device()).unwrap(),
            &cancel,
        )
        .unwrap();
    let result = values(&result);
    assert!(result[..4 * hidden].iter().all(|v| *v == 1.));
    assert!(result[4 * hidden..]
        .iter()
        .all(|v| v.to_bits() == (-0_f32).to_bits()));
}
