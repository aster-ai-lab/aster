use super::*;
use candle_core::{CpuStorage, Storage, Var};

fn state(kind: CompressionKind) -> IndexKvState {
    IndexKvState::from_device(Device::Cpu, InferenceLimits::default(), kind, 64, 32).unwrap()
}
fn encoded(
    kind: CompressionKind,
    start: usize,
    tokens: usize,
    salt: usize,
) -> (Vec<usize>, Tensor, Tensor) {
    let ready: Vec<_> = (start..start + tokens)
        .filter(|p| (p + 1) % kind.ratio() == 0)
        .collect();
    let data: Vec<_> = ready
        .iter()
        .flat_map(|p| vec![((p + salt) % 256) as u8; DATA_BYTES])
        .collect();
    let scales: Vec<_> = ready
        .iter()
        .flat_map(|p| vec![(127 + (p + salt) % 4) as u8; SCALE_BYTES])
        .collect();
    (
        ready.clone(),
        Tensor::from_vec(data, (ready.len(), DATA_BYTES), &Device::Cpu).unwrap(),
        Tensor::from_vec(scales, (ready.len(), SCALE_BYTES), &Device::Cpu).unwrap(),
    )
}
fn feed(state: &mut IndexKvState, tokens: usize, accepted: usize, salt: usize) {
    let (ready, data, scales) = encoded(state.kind, state.position, tokens, salt);
    let proposal = state
        .prepare(
            state.position,
            tokens,
            &ready,
            &data,
            &scales,
            &CancellationToken::new(),
        )
        .unwrap();
    state
        .commit_prefix(proposal, accepted, &CancellationToken::new())
        .unwrap();
}
fn bytes(state: &IndexKvState) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    (
        state.data.to_vec2().unwrap(),
        state.scales.to_vec2().unwrap(),
    )
}

#[test]
fn every_eight_token_partition_keeps_exact_encoded_bytes_and_readiness() {
    for kind in [CompressionKind::C1, CompressionKind::C2] {
        let mut full = state(kind);
        feed(&mut full, 8, 8, 0);
        for mask in 0..128 {
            let mut split = state(kind);
            let mut previous = 0;
            for end in 1..=8 {
                if end == 8 || mask & (1 << (end - 1)) != 0 {
                    feed(&mut split, end - previous, end - previous, 0);
                    previous = end;
                }
            }
            assert_eq!(bytes(&split), bytes(&full));
            assert_eq!(split.position(), 8);
            assert_eq!(
                split.ready_positions().collect::<Vec<_>>(),
                full.ready_positions().collect::<Vec<_>>()
            );
            assert_eq!(split.retained_bytes(), (8 / kind.ratio() * 68) as u64);
        }
    }
}

#[test]
fn accepted_prefixes_from_odd_position_equal_only_accepted_history() {
    for accepted in 0..=5 {
        let mut actual = state(CompressionKind::C2);
        feed(&mut actual, 1, 1, 0);
        feed(&mut actual, 5, accepted, 0);
        let mut expected = state(CompressionKind::C2);
        feed(&mut expected, 1, 1, 0);
        if accepted > 0 {
            feed(&mut expected, accepted, accepted, 0);
        }
        assert_eq!(actual.position(), 1 + accepted);
        assert_eq!(bytes(&actual), bytes(&expected));
        assert_eq!(
            actual.ready_positions().collect::<Vec<_>>(),
            expected.ready_positions().collect::<Vec<_>>()
        );
    }
}

#[test]
fn rejected_rows_are_absent_from_new_continuations_and_backing_storage() {
    for kind in [CompressionKind::C1, CompressionKind::C2] {
        let mut actual = state(kind);
        feed(&mut actual, 12, 3, 0);
        feed(&mut actual, 8, 8, 70);
        let mut expected = state(kind);
        feed(&mut expected, 3, 3, 0);
        feed(&mut expected, 8, 8, 70);
        assert_eq!(bytes(&actual), bytes(&expected));
        for (tensor, width) in [(&actual.data, DATA_BYTES), (&actual.scales, SCALE_BYTES)] {
            let (storage, layout) = tensor.storage_and_layout();
            let Storage::Cpu(CpuStorage::U8(raw)) = &*storage else {
                panic!("expected raw bytes")
            };
            assert_eq!(raw.len(), actual.row_count() * width);
            assert_eq!(layout.start_offset(), 0);
        }
    }
}

#[test]
fn prepare_snapshots_both_mutable_sections_and_read_views_own_separate_rows() {
    let mut actual = state(CompressionKind::C1);
    let (ready, data, scales) = encoded(actual.kind, 0, 3, 0);
    let expected = (
        data.to_vec2::<u8>().unwrap(),
        scales.to_vec2::<u8>().unwrap(),
    );
    let data = Var::from_tensor(&data).unwrap();
    let scales = Var::from_tensor(&scales).unwrap();
    let proposal = actual
        .prepare(
            0,
            3,
            &ready,
            data.as_tensor(),
            scales.as_tensor(),
            &CancellationToken::new(),
        )
        .unwrap();
    data.set(&Tensor::zeros((3, 64), DType::U8, &Device::Cpu).unwrap())
        .unwrap();
    scales
        .set(&Tensor::ones((3, 4), DType::U8, &Device::Cpu).unwrap())
        .unwrap();
    actual
        .commit_prefix(proposal, 3, &CancellationToken::new())
        .unwrap();
    assert_eq!(bytes(&actual), expected);
    let view = Fp4IndexRowsData::new(&actual.limits, &actual.data, &actual.scales).unwrap();
    assert_eq!(view.retained_bytes(), 204);
    assert_eq!(
        view.decode_prefix(3, &actual.limits).unwrap().dims(),
        [3, 128]
    );
}

#[test]
fn lease_drop_bad_scales_and_cancellation_leave_previous_state_unchanged() {
    let mut actual = state(CompressionKind::C2);
    feed(&mut actual, 2, 2, 0);
    let before = bytes(&actual);
    let (ready, data, scales) = encoded(actual.kind, 2, 4, 0);
    let proposal = actual
        .prepare(2, 4, &ready, &data, &scales, &CancellationToken::new())
        .unwrap();
    assert!(actual
        .prepare(2, 4, &ready, &data, &scales, &CancellationToken::new())
        .is_err());
    drop(proposal);
    for code in [0_u8, 255] {
        let invalid = Tensor::full(code, (2, 4), &Device::Cpu).unwrap();
        assert!(actual
            .prepare(2, 4, &ready, &data, &invalid, &CancellationToken::new())
            .is_err());
    }
    let proposal = actual
        .prepare(2, 4, &ready, &data, &scales, &CancellationToken::new())
        .unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        actual.commit_prefix(proposal, 2, &cancellation),
        Err(aster_power::error::PowerError::InferenceCancelled)
    ));
    assert_eq!(actual.position(), 2);
    assert_eq!(bytes(&actual), before);
    assert!(actual
        .prepare(2, 4, &ready, &data, &scales, &CancellationToken::new())
        .is_ok());
}

#[test]
fn foreign_stale_wrong_readiness_and_invalid_prefix_never_mutate_committed_bytes() {
    let mut actual = state(CompressionKind::C1);
    let other = state(CompressionKind::C1);
    let (ready, data, scales) = encoded(actual.kind, 0, 3, 0);
    let foreign = other
        .prepare(0, 3, &ready, &data, &scales, &CancellationToken::new())
        .unwrap();
    assert!(actual
        .commit_prefix(foreign, 2, &CancellationToken::new())
        .is_err());
    let mut stale = actual
        .prepare(0, 3, &ready, &data, &scales, &CancellationToken::new())
        .unwrap();
    stale.generation += 1;
    assert!(actual
        .commit_prefix(stale, 2, &CancellationToken::new())
        .is_err());
    let too_many = actual
        .prepare(0, 3, &ready, &data, &scales, &CancellationToken::new())
        .unwrap();
    assert!(actual
        .commit_prefix(too_many, 4, &CancellationToken::new())
        .is_err());
    assert!(actual
        .prepare(0, 3, &[1, 2, 3], &data, &scales, &CancellationToken::new())
        .is_err());
    assert_eq!(actual.position(), 0);
    assert_eq!(actual.row_count(), 0);
}

#[test]
fn index_state_refuses_invalid_capacity_shapes_and_tiny_memory_budget() {
    for (context, step) in [(0, 1), (8, 0), (8, 9), (MAX_CONTEXT_TOKENS + 1, 1)] {
        assert!(IndexKvState::from_device(
            Device::Cpu,
            InferenceLimits::default(),
            CompressionKind::C1,
            context,
            step
        )
        .is_err());
    }
    let mut limits = InferenceLimits::default();
    limits.max_state_bytes = 1;
    assert!(IndexKvState::from_device(Device::Cpu, limits, CompressionKind::C1, 8, 4).is_err());
    let actual = state(CompressionKind::C1);
    let (ready, data, scales) = encoded(actual.kind, 0, 2, 0);
    assert!(actual
        .prepare(
            0,
            2,
            &ready,
            &data.to_dtype(DType::F32).unwrap(),
            &scales,
            &CancellationToken::new()
        )
        .is_err());
    assert!(actual
        .prepare(
            0,
            2,
            &ready,
            &data,
            &scales.narrow(0, 0, 1).unwrap(),
            &CancellationToken::new()
        )
        .is_err());
}
