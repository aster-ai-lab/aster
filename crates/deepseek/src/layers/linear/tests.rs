use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::Var;

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn floats(t: &Tensor) -> Vec<f32> {
    t.to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}
fn ids(device: &CudaLayerDevice, rows: &[u32]) -> Tensor {
    Tensor::new(rows, device.tensor_device()).unwrap()
}
fn packed(
    device: &CudaLayerDevice,
    layout: MatrixLayout,
    rows: usize,
    columns: usize,
    code: u8,
    scale: u8,
) -> DeviceMatrix {
    let width = if layout == MatrixLayout::Mxfp4Block32 {
        columns / 2
    } else {
        columns
    };
    let scale_rows = if layout == MatrixLayout::Fp8Block32 {
        rows.div_ceil(32)
    } else {
        rows
    };
    DeviceMatrix::from_tensors(
        device,
        layout,
        [rows, columns],
        &Tensor::full(code, (rows, width), device.tensor_device()).unwrap(),
        Some(
            &Tensor::full(
                scale,
                (scale_rows, columns.div_ceil(32)),
                device.tensor_device(),
            )
            .unwrap(),
        ),
    )
    .unwrap()
}

#[test]
fn tiled_fp8_gather_preserves_partial_tiles_repeated_rows_and_scale_orientation() {
    let device = device();
    let data = Tensor::full(56_u8, (33, 35), device.tensor_device()).unwrap();
    let scales = Tensor::new(&[[127_u8, 128], [126, 129]], device.tensor_device()).unwrap();
    let matrix = DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::Fp8Block32,
        [33, 35],
        &data,
        Some(&scales),
    )
    .unwrap();
    assert_eq!(matrix.retained_weight_bytes(), 33 * 35 + 4 + 33 * 4);
    let rows = matrix
        .gather_rows(
            &ids(&device, &[32, 0, 32]),
            DType::F32,
            &CancellationToken::new(),
        )
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(rows[0], [vec![0.5; 32], vec![4.; 3]].concat());
    assert_eq!(rows[1], [vec![1.; 32], vec![2.; 3]].concat());
    assert_eq!(rows[0], rows[2]);
}

#[test]
fn mxfp4_decode_preserves_nibble_order_and_per_row_group_scales() {
    let device = device();
    let bytes: Vec<_> = (0..64)
        .map(|i| ((i * 2) % 16) as u8 | ((((i * 2 + 1) % 16) as u8) << 4))
        .collect();
    let data = Tensor::from_vec(bytes, (2, 32), device.tensor_device()).unwrap();
    let scales = Tensor::new(&[[127_u8, 128], [126, 129]], device.tensor_device()).unwrap();
    let matrix = DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::Mxfp4Block32,
        [2, 64],
        &data,
        Some(&scales),
    )
    .unwrap();
    let actual = floats(
        &matrix
            .gather_rows(
                &ids(&device, &[0, 1]),
                DType::F32,
                &CancellationToken::new(),
            )
            .unwrap(),
    );
    let palette = [
        0_f32, 0.5, 1., 1.5, 2., 3., 4., 6., -0., -0.5, -1., -1.5, -2., -3., -4., -6.,
    ];
    for (index, &actual) in actual.iter().enumerate() {
        assert_eq!(
            actual.to_bits(),
            (palette[index % 16] * [1., 2., 0.5, 4.][index / 32]).to_bits()
        );
    }
    assert_eq!(matrix.retained_weight_bytes(), 68);
}

#[test]
fn raw_checkpoint_scale_zero_keeps_ocp_meaning_while_live_activation_padding_refuses() {
    let device = device();
    let matrix = packed(&device, MatrixLayout::Fp8Row32, 1, 32, 56, 0);
    let decoded = floats(
        &matrix
            .gather_rows(&ids(&device, &[0]), DType::F32, &CancellationToken::new())
            .unwrap(),
    );
    assert_eq!(decoded, vec![f32::from_bits(1 << 22); 32]);
    assert!(Fp8Activations::from_packed(
        &device,
        &Tensor::full(56_u8, (1, 32), device.tensor_device()).unwrap(),
        &Tensor::zeros((1, 1), DType::U8, device.tensor_device()).unwrap()
    )
    .is_err());
}

#[test]
fn plain_projection_retains_fp32_logits_and_typed_input_boundaries() {
    let device = device();
    let cancel = CancellationToken::new();
    let weight = Tensor::new(&[[1_f32, 0.], [1., 1. / 256.]], device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let matrix =
        DeviceMatrix::from_tensors(&device, MatrixLayout::Bf16, [2, 2], &weight, None).unwrap();
    let input = Tensor::ones((1, 2), DType::BF16, device.tensor_device()).unwrap();
    assert_eq!(
        floats(&matrix.forward_bf16(&input, DType::F32, &cancel).unwrap()),
        [1., 1.00390625]
    );
    assert_eq!(
        floats(&matrix.forward_bf16(&input, DType::BF16, &cancel).unwrap()),
        [1., 1.]
    );
    assert!(matrix
        .forward_f32(&input.to_dtype(DType::F32).unwrap(), DType::F32, &cancel)
        .is_err());
    let f32_matrix = DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::F32,
        [2, 2],
        &weight.to_dtype(DType::F32).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(
        floats(
            &f32_matrix
                .forward_f32(&input.to_dtype(DType::F32).unwrap(), DType::F32, &cancel)
                .unwrap()
        ),
        [1., 1.00390625]
    );
}

#[test]
fn fp8_and_mxfp4_gemms_consume_actual_quantized_activations() {
    let device = device();
    let cancel = CancellationToken::new();
    let raw = Tensor::full(1.0625_f32, (1, 32), device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let quantized = Fp8Activations::encode(&device, &raw, &cancel).unwrap();
    assert_eq!(floats(&quantized.decode().unwrap()), vec![1.; 32]);
    for (layout, code) in [
        (MatrixLayout::Fp8Block32, 56),
        (MatrixLayout::Mxfp4Block32, 0x22),
    ] {
        let matrix = packed(&device, layout, 1, 32, code, 127);
        assert_eq!(
            floats(&matrix.forward_fp8(&quantized, DType::F32, &cancel).unwrap()),
            [32.]
        );
        assert!(matrix.forward_bf16(&raw, DType::F32, &cancel).is_err());
    }
    assert_eq!(floats(&raw).iter().sum::<f32>(), 34.);
    let embedding = packed(&device, MatrixLayout::Fp8Row32, 1, 32, 56, 127);
    assert!(embedding
        .forward_fp8(&quantized, DType::F32, &cancel)
        .is_err());
}

#[test]
fn f32_activation_quantization_avoids_an_intermediate_bf16_rounding() {
    let device = device();
    let cancel = CancellationToken::new();
    let input = Tensor::full(1.0625001_f32, (1, 32), device.tensor_device()).unwrap();
    let direct = Fp8Activations::encode(&device, &input, &cancel).unwrap();
    let rounded =
        Fp8Activations::encode(&device, &input.to_dtype(DType::BF16).unwrap(), &cancel).unwrap();
    assert_eq!(floats(&direct.decode().unwrap()), vec![1.125; 32]);
    assert_eq!(floats(&rounded.decode().unwrap()), vec![1.; 32]);
    let matrix = packed(&device, MatrixLayout::Mxfp4Block32, 1, 32, 0x22, 127);
    assert_eq!(
        floats(&matrix.forward_fp8(&direct, DType::F32, &cancel).unwrap()),
        [36.]
    );
    assert_eq!(
        floats(&matrix.forward_fp8(&rounded, DType::F32, &cancel).unwrap()),
        [32.]
    );
}

#[test]
fn output_tiles_use_correct_scale_rows_and_equal_individual_token_results() {
    let device = device();
    let cancel = CancellationToken::new();
    let data = Tensor::full(56_u8, (65, 32), device.tensor_device()).unwrap();
    let scales = Tensor::new(&[[127_u8], [128], [126]], device.tensor_device()).unwrap();
    let matrix = DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::Fp8Block32,
        [65, 32],
        &data,
        Some(&scales),
    )
    .unwrap();
    let input = Tensor::cat(
        &[
            &Tensor::ones((1, 32), DType::BF16, device.tensor_device()).unwrap(),
            &Tensor::full(2_f32, (1, 32), device.tensor_device())
                .unwrap()
                .to_dtype(DType::BF16)
                .unwrap(),
        ],
        0,
    )
    .unwrap();
    let quantized = Fp8Activations::encode(&device, &input, &cancel).unwrap();
    let output = matrix
        .forward_fp8(&quantized, DType::F32, &cancel)
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(
        output[0],
        [vec![32.; 32], vec![64.; 32], vec![16.]].concat()
    );
    for row in 0..2 {
        let one =
            Fp8Activations::encode(&device, &input.narrow(0, row, 1).unwrap(), &cancel).unwrap();
        assert_eq!(
            floats(&matrix.forward_fp8(&one, DType::F32, &cancel).unwrap()),
            output[row]
        );
    }
}

#[test]
fn mixed_sign_multigroup_quantized_products_match_independent_f64_dots() {
    let device = device();
    let cancel = CancellationToken::new();
    let codes = [56_u8, 64, 184, 192];
    let base = [1_f64, 2., -1., -2.];
    let input: Vec<_> = (0..128).map(|i| codes[(i % 64 + i / 64) % 4]).collect();
    let input = Tensor::from_vec(input, (2, 64), device.tensor_device()).unwrap();
    let scales = Tensor::new(&[[127_u8, 126], [128, 127]], device.tensor_device()).unwrap();
    let activations = Fp8Activations::from_packed(&device, &input, &scales).unwrap();
    for layout in [MatrixLayout::Fp8Block32, MatrixLayout::Mxfp4Block32] {
        let mut decoded = Vec::new();
        let (data, scales) = if layout == MatrixLayout::Fp8Block32 {
            let data: Vec<_> = (0..192).map(|i| codes[(i / 64 * 3 + i % 64) % 4]).collect();
            decoded
                .extend((0..192).map(|i| base[(i / 64 * 3 + i % 64) % 4] * [0.5, 2.][i % 64 / 32]));
            (
                Tensor::from_vec(data, (3, 64), device.tensor_device()).unwrap(),
                Tensor::new(&[[126_u8, 128]], device.tensor_device()).unwrap(),
            )
        } else {
            let patterns = [0x21_u8, 0x43, 0xa9, 0xcb];
            let values = [[0.5_f64, 1.], [1.5, 2.], [-0.5, -1.], [-1.5, -2.]];
            let factors = [0.5, 1., 2., 0.5, 1., 4.];
            let data: Vec<_> = (0..96).map(|i| patterns[(i / 32 + i % 32) % 4]).collect();
            for row in 0..3 {
                for column in 0..64 {
                    decoded.push(
                        values[(row + column / 2) % 4][column % 2] * factors[row * 2 + column / 32],
                    );
                }
            }
            (
                Tensor::from_vec(data, (3, 32), device.tensor_device()).unwrap(),
                Tensor::new(
                    &[[126_u8, 127], [128, 126], [127, 129]],
                    device.tensor_device(),
                )
                .unwrap(),
            )
        };
        let matrix =
            DeviceMatrix::from_tensors(&device, layout, [3, 64], &data, Some(&scales)).unwrap();
        let actual = matrix
            .forward_fp8(&activations, DType::F32, &cancel)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap();
        for token in 0..2 {
            for row in 0..3 {
                let expected: f64 = (0..64)
                    .map(|column| {
                        base[(column + token) % 4]
                            * [1., 0.5, 2., 1.][token * 2 + column / 32]
                            * decoded[row * 64 + column]
                    })
                    .sum();
                assert!(
                    (f64::from(actual[token][row]) - expected).abs() < 1e-5,
                    "{layout:?} token={token} row={row}"
                );
            }
        }
    }
}

#[test]
fn immutable_raw_weights_scales_and_packed_activations_ignore_later_mutations() {
    let device = device();
    let cancel = CancellationToken::new();
    let data =
        Var::from_tensor(&Tensor::full(56_u8, (1, 32), device.tensor_device()).unwrap()).unwrap();
    let scales =
        Var::from_tensor(&Tensor::full(127_u8, (1, 1), device.tensor_device()).unwrap()).unwrap();
    let matrix = DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::Fp8Block32,
        [1, 32],
        data.as_tensor(),
        Some(scales.as_tensor()),
    )
    .unwrap();
    let activations =
        Fp8Activations::from_packed(&device, data.as_tensor(), scales.as_tensor()).unwrap();
    data.set(&Tensor::zeros((1, 32), DType::U8, device.tensor_device()).unwrap())
        .unwrap();
    scales
        .set(&Tensor::full(128_u8, (1, 1), device.tensor_device()).unwrap())
        .unwrap();
    assert_eq!(
        floats(
            &matrix
                .forward_fp8(&activations, DType::F32, &cancel)
                .unwrap()
        ),
        [32.]
    );
}

#[test]
fn fp8_expert_dispatch_preserves_bytes_scales_repetitions_and_refuses_bad_ids() {
    let device = device();
    let cancel = CancellationToken::new();
    let input = Tensor::from_vec(
        [
            vec![1_f32 / 32.; 32],
            vec![2_f32 / 32.; 32],
            vec![-1_f32 / 32.; 32],
        ]
        .concat(),
        (3, 32),
        device.tensor_device(),
    )
    .unwrap()
    .to_dtype(DType::BF16)
    .unwrap();
    let packed = Fp8Activations::encode(&device, &input, &cancel).unwrap();
    let data = packed.data_bytes().to_vec2::<u8>().unwrap();
    let scales = packed.scale_bytes().to_vec2::<u8>().unwrap();
    let selected = packed
        .select_rows(&device, &ids(&device, &[2, 0, 2]), &cancel)
        .unwrap();
    assert_eq!(
        selected.data_bytes().to_vec2::<u8>().unwrap(),
        vec![data[2].clone(), data[0].clone(), data[2].clone()]
    );
    assert_eq!(
        selected.scale_bytes().to_vec2::<u8>().unwrap(),
        vec![scales[2].clone(), scales[0].clone(), scales[2].clone()]
    );
    assert_eq!(packed.data_bytes().to_vec2::<u8>().unwrap(), data);
    for invalid in [3_u32, u32::MAX] {
        assert!(packed
            .select_rows(&device, &ids(&device, &[invalid]), &cancel)
            .is_err());
    }
    cancel.cancel();
    assert!(matches!(
        packed.select_rows(&device, &ids(&device, &[0]), &cancel),
        Err(PowerError::InferenceCancelled)
    ));
}

#[test]
fn bad_layouts_scales_data_row_ids_output_dtype_and_budgets_refuse() {
    let device = device();
    let cancel = CancellationToken::new();
    let matrix = packed(&device, MatrixLayout::Fp8Block32, 1, 32, 56, 127);
    for row in [1, u32::MAX] {
        assert!(matrix
            .gather_rows(&ids(&device, &[row]), DType::F32, &cancel)
            .is_err());
    }
    assert!(matrix
        .gather_rows(&ids(&device, &[0]), DType::U8, &cancel)
        .is_err());
    for code in [127_u8, 255] {
        assert!(DeviceMatrix::from_tensors(
            &device,
            MatrixLayout::Fp8Block32,
            [1, 32],
            &Tensor::full(code, (1, 32), device.tensor_device()).unwrap(),
            Some(&Tensor::full(127_u8, (1, 1), device.tensor_device()).unwrap())
        )
        .is_err());
    }
    assert!(DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::Mxfp4Block32,
        [1, 63],
        &Tensor::zeros((1, 31), DType::U8, device.tensor_device()).unwrap(),
        None
    )
    .is_err());
    assert!(DeviceMatrix::from_tensors(
        &device,
        MatrixLayout::Fp8Block32,
        [1, 32],
        &Tensor::zeros((1, 32), DType::U8, device.tensor_device()).unwrap(),
        Some(&Tensor::full(255_u8, (1, 1), device.tensor_device()).unwrap())
    )
    .is_err());
    let mut limits = InferenceLimits::default();
    limits.max_resident_weight_bytes = 1;
    assert!(check_capacity(
        &CudaLayerDevice::cpu_for_tests(limits),
        [1, 32],
        MatrixLayout::Fp8Block32
    )
    .is_err());
    let mut limits = InferenceLimits::default();
    limits.max_state_bytes = 1;
    assert!(Fp8Activations::encode(
        &CudaLayerDevice::cpu_for_tests(limits),
        &Tensor::ones((1, 32), DType::BF16, device.tensor_device()).unwrap(),
        &cancel
    )
    .is_err());
    cancel.cancel();
    assert!(matches!(
        matrix.gather_rows(&ids(&device, &[0]), DType::F32, &cancel),
        Err(PowerError::InferenceCancelled)
    ));
}

#[test]
fn nonfinite_inputs_and_unrepresentable_decoded_or_output_values_refuse() {
    let device = device();
    let cancel = CancellationToken::new();
    assert!(Fp8Activations::encode(
        &device,
        &Tensor::full(f32::NAN, (1, 32), device.tensor_device()).unwrap(),
        &cancel
    )
    .is_err());
    let huge = packed(&device, MatrixLayout::Fp8Block32, 1, 32, 126, 254);
    assert!(huge
        .gather_rows(&ids(&device, &[0]), DType::F32, &cancel)
        .is_err());
    let activation = Fp8Activations::from_packed(
        &device,
        &Tensor::full(56_u8, (1, 32), device.tensor_device()).unwrap(),
        &Tensor::full(127_u8, (1, 1), device.tensor_device()).unwrap(),
    )
    .unwrap();
    let exposed_scale = Var::from_tensor(activation.scale_bytes()).unwrap();
    let matrix = packed(&device, MatrixLayout::Fp8Block32, 1, 32, 56, 127);
    for code in [0_u8, 255] {
        exposed_scale
            .set(&Tensor::full(code, (1, 1), device.tensor_device()).unwrap())
            .unwrap();
        // A Var made from a non-variable Tensor owns a copy in Candle 0.10.2.
        assert_eq!(
            exposed_scale.as_tensor().to_vec2::<u8>().unwrap(),
            vec![vec![code]]
        );
        assert_eq!(
            activation.scale_bytes().to_vec2::<u8>().unwrap(),
            vec![vec![127]]
        );
        assert_eq!(
            floats(
                &matrix
                    .forward_fp8(&activation, DType::F32, &cancel)
                    .unwrap()
            ),
            [32.]
        );
        assert!(Fp8Activations::from_packed(
            &device,
            activation.data_bytes(),
            exposed_scale.as_tensor()
        )
        .is_err());
    }
    let data = Tensor::full(3e38_f32, (1, 2), device.tensor_device()).unwrap();
    let matrix =
        DeviceMatrix::from_tensors(&device, MatrixLayout::F32, [1, 2], &data, None).unwrap();
    assert!(matrix
        .forward_f32(
            &Tensor::full(2_f32, (1, 2), device.tensor_device()).unwrap(),
            DType::F32,
            &cancel
        )
        .is_err());
}

#[test]
fn all_finite_bf16_activations_match_independent_group32_scalar_encoding() {
    let device = device();
    let cancel = CancellationToken::new();
    let input: Vec<f32> = (0..=u16::MAX)
        .map(|bits| f32::from_bits(u32::from(bits) << 16))
        .filter(|x| x.is_finite())
        .collect();
    assert_eq!(input.len(), 65280);
    let matrix = Tensor::from_vec(input.clone(), (1020, 64), device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let encoded = Fp8Activations::encode(&device, &matrix, &cancel).unwrap();
    let actual = encoded
        .data_bytes()
        .flatten_all()
        .unwrap()
        .to_vec1::<u8>()
        .unwrap();
    let scales = encoded
        .scale_bytes()
        .flatten_all()
        .unwrap()
        .to_vec1::<u8>()
        .unwrap();
    let palette: Vec<f32> = (0..127)
        .map(|code| {
            let exponent = code >> 3;
            let fraction = code & 7;
            if exponent == 0 {
                fraction as f32 / 512.
            } else {
                (1. + fraction as f32 / 8.) * 2_f32.powi(exponent - 7)
            }
        })
        .collect();
    for (group, values) in input.chunks_exact(32).enumerate() {
        let amax = values.iter().map(|x| x.abs()).fold(1e-4_f32, f32::max);
        let target = amax * (1_f32 / 448.);
        let code = (1..=247_u32)
            .find(|code| f32::from_bits(code << 23) >= target)
            .unwrap();
        assert_eq!(scales[group], code as u8);
        let reciprocal = 1. / f32::from_bits(code << 23);
        for (offset, &value) in values.iter().enumerate() {
            let normalized = (value * reciprocal).abs().min(448.);
            let mut choice = 0;
            for candidate in 1..127 {
                let distance = (normalized - palette[candidate]).abs();
                let old = (normalized - palette[choice]).abs();
                if distance < old || (distance == old && candidate % 2 == 0) {
                    choice = candidate;
                }
            }
            let expected = choice as u8 | if value.is_sign_negative() { 128 } else { 0 };
            assert_eq!(
                actual[group * 32 + offset],
                expected,
                "input={value},scale={code}"
            );
        }
    }
}
