use super::*;
use candle_core::{Device, Var};

fn values(tensor: &Tensor) -> Vec<f32> {
    tensor
        .to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}

#[test]
fn explicit_f32_profile_matches_scalar_math_and_preserves_unrotated_f32_bits() {
    let limits = InferenceLimits::default();
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let cache_values = [[0.6_f32, -0.8, 0.8, 0.6], [1.2, 0.9, -0.4, 0.3]];
    let input_values: Vec<f32> = (0..32).map(|i| i as f32 * 0.137 + 0.0001 - 1.5).collect();
    let input = Tensor::from_vec(input_values.clone(), (2, 2, 8), &Device::Cpu).unwrap();
    for layout in [RopeLayout::Interleaved, RopeLayout::HalfSplit] {
        let rope = RotaryTransform::from_cache(
            device.clone(),
            8,
            2,
            layout,
            Tensor::new(&cache_values, &Device::Cpu).unwrap(),
        )
        .unwrap();
        for direction in [RopeDirection::Forward, RopeDirection::Conjugate] {
            let actual = rope
                .forward_f32_control(&input, &[1, 0], direction, &CancellationToken::new())
                .unwrap();
            assert_eq!(actual.dtype(), DType::F32);
            let actual = values(&actual);
            for token in 0..2 {
                for head in 0..2 {
                    let start = (token * 2 + head) * 8;
                    for col in [0, 1, 6, 7] {
                        assert_eq!(
                            actual[start + col].to_bits(),
                            input_values[start + col].to_bits()
                        );
                    }
                    for pair in 0..2 {
                        let (a, b) = if layout == RopeLayout::Interleaved {
                            (2 + pair * 2, 3 + pair * 2)
                        } else {
                            (2 + pair, 4 + pair)
                        };
                        let x = f64::from(input_values[start + a]);
                        let y = f64::from(input_values[start + b]);
                        let c = f64::from(cache_values[1 - token][pair]);
                        let s = f64::from(cache_values[1 - token][2 + pair])
                            * if direction == RopeDirection::Forward {
                                1.
                            } else {
                                -1.
                            };
                        for (col, expected) in [(a, x * c - y * s), (b, y * c + x * s)] {
                            assert!(
                                (f64::from(actual[start + col]) - expected).abs()
                                    <= 2e-7
                                        * (x.abs() * c.abs()
                                            + y.abs() * s.abs()
                                            + y.abs() * c.abs()
                                            + x.abs() * s.abs())
                                        .max(1.)
                            );
                        }
                    }
                }
            }
        }
        assert!(rope
            .forward(
                &input,
                &[1, 0],
                RopeDirection::Forward,
                &CancellationToken::new()
            )
            .is_err());
        assert!(rope
            .forward_f32_control(
                &input.to_dtype(DType::BF16).unwrap(),
                &[1, 0],
                RopeDirection::Forward,
                &CancellationToken::new()
            )
            .is_err());
        assert!(rope
            .forward_f32_control(
                &input,
                &[2, 0],
                RopeDirection::Forward,
                &CancellationToken::new()
            )
            .is_err());
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(rope
            .forward_f32_control(&input, &[1, 0], RopeDirection::Forward, &cancel)
            .is_err());
    }
}

fn bf16(data: &[f32], shape: (usize, usize, usize)) -> Tensor {
    Tensor::from_vec(data.to_vec(), shape, &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap()
}

#[test]
fn rotary_closed_sets_roundtrip_and_refuse_unknown_values() {
    for layout in [RopeLayout::Interleaved, RopeLayout::HalfSplit] {
        assert_eq!(RopeLayout::parse(layout.as_str()).unwrap(), layout);
    }
    for direction in [RopeDirection::Forward, RopeDirection::Conjugate] {
        assert_eq!(RopeDirection::parse(direction.as_str()).unwrap(), direction);
    }
    for unknown in ["", "neox", "inverse", "auto"] {
        assert!(RopeLayout::parse(unknown).is_err());
        assert!(RopeDirection::parse(unknown).is_err());
    }
}

#[test]
fn both_layouts_and_directions_match_scalar_math_and_preserve_surrounding_bits() {
    let cache_values = [
        [1.2_f32, 0.6, 0.9, -0.8],
        [1., 1., 0., 0.],
        [0.6, -0.8, 0.8, 0.6],
    ];
    let cache_tensor = Tensor::new(&cache_values, &Device::Cpu).unwrap();
    let data: Vec<f32> = (0..48).map(|i| (i as f32 - 17.) / 8.).collect();
    let input = bf16(&data, (3, 2, 8));
    let positions = [2, 0, 2];
    let original = values(&input);
    for layout in [RopeLayout::Interleaved, RopeLayout::HalfSplit] {
        let cache =
            RotaryCache::new(&InferenceLimits::default(), 8, 2, layout, &cache_tensor).unwrap();
        for direction in [RopeDirection::Forward, RopeDirection::Conjugate] {
            let output = cache
                .forward(
                    &InferenceLimits::default(),
                    &input,
                    &positions,
                    direction,
                    &CancellationToken::new(),
                )
                .unwrap();
            let result = values(&output);
            for token in 0..3 {
                for head in 0..2 {
                    let start = (token * 2 + head) * 8;
                    for feature in [0, 1, 6, 7] {
                        assert_eq!(
                            result[start + feature].to_bits(),
                            original[start + feature].to_bits()
                        );
                    }
                    for pair in 0..2 {
                        let (first, second) = match layout {
                            RopeLayout::Interleaved => (2 + pair * 2, 3 + pair * 2),
                            RopeLayout::HalfSplit => (2 + pair, 4 + pair),
                        };
                        let x = f64::from(original[start + first]);
                        let y = f64::from(original[start + second]);
                        let cos = f64::from(cache_values[positions[token]][pair]);
                        let sin = f64::from(cache_values[positions[token]][pair + 2])
                            * if direction == RopeDirection::Forward {
                                1.
                            } else {
                                -1.
                            };
                        for (feature, expected) in
                            [(first, x * cos - y * sin), (second, y * cos + x * sin)]
                        {
                            assert!(
                                (f64::from(result[start + feature]) - expected).abs()
                                    <= expected.abs() * 0.004 + 1e-5
                            );
                        }
                    }
                }
            }
        }
    }
    assert_eq!(values(&input), original);
}

#[test]
fn conjugate_keeps_nonunit_amplitude_instead_of_dividing_it_away() {
    let cache = RotaryCache::new(
        &InferenceLimits::default(),
        2,
        0,
        RopeLayout::Interleaved,
        &Tensor::new(&[[1.5_f32, 0.]], &Device::Cpu).unwrap(),
    )
    .unwrap();
    let input = bf16(&[2., -4.], (1, 1, 2));
    let forward = cache
        .forward(
            &InferenceLimits::default(),
            &input,
            &[0],
            RopeDirection::Forward,
            &CancellationToken::new(),
        )
        .unwrap();
    let conjugate = cache
        .forward(
            &InferenceLimits::default(),
            &forward,
            &[0],
            RopeDirection::Conjugate,
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(values(&forward), vec![3., -6.]);
    assert_eq!(values(&conjugate), vec![4.5, -9.]);
}

#[test]
fn unit_amplitude_quarter_turn_roundtrips_and_half_split_has_distinct_pairing() {
    let cache_tensor = Tensor::new(&[[0_f32, 0., 1., 1.]], &Device::Cpu).unwrap();
    let input = bf16(&[1., 2., 3., 4.], (1, 1, 4));
    for (layout, expected) in [
        (RopeLayout::Interleaved, vec![-2., 1., -4., 3.]),
        (RopeLayout::HalfSplit, vec![-3., -4., 1., 2.]),
    ] {
        let cache =
            RotaryCache::new(&InferenceLimits::default(), 4, 0, layout, &cache_tensor).unwrap();
        let forward = cache
            .forward(
                &InferenceLimits::default(),
                &input,
                &[0],
                RopeDirection::Forward,
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(values(&forward), expected);
        let back = cache
            .forward(
                &InferenceLimits::default(),
                &forward,
                &[0],
                RopeDirection::Conjugate,
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(values(&back), values(&input));
    }
}

#[test]
fn strided_multihead_chunks_equal_individual_token_transforms() {
    let cache = RotaryCache::new(
        &InferenceLimits::default(),
        4,
        0,
        RopeLayout::Interleaved,
        &Tensor::new(&[[0.6_f32, 0.8, 0.8, -0.6], [1., 1., 0., 0.]], &Device::Cpu).unwrap(),
    )
    .unwrap();
    let data: Vec<f32> = (0..24).map(|i| i as f32 / 8.).collect();
    let input = bf16(&data, (2, 3, 4)).transpose(0, 1).unwrap();
    let positions = [1, 0, 1];
    let output = cache
        .forward(
            &InferenceLimits::default(),
            &input,
            &positions,
            RopeDirection::Forward,
            &CancellationToken::new(),
        )
        .unwrap();
    for (row, position) in positions.iter().enumerate() {
        let single = cache
            .forward(
                &InferenceLimits::default(),
                &input.narrow(0, row, 1).unwrap(),
                &[*position],
                RopeDirection::Forward,
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(values(&output.narrow(0, row, 1).unwrap()), values(&single));
    }
}

#[test]
fn cache_snapshot_is_independent_of_mutable_parameter_storage() {
    let parameter = Var::from_tensor(&Tensor::new(&[[0_f32, 1.]], &Device::Cpu).unwrap()).unwrap();
    let cache = RotaryCache::new(
        &InferenceLimits::default(),
        2,
        0,
        RopeLayout::Interleaved,
        parameter.as_tensor(),
    )
    .unwrap();
    parameter
        .set(&Tensor::zeros((1, 2), DType::F32, &Device::Cpu).unwrap())
        .unwrap();
    let output = cache
        .forward(
            &InferenceLimits::default(),
            &bf16(&[2., 3.], (1, 1, 2)),
            &[0],
            RopeDirection::Forward,
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(values(&output), vec![-3., 2.]);
}

#[test]
fn cache_geometry_dtype_numerics_and_construction_budgets_refuse() {
    let limits = InferenceLimits::default();
    let good = Tensor::new(&[[1_f32, 0.]], &Device::Cpu).unwrap();
    for (head, offset, cache) in [
        (2, 0, good.to_dtype(DType::BF16).unwrap()),
        (
            2,
            0,
            Tensor::ones((1, 3), DType::F32, &Device::Cpu).unwrap(),
        ),
        (
            66,
            0,
            Tensor::ones((1, 66), DType::F32, &Device::Cpu).unwrap(),
        ),
        (2, 1, good.clone()),
        (2, usize::MAX, good.clone()),
        (0, 0, good.clone()),
        (513, 0, good.clone()),
        (2, 0, Tensor::new(&[[f32::NAN, 0.]], &Device::Cpu).unwrap()),
        (
            2,
            0,
            Tensor::new(&[[1_f32, f32::INFINITY]], &Device::Cpu).unwrap(),
        ),
    ] {
        assert!(RotaryCache::new(&limits, head, offset, RopeLayout::Interleaved, &cache).is_err());
    }
    let tiny_bytes = InferenceLimits {
        max_state_bytes: 1,
        ..limits.clone()
    };
    assert!(RotaryCache::new(&tiny_bytes, 2, 0, RopeLayout::Interleaved, &good).is_err());
    let tiny_elements = InferenceLimits {
        max_tensor_elements: 1,
        ..limits.clone()
    };
    assert!(RotaryCache::new(&tiny_elements, 2, 0, RopeLayout::Interleaved, &good).is_err());
    let tiny_context = InferenceLimits {
        max_context_tokens: 1,
        ..limits
    };
    assert!(RotaryCache::new(
        &tiny_context,
        2,
        0,
        RopeLayout::Interleaved,
        &Tensor::ones((2, 2), DType::F32, &Device::Cpu).unwrap()
    )
    .is_err());
}

#[test]
fn bad_input_positions_cancellation_and_live_budget_refuse_without_mutation() {
    let limits = InferenceLimits::default();
    let cache = RotaryCache::new(
        &limits,
        2,
        0,
        RopeLayout::Interleaved,
        &Tensor::new(&[[1_f32, 0.]], &Device::Cpu).unwrap(),
    )
    .unwrap();
    let input = bf16(&[2., 3.], (1, 1, 2));
    for positions in [vec![], vec![1], vec![usize::MAX], vec![0, 0]] {
        assert!(cache
            .forward(
                &limits,
                &input,
                &positions,
                RopeDirection::Forward,
                &CancellationToken::new()
            )
            .is_err());
    }
    for bad in [
        input.to_dtype(DType::F32).unwrap(),
        bf16(&[f32::NAN, 1.], (1, 1, 2)),
        bf16(&[1., 2., 3.], (1, 1, 3)),
    ] {
        assert!(cache
            .forward(
                &limits,
                &bad,
                &[0],
                RopeDirection::Forward,
                &CancellationToken::new()
            )
            .is_err());
    }
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(cache
        .forward(&limits, &input, &[0], RopeDirection::Forward, &cancellation)
        .is_err());
    let tiny_bytes = InferenceLimits {
        max_state_bytes: 1,
        ..limits.clone()
    };
    assert!(cache
        .forward(
            &tiny_bytes,
            &input,
            &[0],
            RopeDirection::Forward,
            &CancellationToken::new()
        )
        .is_err());
    let tiny_elements = InferenceLimits {
        max_tensor_elements: 1,
        ..limits
    };
    assert!(cache
        .forward(
            &tiny_elements,
            &input,
            &[0],
            RopeDirection::Forward,
            &CancellationToken::new()
        )
        .is_err());
    let over_context = MAX_CONTEXT_TOKENS + 1;
    let over_input = Tensor::zeros((over_context, 1, 2), DType::BF16, &Device::Cpu).unwrap();
    let relaxed = InferenceLimits {
        max_context_tokens: over_context,
        ..InferenceLimits::default()
    };
    assert!(cache
        .forward(
            &relaxed,
            &over_input,
            &vec![0; over_context],
            RopeDirection::Forward,
            &CancellationToken::new()
        )
        .is_err());
    assert_eq!(values(&input), vec![2., 3.]);
}

#[test]
fn cache_amplitude_overflow_refuses_instead_of_returning_nonfinite_bf16() {
    let limits = InferenceLimits::default();
    let cache = RotaryCache::new(
        &limits,
        2,
        0,
        RopeLayout::Interleaved,
        &Tensor::new(&[[f32::MAX, f32::MAX]], &Device::Cpu).unwrap(),
    )
    .unwrap();
    assert!(cache
        .forward(
            &limits,
            &bf16(&[2., 3.], (1, 1, 2)),
            &[0],
            RopeDirection::Forward,
            &CancellationToken::new()
        )
        .is_err());
}
