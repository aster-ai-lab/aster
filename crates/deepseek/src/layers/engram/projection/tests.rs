use super::*;
use crate::engram::TABLES_PER_LAYER;
use crate::layers::{
    engram::lookup::{
        tests::{checkpoint, table},
        EMBEDDING_WIDTH,
    },
    linear::DeviceMatrix,
    mhc::{MhcBlock, MhcNumerics, MhcParameters},
};
use aster_power::inference::InferenceLimits;
use candle_core::Tensor;

fn device(limits: InferenceLimits) -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(limits)
}
fn floats(t: &Tensor) -> Vec<f32> {
    t.to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}
pub(crate) fn gate(device: &CudaLayerDevice) -> EngramGate {
    EngramGate::new(
        device.clone(),
        Tensor::ones((4, 32), DType::BF16, device.tensor_device()).unwrap(),
        Tensor::ones((4, 32), DType::BF16, device.tensor_device()).unwrap(),
        1e-20,
        1e-6,
    )
    .unwrap()
}
pub(crate) fn projection(device: &CudaLayerDevice, tiny: bool) -> DeviceMatrix {
    let mut data = vec![0_u8; 160 * PROJECTED_INPUT_WIDTH];
    for r in 128..160 {
        data[r * PROJECTED_INPUT_WIDTH] = 56;
        if !tiny {
            data[r * PROJECTED_INPUT_WIDTH + EMBEDDING_WIDTH] = 64;
        }
    }
    let mut sf = vec![127_u8; 5 * PROJECTED_INPUT_WIDTH / 32];
    if tiny {
        sf[4 * PROJECTED_INPUT_WIDTH / 32] = 247;
    }
    DeviceMatrix::from_tensors_for_tests(
        device,
        MatrixLayout::Fp8Block32,
        [160, PROJECTED_INPUT_WIDTH],
        &Tensor::from_vec(data, (160, PROJECTED_INPUT_WIDTH), device.tensor_device()).unwrap(),
        Some(
            &Tensor::from_vec(sf, (5, PROJECTED_INPUT_WIDTH / 32), device.tensor_device()).unwrap(),
        ),
    )
    .unwrap()
}
fn initial(device: &CudaLayerDevice, tokens: usize, value: f32) -> MhcContinuation {
    let mut base = vec![0_f32; 24];
    base[..4].copy_from_slice(&[2., 0., -2., 1.]);
    let mhc = MhcBlock::new(
        device.clone(),
        MhcParameters {
            norm_fused_projection: Tensor::zeros((24, 128), DType::F32, device.tensor_device())
                .unwrap(),
            scale: Tensor::zeros(3, DType::F32, device.tensor_device()).unwrap(),
            base: Tensor::from_vec(base, 24, device.tensor_device()).unwrap(),
        },
        MhcNumerics {
            norm_epsilon: 1e-20,
            pre_epsilon: 1e-6,
            sinkhorn_epsilon: 1e-6,
            sinkhorn_iterations: 20,
            post_multiplier: 2.,
        },
    )
    .unwrap();
    let input = Tensor::full(value, (tokens, 32), device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    mhc.pre_shifted(mhc.initial_shifted(&input).unwrap())
        .unwrap()
        .finish_shifted(&Tensor::zeros((tokens, 32), DType::BF16, device.tensor_device()).unwrap())
        .unwrap()
}

#[test]
fn actual_packed_table_projection_gate_preserves_images_and_shifted_mhc_carry() {
    let device = device(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let data = [
        vec![56; EMBEDDING_WIDTH],
        vec![184; EMBEDDING_WIDTH],
        vec![64; EMBEDDING_WIDTH],
    ]
    .concat();
    let (_dir, checkpoint) = checkpoint(data, vec![127; 24], 3, 528);
    let lookup = table(&device, &checkpoint, 3);
    let mut h = [0; TABLES_PER_LAYER];
    h[1] = 1;
    let hashes = [h, [-1; TABLES_PER_LAYER], [2; TABLES_PER_LAYER]];
    let embeddings = lookup.fetch(&hashes, &cancel).unwrap();
    let owner =
        EngramProjection::new(device.clone(), projection(&device, false), gate(&device)).unwrap();
    let continuation = initial(&device, 3, 1.);
    let carry = floats(continuation.next_pre_mix());
    let before = floats(continuation.residual());
    let out = owner.forward(continuation, &embeddings, &cancel).unwrap();
    assert_eq!(floats(out.next_pre_mix()), carry);
    // Keys are zero, hence exactly half gate. Value=head0+2*head1.
    assert_eq!(
        floats(out.residual()),
        [vec![0.5; 128], before[128..256].to_vec(), vec![4.; 128]].concat()
    );
    let mut decoded = Vec::new();
    for row in hashes {
        let embeddings = lookup.fetch(&[row], &cancel).unwrap();
        decoded.extend(floats(
            owner
                .forward(initial(&device, 1, 1.), &embeddings, &cancel)
                .unwrap()
                .residual(),
        ));
    }
    assert_eq!(floats(out.residual()), decoded);
}

#[test]
fn table_values_are_not_rounded_to_bf16_before_the_fp8_projection() {
    let device = device(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let mut data = vec![0; EMBEDDING_WIDTH];
    data[0] = 1;
    let mut sf = vec![127; 8];
    sf[0] = 0;
    let (_dir, checkpoint) = checkpoint(data, sf, 1, 264);
    let embeddings = table(&device, &checkpoint, 1)
        .fetch(&[[0; TABLES_PER_LAYER]], &cancel)
        .unwrap();
    let owner =
        EngramProjection::new(device.clone(), projection(&device, true), gate(&device)).unwrap();
    let output = owner
        .forward(initial(&device, 1, 0.), &embeddings, &cancel)
        .unwrap();
    // E4M3 0x01 * UE8M0 0 = 2^-136; BF16 would lose it. The
    // selected value weight is 2^120; half gate gives exactly 2^-17.
    assert_eq!(floats(output.residual()), vec![2_f32.powi(-17); 128]);
}

#[test]
fn projection_refuses_incompatible_lookup_geometry_cancellation_and_aggregate_budgets() {
    let device = device(InferenceLimits {
        max_state_bytes: 20_000_000,
        ..InferenceLimits::default()
    });
    let owner =
        EngramProjection::new(device.clone(), projection(&device, false), gate(&device)).unwrap();
    let (_dir, checkpoint) = checkpoint(vec![56; EMBEDDING_WIDTH], vec![127; 8], 1, 264);
    let lookup = table(&device, &checkpoint, 1);
    let cancel = CancellationToken::new();
    let one = lookup.fetch(&[[0; TABLES_PER_LAYER]], &cancel).unwrap();
    assert!(owner
        .forward(initial(&device, 2, 1.), &one, &cancel)
        .is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        owner.forward(initial(&device, 1, 1.), &one, &cancelled),
        Err(PowerError::InferenceCancelled)
    ));
    // Each individual owner fits, their overlapping 32-token live set does not.
    let many = lookup.fetch(&[[0; TABLES_PER_LAYER]; 32], &cancel).unwrap();
    assert!(owner
        .forward(initial(&device, 32, 1.), &many, &cancel)
        .is_err());
    let resident = self::device(InferenceLimits {
        max_resident_weight_bytes: 985151,
        ..InferenceLimits::default()
    });
    assert!(EngramProjection::new(
        resident.clone(),
        projection(&resident, false),
        gate(&resident)
    )
    .is_err());
}
