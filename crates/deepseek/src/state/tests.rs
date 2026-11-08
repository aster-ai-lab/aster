use super::*;
use crate::reference::compression::{ReferenceCompressionKind, ReferenceCompressionState};
use crate::reference::ReferenceMatrix;
use candle_core::{CpuStorage, Storage, Var};

#[test]
fn compression_profiles_preserve_the_default_and_refuse_unknown_names() {
    for numerics in [
        CompressionNumerics::OfficialBf16,
        CompressionNumerics::XqF32,
    ] {
        assert_eq!(
            CompressionNumerics::parse(numerics.as_str()).unwrap(),
            numerics
        );
    }
    for bad in ["", "auto", "f32", "bf16"] {
        assert!(CompressionNumerics::parse(bad).is_err());
    }
    assert_eq!(
        state(CompressionKind::C1).numerics(),
        CompressionNumerics::OfficialBf16
    );
}

#[test]
fn xq_c1_peak_and_position_metadata_cannot_reuse_smaller_bf16_budgets() {
    let limits = InferenceLimits {
        max_state_bytes: step_peak_bytes(CompressionKind::C1, 1, 64).unwrap(),
        ..InferenceLimits::default()
    };
    let device = CudaLayerDevice::cpu_for_tests(limits);
    assert!(CompressionState::new(&device, CompressionKind::C1, 1, 64, 64).is_ok());
    assert!(CompressionState::new_with_numerics(
        &device,
        CompressionKind::C1,
        CompressionNumerics::XqF32,
        1,
        64,
        64
    )
    .is_err());
    let limits = InferenceLimits {
        max_input_bytes: 4 * std::mem::size_of::<usize>() * 2 - 1,
        ..InferenceLimits::default()
    };
    let device = CudaLayerDevice::cpu_for_tests(limits);
    assert!(CompressionState::new_with_numerics(
        &device,
        CompressionKind::C1,
        CompressionNumerics::XqF32,
        1,
        8,
        4
    )
    .is_err());
    assert!(CompressionState::new_with_numerics(
        &device,
        CompressionKind::C2,
        CompressionNumerics::XqF32,
        1,
        8,
        4
    )
    .is_ok());
}

#[test]
fn xq_partial_acceptance_snapshots_only_the_accepted_pending_f32_half() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let mut state = CompressionState::new_with_numerics(
        &device,
        CompressionKind::C2,
        CompressionNumerics::XqF32,
        2,
        16,
        8,
    )
    .unwrap();
    let original = [1.0001_f32, -2.0003, 3.0007, -4.0009, 5.0013, -6.0021];
    let input =
        Var::from_tensor(&Tensor::from_slice(&original, (3, 2), &Device::Cpu).unwrap()).unwrap();
    let zero = Tensor::zeros((3, 2), DType::F32, &Device::Cpu).unwrap();
    let proposal = state
        .prepare(0, Some(&zero), input.as_tensor(), &cancel)
        .unwrap();
    input
        .set(&Tensor::full(100_f32, (3, 2), &Device::Cpu).unwrap())
        .unwrap();
    let committed = state.commit_prefix(proposal, 1, &cancel).unwrap();
    assert!(committed.emissions().is_none());
    assert_eq!(state.pending_bytes(), 16);
    let next = [9.0001_f32, -10.0003];
    let next = Tensor::from_slice(&next, (1, 2), &Device::Cpu).unwrap();
    let scores = Tensor::zeros((1, 2), DType::F32, &Device::Cpu).unwrap();
    let proposal = state.prepare(1, Some(&scores), &next, &cancel).unwrap();
    let result = state.commit_prefix(proposal, 1, &cancel).unwrap();
    let actual = values(result.emissions().unwrap());
    for col in 0..2 {
        let expected = original[col] * 0.5 + [9.0001_f32, -10.0003][col] * 0.5;
        assert_eq!(actual[col].to_bits(), expected.to_bits());
    }
    assert_eq!(result.emitted_positions(), [1]);
    assert_eq!(state.pending_bytes(), 0);
    assert_eq!(storage_elements(result.emissions().unwrap()), 2);
}

#[test]
fn xq_c1_keeps_f32_bits_and_prefix_copies_charge_four_bytes() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let mut state = CompressionState::new_with_numerics(
        &device,
        CompressionKind::C1,
        CompressionNumerics::XqF32,
        2,
        16,
        8,
    )
    .unwrap();
    let data = [-0_f32, 1.0001, -2.0003, 3.14159];
    let input = Tensor::from_slice(&data, (2, 2), &Device::Cpu).unwrap();
    assert_eq!(
        state.stage_bytes(2).unwrap(),
        (2 * (2 * 4 + std::mem::size_of::<usize>())) as u64
    );
    let proposal = state
        .prepare(0, None, &input, &CancellationToken::new())
        .unwrap();
    assert_eq!(proposal.emissions().unwrap().dtype(), DType::F32);
    let committed = state
        .commit_prefix(proposal, 1, &CancellationToken::new())
        .unwrap();
    assert_eq!(
        values(committed.emissions().unwrap())
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        data[..2].iter().map(|x| x.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(storage_elements(committed.emissions().unwrap()), 2);
    assert_eq!(state.position(), 1);
    assert!(state
        .prepare(
            1,
            None,
            &input.to_dtype(DType::BF16).unwrap(),
            &CancellationToken::new()
        )
        .is_err());
}

#[test]
fn xq_c2_every_chunk_partition_preserves_f32_pairs_and_ready_positions() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let latent: Vec<f32> = (0..6)
        .flat_map(|i| [i as f32 * 0.173 + 1.0001, -(i as f32) * 0.311 - 2.0003])
        .collect();
    let score: Vec<f32> = (0..6)
        .flat_map(|i| [i as f32 * 0.21 - 0.3, 0.5 - i as f32 * 0.13])
        .collect();
    let latents = Tensor::from_vec(latent.clone(), (6, 2), &Device::Cpu).unwrap();
    let scores = Tensor::from_vec(score.clone(), (6, 2), &Device::Cpu).unwrap();
    let mut control = None;
    for partition in 0..32 {
        let mut state = CompressionState::new_with_numerics(
            &device,
            CompressionKind::C2,
            CompressionNumerics::XqF32,
            2,
            16,
            8,
        )
        .unwrap();
        let mut first = 0;
        let mut actual = Vec::new();
        let mut positions = Vec::new();
        for end in 1..=6 {
            if end != 6 && partition & (1 << (end - 1)) == 0 {
                continue;
            }
            let count = end - first;
            let proposal = state
                .prepare(
                    first,
                    Some(&scores.narrow(0, first, count).unwrap()),
                    &latents.narrow(0, first, count).unwrap(),
                    &cancel,
                )
                .unwrap();
            let committed = state.commit_prefix(proposal, count, &cancel).unwrap();
            positions.extend_from_slice(committed.emitted_positions());
            if let Some(tensor) = committed.emissions() {
                assert_eq!(tensor.dtype(), DType::F32);
                actual.extend(values(tensor));
            }
            first = end;
        }
        assert_eq!(positions, [1, 3, 5]);
        assert_eq!(state.pending_bytes(), 0);
        let bits: Vec<_> = actual.iter().map(|x| x.to_bits()).collect();
        if let Some(expected) = &control {
            assert_eq!(&bits, expected);
        } else {
            control = Some(bits);
        }
        for pair in 0..3 {
            for col in 0..2 {
                let a = f64::from(score[(pair * 2) * 2 + col]);
                let b = f64::from(score[(pair * 2 + 1) * 2 + col]);
                let max = a.max(b);
                let expected = ((a - max).exp() * f64::from(latent[(pair * 2) * 2 + col])
                    + (b - max).exp() * f64::from(latent[(pair * 2 + 1) * 2 + col]))
                    / ((a - max).exp() + (b - max).exp());
                assert!((f64::from(actual[pair * 2 + col]) - expected).abs() < 5e-7);
            }
        }
        assert_ne!(
            actual,
            values(
                &Tensor::from_slice(&actual, (3, 2), &Device::Cpu)
                    .unwrap()
                    .to_dtype(DType::BF16)
                    .unwrap()
            )
        );
    }
}

fn state(kind: CompressionKind) -> CompressionState {
    CompressionState::from_device(Device::Cpu, InferenceLimits::default(), kind, 2, 16, 8).unwrap()
}

fn scores(rows: usize) -> Tensor {
    Tensor::from_vec(
        (0..rows)
            .flat_map(|row| {
                if row % 2 == 0 {
                    [0_f32, 3_f32.ln()]
                } else {
                    [3_f32.ln(), 0.]
                }
            })
            .collect::<Vec<_>>(),
        (rows, 2),
        &Device::Cpu,
    )
    .unwrap()
}

fn latents(rows: usize) -> Tensor {
    Tensor::from_vec(
        (0..rows)
            .flat_map(|row| [(2 * row + 2) as f32, (4 * row + 4) as f32])
            .collect::<Vec<_>>(),
        (rows, 2),
        &Device::Cpu,
    )
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

fn storage_elements(tensor: &Tensor) -> usize {
    let (storage, _) = tensor.storage_and_layout();
    match &*storage {
        Storage::Cpu(CpuStorage::F32(values)) => values.len(),
        Storage::Cpu(CpuStorage::BF16(values)) => values.len(),
        _ => panic!("expected CPU fixture"),
    }
}

#[test]
fn compression_kinds_roundtrip_and_refuse_unknown_ratios() {
    for (kind, ratio) in [(CompressionKind::C1, 1), (CompressionKind::C2, 2)] {
        assert_eq!(CompressionKind::parse(kind.as_str()).unwrap(), kind);
        assert_eq!(kind.ratio(), ratio);
    }
    for value in ["c0", "c4", "auto", ""] {
        assert!(CompressionKind::parse(value).is_err());
    }
}

#[test]
fn c1_preserves_bf16_bits_without_score_or_extra_rounding() {
    let mut state = state(CompressionKind::C1);
    let input = Tensor::from_vec(vec![-0_f32, 1.00390625, -5.125, 10.], (2, 2), &Device::Cpu)
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let proposal = state
        .prepare(0, None, &input, &CancellationToken::new())
        .unwrap();
    assert_eq!(proposal.emitted_positions(), &[0, 1]);
    assert_eq!(state.position(), 0);
    let expected = values(&input)
        .iter()
        .map(|value| value.to_bits())
        .collect::<Vec<_>>();
    let committed = state
        .commit_prefix(proposal, 2, &CancellationToken::new())
        .unwrap();
    assert_eq!(
        values(committed.emissions().unwrap())
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(state.pending_bytes(), 0);
    assert_eq!(state.position(), 2);
}

#[test]
fn c2_is_a_per_channel_two_token_softmax_and_emits_only_complete_pairs() {
    let mut state = state(CompressionKind::C2);
    let input = Tensor::new(&[[2_f32, 4.], [10., 20.]], &Device::Cpu).unwrap();
    let proposal = state
        .prepare(0, Some(&scores(2)), &input, &CancellationToken::new())
        .unwrap();
    assert_eq!(values(proposal.emissions().unwrap()), vec![8., 8.]);
    assert_eq!(proposal.emitted_positions(), &[1]);
    state
        .commit_prefix(proposal, 1, &CancellationToken::new())
        .unwrap();
    assert_eq!(state.position(), 1);
    assert_eq!(state.pending_bytes(), 16);
    let proposal = state
        .prepare(
            1,
            Some(&scores(2).narrow(0, 1, 1).unwrap()),
            &input.narrow(0, 1, 1).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(values(proposal.emissions().unwrap()), vec![8., 8.]);
    state
        .commit_prefix(proposal, 1, &CancellationToken::new())
        .unwrap();
    assert_eq!(state.pending_bytes(), 0);
}

#[test]
fn all_chunk_partitions_equal_decode_and_the_independent_f64_oracle() {
    let s = scores(6);
    let x = latents(6);
    let oracle = ReferenceCompressionState::new(ReferenceCompressionKind::C2, 2, 16).unwrap();
    let sm = ReferenceMatrix::new(6, 2, values(&s).into_iter().map(f64::from).collect()).unwrap();
    let xm = ReferenceMatrix::new(6, 2, values(&x).into_iter().map(f64::from).collect()).unwrap();
    let expected = oracle
        .extend(0, Some(&sm), &xm)
        .unwrap()
        .state
        .compressed_values()
        .to_vec();
    for partition in 0..32 {
        let mut state = state(CompressionKind::C2);
        let mut start = 0;
        let mut output = Vec::new();
        let mut positions = Vec::new();
        for end in 1..=6 {
            if end == 6 || partition & (1 << (end - 1)) != 0 {
                let proposal = state
                    .prepare(
                        start,
                        Some(&s.narrow(0, start, end - start).unwrap()),
                        &x.narrow(0, start, end - start).unwrap(),
                        &CancellationToken::new(),
                    )
                    .unwrap();
                let committed = state
                    .commit_prefix(proposal, end - start, &CancellationToken::new())
                    .unwrap();
                if let Some(rows) = committed.emissions() {
                    output.extend(values(rows));
                }
                positions.extend_from_slice(committed.emitted_positions());
                start = end;
            }
        }
        for (actual, expected) in output.iter().zip(&expected) {
            assert!((f64::from(*actual) - expected).abs() < 1e-5);
        }
        assert_eq!(positions, vec![1, 3, 5]);
        assert_eq!(state.position(), 6);
        assert_eq!(state.pending_bytes(), 0);
    }
}

#[test]
fn accepted_prefixes_after_an_odd_boundary_match_fresh_state() {
    let s = scores(7);
    let x = latents(7);
    for accepted in 0..=6 {
        let mut state = state(CompressionKind::C2);
        let first = state
            .prepare(
                0,
                Some(&s.narrow(0, 0, 1).unwrap()),
                &x.narrow(0, 0, 1).unwrap(),
                &CancellationToken::new(),
            )
            .unwrap();
        state
            .commit_prefix(first, 1, &CancellationToken::new())
            .unwrap();
        let proposal = state
            .prepare(
                1,
                Some(&s.narrow(0, 1, 6).unwrap()),
                &x.narrow(0, 1, 6).unwrap(),
                &CancellationToken::new(),
            )
            .unwrap();
        let committed = state
            .commit_prefix(proposal, accepted, &CancellationToken::new())
            .unwrap();
        assert_eq!(committed.emitted_positions().len(), (1 + accepted) / 2);
        if let Some(tensor) = committed.emissions() {
            assert_eq!(storage_elements(tensor), ((1 + accepted) / 2) * 2);
        }
        let mut fresh = CompressionState::from_device(
            Device::Cpu,
            InferenceLimits::default(),
            CompressionKind::C2,
            2,
            16,
            8,
        )
        .unwrap();
        let prefix = fresh
            .prepare(
                0,
                Some(&s.narrow(0, 0, 1 + accepted).unwrap()),
                &x.narrow(0, 0, 1 + accepted).unwrap(),
                &CancellationToken::new(),
            )
            .unwrap();
        fresh
            .commit_prefix(prefix, 1 + accepted, &CancellationToken::new())
            .unwrap();
        let probe_s = Tensor::zeros((1, 2), DType::F32, &Device::Cpu).unwrap();
        let probe_x = Tensor::ones((1, 2), DType::F32, &Device::Cpu).unwrap();
        let actual = state
            .prepare(
                state.position(),
                Some(&probe_s),
                &probe_x,
                &CancellationToken::new(),
            )
            .unwrap();
        let expected = fresh
            .prepare(
                fresh.position(),
                Some(&probe_s),
                &probe_x,
                &CancellationToken::new(),
            )
            .unwrap();
        assert_eq!(
            actual.emissions().map(values),
            expected.emissions().map(values)
        );
        if let Some(pending) = &state.pending {
            assert_eq!(storage_elements(&pending.score), 2);
            assert_eq!(storage_elements(&pending.latent), 2);
        }
    }
}

#[test]
fn pending_rows_own_only_their_row_and_proposals_snapshot_mutable_inputs() {
    let mut state = state(CompressionKind::C2);
    let source = Var::from_tensor(&latents(8)).unwrap();
    let proposal = state
        .prepare(
            0,
            Some(&scores(8)),
            source.as_tensor(),
            &CancellationToken::new(),
        )
        .unwrap();
    source
        .set(&Tensor::full(99_f32, (8, 2), &Device::Cpu).unwrap())
        .unwrap();
    state
        .commit_prefix(proposal, 1, &CancellationToken::new())
        .unwrap();
    let pending = state.pending.as_ref().unwrap();
    assert_eq!(values(&pending.latent), vec![2., 4.]);
    assert_eq!(storage_elements(&pending.score), 2);
    assert_eq!(storage_elements(&pending.latent), 2);
    assert_eq!(pending.latent.storage_and_layout().1.start_offset(), 0);
}

#[test]
fn sliced_input_does_not_retain_the_larger_backing_storage() {
    let state = state(CompressionKind::C2);
    let large = Tensor::ones((128, 2), DType::F32, &Device::Cpu).unwrap();
    let view = large.narrow(0, 50, 4).unwrap();
    let proposal = state
        .prepare(0, Some(&view), &view, &CancellationToken::new())
        .unwrap();
    assert_eq!(storage_elements(&proposal.latents), 8);
    assert_eq!(storage_elements(proposal.scores.as_ref().unwrap()), 8);
}

#[test]
fn one_outstanding_proposal_and_cancelled_or_dropped_work_release_the_lease() {
    let mut state = state(CompressionKind::C2);
    let s = scores(1);
    let x = latents(1);
    let proposal = state
        .prepare(0, Some(&s), &x, &CancellationToken::new())
        .unwrap();
    assert!(state
        .prepare(0, Some(&s), &x, &CancellationToken::new())
        .is_err());
    drop(proposal);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(state.prepare(0, Some(&s), &x, &cancelled).is_err());
    let proposal = state
        .prepare(0, Some(&s), &x, &CancellationToken::new())
        .unwrap();
    assert!(state.commit_prefix(proposal, 1, &cancelled).is_err());
    assert_eq!(state.position(), 0);
    assert!(state
        .prepare(0, Some(&s), &x, &CancellationToken::new())
        .is_ok());
}

#[test]
fn foreign_stale_and_oversized_acceptance_refuse_before_mutation() {
    let mut state = state(CompressionKind::C2);
    let other = CompressionState::from_device(
        Device::Cpu,
        InferenceLimits::default(),
        CompressionKind::C2,
        2,
        16,
        8,
    )
    .unwrap();
    let proposal = other
        .prepare(0, Some(&scores(2)), &latents(2), &CancellationToken::new())
        .unwrap();
    assert!(state
        .commit_prefix(proposal, 2, &CancellationToken::new())
        .is_err());
    let proposal = state
        .prepare(0, Some(&scores(2)), &latents(2), &CancellationToken::new())
        .unwrap();
    state.generation += 1;
    assert!(state
        .commit_prefix(proposal, 2, &CancellationToken::new())
        .is_err());
    let proposal = state
        .prepare(0, Some(&scores(2)), &latents(2), &CancellationToken::new())
        .unwrap();
    assert!(state
        .commit_prefix(proposal, 3, &CancellationToken::new())
        .is_err());
    assert_eq!(state.position(), 0);
    assert_eq!(state.pending_bytes(), 0);
}

#[test]
fn input_format_position_numeric_and_budget_errors_cannot_change_state() {
    let state = state(CompressionKind::C2);
    let s = scores(2);
    let x = latents(2);
    assert!(state
        .prepare(1, Some(&s), &x, &CancellationToken::new())
        .is_err());
    assert!(state
        .prepare(usize::MAX, Some(&s), &x, &CancellationToken::new())
        .is_err());
    assert!(state
        .prepare(0, None, &x, &CancellationToken::new())
        .is_err());
    assert!(state
        .prepare(
            0,
            Some(&s.narrow(0, 0, 1).unwrap()),
            &x,
            &CancellationToken::new()
        )
        .is_err());
    assert!(state
        .prepare(
            0,
            Some(&s),
            &x.to_dtype(DType::BF16).unwrap(),
            &CancellationToken::new()
        )
        .is_err());
    for value in [f32::NAN, f32::INFINITY] {
        let bad = Tensor::full(value, (2, 2), &Device::Cpu).unwrap();
        assert!(state
            .prepare(0, Some(&bad), &x, &CancellationToken::new())
            .is_err());
        assert!(state
            .prepare(0, Some(&s), &bad, &CancellationToken::new())
            .is_err());
    }
    assert_eq!(state.position(), 0);
    assert!(state
        .prepare(0, Some(&s), &x, &CancellationToken::new())
        .is_ok());
    let mut limits = InferenceLimits::default();
    limits.max_state_bytes = step_peak_bytes(CompressionKind::C2, 2, 8).unwrap() - 1;
    assert!(
        CompressionState::from_device(Device::Cpu, limits, CompressionKind::C2, 2, 16, 8).is_err()
    );
    for (width, context, step) in [
        (0, 16, 8),
        (513, 16, 8),
        (2, 0, 8),
        (2, 16, 0),
        (2, 16, 17),
        (2, MAX_CONTEXT_TOKENS + 1, 8),
    ] {
        assert!(CompressionState::from_device(
            Device::Cpu,
            InferenceLimits::default(),
            CompressionKind::C2,
            width,
            context,
            step
        )
        .is_err());
    }
}

#[test]
fn extreme_finite_scores_use_stable_softmax_and_bf16_overflow_refuses() {
    let mut state = state(CompressionKind::C2);
    let s = Tensor::new(&[[1000_f32, -1000.], [-1000., 1000.]], &Device::Cpu).unwrap();
    let proposal = state
        .prepare(0, Some(&s), &latents(2), &CancellationToken::new())
        .unwrap();
    assert_eq!(values(proposal.emissions().unwrap()), vec![2., 8.]);
    state
        .commit_prefix(proposal, 2, &CancellationToken::new())
        .unwrap();
    let bad = Tensor::full(f32::MAX, (2, 2), &Device::Cpu).unwrap();
    assert!(state
        .prepare(2, Some(&scores(2)), &bad, &CancellationToken::new())
        .is_err());
    assert_eq!(state.position(), 2);
}
