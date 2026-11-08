use super::*;

fn matrix(rows: &[&[f64]]) -> ReferenceMatrix {
    ReferenceMatrix::new(
        rows.len(),
        rows[0].len(),
        rows.iter().flat_map(|row| row.iter().copied()).collect(),
    )
    .unwrap()
}

fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!((actual - expected).abs() <= 1e-12 + 1e-12 * expected.abs());
    }
}

#[test]
fn compression_kind_is_closed_and_round_trips() {
    for kind in [ReferenceCompressionKind::C1, ReferenceCompressionKind::C2] {
        assert_eq!(
            ReferenceCompressionKind::parse(kind.as_str()).unwrap(),
            kind
        );
    }
    for name in ["", "C2", "c0", "c4", "2"] {
        assert!(ReferenceCompressionKind::parse(name).is_err());
    }
}

#[test]
fn c2_uses_per_channel_two_token_weights() {
    let state = ReferenceCompressionState::new(ReferenceCompressionKind::C2, 2, 8).unwrap();
    let scores = matrix(&[&[0., 3.0_f64.ln()], &[3.0_f64.ln(), 0.]]);
    let latents = matrix(&[&[2., 10.], &[6., 2.]]);
    let step = state.extend(0, Some(&scores), &latents).unwrap();
    assert_eq!(step.emissions[0], None);
    close(step.emissions[1].as_ref().unwrap(), &[5., 8.]);
    close(step.state.compressed_values(), &[5., 8.]);
    assert_eq!((state.position(), state.compressed_rows()), (0, 0));
    assert_eq!(
        (step.state.position(), step.state.compressed_rows()),
        (2, 1)
    );
}

#[test]
fn c1_emits_every_latent_without_a_score_or_extra_rounding() {
    let state = ReferenceCompressionState::new(ReferenceCompressionKind::C1, 2, 8).unwrap();
    let latents = matrix(&[&[1.25, -2.], &[4., 6.], &[7., 8.]]);
    let step = state.extend(0, None, &latents).unwrap();
    assert_eq!(step.state.compressed_values(), latents.values());
    assert_eq!(step.emissions.iter().filter(|row| row.is_some()).count(), 3);
    let prefix = step.state.prefix(1).unwrap();
    assert_eq!(prefix.compressed_values(), [1.25, -2.]);
}

#[test]
fn all_prefill_chunk_partitions_equal_decode_including_odd_boundaries() {
    let scores = matrix(&[
        &[0., 1.],
        &[2., 3.],
        &[3., -3.],
        &[-1., 2.],
        &[6., -2.],
        &[0., 1.],
    ]);
    let latents = matrix(&[
        &[1., 9.],
        &[3., 2.],
        &[5., 4.],
        &[7., 8.],
        &[11., 10.],
        &[13., 12.],
    ]);
    for kind in [ReferenceCompressionKind::C1, ReferenceCompressionKind::C2] {
        let initial = ReferenceCompressionState::new(kind, 2, 8).unwrap();
        let expected = initial
            .extend(
                0,
                (kind == ReferenceCompressionKind::C2).then_some(&scores),
                &latents,
            )
            .unwrap();
        // Every possible split pattern of the six tokens, including decode.
        for split_mask in 0..32 {
            let mut state = initial.clone();
            let mut emissions = Vec::new();
            let mut start = 0;
            for end in 1..=6 {
                if end != 6 && split_mask & (1 << (end - 1)) == 0 {
                    continue;
                }
                let s = ReferenceMatrix::new(
                    end - start,
                    2,
                    scores.values()[2 * start..2 * end].to_vec(),
                )
                .unwrap();
                let l = ReferenceMatrix::new(
                    end - start,
                    2,
                    latents.values()[2 * start..2 * end].to_vec(),
                )
                .unwrap();
                let step = state
                    .extend(
                        start,
                        (kind == ReferenceCompressionKind::C2).then_some(&s),
                        &l,
                    )
                    .unwrap();
                emissions.extend(step.emissions);
                state = step.state;
                start = end;
            }
            assert_eq!(state, expected.state);
            assert_eq!(emissions, expected.emissions);
        }
    }
}

#[test]
fn partial_acceptance_removes_future_blocks_and_preserves_pending_pair() {
    let initial = ReferenceCompressionState::new(ReferenceCompressionKind::C2, 1, 8).unwrap();
    let scores = matrix(&[&[0.], &[0.], &[0.], &[0.]]);
    let latents = matrix(&[&[2.], &[6.], &[10.], &[90.]]);
    let computed = initial.extend(0, Some(&scores), &latents).unwrap().state;
    assert_eq!(computed.compressed_values(), [4., 50.]);
    let accepted = computed.prefix(3).unwrap();
    assert_eq!(accepted.compressed_values(), [4.]);
    let next_score = matrix(&[&[0.]]);
    let corrected = accepted
        .extend(3, Some(&next_score), &matrix(&[&[14.]]))
        .unwrap();
    assert_eq!(corrected.state.compressed_values(), [4., 12.]);
    assert_eq!(computed.compressed_values(), [4., 50.]);
    let rejected = computed.prefix(0).unwrap();
    assert_eq!(rejected, initial);
    assert_eq!(computed.prefix(4).unwrap(), computed);
    assert!(computed.prefix(5).is_err());
}

#[test]
fn every_accepted_prefix_equals_a_fresh_computation() {
    let scores = matrix(&[&[0.], &[1.], &[2.], &[3.], &[4.]]);
    let latents = matrix(&[&[1.], &[3.], &[5.], &[7.], &[9.]]);
    for kind in [ReferenceCompressionKind::C1, ReferenceCompressionKind::C2] {
        let initial = ReferenceCompressionState::new(kind, 1, 8).unwrap();
        let full = initial
            .extend(
                0,
                (kind == ReferenceCompressionKind::C2).then_some(&scores),
                &latents,
            )
            .unwrap()
            .state;
        for accepted in 1..=5 {
            let s =
                ReferenceMatrix::new(accepted, 1, scores.values()[..accepted].to_vec()).unwrap();
            let l =
                ReferenceMatrix::new(accepted, 1, latents.values()[..accepted].to_vec()).unwrap();
            let fresh = initial
                .extend(0, (kind == ReferenceCompressionKind::C2).then_some(&s), &l)
                .unwrap()
                .state;
            assert_eq!(full.prefix(accepted).unwrap(), fresh);
        }
    }
}

#[test]
fn shape_position_capacity_and_kind_errors_leave_state_unchanged() {
    let state = ReferenceCompressionState::new(ReferenceCompressionKind::C2, 2, 2).unwrap();
    let good = matrix(&[&[1., 2.]]);
    let wrong = matrix(&[&[1.]]);
    assert!(state.extend(1, Some(&good), &good).is_err());
    assert!(state.extend(0, None, &good).is_err());
    assert!(state.extend(0, Some(&wrong), &good).is_err());
    assert!(state.extend(0, Some(&good), &wrong).is_err());
    let full = state
        .extend(
            0,
            Some(&matrix(&[&[0., 0.], &[0., 0.]])),
            &matrix(&[&[1., 2.], &[3., 4.]]),
        )
        .unwrap()
        .state;
    assert!(full.extend(2, Some(&good), &good).is_err());
    let c1 = ReferenceCompressionState::new(ReferenceCompressionKind::C1, 2, 2).unwrap();
    assert!(c1.extend(0, Some(&good), &good).is_err());
    assert_eq!(state.position(), 0);
}

#[test]
fn memory_budget_is_checked_before_allocating_history() {
    for (width, tokens) in [(0, 1), (1, 0), (usize::MAX, 2), (2, 1024)] {
        assert!(
            ReferenceCompressionState::new(ReferenceCompressionKind::C2, width, tokens).is_err()
        );
    }
    assert!(ReferenceCompressionState::new(
        ReferenceCompressionKind::C1,
        1,
        MAX_REFERENCE_ELEMENTS
    )
    .is_ok());
    assert!(ReferenceCompressionState::new(
        ReferenceCompressionKind::C2,
        1,
        MAX_REFERENCE_ELEMENTS / 3
    )
    .is_ok());
}

#[test]
fn arithmetic_failure_in_late_row_cannot_commit_earlier_rows() {
    let state = ReferenceCompressionState::new(ReferenceCompressionKind::C2, 1, 8).unwrap();
    // Extreme score subtraction is outside the finite F64 reference domain.
    let scores = matrix(&[&[0.], &[0.], &[-f64::MAX], &[f64::MAX]]);
    let latents = matrix(&[&[1.], &[3.], &[5.], &[7.]]);
    assert!(state.extend(0, Some(&scores), &latents).is_err());
    assert_eq!(state.position(), 0);
    assert_eq!(state.compressed_rows(), 0);
}
