use super::*;

fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(actual.is_finite());
        assert!(
            (actual - expected).abs() <= 1e-12 + 1e-12 * expected.abs(),
            "actual={actual}, expected={expected}"
        );
    }
}

#[test]
fn linear_uses_output_major_weights_and_independent_integer_result() {
    let x = ReferenceMatrix::new(2, 3, vec![1., 2., 3., -1., 0., 2.]).unwrap();
    let w = ReferenceMatrix::new(2, 3, vec![2., -1., 0., 0., 1., 3.]).unwrap();
    let result = linear(&x, &w).unwrap();
    assert_eq!((result.rows(), result.columns()), (2, 2));
    assert_eq!(result.values(), [0., 11., -2., 6.]);
}

#[test]
fn matrix_rejects_bad_shape_payload_and_nonfinite_values() {
    for (rows, columns, values) in [
        (0, 1, vec![]),
        (1, 0, vec![]),
        (usize::MAX, 2, vec![]),
        (4097, 1, vec![]),
        (1, 2, vec![1.]),
        (1, 1, vec![f64::NAN]),
        (1, 1, vec![f64::INFINITY]),
    ] {
        assert!(ReferenceMatrix::new(rows, columns, values).is_err());
    }
}

#[test]
fn linear_refuses_width_mismatch_result_budget_and_arithmetic_overflow() {
    let row = ReferenceMatrix::new(1, 2, vec![1., 2.]).unwrap();
    let one = ReferenceMatrix::new(1, 1, vec![1.]).unwrap();
    assert!(linear(&row, &one).is_err());
    let wide_result = ReferenceMatrix::new(65, 1, vec![1.; 65]).unwrap();
    assert!(linear(&wide_result, &wide_result).is_err());
    let huge = ReferenceMatrix::new(1, 1, vec![f64::MAX]).unwrap();
    assert!(linear(&huge, &huge).is_err());
}

#[test]
fn rmsnorm_matches_independent_decimal_constants() {
    close(
        &rms_norm(&[1., 2., 3.], &[0.5, 1., 2.], 0.25).unwrap(),
        &[0.22549380840084865, 0.9019752336033946, 2.705925700810184],
    );
    assert_eq!(rms_norm(&[0., 0.], &[1., 2.], 1e-20).unwrap(), [0., 0.]);
}

#[test]
fn rmsnorm_refuses_bad_parameters_and_overflow() {
    for epsilon in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(rms_norm(&[1.], &[1.], epsilon).is_err());
    }
    assert!(rms_norm(&[1.], &[1., 2.], 1e-20).is_err());
    assert!(rms_norm(&[], &[], 1e-20).is_err());
    assert!(rms_norm(&[f64::MAX], &[1.], 1e-20).is_err());
}

#[test]
fn softmax_is_stable_for_large_logits_and_constant_shifts() {
    let expected = [0.09003057317038046, 0.24472847105479764, 0.6652409557748219];
    close(&softmax(&[1., 2., 3.]).unwrap(), &expected);
    close(&softmax(&[1001., 1002., 1003.]).unwrap(), &expected);
    close(&softmax(&[-1000., 1000.]).unwrap(), &[0., 1.]);
    assert!(softmax(&[]).is_err());
    assert!(softmax(&[f64::NAN]).is_err());
    assert!(softmax(&[-f64::MAX, f64::MAX]).is_err());
}

#[test]
fn swiglu_preserves_negative_gate_and_asymmetric_clipping() {
    close(
        &swiglu(&[-20., 20., 2.], &[-50., 20., 3.], 10.).unwrap(),
        &[4.122307236380407e-7, 99.99546021312976, 5.284782467867295],
    );
    assert_eq!(swiglu(&[-1000.], &[1.], 10.).unwrap(), [0.]);
}

#[test]
fn swiglu_rejects_invalid_inputs() {
    for limit in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(swiglu(&[1.], &[1.], limit).is_err());
    }
    assert!(swiglu(&[1.], &[1., 2.], 10.).is_err());
    assert!(swiglu(&[], &[], 10.).is_err());
    assert!(swiglu(&[f64::NAN], &[1.], 10.).is_err());
}

#[test]
fn gate_uses_bias_only_for_selection() {
    let routes = topk_gate(&[-2., 0., 2.], &[3., 0., 0.], 2, 1.5).unwrap();
    assert_eq!(routes.iter().map(|r| r.expert).collect::<Vec<_>>(), [0, 2]);
    close(
        &routes.iter().map(|r| r.weight).collect::<Vec<_>>(),
        &[0.2944914208404129, 1.2055085791595872],
    );
    let image = topk_gate(&[-2., 0., 2.], &[0., 4., 0.], 2, 1.5).unwrap();
    assert_eq!(image.iter().map(|r| r.expert).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn gate_ties_are_stable_and_tiny_scores_keep_epsilon() {
    let tied = topk_gate(&[0.; 4], &[0.; 4], 2, 1.5).unwrap();
    assert_eq!(tied.iter().map(|r| r.expert).collect::<Vec<_>>(), [0, 1]);
    close(&[tied[0].weight, tied[1].weight], &[0.75, 0.75]);
    let tiny = topk_gate(&[-100.; 2], &[0.; 2], 2, 1.5).unwrap();
    close(
        &[tiny[0].weight, tiny[1].weight],
        &[0.027856676507872002; 2],
    );
}

#[test]
fn gate_refuses_unknown_sizes_and_nonfinite_parameters() {
    for k in [0, 3] {
        assert!(topk_gate(&[0.; 2], &[0.; 2], k, 1.5).is_err());
    }
    assert!(topk_gate(&[0.], &[], 1, 1.5).is_err());
    assert!(topk_gate(&[f64::NAN], &[0.], 1, 1.5).is_err());
    assert!(topk_gate(&[0.], &[f64::INFINITY], 1, 1.5).is_err());
    for scale in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(topk_gate(&[0.], &[0.], 1, scale).is_err());
    }
}

#[test]
fn combine_preserves_weights_and_rejects_invalid_routes() {
    let outputs = ReferenceMatrix::new(3, 2, vec![1., 2., 50., 60., 4., 8.]).unwrap();
    let routes = [
        ReferenceRoute {
            expert: 2,
            weight: 0.25,
        },
        ReferenceRoute {
            expert: 0,
            weight: 0.75,
        },
    ];
    close(&combine_experts(&routes, &outputs).unwrap(), &[1.75, 3.5]);
    assert!(combine_experts(&[], &outputs).is_err());
    assert!(combine_experts(&[routes[0].clone(), routes[0].clone()], &outputs).is_err());
    for route in [
        ReferenceRoute {
            expert: 3,
            weight: 1.,
        },
        ReferenceRoute {
            expert: 0,
            weight: -1.,
        },
        ReferenceRoute {
            expert: 0,
            weight: f64::NAN,
        },
    ] {
        assert!(combine_experts(&[route], &outputs).is_err());
    }
}

#[test]
fn e4m3_decodes_spec_boundary_values_and_signed_zero() {
    for (byte, expected) in [
        (0, 0.),
        (1, 0.001953125),
        (7, 0.013671875),
        (8, 0.015625),
        (0x38, 1.),
        (0x3f, 1.875),
        (0x40, 2.),
        (0x7e, 448.),
        (0xfe, -448.),
    ] {
        assert_eq!(decode_e4m3(byte).unwrap(), expected);
    }
    assert_eq!(decode_e4m3(0x80).unwrap().to_bits(), (-0.0_f64).to_bits());
    assert!(decode_e4m3(0x7f).is_err());
    assert!(decode_e4m3(0xff).is_err());
}

#[test]
fn e4m3_exhaustive_finite_order_and_sign_symmetry() {
    for byte in 0..0x7f {
        let value = decode_e4m3(byte).unwrap();
        assert!(value.is_finite());
        assert_eq!(
            decode_e4m3(byte | 128).unwrap().to_bits(),
            (-value).to_bits()
        );
        if byte > 0 {
            assert!(value > decode_e4m3(byte - 1).unwrap());
        }
    }
}

#[test]
fn e2m1_decodes_all_nibbles_and_refuses_high_bits() {
    for (nibble, value) in [0., 0.5, 1., 1.5, 2., 3., 4., 6.].into_iter().enumerate() {
        assert_eq!(decode_e2m1(nibble as u8).unwrap(), value);
        assert_eq!(
            decode_e2m1(nibble as u8 | 8).unwrap().to_bits(),
            (-value).to_bits()
        );
    }
    for byte in 16..=255 {
        assert!(decode_e2m1(byte).is_err());
    }
}

#[test]
fn ue8m0_decodes_scale_extremes_and_refuses_nan() {
    assert_eq!(decode_ue8m0(0).unwrap(), 2.0_f64.powi(-127));
    assert_eq!(decode_ue8m0(127).unwrap(), 1.);
    assert_eq!(decode_ue8m0(128).unwrap(), 2.);
    assert_eq!(decode_ue8m0(254).unwrap(), 2.0_f64.powi(127));
    assert!(decode_ue8m0(255).is_err());
    for byte in 1..=254 {
        assert_eq!(
            decode_ue8m0(byte).unwrap(),
            decode_ue8m0(byte - 1).unwrap() * 2.
        );
    }
}
