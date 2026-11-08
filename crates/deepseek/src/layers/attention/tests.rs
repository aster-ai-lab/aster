use super::*;
use candle_core::Device;

fn packed_rows(
    format: MainKvFormat,
    codes: &[[u8; 2]],
    limits: &InferenceLimits,
) -> MainKvRowsData {
    let mut records = vec![0_u8; codes.len() * format.bytes_per_token()];
    let data_bytes = if format == MainKvFormat::Fp8Ue8m0 {
        512
    } else {
        256
    };
    for (row, values) in codes.iter().enumerate() {
        let start = row * format.bytes_per_token();
        if format == MainKvFormat::Fp8Ue8m0 {
            records[start..start + 2].copy_from_slice(values);
        } else {
            records[start] = values[0] | values[1] << 4;
        }
        records[start + data_bytes..start + format.bytes_per_token()].fill(
            if format == MainKvFormat::Fp8Ue8m0 {
                127
            } else {
                56
            },
        );
    }
    MainKvRowsData::new(
        format,
        &Tensor::from_vec(
            records,
            (codes.len(), format.bytes_per_token()),
            &Device::Cpu,
        )
        .unwrap(),
        limits,
    )
    .unwrap()
}

#[test]
fn explicit_xq_f32_attention_matches_f64_sink_math_and_the_f16_kv_boundary() {
    let limits = InferenceLimits::default();
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let mut raw = packed_rows(MainKvFormat::Fp8Ue8m0, &[[56, 64], [64, 184]], &limits)
        .records()
        .to_vec2::<u8>()
        .unwrap();
    // This logical 2^-31 is exactly representable in the packed grid/BF16,
    // and must round to zero at the reference's explicit F16 attention cast.
    raw[0][32] = 1;
    raw[0][513] = 105;
    let window = MainKvRows::from_records(
        &device,
        MainKvFormat::Fp8Ue8m0,
        &Tensor::new(raw, &Device::Cpu).unwrap(),
    )
    .unwrap();
    let compressed = packed_rows(MainKvFormat::Fp4E4m3, &[[5, 6], [10, 4]], &limits);
    let compressed =
        MainKvRows::from_records(&device, MainKvFormat::Fp4E4m3, compressed.records()).unwrap();
    let candidates = [
        candidate(KvSource::Window, 4),
        candidate(KvSource::Window, 5),
        candidate(KvSource::Compressed, 1),
        candidate(KvSource::Compressed, 3),
    ];
    let plan =
        SparseAttentionPlan::new(&limits, 128, &candidates, &[5, 6], &[vec![2, 0, 3], vec![]])
            .unwrap();
    let mut query = vec![0_f32; 2 * 2 * MAIN_KV_DIM];
    query[0] = 1.0001;
    query[MAIN_KV_DIM + 1] = -0.3333;
    let query = Tensor::from_vec(query, (2, 2, MAIN_KV_DIM), &Device::Cpu).unwrap();
    let query = query
        .permute((1, 2, 0))
        .unwrap()
        .contiguous()
        .unwrap()
        .permute((2, 0, 1))
        .unwrap();
    assert!(!query.is_contiguous());
    let sink = [0.137_f32, -0.791];
    let attention = SparseMqaAttention::new(device, 2, MAIN_KV_DIM, sinks(&sink)).unwrap();
    let output = attention
        .forward_xq_f32_packed(
            &query,
            &window,
            Some(&compressed),
            &plan,
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(output.dtype(), DType::F32);
    let actual = values(&output);
    let selected = [[3_f64, 4.], [1., 2.], [-1., 2.]];
    for head in 0..2 {
        let logits: Vec<_>=selected.iter().map(|v| if head==0 {f64::from(1.0001_f32)*v[0]}else{f64::from(-0.3333_f32)*v[1]} / (MAIN_KV_DIM as f64).sqrt()).collect();
        let denominator = f64::from(sink[head]).exp() + logits.iter().map(|x| x.exp()).sum::<f64>();
        for col in 0..2 {
            let expected = logits
                .iter()
                .zip(selected)
                .map(|(x, v)| x.exp() * v[col] / denominator)
                .sum::<f64>();
            assert!((f64::from(actual[head * MAIN_KV_DIM + col]) - expected).abs() < 2e-6);
        }
        assert_eq!(actual[head * MAIN_KV_DIM + 32], 0.);
    }
    assert!(actual[2 * MAIN_KV_DIM..].iter().all(|x| *x == 0.));
    assert_ne!(values(&output.to_dtype(DType::BF16).unwrap()), actual);
    assert!(attention
        .forward_xq_f32_packed(
            &query.to_dtype(DType::BF16).unwrap(),
            &window,
            Some(&compressed),
            &plan,
            &CancellationToken::new()
        )
        .is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(attention
        .forward_xq_f32_packed(&query, &window, Some(&compressed), &plan, &cancel)
        .is_err());
}

#[test]
fn xq_f16_rounding_refuses_selected_half_overflow_and_ignores_unselected_rows() {
    let limits = InferenceLimits::default();
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let mut raw = packed_rows(MainKvFormat::Fp8Ue8m0, &[[56, 64], [126, 126]], &limits)
        .records()
        .to_vec2::<u8>()
        .unwrap();
    raw[1][512] = 142;
    let window = MainKvRows::from_records(
        &device,
        MainKvFormat::Fp8Ue8m0,
        &Tensor::new(raw, &Device::Cpu).unwrap(),
    )
    .unwrap();
    let candidates = [
        candidate(KvSource::Window, 0),
        candidate(KvSource::Window, 1),
    ];
    let query = Tensor::ones((1, 1, MAIN_KV_DIM), DType::F32, &Device::Cpu).unwrap();
    let attention = SparseMqaAttention::new(device, 1, MAIN_KV_DIM, sinks(&[0.])).unwrap();
    let plan = |ids| SparseAttentionPlan::new(&limits, 128, &candidates, &[1], &[ids]).unwrap();
    assert!(attention
        .forward_xq_f32_packed(
            &query,
            &window,
            None,
            &plan(vec![0]),
            &CancellationToken::new()
        )
        .is_ok());
    assert!(attention
        .forward_xq_f32_packed(
            &query,
            &window,
            None,
            &plan(vec![1]),
            &CancellationToken::new()
        )
        .is_err());
}

#[test]
fn mixed_packed_formats_preserve_selection_order_and_match_f64_attention() {
    let limits = InferenceLimits::default();
    let window = packed_rows(MainKvFormat::Fp8Ue8m0, &[[56, 64], [64, 184]], &limits);
    let candidates = [
        candidate(KvSource::Window, 4),
        candidate(KvSource::Window, 5),
        candidate(KvSource::Compressed, 1),
        candidate(KvSource::Compressed, 3),
    ];
    let plan = SparseAttentionPlan::new(&limits, 128, &candidates, &[5], &[vec![2, 0, 3]]).unwrap();
    let mut q = vec![0.; 2 * MAIN_KV_DIM];
    q[0] = 1.;
    q[MAIN_KV_DIM + 1] = -1.;
    let query = bf16(&q, (1, 2, MAIN_KV_DIM));
    let sink = sinks(&[1., -2.]);
    for format in [MainKvFormat::Fp4E4m3, MainKvFormat::Fp8Ue8m0] {
        let codes = if format == MainKvFormat::Fp4E4m3 {
            [[5, 6], [10, 4]]
        } else {
            [[68, 72], [184, 64]]
        };
        let compressed = packed_rows(format, &codes, &limits);
        let packed = PackedAttentionRows {
            window: &window,
            compressed: Some(&compressed),
        };
        let actual = values(
            &attend_math(
                &query,
                &packed,
                &sink,
                &plan,
                &limits,
                &CancellationToken::new(),
            )
            .unwrap(),
        );
        let mut decoded = vec![0_f32; 4 * MAIN_KV_DIM];
        for (row, value) in [[1., 2.], [2., -1.], [3., 4.], [-1., 2.]]
            .iter()
            .enumerate()
        {
            decoded[row * MAIN_KV_DIM..row * MAIN_KV_DIM + 2].copy_from_slice(value);
        }
        let baseline = attend_math(
            &query,
            &bf16(&decoded, (4, MAIN_KV_DIM)),
            &sink,
            &plan,
            &limits,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(actual, values(&baseline));
        let selected = [[3_f64, 4.], [1., 2.], [-1., 2.]];
        for head in 0..2 {
            let logits: Vec<_> = selected
                .iter()
                .map(|v| if head == 0 { v[0] } else { -v[1] } / (MAIN_KV_DIM as f64).sqrt())
                .collect();
            let sink_logit = if head == 0 { 1_f64 } else { -2. };
            let denominator = sink_logit.exp() + logits.iter().map(|v| v.exp()).sum::<f64>();
            for feature in 0..2 {
                let expected: f64 = logits
                    .iter()
                    .zip(selected)
                    .map(|(logit, value)| logit.exp() * value[feature] / denominator)
                    .sum();
                assert!((f64::from(actual[head * MAIN_KV_DIM + feature]) - expected).abs() < 0.012);
            }
            assert!(actual[head * MAIN_KV_DIM + 2..(head + 1) * MAIN_KV_DIM]
                .iter()
                .all(|v| *v == 0.));
        }
    }
}

#[test]
fn packed_attention_ignores_unselected_overflowing_fp8_rows() {
    let limits = InferenceLimits::default();
    let mut records = vec![56_u8; 512];
    records.extend(vec![127; 16]);
    records.extend(vec![126; 512]);
    records.extend(vec![254; 16]);
    let window = MainKvRowsData::new(
        MainKvFormat::Fp8Ue8m0,
        &Tensor::from_vec(records, (2, 528), &Device::Cpu).unwrap(),
        &limits,
    )
    .unwrap();
    let keys = PackedAttentionRows {
        window: &window,
        compressed: None,
    };
    let candidates = [
        candidate(KvSource::Window, 0),
        candidate(KvSource::Window, 1),
    ];
    let q = bf16(&vec![0.; MAIN_KV_DIM], (1, 1, MAIN_KV_DIM));
    let good = SparseAttentionPlan::new(&limits, 128, &candidates, &[1], &[vec![0]]).unwrap();
    let output = attend_math(
        &q,
        &keys,
        &sinks(&[0.]),
        &good,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(values(&output), vec![0.5; MAIN_KV_DIM]);
    let bad = SparseAttentionPlan::new(&limits, 128, &candidates, &[1], &[vec![1]]).unwrap();
    assert!(attend_math(
        &q,
        &keys,
        &sinks(&[0.]),
        &bad,
        &limits,
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn packed_role_mismatches_and_fp4_window_refuse_before_attention() {
    let limits = InferenceLimits::default();
    let window = packed_rows(MainKvFormat::Fp8Ue8m0, &[[56, 56]], &limits);
    let compressed = packed_rows(MainKvFormat::Fp4E4m3, &[[2, 2]], &limits);
    let keys = PackedAttentionRows {
        window: &window,
        compressed: Some(&compressed),
    };
    let q = bf16(&vec![0.; MAIN_KV_DIM], (1, 1, MAIN_KV_DIM));
    let sink = sinks(&[0.]);
    for candidates in [
        [
            candidate(KvSource::Compressed, 0),
            candidate(KvSource::Window, 0),
        ],
        [
            candidate(KvSource::Window, 0),
            candidate(KvSource::Window, 0),
        ],
    ] {
        let plan =
            SparseAttentionPlan::new(&limits, 128, &candidates, &[0], &[vec![0, 1]]).unwrap();
        assert!(attend_math(&q, &keys, &sink, &plan, &limits, &CancellationToken::new()).is_err());
    }
    let plan = SparseAttentionPlan::new(
        &limits,
        128,
        &[candidate(KvSource::Window, 0)],
        &[0],
        &[vec![0]],
    )
    .unwrap();
    let wrong_window = PackedAttentionRows {
        window: &compressed,
        compressed: None,
    };
    assert!(attend_math(
        &q,
        &wrong_window,
        &sink,
        &plan,
        &limits,
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn packed_empty_cache_returns_sink_only_zero_and_prefill_holds_both_source_tables() {
    let limits = InferenceLimits {
        max_context_tokens: 129,
        ..InferenceLimits::default()
    };
    let empty = packed_rows(MainKvFormat::Fp8Ue8m0, &[], &limits);
    let keys = PackedAttentionRows {
        window: &empty,
        compressed: None,
    };
    let q = bf16(&vec![0.; MAIN_KV_DIM], (1, 1, MAIN_KV_DIM));
    let plan = SparseAttentionPlan::new(&limits, 128, &[], &[0], &[vec![]]).unwrap();
    assert_eq!(
        values(
            &attend_math(
                &q,
                &keys,
                &sinks(&[0.]),
                &plan,
                &limits,
                &CancellationToken::new()
            )
            .unwrap()
        ),
        vec![0.; MAIN_KV_DIM]
    );
    let window = packed_rows(MainKvFormat::Fp8Ue8m0, &vec![[0, 0]; 129], &limits);
    let compressed = packed_rows(MainKvFormat::Fp4E4m3, &vec![[0, 0]; 129], &limits);
    let mut candidates: Vec<_> = (0..129).map(|p| candidate(KvSource::Window, p)).collect();
    candidates.extend((0..129).map(|p| candidate(KvSource::Compressed, p)));
    let plan =
        SparseAttentionPlan::new(&limits, 128, &candidates, &[128], &[vec![128, 257]]).unwrap();
    let keys = PackedAttentionRows {
        window: &window,
        compressed: Some(&compressed),
    };
    assert_eq!(
        values(
            &attend_math(
                &q,
                &keys,
                &sinks(&[0.]),
                &plan,
                &limits,
                &CancellationToken::new()
            )
            .unwrap()
        ),
        vec![0.; MAIN_KV_DIM]
    );
    assert!(SparseAttentionPlan::new(&limits, 128, &candidates, &[128], &[vec![0]]).is_err());
}

fn bf16(values: &[f32], shape: impl Into<candle_core::Shape>) -> Tensor {
    Tensor::from_slice(values, shape, &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap()
}
fn values(tensor: &Tensor) -> Vec<f32> {
    tensor
        .to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}
fn sinks(values: &[f32]) -> Tensor {
    Tensor::new(values, &Device::Cpu).unwrap()
}
fn candidate(source: KvSource, position: usize) -> KvCandidate {
    KvCandidate::new(source, position)
}

#[test]
fn sources_roundtrip_and_unknown_roles_refuse() {
    for source in [KvSource::Window, KvSource::Compressed] {
        assert_eq!(KvSource::parse(source.as_str()).unwrap(), source);
    }
    for value in ["window", "auto", "index", ""] {
        assert!(KvSource::parse(value).is_err());
    }
}

#[test]
fn sink_is_a_zero_value_denominator_entry_and_is_not_qk_scaled() {
    let limits = InferenceLimits::default();
    let candidates = [
        candidate(KvSource::Window, 0),
        candidate(KvSource::Compressed, 0),
    ];
    let plan = SparseAttentionPlan::new(&limits, 128, &candidates, &[0], &[vec![0, 1]]).unwrap();
    let query = bf16(&[0., 0.], (1, 1, 2));
    let kv = bf16(&[2., 4., 4., 8.], (2, 2));
    // exp(sink)=2, so two zero QK logits plus the sink divide by four.
    let output = attend_math(
        &query,
        &kv,
        &sinks(&[2_f32.ln()]),
        &plan,
        &limits,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(values(&output), vec![1.5, 3.]);
}

#[test]
fn multiple_heads_and_ragged_queries_match_independent_f64_attention() {
    let limits = InferenceLimits::default();
    let candidates = [
        candidate(KvSource::Window, 0),
        candidate(KvSource::Window, 1),
        candidate(KvSource::Compressed, 1),
    ];
    let selections = vec![vec![0], vec![0, 1, 2]];
    let plan = SparseAttentionPlan::new(&limits, 128, &candidates, &[0, 1], &selections).unwrap();
    let q_values = [1_f32, 0., 0., 1., 1., 1., -1., 1.];
    let kv_values = [1_f32, 0., 0., 1., 1., 1.];
    let sink_values = [0.2_f32, -0.3];
    let query = bf16(&q_values, (2, 2, 2));
    let kv = bf16(&kv_values, (3, 2));
    let actual = values(
        &attend_math(
            &query,
            &kv,
            &sinks(&sink_values),
            &plan,
            &limits,
            &CancellationToken::new(),
        )
        .unwrap(),
    );
    for row in 0..2 {
        for head in 0..2 {
            let logits = selections[row]
                .iter()
                .map(|index| {
                    (0..2)
                        .map(|channel| {
                            f64::from(q_values[(row * 2 + head) * 2 + channel])
                                * f64::from(kv_values[index * 2 + channel])
                        })
                        .sum::<f64>()
                        / 2_f64.sqrt()
                })
                .collect::<Vec<_>>();
            let maximum = logits
                .iter()
                .copied()
                .fold(f64::from(sink_values[head]), f64::max);
            let denominator = (f64::from(sink_values[head]) - maximum).exp()
                + logits
                    .iter()
                    .map(|logit| (logit - maximum).exp())
                    .sum::<f64>();
            for channel in 0..2 {
                let expected = selections[row]
                    .iter()
                    .zip(&logits)
                    .map(|(index, logit)| {
                        (logit - maximum).exp() * f64::from(kv_values[index * 2 + channel])
                    })
                    .sum::<f64>()
                    / denominator;
                assert!(
                    (f64::from(actual[(row * 2 + head) * 2 + channel]) - expected).abs() < 0.004
                );
            }
        }
    }
}

#[test]
fn every_query_matches_its_single_token_decode_and_empty_rows_return_zero() {
    let limits = InferenceLimits::default();
    let candidates = [candidate(KvSource::Compressed, 0)];
    let kv = bf16(&[1., 2.], (1, 2));
    let query = bf16(&[1., 0., 0., 1.], (2, 1, 2));
    let sink = sinks(&[0.]);
    let plan =
        SparseAttentionPlan::new(&limits, 128, &candidates, &[0, 1], &[vec![], vec![0]]).unwrap();
    let batch = values(
        &attend_math(
            &query,
            &kv,
            &sink,
            &plan,
            &limits,
            &CancellationToken::new(),
        )
        .unwrap(),
    );
    assert_eq!(&batch[..2], &[0., 0.]);
    for row in 0..2 {
        let single_plan = SparseAttentionPlan::new(
            &limits,
            128,
            &candidates,
            &[row],
            &[plan.selections[row]
                .iter()
                .map(|value| *value as usize)
                .collect()],
        )
        .unwrap();
        let one = attend_math(
            &query.narrow(0, row, 1).unwrap(),
            &kv,
            &sink,
            &single_plan,
            &limits,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(&batch[row * 2..row * 2 + 2], values(&one));
    }
    let no_keys = SparseAttentionPlan::new(&limits, 128, &[], &[0], &[vec![]]).unwrap();
    assert_eq!(
        values(
            &attend_math(
                &query.narrow(0, 0, 1).unwrap(),
                &bf16(&[], (0, 2)),
                &sink,
                &no_keys,
                &limits,
                &CancellationToken::new()
            )
            .unwrap()
        ),
        vec![0., 0.]
    );
}

#[test]
fn future_expired_duplicate_and_invalid_indices_refuse_before_tensor_reads() {
    let limits = InferenceLimits::default();
    let candidates = [
        candidate(KvSource::Window, 0),
        candidate(KvSource::Window, 1),
        candidate(KvSource::Window, 128),
        candidate(KvSource::Compressed, 0),
        candidate(KvSource::Compressed, 129),
    ];
    for selected in [vec![0], vec![4], vec![1, 1], vec![5]] {
        assert!(SparseAttentionPlan::new(&limits, 128, &candidates, &[128], &[selected]).is_err());
    }
    assert!(SparseAttentionPlan::new(&limits, 128, &candidates, &[128], &[vec![1, 2, 3]]).is_ok());
    assert!(
        SparseAttentionPlan::new(&limits, 128, &candidates, &[128, 128], &[vec![], vec![]])
            .is_err()
    );
    assert!(SparseAttentionPlan::new(&limits, 0, &[], &[0], &[vec![]]).is_err());
    assert!(SparseAttentionPlan::new(&limits, 129, &[], &[0], &[vec![]]).is_err());
    let mut small = limits;
    small.max_graph_plan_bytes = 1;
    assert!(SparseAttentionPlan::new(&small, 128, &[], &[0], &[vec![]]).is_err());
}

#[test]
fn only_selected_values_are_read_and_nonfinite_selected_values_refuse() {
    let limits = InferenceLimits::default();
    let candidates = [
        candidate(KvSource::Window, 0),
        candidate(KvSource::Compressed, 0),
    ];
    let query = bf16(&[0.], (1, 1, 1));
    let kv = bf16(&[2., f32::NAN], (2, 1));
    let sink = sinks(&[0.]);
    let selected = SparseAttentionPlan::new(&limits, 128, &candidates, &[0], &[vec![0]]).unwrap();
    assert_eq!(
        values(
            &attend_math(
                &query,
                &kv,
                &sink,
                &selected,
                &limits,
                &CancellationToken::new()
            )
            .unwrap()
        ),
        vec![1.]
    );
    let bad = SparseAttentionPlan::new(&limits, 128, &candidates, &[0], &[vec![1]]).unwrap();
    assert!(attend_math(&query, &kv, &sink, &bad, &limits, &CancellationToken::new()).is_err());
}

#[test]
fn wrong_shapes_dtypes_nonfinite_logits_cancellation_and_peak_budget_refuse() {
    let mut limits = InferenceLimits::default();
    let candidate = [candidate(KvSource::Window, 0)];
    let plan = SparseAttentionPlan::new(&limits, 128, &candidate, &[0], &[vec![0]]).unwrap();
    let query = bf16(&[1.], (1, 1, 1));
    let kv = bf16(&[2.], (1, 1));
    let sink = sinks(&[0.]);
    assert!(attend_math(
        &query.to_dtype(DType::F32).unwrap(),
        &kv,
        &sink,
        &plan,
        &limits,
        &CancellationToken::new()
    )
    .is_err());
    assert!(attend_math(
        &query,
        &kv,
        &sinks(&[f32::NAN]),
        &plan,
        &limits,
        &CancellationToken::new()
    )
    .is_err());
    assert!(attend_math(
        &query,
        &bf16(&[1., 2.], (2, 1)),
        &sink,
        &plan,
        &limits,
        &CancellationToken::new()
    )
    .is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(attend_math(&query, &kv, &sink, &plan, &limits, &cancelled).is_err());
    let huge = bf16(&[1e30], (1, 1, 1));
    let huge_kv = bf16(&[1e30], (1, 1));
    assert!(attend_math(
        &huge,
        &huge_kv,
        &sink,
        &plan,
        &limits,
        &CancellationToken::new()
    )
    .is_err());
    limits.max_state_bytes = attention_peak_bytes(1, 1, 1, 1, 1).unwrap() - 1;
    assert!(attend_math(
        &query,
        &kv,
        &sink,
        &plan,
        &limits,
        &CancellationToken::new()
    )
    .is_err());
}
