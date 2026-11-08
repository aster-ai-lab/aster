//! Full small MoE/shifted-FFN composition on an owned H200. The codecs have
//! separate independent golden-bit tests; this oracle uses their decoded
//! values and independent F64 routing, dots, activation and mHC equations.
use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};

struct HostMatrix {
    raw: Vec<u8>,
    values: Vec<Vec<f64>>,
}

impl HostMatrix {
    fn new(kind: XqTensorType, rows: usize, columns: usize, seed: usize) -> Self {
        let (block_columns, block_bytes) = kind.block_layout();
        let mut raw = Vec::new();
        let mut values = Vec::new();
        for row in 0..rows {
            let mut bytes = Vec::new();
            for block in 0..columns / block_columns as usize {
                let key = seed * 19 + row * 13 + block * 7;
                let mut encoded: Vec<u8> = (0..block_bytes as usize)
                    .map(|i| ((key + i * 17 + i * key) % 243) as u8)
                    .collect();
                match kind {
                    XqTensorType::F32 => {
                        let x = (((key % 23) as f32 - 11.) * 0.001).to_le_bytes();
                        encoded.copy_from_slice(&x);
                    }
                    XqTensorType::Q8_0 => {
                        encoded[..2].copy_from_slice(&0x1400_u16.to_le_bytes()); // 2^-10
                        for (i, code) in encoded[2..].iter_mut().enumerate() {
                            *code = (((key + i * 3) % 15) as i8 - 7) as u8;
                        }
                    }
                    XqTensorType::Q2_0 => encoded[..2].copy_from_slice(&0x2400_u16.to_le_bytes()), // 2^-6
                    XqTensorType::Ptq1_0 => {
                        encoded[26..].copy_from_slice(&0x2800_u16.to_le_bytes())
                    } // 2^-5
                    XqTensorType::Q3K => encoded[108..].copy_from_slice(&0x0c00_u16.to_le_bytes()), // 2^-12
                    _ => panic!("fixture only contains router, shared and routed formats"),
                }
                bytes.extend(encoded);
            }
            values.push(
                kind.decode_f32_row(&bytes, columns, columns as u64 * 4)
                    .unwrap()
                    .into_iter()
                    .map(f64::from)
                    .collect(),
            );
            raw.extend(bytes);
        }
        Self { raw, values }
    }

    fn upload(&self, device: &CudaLayerDevice, kind: XqTensorType) -> XqDeviceMatrix {
        XqDeviceMatrix::from_bytes_for_tests(
            device,
            kind,
            self.values.len(),
            self.values[0].len(),
            &self.raw,
        )
        .unwrap()
    }

    fn dot(&self, first: usize, rows: usize, input: &[f64]) -> Vec<f64> {
        self.values[first..first + rows]
            .iter()
            .map(|row| row.iter().zip(input).map(|(w, x)| w * x).sum())
            .collect()
    }
}

struct Oracle {
    router: HostMatrix,
    shared: [HostMatrix; 3],
    stacks: BTreeMap<String, HostMatrix>,
    experts: Vec<[MatrixRows; 3]>,
    text_bias: [f32; 4],
    image_bias: [f32; 4],
}

impl Oracle {
    fn build(device: &CudaLayerDevice) -> (Self, XqMoeBlock) {
        let (_, _, stack_shapes, experts) = parts(device);
        let router = HostMatrix::new(XqTensorType::F32, 4, TEST_WIDTH, 1);
        let shared = std::array::from_fn(|role| {
            HostMatrix::new(XqTensorType::Q8_0, TEST_WIDTH, TEST_WIDTH, 11 + role)
        });
        let stacks: BTreeMap<_, _> = stack_shapes
            .iter()
            .enumerate()
            .map(|(i, (name, matrix))| {
                let [rows, columns] = matrix.logical_shape();
                (
                    name.clone(),
                    HostMatrix::new(matrix.kind(), rows, columns, 31 + i),
                )
            })
            .collect();
        let uploaded = stack_shapes
            .iter()
            .map(|(name, matrix)| (name.clone(), stacks[name].upload(device, matrix.kind())))
            .collect();
        let text_bias = [0.3, 1.3, -1.7, -0.7];
        let image_bias = [-1.8, -0.8, 1.2, 0.2];
        let block = XqMoeBlock::from_parts(
            device.clone(),
            router.upload(device, XqTensorType::F32),
            Tensor::new(&text_bias, device.tensor_device()).unwrap(),
            Tensor::new(&image_bias, device.tensor_device()).unwrap(),
            std::array::from_fn(|role| shared[role].upload(device, XqTensorType::Q8_0)),
            uploaded,
            experts.clone(),
            3,
        )
        .unwrap();
        (
            Self {
                router,
                shared,
                stacks,
                experts,
                text_bias,
                image_bias,
            },
            block,
        )
    }

    fn activation(gate: Vec<f64>, up: Vec<f64>) -> Vec<f64> {
        gate.into_iter()
            .zip(up)
            .map(|(g, u)| {
                let g = g.min(10.);
                g / (1. + (-g).exp()) * u.clamp(-10., 10.)
            })
            .collect()
    }

    fn routes(&self, input: &[f64], image: bool) -> (Vec<usize>, Vec<f64>) {
        let scores: Vec<f64> = self
            .router
            .dot(0, 4, input)
            .into_iter()
            .map(|logit| (logit.max(0.) + (-logit.abs()).exp().ln_1p()).sqrt())
            .collect();
        let bias = if image {
            self.image_bias
        } else {
            self.text_bias
        };
        let mut ids: Vec<usize> = (0..4).collect();
        ids.sort_by(|&a, &b| {
            (scores[b] + f64::from(bias[b]))
                .total_cmp(&(scores[a] + f64::from(bias[a])))
                .then_with(|| a.cmp(&b))
        });
        ids.truncate(3);
        let denominator: f64 = ids.iter().map(|&i| scores[i]).sum::<f64>() + 1e-20;
        let weights = ids.iter().map(|&i| scores[i] / denominator * 1.5).collect();
        (ids, weights)
    }

    fn forward(&self, input: &[f64], image: bool) -> (Vec<f64>, Vec<f64>) {
        let (ids, weights) = self.routes(input, image);
        let mut output = vec![0_f64; TEST_WIDTH];
        let mut absolute = output.clone();
        for (id, weight) in ids.into_iter().zip(weights) {
            let ranges = &self.experts[id];
            let project = |role: usize, value: &[f64]| {
                let p = &ranges[role];
                self.stacks[&p.stack].dot(p.first, p.rows, value)
            };
            let activated = Self::activation(project(0, input), project(1, input));
            let down = project(2, &activated);
            for col in 0..TEST_WIDTH {
                output[col] += down[col] * weight;
                absolute[col] += (down[col] * weight).abs();
            }
        }
        let activated = Self::activation(
            self.shared[0].dot(0, TEST_WIDTH, input),
            self.shared[1].dot(0, TEST_WIDTH, input),
        );
        let shared = self.shared[2].dot(0, TEST_WIDTH, &activated);
        for col in 0..TEST_WIDTH {
            output[col] += shared[col];
            absolute[col] += shared[col].abs();
        }
        (output, absolute)
    }
}

fn input_rows(tokens: usize) -> Vec<Vec<f32>> {
    (0..tokens)
        .map(|token| {
            (0..TEST_WIDTH)
                .map(|col| {
                    ((token * 13 + col * 7) % 29) as f32 * 0.03125 - 0.4375 + token as f32 * 0.0001
                })
                .collect()
        })
        .collect()
}

fn assert_reference(actual: &[f32], expected: &[f64], absolute: &[f64]) {
    // Frozen before hardware execution: includes F32 reductions, exponential
    // rounding and their propagation through three 256-column projections.
    for col in 0..TEST_WIDTH {
        let budget = 2e-4 + absolute[col] * 5e-5;
        assert!(
            (f64::from(actual[col]) - expected[col]).abs() <= budget,
            "column {col}: actual={} expected={} budget={budget}",
            actual[col],
            expected[col]
        );
    }
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_xq_mixed_experts_and_shifted_ffn_match_independent_f64_math() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let (oracle, moe) = Oracle::build(&device);
    let rows = input_rows(3);
    let input = Tensor::new(rows.clone(), device.tensor_device()).unwrap();
    let mask = Tensor::new(&[0_u8, 1, 0], device.tensor_device()).unwrap();
    let output = moe.forward(&input, Some(&mask), &cancel).unwrap();
    assert_eq!(output.dtype(), DType::F32);
    let actual = output.to_vec2::<f32>().unwrap();
    let (logits, _) = moe.router.forward_f32_control(&input, &cancel).unwrap();
    let selected = moe
        .selection
        .select_scores(&score_math(&logits).unwrap(), Some(&mask))
        .unwrap();
    let ids = selected.logical_experts().to_vec2::<u32>().unwrap();
    let weights = selected.weights().to_vec2::<f32>().unwrap();
    for token in 0..3 {
        let source: Vec<_> = rows[token].iter().copied().map(f64::from).collect();
        let (expected, absolute) = oracle.forward(&source, token == 1);
        assert!(expected.iter().any(|x| x.abs() > 1e-4));
        assert_reference(&actual[token], &expected, &absolute);
        let (logical, routes) = oracle.routes(&source, token == 1);
        assert_eq!(
            ids[token],
            logical.iter().map(|&x| x as u32).collect::<Vec<_>>()
        );
        for lane in 0..3 {
            assert!((f64::from(weights[token][lane]) - routes[lane]).abs() < 2e-6);
        }
        let single = input.narrow(0, token, 1).unwrap();
        let single_mask = mask.narrow(0, token, 1).unwrap();
        let control = moe
            .forward(&single, Some(&single_mask), &cancel)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        // Batching must not change row semantics; no cross-token reduction.
        assert_reference(
            &actual[token],
            &control[0]
                .iter()
                .copied()
                .map(f64::from)
                .collect::<Vec<_>>(),
            &absolute,
        );
    }

    let norm_weight: Vec<f32> = (0..TEST_WIDTH)
        .map(|i| 0.9 + (i % 7) as f32 * 0.03)
        .collect();
    let base: Vec<f32> = (0..24).map(|i| (i % 9) as f32 * 0.03 - 0.12).collect();
    let mhc = XqMhcBlock::new(
        device.clone(),
        zeros_matrix(&device, XqTensorType::F16, 24, TEST_WIDTH * 4),
        Tensor::ones(3, DType::F32, device.tensor_device()).unwrap(),
        Tensor::new(base.as_slice(), device.tensor_device()).unwrap(),
    )
    .unwrap();
    let norm = XqRmsNorm::new(
        device.clone(),
        Tensor::new(norm_weight.as_slice(), device.tensor_device()).unwrap(),
    )
    .unwrap();
    let ffn = XqFfnSublayer::from_parts(device.clone(), 5, mhc, norm, moe).unwrap();
    let residual: Vec<Vec<Vec<f32>>> = rows
        .iter()
        .map(|row| {
            (0..4)
                .map(|stream| {
                    row.iter()
                        .enumerate()
                        .map(|(col, x)| {
                            *x * (1. + stream as f32 * 0.1)
                                + (col % 5) as f32 * 0.002 * stream as f32
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
    let continuation = XqMhcContinuation::from_embedding(&device, &input, &cancel)
        .unwrap()
        .with_residual(&Tensor::new(residual.clone(), device.tensor_device()).unwrap())
        .unwrap();
    let result = ffn.forward(continuation, Some(&mask), &cancel).unwrap();
    let actual = result.residual().to_vec3::<f32>().unwrap();
    let pre = result.next_pre_mix().to_vec3::<f32>().unwrap();
    let mut mix = [[0_f64; 4]; 4];
    for src in 0..4 {
        let sum: f64 = (0..4)
            .map(|dst| f64::from(base[8 + src * 4 + dst]).exp())
            .sum();
        for dst in 0..4 {
            mix[src][dst] = f64::from(base[8 + src * 4 + dst]).exp() / sum + 1e-6;
        }
    }
    for iteration in 0..20 {
        if iteration > 0 {
            for row in &mut mix {
                let sum = row.iter().sum::<f64>() + 1e-6;
                for x in row {
                    *x /= sum;
                }
            }
        }
        for dst in 0..4 {
            let sum = (0..4).map(|src| mix[src][dst]).sum::<f64>() + 1e-6;
            for row in &mut mix {
                row[dst] /= sum;
            }
        }
    }
    for token in 0..3 {
        let source: Vec<f64> = rows[token].iter().copied().map(f64::from).collect();
        let rms = (source.iter().map(|x| x * x).sum::<f64>() / TEST_WIDTH as f64 + 1e-20).sqrt();
        let normalized: Vec<_> = source
            .iter()
            .zip(&norm_weight)
            .map(|(x, w)| x / rms * f64::from(*w))
            .collect();
        let (branch, branch_abs) = oracle.forward(&normalized, token == 1);
        for dst in 0..4 {
            assert!(
                (f64::from(pre[token][dst][0])
                    - (1. / (1. + (-f64::from(base[dst])).exp()) + 1e-6))
                    .abs()
                    < 2e-6
            );
            let post = 2. / (1. + (-f64::from(base[4 + dst])).exp());
            let mut expected = Vec::new();
            let mut absolute = Vec::new();
            for col in 0..TEST_WIDTH {
                let residual_terms: Vec<_> = (0..4)
                    .map(|src| mix[src][dst] * f64::from(residual[token][src][col]))
                    .collect();
                expected.push(residual_terms.iter().sum::<f64>() + post * branch[col]);
                absolute.push(
                    residual_terms.iter().map(|x| x.abs()).sum::<f64>() + post * branch_abs[col],
                );
            }
            assert_reference(&actual[token][dst], &expected, &absolute);
        }
    }
    println!("XQ F32 mixed experts + shifted FFN: nonzero Q2_0/Q3_K/PTQ1_0, independent down partitions, text/image routes, batch rows and carried mHC qualified; not a complete model");
}
