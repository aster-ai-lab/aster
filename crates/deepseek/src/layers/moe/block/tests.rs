use super::*;
use crate::layers::linear::DeviceMatrix;
use aster_power::inference::InferenceLimits;

fn matrix(device: &CudaLayerDevice, layout: MatrixLayout, code: u8) -> DeviceMatrix {
    let columns = if layout == MatrixLayout::Mxfp4Block32 {
        16
    } else {
        32
    };
    let rows = if layout == MatrixLayout::Mxfp4Block32 {
        32
    } else {
        1
    };
    DeviceMatrix::from_tensors_for_tests(
        device,
        layout,
        [32, 32],
        &Tensor::full(code, (32, columns), device.tensor_device()).unwrap(),
        Some(&Tensor::full(127_u8, (rows, 1), device.tensor_device()).unwrap()),
    )
    .unwrap()
}
fn expert(device: &CudaLayerDevice, layout: MatrixLayout, down: u8) -> QuantizedExpert {
    let code = if layout == MatrixLayout::Mxfp4Block32 {
        0x22
    } else {
        56
    };
    QuantizedExpert::new(
        device.clone(),
        matrix(device, layout, code),
        matrix(device, layout, code),
        matrix(device, layout, down),
        10.,
    )
    .unwrap()
}
fn router(device: &CudaLayerDevice, varied: bool) -> MoeRouter {
    let gate: Vec<_> = (0..4)
        .flat_map(|e| vec![if varied { e as f32 } else { 0. }; 32])
        .collect();
    let gate = Tensor::from_vec(gate, (4, 32), device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    MoeRouter::new(
        device.clone(),
        gate,
        Tensor::new(&[3_f32, 0., 4., 1.], device.tensor_device()).unwrap(),
        Tensor::new(&[0_f32, 3., 1., 4.], device.tensor_device()).unwrap(),
        2,
        1.5,
    )
    .unwrap()
}
fn block(limits: InferenceLimits, varied: bool) -> Result<MoeBlock> {
    let device = CudaLayerDevice::cpu_for_tests(limits);
    let experts = [0x22_u8, 0x44, 0x55, 0x66]
        .into_iter()
        .map(|down| expert(&device, MatrixLayout::Mxfp4Block32, down))
        .collect();
    let shared = expert(&device, MatrixLayout::Fp8Block32, 56);
    MoeBlock::new(device.clone(), router(&device, varied), experts, shared)
}
fn input(block: &MoeBlock, values: &[f32]) -> Tensor {
    Tensor::from_vec(
        values.iter().flat_map(|x| vec![*x; 32]).collect::<Vec<_>>(),
        (values.len(), 32),
        block.device.tensor_device(),
    )
    .unwrap()
    .to_dtype(DType::BF16)
    .unwrap()
}
fn values(t: &Tensor) -> Vec<Vec<f32>> {
    t.to_dtype(DType::F32).unwrap().to_vec2().unwrap()
}

#[test]
fn complete_moe_routes_text_and_image_rows_to_their_own_experts_and_adds_shared_once() {
    let block = block(InferenceLimits::default(), false).unwrap();
    let cancel = CancellationToken::new();
    let hidden = input(&block, &[1. / 32., 1. / 32.]);
    let before = values(&hidden);
    let mask = Tensor::new(&[0_u8, 1], block.device.tensor_device()).unwrap();
    let routes = block.router.forward(&hidden, Some(&mask), &cancel).unwrap();
    assert_eq!(
        routes.logical_experts().to_vec2::<u32>().unwrap(),
        vec![vec![2, 0], vec![3, 1]]
    );
    assert_eq!(values(routes.weights()), vec![vec![0.75, 0.75]; 2]);
    let output = block.forward(&hidden, Some(&mask), &cancel).unwrap();
    assert_eq!(output.dtype(), DType::BF16);
    assert_eq!(values(&output), vec![vec![96.; 32], vec![132.; 32]]);
    assert_eq!(values(&hidden), before);
}

#[test]
fn shared_bias_starts_fp32_accumulation_before_topk_lanes() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let down = DeviceMatrix::from_tensors_for_tests(
        &device,
        MatrixLayout::Mxfp4Block32,
        [32, 32],
        &Tensor::full(0xaa_u8, (32, 16), device.tensor_device()).unwrap(),
        Some(&Tensor::full(152_u8, (32, 1), device.tensor_device()).unwrap()),
    )
    .unwrap();
    let negative = QuantizedExpert::new(
        device.clone(),
        matrix(&device, MatrixLayout::Mxfp4Block32, 0x22),
        matrix(&device, MatrixLayout::Mxfp4Block32, 0x22),
        down,
        10.,
    )
    .unwrap();
    let shared_down = DeviceMatrix::from_tensors_for_tests(
        &device,
        MatrixLayout::Fp8Block32,
        [32, 32],
        &Tensor::full(52_u8, (32, 32), device.tensor_device()).unwrap(),
        Some(&Tensor::full(152_u8, (1, 1), device.tensor_device()).unwrap()),
    )
    .unwrap();
    let shared = QuantizedExpert::new(
        device.clone(),
        matrix(&device, MatrixLayout::Fp8Block32, 56),
        matrix(&device, MatrixLayout::Fp8Block32, 56),
        shared_down,
        10.,
    )
    .unwrap();
    let router = MoeRouter::new(
        device.clone(),
        Tensor::zeros((2, 32), DType::BF16, device.tensor_device()).unwrap(),
        Tensor::new(&[1_f32, 0.], device.tensor_device()).unwrap(),
        Tensor::zeros(2, DType::F32, device.tensor_device()).unwrap(),
        2,
        1.5,
    )
    .unwrap();
    let block = MoeBlock::new(
        device.clone(),
        router,
        vec![negative, expert(&device, MatrixLayout::Mxfp4Block32, 0x22)],
        shared,
    )
    .unwrap();
    // Shared A=18*2^25, then lanes [-A,18]: (A-A)+18 is 18.
    // Accumulating lanes first loses 18 at FP32 precision and returns zero.
    let output = block
        .forward(&input(&block, &[1. / 32.]), None, &cancel)
        .unwrap();
    assert_eq!(values(&output), vec![vec![18.; 32]]);
}

fn bf16(value: f32) -> f32 {
    let bits = value.to_bits();
    f32::from_bits(((bits + 0x7fff + ((bits >> 16) & 1)) >> 16) << 16)
}
fn fp8(value: f32) -> f32 {
    let target = value.abs().max(1e-4) * (1_f32 / 448.);
    let scale = (1..=247_u32)
        .map(|b| f32::from_bits(b << 23))
        .find(|s| *s >= target)
        .unwrap();
    let x = (value * (1. / scale)).abs().min(448.);
    let mut selected = (0_usize, 0_f32);
    for code in 1..127_usize {
        let exponent = code >> 3;
        let mantissa = code & 7;
        let next = if exponent == 0 {
            mantissa as f32 / 512.
        } else {
            (1. + mantissa as f32 / 8.) * 2_f32.powi(exponent as i32 - 7)
        };
        let distance = (x - next).abs();
        let old = (x - selected.1).abs();
        if distance < old || (distance == old && code % 2 == 0) {
            selected = (code, next);
        }
    }
    selected.1 * scale * if value.is_sign_negative() { -1. } else { 1. }
}

#[test]
fn varied_routes_weights_and_quantized_expert_outputs_match_independent_scalar_pipeline() {
    let block = block(InferenceLimits::default(), true).unwrap();
    let cancel = CancellationToken::new();
    let hidden = input(&block, &[1. / 32., 2. / 32., -1. / 32.]);
    let mask = Tensor::new(&[0_u8, 1, 0], block.device.tensor_device()).unwrap();
    let actual = values(&block.forward(&hidden, Some(&mask), &cancel).unwrap());
    for (token, x) in [1_f64, 2., -1.].into_iter().enumerate() {
        let logits: Vec<_> = (0..4).map(|e| e as f64 * x).collect();
        let bias = if token == 1 {
            [0., 3., 1., 4.]
        } else {
            [3., 0., 4., 1.]
        };
        let routes = crate::reference::topk_gate(&logits, &bias, 2, 1.5).unwrap();
        let activation = (x / (1. + (-x).exp()) * x) as f32;
        let mut sum = bf16(32. * fp8(activation));
        for route in routes {
            sum += bf16(32. * fp8(activation * route.weight as f32) * (route.expert + 1) as f32);
        }
        let expected = bf16(sum);
        assert_eq!(actual[token], vec![expected; 32], "token={token}");
    }
}

#[test]
fn ragged_expert_groups_preserve_batch_results_across_each_token_partition() {
    let block = block(InferenceLimits::default(), true).unwrap();
    let cancel = CancellationToken::new();
    let hidden = input(&block, &[1. / 32., 2. / 32., -1. / 32., 1. / 16.]);
    let mask = Tensor::new(&[0_u8, 1, 0, 1], block.device.tensor_device()).unwrap();
    let expected = values(&block.forward(&hidden, Some(&mask), &cancel).unwrap());
    for split in 1..4 {
        let mut actual = values(
            &block
                .forward(
                    &hidden.narrow(0, 0, split).unwrap(),
                    Some(&mask.narrow(0, 0, split).unwrap()),
                    &cancel,
                )
                .unwrap(),
        );
        actual.extend(values(
            &block
                .forward(
                    &hidden.narrow(0, split, 4 - split).unwrap(),
                    Some(&mask.narrow(0, split, 4 - split).unwrap()),
                    &cancel,
                )
                .unwrap(),
        ));
        assert_eq!(actual, expected);
    }
}

#[test]
fn unknown_duplicate_padding_empty_and_malformed_route_metadata_refuse() {
    assert!(group_routes(&[], 0, 2, 4).is_err());
    assert!(group_routes(&[0, 0], 1, 2, 4).is_err());
    assert!(group_routes(&[0, 4], 1, 2, 4).is_err());
    assert!(group_routes(&[0, u32::MAX], 1, 2, 4).is_err());
    assert!(group_routes(&[0], 1, 2, 4).is_err());
    assert!(group_routes(&[0], 1, 1, 0).is_err());
    assert!(group_routes(&[0], 1, 0, 4).is_err());
    let groups = group_routes(&[2, 0, 3, 1, 2, 1], 3, 2, 4).unwrap();
    assert_eq!(
        groups,
        vec![
            vec![(0, 1)],
            vec![(1, 3), (2, 5)],
            vec![(0, 0), (2, 4)],
            vec![(1, 2)]
        ]
    );
}

#[test]
fn missing_experts_wrong_roles_and_aggregate_weight_budget_refuse_at_binding() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let missing = (0..3)
        .map(|_| expert(&device, MatrixLayout::Mxfp4Block32, 0x22))
        .collect();
    assert!(MoeBlock::new(
        device.clone(),
        router(&device, false),
        missing,
        expert(&device, MatrixLayout::Fp8Block32, 56)
    )
    .is_err());
    let wrong = (0..4)
        .map(|_| expert(&device, MatrixLayout::Fp8Block32, 56))
        .collect();
    assert!(MoeBlock::new(
        device.clone(),
        router(&device, false),
        wrong,
        expert(&device, MatrixLayout::Fp8Block32, 56)
    )
    .is_err());
    let mut limits = InferenceLimits::default();
    limits.max_resident_weight_bytes = 4000;
    assert!(block(limits, false).is_err());
}

#[test]
fn bad_inputs_host_metadata_and_live_tensor_budgets_and_cancellation_refuse() {
    let normal = block(InferenceLimits::default(), false).unwrap();
    let cancel = CancellationToken::new();
    let hidden = input(&normal, &[1. / 32., 1. / 32.]);
    assert!(normal
        .forward(&hidden.to_dtype(DType::F32).unwrap(), None, &cancel)
        .is_err());
    assert!(normal
        .forward(
            &hidden,
            Some(&Tensor::new(&[0_u8, 2], normal.device.tensor_device()).unwrap()),
            &cancel
        )
        .is_err());
    let mut limits = InferenceLimits::default();
    limits.max_input_bytes = 1024;
    let small = block(limits, false).unwrap();
    assert!(small.forward(&hidden, None, &cancel).is_err());
    let mut limits = InferenceLimits::default();
    limits.max_state_bytes = 65536;
    let small = block(limits, false).unwrap();
    assert!(small.forward(&hidden, None, &cancel).is_err());
    cancel.cancel();
    assert!(matches!(
        normal.forward(&hidden, None, &cancel),
        Err(PowerError::InferenceCancelled)
    ));
}
