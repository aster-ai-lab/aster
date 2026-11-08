use super::*;
use crate::layers::{
    linear::{expert::QuantizedExpert, DeviceMatrix},
    mhc::{MhcNumerics, MhcParameters},
    moe::router::MoeRouter,
};
use crate::weights::MatrixLayout;
use aster_power::inference::InferenceLimits;
use candle_core::DType;

fn mhc(device: &CudaLayerDevice) -> MhcBlock {
    MhcBlock::new(
        device.clone(),
        MhcParameters {
            norm_fused_projection: Tensor::zeros((24, 128), DType::F32, device.tensor_device())
                .unwrap(),
            scale: Tensor::zeros(3, DType::F32, device.tensor_device()).unwrap(),
            base: Tensor::zeros(24, DType::F32, device.tensor_device()).unwrap(),
        },
        MhcNumerics {
            norm_epsilon: 1e-20,
            pre_epsilon: 1e-6,
            sinkhorn_epsilon: 1e-6,
            sinkhorn_iterations: 20,
            post_multiplier: 2.,
        },
    )
    .unwrap()
}
fn norm(device: &CudaLayerDevice, width: usize, with_weight: bool) -> EffectiveRmsNorm {
    EffectiveRmsNorm::new(
        device.clone(),
        width,
        1e-20,
        with_weight.then(|| Tensor::full(1_f32 / 32., width, device.tensor_device()).unwrap()),
    )
    .unwrap()
}
fn matrix(device: &CudaLayerDevice, layout: MatrixLayout, code: u8) -> DeviceMatrix {
    let packed = layout == MatrixLayout::Mxfp4Block32;
    DeviceMatrix::from_tensors_for_tests(
        device,
        layout,
        [32, 32],
        &Tensor::full(
            code,
            (32, if packed { 16 } else { 32 }),
            device.tensor_device(),
        )
        .unwrap(),
        Some(
            &Tensor::full(
                127_u8,
                (if packed { 32 } else { 1 }, 1),
                device.tensor_device(),
            )
            .unwrap(),
        ),
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
fn moe(device: &CudaLayerDevice) -> MoeBlock {
    let router = MoeRouter::new(
        device.clone(),
        Tensor::zeros((2, 32), DType::BF16, device.tensor_device()).unwrap(),
        Tensor::zeros(2, DType::F32, device.tensor_device()).unwrap(),
        Tensor::zeros(2, DType::F32, device.tensor_device()).unwrap(),
        2,
        1.5,
    )
    .unwrap();
    MoeBlock::new(
        device.clone(),
        router,
        vec![
            expert(device, MatrixLayout::Mxfp4Block32, 0x22),
            expert(device, MatrixLayout::Mxfp4Block32, 0x55),
        ],
        expert(device, MatrixLayout::Fp8Block32, 56),
    )
    .unwrap()
}
pub(crate) fn sublayer(limits: InferenceLimits) -> MoeSublayer {
    let device = CudaLayerDevice::cpu_for_tests(limits);
    MoeSublayer::new(
        device.clone(),
        mhc(&device),
        norm(&device, 32, true),
        moe(&device),
    )
    .unwrap()
}
fn initial(layer: &MoeSublayer, values: &[f32]) -> MhcContinuation {
    let tensor = Tensor::from_vec(
        values.iter().flat_map(|x| vec![*x; 32]).collect::<Vec<_>>(),
        (values.len(), 32),
        layer.device.tensor_device(),
    )
    .unwrap()
    .to_dtype(DType::BF16)
    .unwrap();
    layer.mhc.initial_shifted(&tensor).unwrap()
}
fn floats(t: &Tensor) -> Vec<f32> {
    t.to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}

#[test]
fn complete_feed_forward_half_uses_carried_mix_before_norm_and_returns_next_mix() {
    let layer = sublayer(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let values = [
        vec![1_f32 / 32.; 32],
        vec![-1_f32 / 32.; 32],
        [vec![1_f32 / 32.; 16], vec![-1_f32 / 32.; 16]].concat(),
        vec![-1_f32 / 16.; 32],
    ]
    .concat();
    let trunk = Tensor::from_vec(values, (1, 4, 32), layer.device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let wrong_input = layer.mhc.pre(trunk.clone()).unwrap();
    assert!(floats(wrong_input.input()).iter().all(|x| *x < 0.));
    let state = initial(&layer, &[1. / 32.]).with_residual(&trunk).unwrap();
    let output = layer.forward(state, None, &cancel).unwrap();
    assert_eq!(floats(output.residual()), vec![96.; 128]);
    assert_eq!(floats(&output.head_input().unwrap()), vec![192.; 32]);
    assert!(floats(output.next_pre_mix())
        .iter()
        .all(|x| (*x - 0.500001).abs() < 1e-7));
}

#[test]
fn token_chunks_and_repeated_feed_forward_halves_keep_the_same_residual_state() {
    let layer = sublayer(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let input = [1_f32 / 32., 2. / 32., -1. / 32.];
    let batch = layer
        .forward(initial(&layer, &input), None, &cancel)
        .unwrap();
    let expected: [f32; 3] = [96., 96., 35.];
    assert_eq!(
        floats(batch.residual()),
        expected
            .iter()
            .flat_map(|x| vec![*x; 128])
            .collect::<Vec<_>>()
    );
    for (row, &value) in input.iter().enumerate() {
        let one = layer
            .forward(initial(&layer, &[value]), None, &cancel)
            .unwrap();
        assert_eq!(
            floats(one.residual()),
            floats(batch.residual())[row * 128..(row + 1) * 128]
        );
    }
    let twice = layer.forward(batch, None, &cancel).unwrap();
    assert_eq!(
        floats(twice.residual()),
        [vec![192.; 256], vec![131.; 128]].concat()
    );
    assert_eq!(
        floats(&twice.head_input().unwrap()),
        [vec![384.; 64], vec![262.; 32]].concat()
    );
}

#[test]
fn missing_norm_weights_mismatched_geometry_joint_budget_and_cancel_refuse() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    assert!(MoeSublayer::new(
        device.clone(),
        mhc(&device),
        norm(&device, 64, true),
        moe(&device)
    )
    .is_err());
    assert!(MoeSublayer::new(
        device.clone(),
        mhc(&device),
        norm(&device, 32, false),
        moe(&device)
    )
    .is_err());
    let mut limits = InferenceLimits::default();
    limits.max_state_bytes = 120000;
    let limited = sublayer(limits);
    let state = initial(&limited, &[1. / 32.]);
    assert!(limited
        .forward(state, None, &CancellationToken::new())
        .is_err());
    let layer = sublayer(InferenceLimits::default());
    let state = initial(&layer, &[1. / 32.]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        layer.forward(state, None, &cancel),
        Err(PowerError::InferenceCancelled)
    ));
}
