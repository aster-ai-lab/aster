use super::*;
use crate::reference;
use candle_core::Device;

fn tensor(values: &[f32]) -> Tensor {
    Tensor::new(values, &Device::Cpu).unwrap()
}
fn values(tensor: &Tensor) -> Vec<f32> {
    tensor.flatten_all().unwrap().to_vec1().unwrap()
}
fn ids(routes: &MoeRoutes) -> Vec<u32> {
    routes
        .logical_experts()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}
fn zero(experts: usize) -> Tensor {
    Tensor::zeros(experts, DType::F32, &Device::Cpu).unwrap()
}

#[test]
fn selection_uses_text_image_bias_only_for_ranking() {
    let scores = Tensor::new(&[[1_f32, 2., 4., 8.], [1., 2., 4., 8.]], &Device::Cpu).unwrap();
    let text = tensor(&[10., 0., 0., 0.]);
    let image = tensor(&[0., 0., 20., 0.]);
    let mask = Tensor::new(&[0_u8, 1], &Device::Cpu).unwrap();
    let routes = select_math(&scores, &text, &image, Some(&mask), 2, 1.5).unwrap();
    assert_eq!(ids(&routes), vec![0, 3, 2, 3]);
    let expected = [1.5 / 9., 12. / 9., 0.5, 1.0];
    for (weight, expected) in values(routes.weights()).iter().zip(expected) {
        assert!((weight - expected).abs() < 1e-6);
    }
}

#[test]
fn exact_ties_choose_distinct_lowest_logical_ids_without_score_perturbation() {
    let scores = Tensor::ones((2, 4), DType::F32, &Device::Cpu).unwrap();
    let routes = select_math(&scores, &zero(4), &zero(4), None, 3, 1.5).unwrap();
    assert_eq!(ids(&routes), vec![0, 1, 2, 0, 1, 2]);
    assert_eq!(values(routes.weights()), vec![0.5; 6]);
}

#[test]
fn score_selection_matches_scalar_gate_including_tiny_score_epsilon() {
    for logits in [vec![-80_f64, -81., 0., 3., -2.], vec![-90., -91., -92.]] {
        let expert_count = logits.len();
        let bias = vec![0_f64; expert_count];
        let scores = logits
            .iter()
            .map(|value| ((value.max(0.) + (-value.abs()).exp().ln_1p()).sqrt()) as f32)
            .collect::<Vec<_>>();
        let scores = tensor(&scores).reshape((1, expert_count)).unwrap();
        let routes = select_math(
            &scores,
            &zero(expert_count),
            &zero(expert_count),
            None,
            2,
            1.5,
        )
        .unwrap();
        let expected = reference::topk_gate(&logits, &bias, 2, 1.5).unwrap();
        assert_eq!(
            ids(&routes),
            expected
                .iter()
                .map(|route| route.expert as u32)
                .collect::<Vec<_>>()
        );
        for (weight, expected) in values(routes.weights()).iter().zip(expected) {
            assert!((f64::from(*weight) - expected.weight).abs() < 1e-6);
        }
    }
}

#[test]
fn invalid_scores_biases_overflow_masks_and_policy_refuse() {
    for scores in [
        [-1_f32, 1.],
        [f32::NAN, 1.],
        [f32::INFINITY, 1.],
        [f32::MAX, f32::MAX],
    ] {
        assert!(select_math(
            &tensor(&scores).reshape((1, 2)).unwrap(),
            &zero(2),
            &zero(2),
            None,
            2,
            1.5
        )
        .is_err());
    }
    let scores = Tensor::ones((1, 4), DType::F32, &Device::Cpu).unwrap();
    assert!(select_math(
        &scores,
        &tensor(&[0., f32::NAN, 0., 0.]),
        &zero(4),
        None,
        2,
        1.5
    )
    .is_err());
    assert!(select_math(&scores, &zero(3), &zero(4), None, 2, 1.5).is_err());
    assert!(select_math(
        &scores.to_dtype(DType::BF16).unwrap(),
        &zero(4),
        &zero(4),
        None,
        2,
        1.5
    )
    .is_err());
    let mask = Tensor::zeros(2, DType::U8, &Device::Cpu).unwrap();
    assert!(select_math(&scores, &zero(4), &zero(4), Some(&mask), 2, 1.5).is_err());
    for (topk, scale) in [(0, 1.5), (5, 1.5), (2, 0.), (2, f64::NAN)] {
        assert!(select_math(&scores, &zero(4), &zero(4), None, topk, scale).is_err());
    }
}

#[test]
fn route_weighted_swiglu_preserves_asymmetric_clip_and_fp32_output() {
    let gate = tensor(&[-20., 20., 0., 1.])
        .reshape((2, 2))
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let value = tensor(&[20., -20., 2., 3.])
        .reshape((2, 2))
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let weights = tensor(&[0.25, 0.5]);
    let actual = swiglu_math(&gate, &value, Some(&weights), 10.).unwrap();
    assert_eq!(actual.dtype(), DType::F32);
    let expected = reference::swiglu(&[-20., 20., 0., 1.], &[20., -20., 2., 3.], 10.).unwrap();
    for (index, (actual, expected)) in values(&actual).iter().zip(expected).enumerate() {
        let expected = expected * [0.25, 0.5][index / 2];
        assert!((f64::from(*actual) - expected).abs() < 1e-5);
    }
    // The negative gate is not clipped to -limit.
    assert!(values(&actual)[0].abs() < 1e-6);
}

#[test]
fn shared_swiglu_omits_route_weight_and_bad_activation_bindings_refuse() {
    let gate = tensor(&[1., 2.])
        .reshape((1, 2))
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let value = tensor(&[3., 4.])
        .reshape((1, 2))
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let shared = swiglu_math(&gate, &value, None, 10.).unwrap();
    let weighted = swiglu_math(&gate, &value, Some(&tensor(&[0.5])), 10.).unwrap();
    for (shared, weighted) in values(&shared).iter().zip(values(&weighted)) {
        assert!((shared * 0.5 - weighted).abs() < 1e-6);
    }
    assert!(swiglu_math(&gate, &value, Some(&tensor(&[-1.])), 10.).is_err());
    assert!(swiglu_math(&gate, &value, Some(&tensor(&[1., 2.])), 10.).is_err());
    assert!(swiglu_math(&gate, &value.to_dtype(DType::F32).unwrap(), None, 10.).is_err());
    assert!(swiglu_math(&gate, &value, None, 0.).is_err());
}

#[test]
fn nonfinite_inputs_and_weighted_overflow_refuse_before_clipping_can_hide_them() {
    let finite = Tensor::full(10_f32, (1, 2), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let bad = Tensor::full(value, (1, 2), &Device::Cpu)
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap();
        assert!(swiglu_math(&bad, &finite, None, 10.).is_err());
        assert!(swiglu_math(&finite, &bad, None, 10.).is_err());
    }
    assert!(swiglu_math(&finite, &finite, Some(&tensor(&[f32::MAX])), 10.).is_err());
}
