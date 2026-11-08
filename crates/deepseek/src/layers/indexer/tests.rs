use super::*;
use candle_core::Device;

fn bf16(data: &[f32], shape: impl Into<candle_core::Shape>) -> Tensor {
    Tensor::from_vec(data.to_vec(), shape, &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap()
}

fn rows(tensor: &Tensor) -> Vec<Vec<u32>> {
    tensor.to_vec2().unwrap()
}

// Independent finite FP32 -> BF16 round-to-nearest-even for scalar oracles.
fn round_bf16(value: f64) -> f64 {
    let bits = (value as f32).to_bits();
    let upper = bits >> 16;
    let lower = bits & 0xffff;
    let rounded = upper + u32::from(lower > 0x8000 || (lower == 0x8000 && upper & 1 != 0));
    f64::from(f32::from_bits(rounded << 16))
}

#[test]
fn index_selection_matches_independent_multihead_f64_relu_weight_reduce() {
    let limits = InferenceLimits::default();
    let ready = [0, 2, 3, 4, 5, 6, 9];
    let positions = [3, 8];
    let plan = SparseIndexPlan::new(&limits, &ready, &positions, None).unwrap();
    let q = [1., 2., -1., -2., 1., 0.5, 0.5, -1., 2., 1., 1., -1.];
    let k = [
        1., 0., 2., -1., 2., 0.5, 1., 1., -1., 2., -2., 1., 0.5, 0.5, 3., -1., -1., -1., 20., 20.,
        20.,
    ];
    let w = [2., -0.5, 1., 0.25];
    let result = select_math(
        &bf16(&q, (2, 2, 3)),
        &bf16(&k, (7, 3)),
        &bf16(&w, (2, 2)),
        &plan,
        2,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    let mut expected = Vec::new();
    for token in 0..2 {
        let mut scores = Vec::new();
        for (key, ready) in ready.iter().enumerate() {
            if *ready > positions[token] {
                continue;
            }
            let mut score = 0_f64;
            for head in 0..2 {
                let dot: f64 = (0..3)
                    .map(|d| f64::from(q[(token * 2 + head) * 3 + d]) * f64::from(k[key * 3 + d]))
                    .sum();
                score += dot.max(0.) * f64::from(w[token * 2 + head]);
            }
            scores.push((key as u32, round_bf16(score)));
        }
        scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut selected: Vec<_> = scores[..2].iter().map(|v| v.0).collect();
        selected.sort();
        expected.push(selected);
    }
    assert_eq!(rows(result.rows()), expected);
    assert_eq!(result.valid_lengths(), &[2, 2]);
}

#[test]
fn bf16_score_rounding_precedes_topk_and_ties_choose_lowest_row() {
    let limits = InferenceLimits::default();
    let plan = SparseIndexPlan::new(&limits, &[0, 1], &[2], None).unwrap();
    let result = select_math(
        &bf16(&[1., 1. / 512.], (1, 1, 2)),
        &bf16(&[1., 0., 1., 1.], (2, 2)),
        &bf16(&[1.], (1, 1)),
        &plan,
        1,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(1_f64 + 1. / 512. > 1.);
    assert_eq!(round_bf16(1. + 1. / 512.), 1.);
    assert_eq!(rows(result.rows()), vec![vec![0]]);
}

#[test]
fn relu_is_per_head_before_signed_effective_weights_and_reduction() {
    let limits = InferenceLimits::default();
    let plan = SparseIndexPlan::new(&limits, &[0, 1], &[2], None).unwrap();
    let q = bf16(&[1., 0., 0., 1.], (1, 2, 2));
    let k = bf16(&[5., -5., 1., 1.], (2, 2));
    let result = select_math(
        &q,
        &k,
        &bf16(&[1., 1.], (1, 2)),
        &plan,
        1,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(rows(result.rows()), vec![vec![0]]);
    let signed = select_math(
        &q,
        &k,
        &bf16(&[-1., 1.], (1, 2)),
        &plan,
        1,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(rows(signed.rows()), vec![vec![1]]);
}

#[test]
fn empty_and_short_prefixes_have_only_invalid_tail_padding() {
    let limits = InferenceLimits::default();
    let plan = SparseIndexPlan::new(&limits, &[1, 3], &[0, 2, 4], None).unwrap();
    let policy = Some(CandidateBlockPolicy::new(CandidateBlockWidth::Eight, 3).unwrap());
    let result = select_math(
        &bf16(&[1., 1., 1.], (3, 1, 1)),
        &bf16(&[3., 2.], (2, 1)),
        &bf16(&[1., 1., 1.], (3, 1)),
        &plan,
        4,
        policy,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    let p = INVALID_INDEX_ROW;
    assert_eq!(
        rows(result.rows()),
        vec![vec![p; 4], vec![0, p, p, p], vec![0, 1, p, p]]
    );
    assert_eq!(result.valid_lengths(), &[0, 1, 2]);
    assert_eq!(
        rows(result.candidate_blocks().unwrap()),
        vec![vec![p; 3], vec![0, p, p], vec![0, p, p]]
    );
}

#[test]
fn restricted_search_restores_actual_rows_and_does_not_read_other_keys() {
    let limits = InferenceLimits::default();
    let restrictions = [vec![0, 3, 5]];
    let plan =
        SparseIndexPlan::new(&limits, &[0, 1, 2, 3, 4, 5, 10], &[6], Some(&restrictions)).unwrap();
    let k = bf16(
        &[1., f32::NAN, f32::NAN, 5., f32::NAN, 9., f32::NAN],
        (7, 1),
    );
    let result = select_math(
        &bf16(&[1.], (1, 1, 1)),
        &k,
        &bf16(&[1.], (1, 1)),
        &plan,
        2,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(rows(result.rows()), vec![vec![3, 5]]);
    assert!(select_math(
        &bf16(&[1.], (1, 1, 1)),
        &k,
        &bf16(&[1.], (1, 1)),
        &plan,
        2,
        Some(CandidateBlockPolicy::new(CandidateBlockWidth::Eight, 1).unwrap()),
        &limits,
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn dense_causal_search_ignores_future_nonfinite_keys() {
    let limits = InferenceLimits::default();
    let plan = SparseIndexPlan::new(&limits, &[0, 3], &[1], None).unwrap();
    let result = select_math(
        &bf16(&[1.], (1, 1, 1)),
        &bf16(&[2., f32::NAN], (2, 1)),
        &bf16(&[1.], (1, 1)),
        &plan,
        1,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(rows(result.rows()), vec![vec![0]]);
}

#[test]
fn candidate_source_uses_block_max_and_forces_last_full_or_partial_block() {
    let limits = InferenceLimits::default();
    for (count, width) in [
        (18, CandidateBlockWidth::Eight),
        (24, CandidateBlockWidth::Eight),
        (35, CandidateBlockWidth::Sixteen),
        (48, CandidateBlockWidth::Sixteen),
    ] {
        let ready: Vec<_> = (0..count + 4).collect();
        let plan = SparseIndexPlan::new(&limits, &ready, &[count - 1], None).unwrap();
        let mut k = vec![1_f32; count + 4];
        k[0] = 30.;
        // Block0 wins by max but loses by mean; source reduction must be max.
        k[width.tokens()..2 * width.tokens()].fill(20.);
        k[count..].fill(f32::NAN);
        let result = select_math(
            &bf16(&[1.], (1, 1, 1)),
            &bf16(&k, (count + 4, 1)),
            &bf16(&[1.], (1, 1)),
            &plan,
            2,
            Some(CandidateBlockPolicy::new(width, 2).unwrap()),
            &limits,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(rows(result.rows()), vec![vec![0, width.tokens() as u32]]);
        assert_eq!(rows(result.candidate_blocks().unwrap()), vec![vec![0, 2]]);
    }
}

#[test]
fn strided_prefill_equals_individual_decode_with_same_causal_prefixes() {
    let limits = InferenceLimits::default();
    let q = bf16(
        &[1., 0., 2., 1., 3., -1., 0., 1., 1., 2., -1., 3.],
        (2, 3, 2),
    )
    .transpose(0, 1)
    .unwrap();
    let k = bf16(&[1., 2., 2., 1., 3., 0., -1., 1.], (4, 2));
    let weights = bf16(&[1., 2., 2., -1., 1., 0.5], (3, 2));
    let ready = [0, 1, 3, 5];
    let positions = [1, 3, 5];
    let plan = SparseIndexPlan::new(&limits, &ready, &positions, None).unwrap();
    let batch = select_math(
        &q,
        &k,
        &weights,
        &plan,
        2,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    for (row, position) in positions.iter().enumerate() {
        let plan = SparseIndexPlan::new(&limits, &ready, &[*position], None).unwrap();
        let single = select_math(
            &q.narrow(0, row, 1).unwrap(),
            &k,
            &weights.narrow(0, row, 1).unwrap(),
            &plan,
            2,
            None,
            &limits,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(rows(batch.rows())[row], rows(single.rows())[0]);
    }
}

#[test]
fn plan_refuses_future_duplicate_unordered_rows_context_and_metadata_overflow() {
    let limits = InferenceLimits::default();
    for (ready, queries, restrictions) in [
        (vec![0, 3], vec![1], Some(vec![vec![1]])),
        (vec![0, 1], vec![2], Some(vec![vec![0, 0]])),
        (vec![0, 1], vec![2], Some(vec![vec![1, 0]])),
        (vec![0, 0], vec![1], None),
        (vec![0], vec![1, 1], None),
        (vec![0], vec![limits.max_context_tokens], None),
        (vec![0], vec![], None),
        (vec![0], vec![1], Some(vec![])),
    ] {
        assert!(SparseIndexPlan::new(&limits, &ready, &queries, restrictions.as_deref()).is_err());
    }
    let tiny = InferenceLimits {
        max_graph_plan_bytes: 1,
        ..limits.clone()
    };
    assert!(SparseIndexPlan::new(&tiny, &[0], &[1], None).is_err());
    let excess = InferenceLimits {
        max_context_tokens: MAX_CONTEXT_TOKENS + 1,
        ..limits
    };
    assert!(SparseIndexPlan::new(&excess, &[0], &[1], None).is_err());
}

#[test]
fn invalid_formats_numerics_cancellation_and_budgets_refuse_without_input_mutation() {
    let limits = InferenceLimits::default();
    let plan = SparseIndexPlan::new(&limits, &[0, 1], &[2], None).unwrap();
    let q = bf16(&[1., 2.], (1, 1, 2));
    let k = bf16(&[3., 4., 5., 6.], (2, 2));
    let w = bf16(&[1.], (1, 1));
    for (bad_q, bad_k, bad_w) in [
        (q.to_dtype(DType::F32).unwrap(), k.clone(), w.clone()),
        (q.clone(), k.to_dtype(DType::F32).unwrap(), w.clone()),
        (q.clone(), k.clone(), w.to_dtype(DType::F32).unwrap()),
        (bf16(&[f32::NAN, 2.], (1, 1, 2)), k.clone(), w.clone()),
        (
            q.clone(),
            bf16(&[f32::INFINITY, 4., 5., 6.], (2, 2)),
            w.clone(),
        ),
        (q.clone(), k.clone(), bf16(&[f32::NAN], (1, 1))),
        (bf16(&[3e38, 3e38], (1, 1, 2)), k.clone(), w.clone()),
        (q.clone(), k.clone(), bf16(&[3e38], (1, 1))),
    ] {
        assert!(select_math(
            &bad_q,
            &bad_k,
            &bad_w,
            &plan,
            1,
            None,
            &limits,
            &CancellationToken::new()
        )
        .is_err());
    }
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(select_math(&q, &k, &w, &plan, 1, None, &limits, &cancellation).is_err());
    let bytes = InferenceLimits {
        max_state_bytes: 1,
        ..limits.clone()
    };
    assert!(select_math(
        &q,
        &k,
        &w,
        &plan,
        1,
        None,
        &bytes,
        &CancellationToken::new()
    )
    .is_err());
    let elements = InferenceLimits {
        max_tensor_elements: 1,
        ..limits
    };
    assert!(select_math(
        &q,
        &k,
        &w,
        &plan,
        1,
        None,
        &elements,
        &CancellationToken::new()
    )
    .is_err());
    assert_eq!(
        q.to_dtype(DType::F32).unwrap().to_vec3::<f32>().unwrap(),
        vec![vec![vec![1., 2.]]]
    );
    assert!(peak_bytes(usize::MAX, 32, 128, 1, 1, 1).is_err());
}

#[test]
fn closed_block_width_and_geometry_bounds_refuse_unsupported_values() {
    for width in [CandidateBlockWidth::Eight, CandidateBlockWidth::Sixteen] {
        assert_eq!(CandidateBlockWidth::parse(width.as_str()).unwrap(), width);
    }
    for bad in ["", "4", "32", "auto"] {
        assert!(CandidateBlockWidth::parse(bad).is_err());
    }
    for topk in [0, 4097] {
        assert!(CandidateBlockPolicy::new(CandidateBlockWidth::Eight, topk).is_err());
    }
    for (heads, dim, topk) in [
        (0, 128, 512),
        (33, 128, 512),
        (32, 0, 512),
        (32, 129, 512),
        (32, 128, 0),
        (32, 128, 513),
    ] {
        assert!(validate_geometry(heads, dim, topk).is_err());
    }
}

#[test]
fn packed_fp4_query_and_keys_reach_topk_without_unquantized_key_substitution() {
    let limits = InferenceLimits::default();
    let mut query_data = vec![0_u8; 64];
    query_data[0] = 2;
    let mut key_data = vec![0_u8; 3 * 64];
    for row in 0..3 {
        key_data[row * 64] = (row + 1) as u8;
    }
    let query = Fp4IndexRowsData::new(
        &limits,
        &Tensor::from_vec(query_data, (1, 64), &Device::Cpu).unwrap(),
        &Tensor::from_vec(vec![127_u8; 4], (1, 4), &Device::Cpu).unwrap(),
    )
    .unwrap();
    let keys = Fp4IndexRowsData::new(
        &limits,
        &Tensor::from_vec(key_data, (3, 64), &Device::Cpu).unwrap(),
        &Tensor::from_vec(vec![127_u8; 12], (3, 4), &Device::Cpu).unwrap(),
    )
    .unwrap();
    let q = query
        .decode_prefix(1, &limits)
        .unwrap()
        .reshape((1, 1, 128))
        .unwrap();
    let plan = SparseIndexPlan::new(&limits, &[0, 1, 2], &[2], None).unwrap();
    let selected = select_math(
        &q,
        &keys,
        &bf16(&[1.], (1, 1)),
        &plan,
        2,
        None,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(rows(selected.rows()), vec![vec![1, 2]]);
}
