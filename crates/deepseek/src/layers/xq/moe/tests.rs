use super::*;
use aster_power::inference::InferenceLimits;
use candle_core::Device;

const TEST_WIDTH: usize = 256;
const TEST_EXPERTS: usize = 4;

fn zeros_matrix(
    device: &CudaLayerDevice,
    kind: XqTensorType,
    rows: usize,
    columns: usize,
) -> XqDeviceMatrix {
    let bytes = kind.row_bytes(columns as u64).unwrap() * rows as u64;
    XqDeviceMatrix::from_bytes_for_tests(device, kind, rows, columns, &vec![0; bytes as usize])
        .unwrap()
}

fn parts(
    device: &CudaLayerDevice,
) -> (
    XqDeviceMatrix,
    [XqDeviceMatrix; 3],
    BTreeMap<String, XqDeviceMatrix>,
    Vec<[MatrixRows; 3]>,
) {
    let kinds = [
        XqTensorType::Q2_0,
        XqTensorType::Ptq1_0,
        XqTensorType::Q3K,
        XqTensorType::Ptq1_0,
    ];
    let down = [
        XqTensorType::Ptq1_0,
        XqTensorType::Q3K,
        XqTensorType::Q2_0,
        XqTensorType::Ptq1_0,
    ];
    let mut stacks = BTreeMap::new();
    let mut experts = Vec::new();
    for expert in 0..TEST_EXPERTS {
        let projection = |role: usize| {
            let types = if role == 2 { &down } else { &kinds };
            let kind = types[expert];
            MatrixRows {
                stack: format!("role{role}-{}", kind.as_str()),
                kind,
                first: types[..expert].iter().filter(|&&x| x == kind).count() * TEST_WIDTH,
                rows: TEST_WIDTH,
                columns: TEST_WIDTH,
            }
        };
        let entry = [projection(0), projection(1), projection(2)];
        for (role, p) in entry.iter().enumerate() {
            let types = if role == 2 { &down } else { &kinds };
            let count = types.iter().filter(|&&x| x == p.kind).count();
            stacks
                .entry(p.stack.clone())
                .or_insert_with(|| zeros_matrix(device, p.kind, count * TEST_WIDTH, TEST_WIDTH));
        }
        experts.push(entry);
    }
    (
        zeros_matrix(device, XqTensorType::F32, TEST_EXPERTS, TEST_WIDTH),
        std::array::from_fn(|_| zeros_matrix(device, XqTensorType::Q8_0, TEST_WIDTH, TEST_WIDTH)),
        stacks,
        experts,
    )
}

fn block(device: &CudaLayerDevice) -> XqMoeBlock {
    let (router, shared, stacks, experts) = parts(device);
    XqMoeBlock::from_parts(
        device.clone(),
        router,
        Tensor::zeros(TEST_EXPERTS, DType::F32, device.tensor_device()).unwrap(),
        Tensor::zeros(TEST_EXPERTS, DType::F32, device.tensor_device()).unwrap(),
        shared,
        stacks,
        experts,
        3,
    )
    .unwrap()
}

pub(crate) fn sublayer_for_pipeline(device: &CudaLayerDevice, layer: usize) -> XqFfnSublayer {
    use crate::layers::xq::attention::sublayer::tests::{mhc, norm, q8};
    let (router, _, stacks, experts) = parts(device);
    let shared = std::array::from_fn(|role| q8(device, TEST_WIDTH, TEST_WIDTH, 6 + role));
    let moe = XqMoeBlock::from_parts(
        device.clone(),
        router,
        Tensor::zeros(TEST_EXPERTS, DType::F32, device.tensor_device()).unwrap(),
        Tensor::zeros(TEST_EXPERTS, DType::F32, device.tensor_device()).unwrap(),
        shared,
        stacks,
        experts,
        3,
    )
    .unwrap();
    XqFfnSublayer::from_parts(
        device.clone(),
        layer,
        mhc(device, TEST_WIDTH),
        norm(device, TEST_WIDTH),
        moe,
    )
    .unwrap()
}

#[test]
fn resident_stacks_are_counted_once_with_independent_down_partitions() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let block = block(&device);
    assert_eq!(block.expert_count(), 4);
    assert_eq!(block.hidden_size(), TEST_WIDTH);
    let expected: u64 = block
        .stacks
        .values()
        .map(XqDeviceMatrix::retained_weight_bytes)
        .sum::<u64>()
        + block
            .shared
            .iter()
            .map(XqDeviceMatrix::retained_weight_bytes)
            .sum::<u64>()
        + block.router.retained_weight_bytes()
        + 32;
    assert_eq!(block.retained_bytes(), expected);
    assert_eq!(block.experts[1][0].kind, XqTensorType::Ptq1_0);
    assert_eq!(block.experts[1][2].kind, XqTensorType::Q3K);
    assert_eq!(block.experts[3][0].first, TEST_WIDTH);
    assert_eq!(block.experts[3][2].first, TEST_WIDTH);
}

#[test]
fn malformed_physical_expert_ranges_never_reach_computation() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (_, _, mut stacks, experts) = parts(&device);
    for change in 0..5 {
        let mut bad = experts.clone();
        match change {
            0 => bad[3][0].first = bad[1][0].first,
            1 => bad[0][0].first = usize::MAX,
            2 => bad[0][2].columns = 128,
            3 => bad[0][0].kind = XqTensorType::Q8_0,
            4 => bad[0][0].stack = "missing".into(),
            _ => unreachable!(),
        }
        assert!(validate_ranges(&bad, &stacks, TEST_WIDTH, TEST_WIDTH).is_err());
    }
    stacks.insert(
        "unused".into(),
        zeros_matrix(&device, XqTensorType::Q2_0, TEST_WIDTH, TEST_WIDTH),
    );
    assert!(validate_ranges(&experts, &stacks, TEST_WIDTH, TEST_WIDTH).is_err());
    assert!(validate_ranges(&experts, &stacks, 0, TEST_WIDTH).is_err());
}

#[test]
fn f32_swiglu_clamps_asymmetrically_and_keeps_unrounded_values() {
    let gate = [-20.5_f32, 20.3, -2.8, 0., 1.0001];
    let up = [-100_f32, 99., 1.003, 0.75, 2.999];
    let actual = swiglu(
        &Tensor::from_slice(&gate, (1, 5), &Device::Cpu).unwrap(),
        &Tensor::from_slice(&up, (1, 5), &Device::Cpu).unwrap(),
    )
    .unwrap();
    assert_eq!(actual.dtype(), DType::F32);
    let values = actual.to_vec2::<f32>().unwrap();
    for col in 0..5 {
        let g = f64::from(gate[col]).min(10.);
        let u = f64::from(up[col]).clamp(-10., 10.);
        let expected = g / (1. + (-g).exp()) * u;
        assert!((f64::from(values[0][col]) - expected).abs() < 1e-5 * expected.abs().max(1.));
    }
    assert_ne!(
        actual
            .to_dtype(DType::BF16)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap(),
        values
    );
}

#[test]
fn routed_lanes_accumulate_before_shared_branch_without_bf16_cast() {
    let slots = Tensor::from_vec(
        vec![1e20_f32, 1e-20, -1e20, -1e-20, 3.25, 1.0001],
        (1, 3, 2),
        &Device::Cpu,
    )
    .unwrap();
    let shared = Tensor::from_vec(vec![10_f32, 0.0003], (1, 2), &Device::Cpu).unwrap();
    let output = combine(&slots, &shared, &CancellationToken::new())
        .unwrap()
        .to_vec2::<f32>()
        .unwrap();
    assert_eq!(output[0][0], 13.25);
    // Decimal 1.0004 rounds differently from this two-operand F32 sum.
    assert_eq!(output[0][1].to_bits(), (1.0001_f32 + 0.0003_f32).to_bits());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(combine(&slots, &shared, &cancel).is_err());
    assert!(combine(
        &slots,
        &shared.to_dtype(DType::BF16).unwrap(),
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn expert_execution_supplies_f32_swiglu_to_down_without_activation_requantization() {
    let input = Tensor::from_vec(vec![1.0001_f32, -2.0003], (1, 2), &Device::Cpu).unwrap();
    let mut roles = Vec::new();
    let output = execute_expert(
        &input,
        |role, value| {
            roles.push(role);
            if role < 2 {
                assert_eq!(
                    value.to_vec2::<f32>().unwrap(),
                    input.to_vec2::<f32>().unwrap()
                );
                Ok(if role == 0 {
                    value.clone()
                } else {
                    value.affine(1.5, 0.).unwrap()
                })
            } else {
                assert_eq!(value.dtype(), DType::F32);
                Ok(value.affine(0.75, 0.1).unwrap())
            }
        },
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(roles, [0, 1, 2]);
    let actual = output.to_vec2::<f32>().unwrap();
    for (col, g) in [1.0001_f32, -2.0003].into_iter().enumerate() {
        let expected =
            f64::from(g) / (1. + (-f64::from(g)).exp()) * f64::from(g) * 1.5 * 0.75 + 0.1;
        assert!((f64::from(actual[0][col]) - expected).abs() < 2e-7);
    }
}

#[test]
fn construction_and_forward_refuse_mismatched_limits_bad_types_and_cancellation() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let block = block(&device);
    let input = Tensor::zeros((1, TEST_WIDTH), DType::F32, &Device::Cpu).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(block.forward(&input, None, &cancel).is_err());
    assert!(block
        .forward(&input, None, &CancellationToken::new())
        .is_err()); // no CPU numerical fallback
    assert!(block
        .forward(
            &input.to_dtype(DType::BF16).unwrap(),
            None,
            &CancellationToken::new()
        )
        .is_err());
    let (router, shared, stacks, experts) = parts(&device);
    let strict = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 128,
        ..InferenceLimits::default()
    });
    assert!(XqMoeBlock::from_parts(
        strict,
        router,
        Tensor::zeros(4, DType::F32, &Device::Cpu).unwrap(),
        Tensor::zeros(4, DType::F32, &Device::Cpu).unwrap(),
        shared,
        stacks,
        experts,
        3
    )
    .is_err());
}

fn mhc(device: &CudaLayerDevice) -> XqMhcBlock {
    XqMhcBlock::new(
        device.clone(),
        zeros_matrix(device, XqTensorType::F16, 24, TEST_WIDTH * 4),
        Tensor::ones(3, DType::F32, device.tensor_device()).unwrap(),
        Tensor::zeros(24, DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap()
}

#[test]
fn shifted_ffn_requires_one_device_limits_and_aggregate_resident_admission() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let norm = || {
        XqRmsNorm::new(
            device.clone(),
            Tensor::ones(TEST_WIDTH, DType::F32, device.tensor_device()).unwrap(),
        )
        .unwrap()
    };
    let ffn =
        XqFfnSublayer::from_parts(device.clone(), 3, mhc(&device), norm(), block(&device)).unwrap();
    assert_eq!(ffn.layer(), 3);
    assert_eq!(
        ffn.retained_bytes(),
        ffn.mhc.retained_bytes() + ffn.norm.retained_bytes() + ffn.moe.retained_bytes()
    );
    assert!(
        XqFfnSublayer::from_parts(device.clone(), 40, mhc(&device), norm(), block(&device))
            .is_err()
    );
    let small = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: ffn.retained_bytes() - 1,
        ..InferenceLimits::default()
    });
    // Every component fits alone; the combination still has to refuse.
    let small_norm = XqRmsNorm::new(
        small.clone(),
        Tensor::ones(TEST_WIDTH, DType::F32, small.tensor_device()).unwrap(),
    )
    .unwrap();
    assert!(
        XqFfnSublayer::from_parts(small.clone(), 3, mhc(&small), small_norm, block(&small))
            .is_err()
    );
    let other = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 1 << 30,
        ..InferenceLimits::default()
    });
    assert!(
        XqFfnSublayer::from_parts(device.clone(), 3, mhc(&other), norm(), block(&device)).is_err()
    );
    let input = Tensor::ones((1, TEST_WIDTH), DType::F32, device.tensor_device()).unwrap();
    let continuation =
        XqMhcContinuation::from_embedding(&device, &input, &CancellationToken::new()).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(ffn.forward(continuation, None, &cancel).is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
