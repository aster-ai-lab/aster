use super::*;
use candle_core::{CpuStorage, Storage, Var};

fn state(kind: KvCacheKind, format: MainKvFormat, context: usize, step: usize) -> KvState {
    KvState::from_device(
        Device::Cpu,
        InferenceLimits::default(),
        kind,
        format,
        context,
        step,
    )
    .unwrap()
}

fn encoded(
    kind: KvCacheKind,
    format: MainKvFormat,
    start: usize,
    tokens: usize,
) -> (Vec<usize>, Tensor) {
    let ready: Vec<_> = (start..start + tokens)
        .filter(|position| kind != KvCacheKind::C2 || position % 2 == 1)
        .collect();
    let mut records = Vec::new();
    for position in &ready {
        if format == MainKvFormat::Fp8Ue8m0 {
            records.extend(vec![(position % 120 + 1) as u8; 512]);
            records.extend(vec![127_u8; 16]);
        } else {
            let nibble = (position % 8) as u8;
            records.extend(vec![nibble | nibble << 4; 256]);
            records.extend(vec![56_u8; 32]);
        }
    }
    let tensor = Tensor::from_vec(
        records,
        (ready.len(), format.bytes_per_token()),
        &Device::Cpu,
    )
    .unwrap();
    (ready, tensor)
}

fn feed(state: &mut KvState, tokens: usize, accepted: usize) {
    let (ready, rows) = encoded(state.kind, state.format, state.position, tokens);
    let proposal = state
        .prepare(
            state.position,
            tokens,
            &ready,
            &rows,
            &CancellationToken::new(),
        )
        .unwrap();
    state
        .commit_prefix(proposal, accepted, &CancellationToken::new())
        .unwrap();
}

fn records(state: &KvState) -> Vec<Vec<u8>> {
    state.records.to_vec2().unwrap()
}

#[test]
fn staged_apply_holds_old_tensor_until_the_guard_is_retired() {
    let mut actual = state(KvCacheKind::C1, MainKvFormat::Fp8Ue8m0, 16, 8);
    feed(&mut actual, 2, 2);
    let old = records(&actual);
    let (ready, rows) = encoded(actual.kind, actual.format, 2, 4);
    let cancellation = CancellationToken::new();
    let proposal = actual.prepare(2, 4, &ready, &rows, &cancellation).unwrap();
    let mut staged = actual.stage_prefix(proposal, 3, &cancellation).unwrap();
    staged.apply();
    assert_eq!(staged.state.position(), 5);
    assert_eq!(
        staged.records.as_ref().unwrap().to_vec2::<u8>().unwrap(),
        old
    );
    assert_eq!(staged.records.as_ref().unwrap().dims(), [2, 528]);
    drop(staged);
    assert_eq!(actual.row_count(), 5);
}

#[test]
fn kinds_and_policy_are_closed_and_window_always_requires_fp8() {
    for kind in [KvCacheKind::Window, KvCacheKind::C1, KvCacheKind::C2] {
        assert_eq!(KvCacheKind::parse(kind.as_str()).unwrap(), kind);
    }
    for bad in ["", "window", "c3", "auto"] {
        assert!(KvCacheKind::parse(bad).is_err());
    }
    for (kind, format, context, step) in [
        (KvCacheKind::Window, MainKvFormat::Fp4E4m3, 8, 4),
        (KvCacheKind::C1, MainKvFormat::Fp4E4m3, 0, 4),
        (KvCacheKind::C1, MainKvFormat::Fp4E4m3, 8, 0),
        (KvCacheKind::C1, MainKvFormat::Fp4E4m3, 8, 9),
        (
            KvCacheKind::C1,
            MainKvFormat::Fp4E4m3,
            MAX_CONTEXT_TOKENS + 1,
            1,
        ),
    ] {
        assert!(KvState::from_device(
            Device::Cpu,
            InferenceLimits::default(),
            kind,
            format,
            context,
            step
        )
        .is_err());
    }
    let tiny = InferenceLimits {
        max_state_bytes: 1,
        ..InferenceLimits::default()
    };
    assert!(KvState::from_device(
        Device::Cpu,
        tiny,
        KvCacheKind::C1,
        MainKvFormat::Fp4E4m3,
        8,
        4
    )
    .is_err());
}

#[test]
fn every_chunk_partition_matches_single_prefill_bytes_and_ready_positions() {
    for (kind, format) in [
        (KvCacheKind::Window, MainKvFormat::Fp8Ue8m0),
        (KvCacheKind::C1, MainKvFormat::Fp4E4m3),
        (KvCacheKind::C2, MainKvFormat::Fp4E4m3),
        (KvCacheKind::C1, MainKvFormat::Fp8Ue8m0),
    ] {
        let mut baseline = state(kind, format, 8, 8);
        feed(&mut baseline, 8, 8);
        for mask in 0..128 {
            let mut candidate = state(kind, format, 8, 8);
            let mut start = 0;
            for end in 1..=8 {
                if end == 8 || mask & (1 << (end - 1)) != 0 {
                    feed(&mut candidate, end - start, end - start);
                    start = end;
                }
            }
            assert_eq!(candidate.position(), 8);
            assert_eq!(records(&candidate), records(&baseline));
            assert_eq!(
                candidate.ready_positions().collect::<Vec<_>>(),
                baseline.ready_positions().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn every_accepted_prefix_after_odd_c2_position_matches_fresh_history() {
    for accepted in 0..=5 {
        let mut candidate = state(KvCacheKind::C2, MainKvFormat::Fp4E4m3, 8, 8);
        feed(&mut candidate, 1, 1);
        let (ready, rows) = encoded(KvCacheKind::C2, MainKvFormat::Fp4E4m3, 1, 5);
        let proposal = candidate
            .prepare(1, 5, &ready, &rows, &CancellationToken::new())
            .unwrap();
        assert_eq!(
            proposal.ready_positions().collect::<Vec<_>>(),
            vec![1, 3, 5]
        );
        candidate
            .commit_prefix(proposal, accepted, &CancellationToken::new())
            .unwrap();
        let mut expected = state(KvCacheKind::C2, MainKvFormat::Fp4E4m3, 8, 8);
        feed(&mut expected, 1 + accepted, 1 + accepted);
        assert_eq!(candidate.position(), 1 + accepted);
        assert_eq!(records(&candidate), records(&expected));
        assert_eq!(
            candidate.ready_positions().collect::<Vec<_>>(),
            expected.ready_positions().collect::<Vec<_>>()
        );
    }
}

#[test]
fn window_proposal_keeps_prefill_rows_but_commit_owns_only_last_128_accepted_rows() {
    let mut candidate = state(KvCacheKind::Window, MainKvFormat::Fp8Ue8m0, 192, 160);
    let (ready, rows) = encoded(candidate.kind, candidate.format, 0, 160);
    let proposal = candidate
        .prepare(0, 160, &ready, &rows, &CancellationToken::new())
        .unwrap();
    assert_eq!(candidate.row_count(), 0);
    assert_eq!(proposal.row_count(), 160);
    assert_eq!(
        proposal.ready_positions().collect::<Vec<_>>(),
        (0..160).collect::<Vec<_>>()
    );
    candidate
        .commit_prefix(proposal, 129, &CancellationToken::new())
        .unwrap();
    assert_eq!(candidate.row_count(), 128);
    assert_eq!(
        candidate.ready_positions().collect::<Vec<_>>(),
        (1..129).collect::<Vec<_>>()
    );
    feed(&mut candidate, 10, 3);
    assert_eq!(candidate.position(), 132);
    assert_eq!(
        candidate.ready_positions().collect::<Vec<_>>(),
        (4..132).collect::<Vec<_>>()
    );
    let (storage, layout) = candidate.records.storage_and_layout();
    match &*storage {
        Storage::Cpu(CpuStorage::U8(bytes)) => assert_eq!(bytes.len(), 128 * 528),
        _ => panic!("expected CPU byte fixture"),
    }
    assert_eq!(layout.start_offset(), 0);
    assert_eq!(candidate.retained_bytes(), (128 * 528) as u64);
}

#[test]
fn rejected_bytes_cannot_reappear_in_a_different_continuation() {
    let mut candidate = state(KvCacheKind::C1, MainKvFormat::Fp8Ue8m0, 8, 8);
    feed(&mut candidate, 5, 2);
    let (ready, rows) = encoded(candidate.kind, candidate.format, 2, 2);
    let mut replacement = rows.to_vec2::<u8>().unwrap();
    for row in &mut replacement {
        row[..512].fill(64);
    }
    let replacement = Tensor::from_vec(
        replacement.into_iter().flatten().collect::<Vec<_>>(),
        (2, 528),
        &Device::Cpu,
    )
    .unwrap();
    let proposal = candidate
        .prepare(2, 2, &ready, &replacement, &CancellationToken::new())
        .unwrap();
    candidate
        .commit_prefix(proposal, 2, &CancellationToken::new())
        .unwrap();
    assert!(records(&candidate)[2..]
        .iter()
        .all(|row| row[..512].iter().all(|byte| *byte == 64)));
    let (storage, _) = candidate.records.storage_and_layout();
    match &*storage {
        Storage::Cpu(CpuStorage::U8(bytes)) => assert_eq!(bytes.len(), 4 * 528),
        _ => panic!("expected CPU byte fixture"),
    }
}

#[test]
fn prepare_snapshots_mutable_encoded_input_before_accepted_commit() {
    let mut candidate = state(KvCacheKind::C1, MainKvFormat::Fp4E4m3, 8, 8);
    let (ready, rows) = encoded(candidate.kind, candidate.format, 0, 3);
    let expected = rows.to_vec2::<u8>().unwrap();
    let variable = Var::from_tensor(&rows).unwrap();
    let proposal = candidate
        .prepare(
            0,
            3,
            &ready,
            variable.as_tensor(),
            &CancellationToken::new(),
        )
        .unwrap();
    variable
        .set(&Tensor::zeros((3, 288), DType::U8, &Device::Cpu).unwrap())
        .unwrap();
    candidate
        .commit_prefix(proposal, 2, &CancellationToken::new())
        .unwrap();
    assert_eq!(records(&candidate), expected[..2]);
}

#[test]
fn drop_cancellation_and_bad_payload_release_the_one_proposal_lease() {
    let mut candidate = state(KvCacheKind::C1, MainKvFormat::Fp4E4m3, 8, 8);
    let (ready, rows) = encoded(candidate.kind, candidate.format, 0, 2);
    let proposal = candidate
        .prepare(0, 2, &ready, &rows, &CancellationToken::new())
        .unwrap();
    assert!(candidate
        .prepare(0, 2, &ready, &rows, &CancellationToken::new())
        .is_err());
    drop(proposal);
    let proposal = candidate
        .prepare(0, 2, &ready, &rows, &CancellationToken::new())
        .unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        candidate.commit_prefix(proposal, 1, &cancellation),
        Err(PowerError::InferenceCancelled)
    ));
    assert_eq!(candidate.position(), 0);
    let bad = Tensor::zeros((2, 288), DType::U8, &Device::Cpu).unwrap();
    assert!(candidate
        .prepare(0, 2, &ready, &bad, &CancellationToken::new())
        .is_err());
    feed(&mut candidate, 2, 2);
    assert_eq!(candidate.position(), 2);
}

#[test]
fn foreign_stale_wrong_schedule_and_oversized_acceptance_cannot_mutate_state() {
    let mut candidate = state(KvCacheKind::C1, MainKvFormat::Fp4E4m3, 8, 4);
    let foreign = state(KvCacheKind::C1, MainKvFormat::Fp4E4m3, 8, 4);
    let (ready, rows) = encoded(candidate.kind, candidate.format, 0, 2);
    let proposal = foreign
        .prepare(0, 2, &ready, &rows, &CancellationToken::new())
        .unwrap();
    assert!(candidate
        .commit_prefix(proposal, 1, &CancellationToken::new())
        .is_err());
    for (start, tokens, positions) in [
        (1, 2, ready.clone()),
        (0, 0, vec![]),
        (0, 5, vec![]),
        (0, 2, vec![1, 0]),
    ] {
        assert!(candidate
            .prepare(start, tokens, &positions, &rows, &CancellationToken::new())
            .is_err());
    }
    let proposal = candidate
        .prepare(0, 2, &ready, &rows, &CancellationToken::new())
        .unwrap();
    assert!(candidate
        .commit_prefix(proposal, 3, &CancellationToken::new())
        .is_err());
    let proposal = candidate
        .prepare(0, 2, &ready, &rows, &CancellationToken::new())
        .unwrap();
    candidate.generation += 1;
    assert!(candidate
        .commit_prefix(proposal, 1, &CancellationToken::new())
        .is_err());
    assert_eq!(candidate.position(), 0);
    assert_eq!(candidate.row_count(), 0);
    assert!(foreign
        .prepare(0, 2, &ready, &rows, &CancellationToken::new())
        .is_ok());
}
