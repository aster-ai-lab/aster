//! Owned-H200 endpoint/mHC component qualification, not complete generation.
use super::*;
use crate::layers::xq::mhc::XqMhcBlock;
use aster_power::inference::{DevicePreference, RuntimeDevice};

fn reference_step(
    residual: &[Vec<f64>],
    prior: &[f64],
    base: &[f32],
    branch: &[f32],
) -> (Vec<Vec<f64>>, Vec<f64>) {
    let sigmoid = |v: f64| 1. / (1. + (-v).exp());
    let flat: Vec<f64> = residual.iter().flatten().copied().collect();
    let rms = (flat.iter().map(|x| x * x).sum::<f64>() / flat.len() as f64 + 1e-20).sqrt();
    let projected: Vec<f64> = (0..24)
        .map(|row| {
            flat.iter()
                .enumerate()
                .map(|(col, x)| {
                    x / rms
                        * [0.0009765625_f64, -0.00048828125, 0.][(row * flat.len() + col + row) % 3]
                })
                .sum()
        })
        .collect();
    let pre: Vec<_> = (0..4)
        .map(|i| sigmoid(projected[i] + f64::from(base[i])) + 1e-6)
        .collect();
    let post: Vec<_> = (4..8)
        .map(|i| 2. * sigmoid(projected[i] + f64::from(base[i])))
        .collect();
    let mut matrix = [[0_f64; 4]; 4];
    for (src, row) in matrix.iter_mut().enumerate() {
        let values: Vec<_> = (0..4)
            .map(|dst| {
                let index = 8 + src * 4 + dst;
                (projected[index] + f64::from(base[index])).exp()
            })
            .collect();
        let sum: f64 = values.iter().sum();
        for dst in 0..4 {
            row[dst] = values[dst] / sum + 1e-6;
        }
    }
    for iteration in 0..20 {
        if iteration > 0 {
            for row in &mut matrix {
                let sum = row.iter().sum::<f64>() + 1e-6;
                for v in row {
                    *v /= sum;
                }
            }
        }
        for dst in 0..4 {
            let sum = (0..4).map(|src| matrix[src][dst]).sum::<f64>() + 1e-6;
            for row in &mut matrix {
                row[dst] /= sum;
            }
        }
    }
    // Retain the independent prior-input formula in the component oracle.
    let mixed_input: Vec<f64> = (0..branch.len())
        .map(|col| (0..4).map(|src| residual[src][col] * prior[src]).sum())
        .collect();
    assert!(mixed_input.iter().all(|x| x.is_finite()));
    let next = (0..4)
        .map(|dst| {
            (0..branch.len())
                .map(|col| {
                    (0..4)
                        .map(|src| matrix[src][dst] * residual[src][col])
                        .sum::<f64>()
                        + post[dst] * f64::from(branch[col])
                })
                .collect()
        })
        .collect();
    (next, pre)
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_embedding_shifted_f32_mhc_and_output_head_match_independent_math() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let hidden = 32;
    let vocabulary = 3;
    let codes = [0x3c00_u16, 0xc000, 0x3800, 0xb400, 0x3400];
    let raw: Vec<u8> = (0..vocabulary * hidden)
        .flat_map(|i| codes[(i + i / hidden) % codes.len()].to_le_bytes())
        .collect();
    let embedding = XqTokenEmbedding::new(
        device.clone(),
        XqDeviceMatrix::from_bytes_for_tests(&device, XqTensorType::F16, vocabulary, hidden, &raw)
            .unwrap(),
    )
    .unwrap();
    let tokens = [2_u32, 0, 2];
    let (embedded, gather_receipt) = embedding.forward(&tokens, &cancel).unwrap();
    let mut continuation = embedding.initial_continuation(&tokens, &cancel).unwrap();
    let projection_bytes: Vec<u8> = (0..24 * hidden * 4)
        .flat_map(|i| [0x1400_u16, 0x9000, 0][(i + i / (hidden * 4)) % 3].to_le_bytes())
        .collect();
    let projection = XqDeviceMatrix::from_bytes_for_tests(
        &device,
        XqTensorType::F16,
        24,
        hidden * 4,
        &projection_bytes,
    )
    .unwrap();
    let base: Vec<f32> = (0..24).map(|i| ((i * 7 % 19) as f32 - 9.) * 0.07).collect();
    let block = XqMhcBlock::new(
        device.clone(),
        projection,
        Tensor::ones(3, DType::F32, device.tensor_device()).unwrap(),
        Tensor::new(base.as_slice(), device.tensor_device()).unwrap(),
    )
    .unwrap();
    let mut reference: Vec<Vec<Vec<f64>>> = tokens
        .iter()
        .map(|&id| {
            let row: Vec<f64> = (0..hidden)
                .map(|col| {
                    [1_f64, -2., 0.5, -0.25, 0.25][((id as usize * hidden + col) + id as usize) % 5]
                })
                .collect();
            vec![row; 4]
        })
        .collect();
    let mut mixes = vec![vec![1_f64, 0., 0., 0.]; tokens.len()];
    let actual_embedding = embedded.to_vec2::<f32>().unwrap();
    for token in 0..tokens.len() {
        for col in 0..hidden {
            assert_eq!(
                f64::from(actual_embedding[token][col]),
                reference[token][0][col]
            );
        }
    }
    for iteration in 0..2 {
        let step = block.prepare(continuation, &cancel).unwrap();
        let input = step.input().to_vec2::<f32>().unwrap();
        for token in 0..tokens.len() {
            for col in 0..hidden {
                let expected: f64 = (0..4)
                    .map(|src| reference[token][src][col] * mixes[token][src])
                    .sum();
                assert!((f64::from(input[token][col]) - expected).abs() < 8e-6);
            }
        }
        let branch: Vec<f32> = (0..tokens.len() * hidden)
            .map(|i| ((i * 3 % 17) as f32 - 8.) * 0.03125 + iteration as f32 * 0.002)
            .collect();
        continuation = step
            .finish(
                &Tensor::from_slice(&branch, (tokens.len(), hidden), device.tensor_device())
                    .unwrap(),
                &cancel,
            )
            .unwrap();
        for token in 0..tokens.len() {
            let (next, pre) = reference_step(
                &reference[token],
                &mixes[token],
                &base,
                &branch[token * hidden..(token + 1) * hidden],
            );
            reference[token] = next;
            mixes[token] = pre;
        }
    }
    let head_weights: Vec<u8> = (0..vocabulary)
        .flat_map(|row| {
            let mut data = 0x3000_u16.to_le_bytes().to_vec(); // exact scale 0.125
            data.extend((0..hidden).map(|col| (((row * 7 + col * 3) % 13) as i8 - 6) as u8));
            data
        })
        .collect();
    let norm_weight: Vec<f32> = (0..hidden).map(|col| 1. + col as f32 * 0.0001).collect();
    let head = XqTextHead::new(
        device.clone(),
        XqRmsNorm::new(
            device.clone(),
            Tensor::new(norm_weight.as_slice(), device.tensor_device()).unwrap(),
        )
        .unwrap(),
        XqDeviceMatrix::from_bytes_for_tests(
            &device,
            XqTensorType::Q8_0,
            vocabulary,
            hidden,
            &head_weights,
        )
        .unwrap(),
    )
    .unwrap();
    let (logits, receipt) = head.forward(&continuation, &cancel).unwrap();
    let logits = logits.to_vec2::<f32>().unwrap();
    for token in 0..tokens.len() {
        let reduced: Vec<f64> = (0..hidden)
            .map(|col| {
                (0..4)
                    .map(|s| reference[token][s][col] * mixes[token][s])
                    .sum()
            })
            .collect();
        let denominator =
            (reduced.iter().map(|x| x * x).sum::<f64>() / hidden as f64 + 1e-20).sqrt();
        for output in 0..vocabulary {
            let expected: f64 = (0..hidden)
                .map(|col| {
                    reduced[col] / denominator
                        * f64::from(norm_weight[col])
                        * (((output * 7 + col * 3) % 13) as i32 - 6) as f64
                        * 0.125
                })
                .sum();
            assert!(
                (f64::from(logits[token][output]) - expected).abs() < 3e-5,
                "token={token} output={output}"
            );
        }
    }
    println!(
        "qualified XQ component endpoints source={} ptx={} gather={}",
        receipt.source_sha256, receipt.ptx_sha256, gather_receipt.abi
    );
}
