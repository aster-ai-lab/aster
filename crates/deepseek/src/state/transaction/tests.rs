use super::*;
use crate::layers::{kv::MainKvFormat, CudaLayerDevice};
use crate::state::{kv::KvCacheKind, CompressionKind};
use aster_power::inference::InferenceLimits;
use candle_core::{DType, Tensor};

struct Bundle {
    device: CudaLayerDevice,
    compression: CompressionState,
    window: KvState,
    main: KvState,
    index: IndexKvState,
}
struct Proposals {
    compression: PreparedCompression,
    window: PreparedKv,
    main: PreparedKv,
    index: PreparedIndexKv,
}
fn bundle(kind: CompressionKind) -> Bundle {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    Bundle {
        compression: CompressionState::new(&device, kind, 2, 64, 32).unwrap(),
        window: KvState::new(&device, KvCacheKind::Window, MainKvFormat::Fp8Ue8m0, 64, 32).unwrap(),
        main: KvState::new(
            &device,
            if kind == CompressionKind::C1 {
                KvCacheKind::C1
            } else {
                KvCacheKind::C2
            },
            MainKvFormat::Fp4E4m3,
            64,
            32,
        )
        .unwrap(),
        index: IndexKvState::new(&device, kind, 64, 32).unwrap(),
        device,
    }
}
fn prepare(bundle: &Bundle, tokens: usize) -> Proposals {
    let cancel = CancellationToken::new();
    let start = bundle.compression.position();
    let kind = bundle.index.kind();
    let device = bundle.device.tensor_device();
    let latents: Vec<_> = (start..start + tokens)
        .flat_map(|p| [p as f32 + 0.25, p as f32 + 1.5])
        .collect();
    let latents = Tensor::from_vec(latents, (tokens, 2), device)
        .unwrap()
        .to_dtype(if kind == CompressionKind::C1 {
            DType::BF16
        } else {
            DType::F32
        })
        .unwrap();
    let scores = Tensor::zeros((tokens, 2), DType::F32, device).unwrap();
    let compression = bundle
        .compression
        .prepare(
            start,
            (kind == CompressionKind::C2).then_some(&scores),
            &latents,
            &cancel,
        )
        .unwrap();
    let positions: Vec<_> = (start..start + tokens).collect();
    let mut records = Vec::new();
    for p in &positions {
        records.extend(vec![(p % 60 + 1) as u8; 512]);
        records.extend(vec![127_u8; 16]);
    }
    let records = Tensor::from_vec(records, (tokens, 528), device).unwrap();
    let window = bundle
        .window
        .prepare(start, tokens, &positions, &records, &cancel)
        .unwrap();
    let ready: Vec<_> = positions
        .into_iter()
        .filter(|p| (p + 1) % kind.ratio() == 0)
        .collect();
    let mut records = Vec::new();
    for p in &ready {
        records.extend(vec![(p % 100 + 1) as u8; 256]);
        records.extend(vec![56_u8; 32]);
    }
    let records = Tensor::from_vec(records, (ready.len(), 288), device).unwrap();
    let main = bundle
        .main
        .prepare(start, tokens, &ready, &records, &cancel)
        .unwrap();
    let data: Vec<_> = ready
        .iter()
        .flat_map(|p| vec![(p % 100 + 1) as u8; 64])
        .collect();
    let data = Tensor::from_vec(data, (ready.len(), 64), device).unwrap();
    let scales = Tensor::full(127_u8, (ready.len(), 4), device).unwrap();
    let index = bundle
        .index
        .prepare(start, tokens, &ready, &data, &scales, &cancel)
        .unwrap();
    Proposals {
        compression,
        window,
        main,
        index,
    }
}
fn commit(
    bundle: &mut Bundle,
    tokens: usize,
    accepted: usize,
    atomic: bool,
) -> CommittedCompression {
    let start = bundle.compression.position();
    let proposals = prepare(bundle, tokens);
    let cancel = CancellationToken::new();
    if atomic {
        let mut batch = StateCommitBatch::new(start, tokens, accepted, 1 << 20).unwrap();
        batch
            .add_compression(&mut bundle.compression, proposals.compression, &cancel)
            .unwrap();
        batch
            .add_main_kv(&mut bundle.window, proposals.window, &cancel)
            .unwrap();
        batch
            .add_main_kv(&mut bundle.main, proposals.main, &cancel)
            .unwrap();
        batch
            .add_index_kv(&mut bundle.index, proposals.index, &cancel)
            .unwrap();
        batch.commit(&cancel).unwrap().remove(0)
    } else {
        let result = bundle
            .compression
            .commit_prefix(proposals.compression, accepted, &cancel)
            .unwrap();
        bundle
            .window
            .commit_prefix(proposals.window, accepted, &cancel)
            .unwrap();
        bundle
            .main
            .commit_prefix(proposals.main, accepted, &cancel)
            .unwrap();
        bundle
            .index
            .commit_prefix(proposals.index, accepted, &cancel)
            .unwrap();
        result
    }
}
fn snapshot(
    bundle: &Bundle,
) -> (
    Vec<usize>,
    Vec<Vec<u8>>,
    Vec<Vec<u8>>,
    Vec<Vec<u8>>,
    Vec<Vec<u8>>,
    Option<Vec<f32>>,
) {
    let positions = vec![
        bundle.compression.position(),
        bundle.window.position(),
        bundle.main.position(),
        bundle.index.position(),
    ];
    let window = bundle
        .window
        .attention_rows(&bundle.device)
        .unwrap()
        .records()
        .to_vec2()
        .unwrap();
    let main = bundle
        .main
        .attention_rows(&bundle.device)
        .unwrap()
        .records()
        .to_vec2()
        .unwrap();
    let index = bundle.index.index_rows(&bundle.device).unwrap();
    let data = index.data_bytes().to_vec2().unwrap();
    let scales = index.scale_bytes().to_vec2().unwrap();
    let pending = bundle
        .compression
        .pending
        .as_ref()
        .map(|p| p.latent.flatten_all().unwrap().to_vec1().unwrap());
    (positions, window, main, data, scales, pending)
}

#[test]
fn atomic_and_individual_commits_match_every_prefix_after_an_odd_boundary() {
    for kind in [CompressionKind::C1, CompressionKind::C2] {
        for accepted in 0..=5 {
            let mut actual = bundle(kind);
            let mut expected = bundle(kind);
            commit(&mut actual, 1, 1, true);
            commit(&mut expected, 1, 1, false);
            let actual_result = commit(&mut actual, 5, accepted, true);
            let expected_result = commit(&mut expected, 5, accepted, false);
            assert_eq!(snapshot(&actual), snapshot(&expected));
            assert_eq!(
                actual_result.emitted_positions(),
                expected_result.emitted_positions()
            );
            assert_eq!(
                actual_result.emissions().map(|t| t
                    .to_dtype(DType::F32)
                    .unwrap()
                    .to_vec2::<f32>()
                    .unwrap()),
                expected_result.emissions().map(|t| t
                    .to_dtype(DType::F32)
                    .unwrap()
                    .to_vec2::<f32>()
                    .unwrap())
            );
        }
    }
}

#[test]
fn late_foreign_index_proposal_poisoning_prevents_any_partial_commit() {
    let mut actual = bundle(CompressionKind::C2);
    commit(&mut actual, 1, 1, true);
    let before = snapshot(&actual);
    let mut foreign = bundle(CompressionKind::C2);
    commit(&mut foreign, 1, 1, true);
    let Proposals {
        compression,
        window,
        main,
        index,
    } = prepare(&actual, 5);
    drop(index);
    let foreign_index = prepare(&foreign, 5).index;
    let cancel = CancellationToken::new();
    let mut batch = StateCommitBatch::new(1, 5, 3, 1 << 20).unwrap();
    batch
        .add_compression(&mut actual.compression, compression, &cancel)
        .unwrap();
    batch
        .add_main_kv(&mut actual.window, window, &cancel)
        .unwrap();
    batch.add_main_kv(&mut actual.main, main, &cancel).unwrap();
    assert!(batch
        .add_index_kv(&mut actual.index, foreign_index, &cancel)
        .is_err());
    assert!(batch.commit(&cancel).is_err());
    assert_eq!(snapshot(&actual), before);
    drop(prepare(&actual, 5));
}

#[test]
fn cancellation_after_all_staging_preserves_every_old_state_and_releases_leases() {
    let mut actual = bundle(CompressionKind::C1);
    commit(&mut actual, 2, 2, true);
    let before = snapshot(&actual);
    let proposals = prepare(&actual, 5);
    let cancel = CancellationToken::new();
    let mut batch = StateCommitBatch::new(2, 5, 4, 1 << 20).unwrap();
    batch
        .add_compression(&mut actual.compression, proposals.compression, &cancel)
        .unwrap();
    batch
        .add_main_kv(&mut actual.window, proposals.window, &cancel)
        .unwrap();
    batch
        .add_main_kv(&mut actual.main, proposals.main, &cancel)
        .unwrap();
    batch
        .add_index_kv(&mut actual.index, proposals.index, &cancel)
        .unwrap();
    cancel.cancel();
    assert!(matches!(
        batch.commit(&cancel),
        Err(aster_power::error::PowerError::InferenceCancelled)
    ));
    assert_eq!(snapshot(&actual), before);
    drop(prepare(&actual, 5));
}

#[test]
fn dropping_prepared_batch_cannot_publish_or_erase_the_pending_c2_half() {
    let mut actual = bundle(CompressionKind::C2);
    commit(&mut actual, 1, 1, true);
    let before = snapshot(&actual);
    let proposals = prepare(&actual, 5);
    let cancel = CancellationToken::new();
    let mut batch = StateCommitBatch::new(1, 5, 2, 1 << 20).unwrap();
    batch
        .add_compression(&mut actual.compression, proposals.compression, &cancel)
        .unwrap();
    batch
        .add_main_kv(&mut actual.window, proposals.window, &cancel)
        .unwrap();
    batch
        .add_main_kv(&mut actual.main, proposals.main, &cancel)
        .unwrap();
    batch
        .add_index_kv(&mut actual.index, proposals.index, &cancel)
        .unwrap();
    drop(batch);
    assert_eq!(snapshot(&actual), before);
    drop(prepare(&actual, 5));
}

#[test]
fn aggregate_staged_budget_refusal_poisoning_keeps_already_staged_states_old() {
    let mut actual = bundle(CompressionKind::C1);
    let before = snapshot(&actual);
    let proposals = prepare(&actual, 5);
    let metadata = (std::mem::size_of::<Mutation<'static>>()
        + std::mem::size_of::<CommittedCompression>()
        + 64) as u64;
    let budget = actual.compression.stage_bytes(3).unwrap() + metadata + 100;
    let cancel = CancellationToken::new();
    let mut batch = StateCommitBatch::new(0, 5, 3, budget).unwrap();
    batch
        .add_compression(&mut actual.compression, proposals.compression, &cancel)
        .unwrap();
    assert!(batch.staged_bytes() <= budget);
    assert!(batch
        .add_main_kv(&mut actual.window, proposals.window, &cancel)
        .is_err());
    assert!(batch.commit(&cancel).is_err());
    drop(proposals.main);
    drop(proposals.index);
    assert_eq!(snapshot(&actual), before);
    drop(prepare(&actual, 5));
}

#[test]
fn mismatched_ranges_empty_poisoned_batches_and_invalid_prefixes_refuse() {
    for (start, tokens, accepted, bytes) in [
        (0, 0, 0, 1024),
        (0, 2, 3, 1024),
        (usize::MAX, 1, 1, 1024),
        (0, 1, 1, 0),
    ] {
        assert!(StateCommitBatch::new(start, tokens, accepted, bytes).is_err());
    }
    assert!(StateCommitBatch::new(0, 1, 1, 1024)
        .unwrap()
        .commit(&CancellationToken::new())
        .is_err());
    let mut actual = bundle(CompressionKind::C1);
    let before = snapshot(&actual);
    let proposals = prepare(&actual, 2);
    let cancel = CancellationToken::new();
    let mut batch = StateCommitBatch::new(0, 3, 2, 1 << 20).unwrap();
    assert!(batch
        .add_compression(&mut actual.compression, proposals.compression, &cancel)
        .is_err());
    assert!(batch
        .add_main_kv(&mut actual.window, proposals.window, &cancel)
        .is_err());
    assert!(batch.commit(&cancel).is_err());
    drop(proposals.main);
    drop(proposals.index);
    assert_eq!(snapshot(&actual), before);
}

#[test]
fn proposed_index_cache_reaches_packed_indexer_with_causality_and_accepted_visibility() {
    use crate::layers::{
        indexer::{SparseIndexPlan, SparseIndexer, INVALID_INDEX_ROW},
        quant::Fp4IndexRows,
    };
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let mut state = IndexKvState::new(&device, CompressionKind::C2, 16, 8).unwrap();
    let keys = Tensor::cat(
        &[
            &Tensor::ones((1, 128), DType::BF16, device.tensor_device()).unwrap(),
            &Tensor::full(2_f32, (1, 128), device.tensor_device())
                .unwrap()
                .to_dtype(DType::BF16)
                .unwrap(),
        ],
        0,
    )
    .unwrap();
    let keys = Fp4IndexRows::encode(&device, &keys, &cancel).unwrap();
    let proposal = state
        .prepare(
            0,
            5,
            &[1, 3],
            keys.data_bytes(),
            keys.scale_bytes(),
            &cancel,
        )
        .unwrap();
    let plan = SparseIndexPlan::new(
        device.limits(),
        &proposal.ready_positions().collect::<Vec<_>>(),
        &[0, 1, 2, 3, 4],
        None,
    )
    .unwrap();
    let query = Fp4IndexRows::encode(
        &device,
        &Tensor::ones((5, 128), DType::BF16, device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    let weights = Tensor::ones((5, 1), DType::BF16, device.tensor_device()).unwrap();
    let indexer = SparseIndexer::new(device.clone(), 1, 128, 1, None).unwrap();
    let selected = indexer
        .select_packed(
            &query,
            &proposal.index_rows(&device).unwrap(),
            &weights,
            &plan,
            &cancel,
        )
        .unwrap();
    assert_eq!(
        selected.rows().to_vec2::<u32>().unwrap(),
        vec![vec![INVALID_INDEX_ROW], vec![0], vec![0], vec![1], vec![1]]
    );
    state.commit_prefix(proposal, 3, &cancel).unwrap();
    assert_eq!(state.position(), 3);
    assert_eq!(state.row_count(), 1);
    let plan = SparseIndexPlan::new(
        device.limits(),
        &state.ready_positions().collect::<Vec<_>>(),
        &[4],
        None,
    )
    .unwrap();
    let query = Fp4IndexRows::encode(
        &device,
        &Tensor::ones((1, 128), DType::BF16, device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    let selected = indexer
        .select_packed(
            &query,
            &state.index_rows(&device).unwrap(),
            &weights.narrow(0, 0, 1).unwrap(),
            &plan,
            &cancel,
        )
        .unwrap();
    assert_eq!(selected.rows().to_vec2::<u32>().unwrap(), vec![vec![0]]);
}
