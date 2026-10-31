use super::*;
use crate::layers::linear::activation::Fp8Activations;
use crate::layers::linear::DeviceMatrix;
use crate::layers::CudaLayerDevice;
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Tensor};
use tokio_util::sync::CancellationToken;

#[test]
fn mma_dispatch_is_closed_and_bounds_output_before_allocation() {
    for arch in [(8, 9), (8, 0), (10, 0), (9, 1), (0, 0)] {
        assert!(MmaPlan::new(arch, MatrixLayout::Fp8Block32, 1, 1, 32, 4).is_err());
    }
    for layout in [
        MatrixLayout::Bf16,
        MatrixLayout::F32,
        MatrixLayout::Fp8Row32,
    ] {
        assert!(MmaPlan::new((9, 0), layout, 1, 1, 32, 4).is_err());
    }
    for (m, n, k) in [
        (0, 1, 32),
        (1, 0, 32),
        (1, 1, 0),
        (1, 1, 31),
        (65535 * 16 + 1, 1, 32),
        (1, 1, usize::MAX),
    ] {
        assert!(MmaPlan::new((9, 0), MatrixLayout::Fp8Block32, m, n, k, u64::MAX).is_err());
    }
    assert!(MmaPlan::new((9, 0), MatrixLayout::Fp8Block32, 17, 33, 64, 2243).is_err());
    let p = MmaPlan::new((9, 0), MatrixLayout::Mxfp4Block32, 17, 33, 64, 2244).unwrap();
    assert_eq!((p.tokens, p.rows, p.columns, p.packed_fp4), (17, 33, 64, 1));
    let receipt = p.receipt();
    assert_eq!(receipt.scratch_bytes, 0);
    assert_eq!(receipt.output_bytes, 2244);
    assert_eq!(receipt.source_sha256.len(), 64);
    assert_eq!(receipt.abi, SM90_ABI);
}

#[test]
fn mma_refuses_unqualified_exponents_without_fallback() {
    assert!(MmaPlan::validate_scales((110, 130), (100, 128)).is_ok());
    assert!(MmaPlan::validate_scales((16, 16), (130, 130)).is_ok());
    assert!(MmaPlan::validate_scales((179, 179), (179, 179)).is_ok());
    for (a, w) in [
        ((0, 0), (247, 247)),
        ((15, 128), (128, 128)),
        ((16, 16), (129, 129)),
        ((180, 180), (179, 179)),
        ((128, 240), (128, 128)),
        ((255, 255), (128, 128)),
        ((130, 129), (128, 128)),
    ] {
        let error = MmaPlan::validate_scales(a, w).unwrap_err().to_string();
        assert!(error.contains("explicitly select Sm90ScalarF32V1 or TensorControlV1"));
    }
}

#[test]
fn explicit_control_retains_extreme_checkpoint_precision_and_identity() {
    let d = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    assert_eq!(d.projection_profile(), ProjectionProfile::TensorControlV1);
    assert_eq!(
        d.clone()
            .with_projection_profile(ProjectionProfile::Sm90ScalarF32V1)
            .projection_profile(),
        ProjectionProfile::Sm90ScalarF32V1
    );
    let td = d.tensor_device();
    let a = Fp8Activations::from_checkpoint_rows(
        &d,
        &Tensor::from_vec(vec![1u8; 32], (1, 32), td).unwrap(),
        &Tensor::from_vec(vec![0u8], (1, 1), td).unwrap(),
    )
    .unwrap();
    let w = DeviceMatrix::from_tensors_for_tests(
        &d,
        MatrixLayout::Fp8Block32,
        [1, 32],
        &Tensor::from_vec(vec![0x38u8; 32], (1, 32), td).unwrap(),
        Some(&Tensor::from_vec(vec![247u8], (1, 1), td).unwrap()),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let (out, receipt) = w
        .forward_fp8_with_profile(&a, DType::F32, ProjectionProfile::TensorControlV1, &cancel)
        .unwrap();
    assert_eq!(out.to_vec2::<f32>().unwrap(), vec![vec![2f32.powi(-11)]]);
    assert_eq!(receipt.profile, ProjectionProfile::TensorControlV1);
    assert_eq!(receipt.abi, "aster-f32-tiled-control-v1");
    assert!(receipt.scratch_bytes > 0);
    let (bf16, receipt) = w
        .forward_fp8_with_profile(&a, DType::BF16, ProjectionProfile::TensorControlV1, &cancel)
        .unwrap();
    assert_eq!(bf16.dtype(), DType::BF16);
    assert_eq!(receipt.kernel_output_bytes, 4);
    assert_eq!(receipt.output_bytes, 2);
    assert!(w
        .forward_fp8_with_profile(&a, DType::F32, ProjectionProfile::Sm90Group32MmaV1, &cancel)
        .is_err());
    cancel.cancel();
    assert!(w
        .forward_fp8_with_profile(&a, DType::F32, ProjectionProfile::TensorControlV1, &cancel)
        .is_err());
}

#[cfg(feature = "cuda")]
mod gpu;
