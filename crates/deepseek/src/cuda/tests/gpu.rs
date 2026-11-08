//! Opt-in qualification: an owned H200 is required. Never substitutes the
//! developer workstation's different architecture or silently skips a case.
use super::*;
use aster_power::inference::{DevicePreference, RuntimeDevice};

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_raw_group32_mma_matches_independent_scalar_inputs() {
    let device = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    for layout in [MatrixLayout::Fp8Block32, MatrixLayout::Mxfp4Block32] {
        for (m, n, k) in [(1, 1, 32), (3, 9, 64), (17, 33, 96)] {
            let td = device.tensor_device();
            let ac: Vec<u8> = (0..m * k)
                .map(|i| [0, 1, 0x28, 0x38, 0x42, 0xb8, 0xc0][i % 7])
                .collect();
            let asc: Vec<u8> = (0..m * k / 32).map(|i| 120 + (i % 9) as u8).collect();
            let wc: Vec<u8> = (0..n * k)
                .map(|i| {
                    if layout == MatrixLayout::Mxfp4Block32 {
                        (i % 16) as u8
                    } else {
                        [0, 0x30, 0x38, 0x40, 0xb0, 0xc0][i % 6]
                    }
                })
                .collect();
            let sfrows = if layout == MatrixLayout::Fp8Block32 {
                n.div_ceil(32)
            } else {
                n
            };
            let wsc: Vec<u8> = (0..sfrows * k / 32).map(|i| 121 + (i % 8) as u8).collect();
            let wb: Vec<u8> = if layout == MatrixLayout::Mxfp4Block32 {
                wc.chunks_exact(2).map(|c| c[0] | c[1] << 4).collect()
            } else {
                wc.clone()
            };
            let a = Fp8Activations::from_packed(
                &device,
                &Tensor::from_vec(ac.clone(), (m, k), td).unwrap(),
                &Tensor::from_vec(asc.clone(), (m, k / 32), td).unwrap(),
            )
            .unwrap();
            let w = DeviceMatrix::from_tensors_for_tests(
                &device,
                layout,
                [n, k],
                &Tensor::from_vec(
                    wb,
                    (
                        n,
                        if layout == MatrixLayout::Mxfp4Block32 {
                            k / 2
                        } else {
                            k
                        },
                    ),
                    td,
                )
                .unwrap(),
                Some(&Tensor::from_vec(wsc.clone(), (sfrows, k / 32), td).unwrap()),
            )
            .unwrap();
            let (output, receipt) = w
                .forward_fp8_with_profile(
                    &a,
                    DType::F32,
                    ProjectionProfile::Sm90Group32MmaV1,
                    &cancel,
                )
                .unwrap();
            let actual = output.to_vec2::<f32>().unwrap();
            for r in 0..m {
                for c in 0..n {
                    let expected: f64 = (0..k)
                        .map(|q| {
                            let av = crate::reference::decode_e4m3(ac[r * k + q]).unwrap();
                            let wv = if layout == MatrixLayout::Mxfp4Block32 {
                                crate::reference::decode_e2m1(wc[c * k + q]).unwrap()
                            } else {
                                crate::reference::decode_e4m3(wc[c * k + q]).unwrap()
                            };
                            let wr = if layout == MatrixLayout::Fp8Block32 {
                                c / 32
                            } else {
                                c
                            };
                            av * wv
                                * 2f64.powi(
                                    i32::from(asc[r * k / 32 + q / 32])
                                        + i32::from(wsc[wr * k / 32 + q / 32])
                                        - 254,
                                )
                        })
                        .sum();
                    assert!(
                        (f64::from(actual[r][c]) - expected).abs() <= 1e-5 * expected.abs().max(1.),
                        "{layout:?} {m}/{n}/{k} {r}/{c}: {} != {expected}",
                        actual[r][c]
                    );
                }
            }
            assert_eq!(receipt.scratch_bytes, 0);
            assert_eq!(receipt.abi, SM90_ABI);
            assert_eq!(receipt.ptx_sha256.as_ref().unwrap().len(), 64);
            println!(
                "qualified {layout:?} m={m} n={n} k={k} source={} scratch={}",
                receipt.source_sha256, receipt.scratch_bytes
            );
        }
    }
}

#[test]
#[ignore = "requires an owned H200; run explicitly with --ignored --nocapture"]
fn h200_explicit_scalar_profile_preserves_checkpoint_subnormal_product() {
    let d = CudaLayerDevice::new(
        RuntimeDevice::resolve(DevicePreference::Cuda { ordinal: 0 }).unwrap(),
        InferenceLimits::default(),
    )
    .unwrap()
    .with_projection_profile(ProjectionProfile::Sm90ScalarF32V1);
    let td = d.tensor_device();
    let input = Fp8Activations::from_checkpoint_rows(
        &d,
        &Tensor::from_vec(vec![1u8; 32], (1, 32), td).unwrap(),
        &Tensor::from_vec(vec![0u8], (1, 1), td).unwrap(),
    )
    .unwrap();
    let matrix = DeviceMatrix::from_tensors_for_tests(
        &d,
        MatrixLayout::Fp8Block32,
        [1, 32],
        &Tensor::from_vec(vec![0x38u8; 32], (1, 32), td).unwrap(),
        Some(&Tensor::from_vec(vec![247u8], (1, 1), td).unwrap()),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let rejection = matrix
        .forward_fp8_with_profile(
            &input,
            DType::F32,
            ProjectionProfile::Sm90Group32MmaV1,
            &cancel,
        )
        .err()
        .unwrap()
        .to_string();
    assert!(rejection.contains("scale range"));
    let (out, receipt) = matrix
        .forward_fp8_with_profile(
            &input,
            DType::F32,
            ProjectionProfile::Sm90ScalarF32V1,
            &cancel,
        )
        .unwrap();
    assert_eq!(out.to_vec2::<f32>().unwrap(), vec![vec![2f32.powi(-11)]]);
    assert_eq!(receipt.profile, ProjectionProfile::Sm90ScalarF32V1);
    assert_eq!(receipt.abi, super::super::SM90_SCALAR_ABI);
    assert_eq!(receipt.scratch_bytes, 0);
    assert!(receipt.ptx_sha256.is_some());
    assert_eq!(
        matrix
            .forward_fp8(&input, DType::BF16, &cancel)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .to_vec2::<f32>()
            .unwrap(),
        vec![vec![2f32.powi(-11)]]
    );
    cancel.cancel();
    assert!(matrix.forward_fp8(&input, DType::F32, &cancel).is_err());
    println!(
        "qualified subnormal scalar fallback ABI={} source={} PTX={:?}",
        receipt.abi, receipt.source_sha256, receipt.ptx_sha256
    );
}
