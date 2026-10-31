use super::*;

pub(crate) fn text(value: u32) -> EngramToken {
    EngramToken {
        compressed: Some(value),
    }
}

#[test]
fn atomic_history_prefix_matches_direct_history_across_image_barriers() {
    use crate::state::transaction::StateCommitBatch;
    use tokio_util::sync::CancellationToken;
    let tokens = [text(10), text(20), EngramToken::image(), text(30), text(40)];
    for accepted in 0..=tokens.len() {
        let mut actual = EngramHistory::new(32, 8).unwrap();
        let mut expected = EngramHistory::new(32, 8).unwrap();
        let proposal = actual.prepare(&tokens).unwrap();
        let reference = expected.prepare(&tokens).unwrap();
        let cancel = CancellationToken::new();
        let mut batch = StateCommitBatch::new(0, tokens.len(), accepted, 4096).unwrap();
        batch
            .add_engram_history(&mut actual, proposal, &cancel)
            .unwrap();
        assert!(batch.commit(&cancel).unwrap().is_empty());
        expected.commit_prefix(reference, accepted).unwrap();
        assert_eq!(actual.position(), expected.position());
        assert_eq!(actual.previous, expected.previous);
        assert_eq!(
            actual.prepare(&[text(50)]).unwrap().rows(),
            expected.prepare(&[text(50)]).unwrap().rows()
        );
    }
}

#[test]
fn cancelled_atomic_history_compressor_main_and_index_cannot_publish_any_state() {
    use crate::layers::{kv::MainKvFormat, CudaLayerDevice};
    use crate::state::{
        index::IndexKvState,
        kv::{KvCacheKind, KvState},
        transaction::StateCommitBatch,
        CompressionKind, CompressionState,
    };
    use aster_power::inference::InferenceLimits;
    use candle_core::{DType, Tensor};
    use tokio_util::sync::CancellationToken;
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let mut history = EngramHistory::new(16, 8).unwrap();
    let before = history.previous;
    let history_proposal = history
        .prepare(&[text(10), EngramToken::image(), text(20)])
        .unwrap();
    let mut compression = CompressionState::new(&device, CompressionKind::C1, 2, 16, 8).unwrap();
    let mut main = KvState::new(&device, KvCacheKind::C1, MainKvFormat::Fp8Ue8m0, 16, 8).unwrap();
    let mut index = IndexKvState::new(&device, CompressionKind::C1, 16, 8).unwrap();
    let cancel = CancellationToken::new();
    let latent = Tensor::ones((3, 2), DType::BF16, device.tensor_device()).unwrap();
    let compression_proposal = compression.prepare(0, None, &latent, &cancel).unwrap();
    let records = Tensor::cat(
        &[
            &Tensor::ones((3, 512), DType::U8, device.tensor_device()).unwrap(),
            &Tensor::full(127_u8, (3, 16), device.tensor_device()).unwrap(),
        ],
        1,
    )
    .unwrap();
    let main_proposal = main.prepare(0, 3, &[0, 1, 2], &records, &cancel).unwrap();
    let index_proposal = index
        .prepare(
            0,
            3,
            &[0, 1, 2],
            &Tensor::ones((3, 64), DType::U8, device.tensor_device()).unwrap(),
            &Tensor::full(127_u8, (3, 4), device.tensor_device()).unwrap(),
            &cancel,
        )
        .unwrap();
    let mut batch = StateCommitBatch::new(0, 3, 2, 65536).unwrap();
    batch
        .add_engram_history(&mut history, history_proposal, &cancel)
        .unwrap();
    batch
        .add_compression(&mut compression, compression_proposal, &cancel)
        .unwrap();
    batch
        .add_main_kv(&mut main, main_proposal, &cancel)
        .unwrap();
    batch
        .add_index_kv(&mut index, index_proposal, &cancel)
        .unwrap();
    cancel.cancel();
    assert!(matches!(
        batch.commit(&cancel),
        Err(PowerError::InferenceCancelled)
    ));
    assert_eq!(
        [
            history.position(),
            compression.position(),
            main.position(),
            index.position()
        ],
        [0; 4]
    );
    assert_eq!(history.previous, before);
    assert_eq!(main.row_count(), 0);
    assert_eq!(index.row_count(), 0);
}

#[test]
fn late_foreign_history_poisoning_preserves_earlier_staged_kv() {
    use crate::layers::{kv::MainKvFormat, CudaLayerDevice};
    use crate::state::{
        kv::{KvCacheKind, KvState},
        transaction::StateCommitBatch,
    };
    use aster_power::inference::InferenceLimits;
    use candle_core::{DType, Tensor};
    use tokio_util::sync::CancellationToken;
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let mut main =
        KvState::new(&device, KvCacheKind::Window, MainKvFormat::Fp8Ue8m0, 16, 8).unwrap();
    let mut history = EngramHistory::new(16, 8).unwrap();
    let other = EngramHistory::new(16, 8).unwrap();
    let foreign = other.prepare(&[text(10)]).unwrap();
    let records = Tensor::cat(
        &[
            &Tensor::ones((1, 512), DType::U8, device.tensor_device()).unwrap(),
            &Tensor::full(127_u8, (1, 16), device.tensor_device()).unwrap(),
        ],
        1,
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let proposal = main.prepare(0, 1, &[0], &records, &cancel).unwrap();
    let mut batch = StateCommitBatch::new(0, 1, 1, 4096).unwrap();
    batch.add_main_kv(&mut main, proposal, &cancel).unwrap();
    assert!(batch
        .add_engram_history(&mut history, foreign, &cancel)
        .is_err());
    assert!(batch.commit(&cancel).is_err());
    assert_eq!(main.position(), 0);
    assert_eq!(history.position(), 0);
}

#[test]
fn pinned_layers_and_table_sizes_are_exact_closed_sets() {
    for (layer, rows) in [
        (EngramLayer::Layer1, 384006168),
        (EngramLayer::Layer14, 384016682),
    ] {
        assert_eq!(EngramLayer::parse(layer.as_str()).unwrap(), layer);
        assert_eq!(layer.table_rows(), rows);
    }
    for value in ["0", "2", "layer1", ""] {
        assert!(EngramLayer::parse(value).is_err());
    }
}

#[test]
fn official_hashes_match_independent_int64_reference() {
    let fixture: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../fixtures/engram-hash-golden.json")).unwrap();
    for case in fixture.as_array().unwrap() {
        let tokens: Vec<u32> = serde_json::from_value(case["tokens"].clone()).unwrap();
        let row = NgramRow {
            tokens: tokens.try_into().unwrap(),
            is_image: false,
        };
        for layer in [EngramLayer::Layer1, EngramLayer::Layer14] {
            let expected: Vec<i32> = serde_json::from_value(case[layer.as_str()].clone()).unwrap();
            assert_eq!(layer.hash(&row).unwrap().as_slice(), expected);
        }
    }
}

#[test]
fn current_first_padding_and_image_barriers_match_reference_gathering() {
    let history = EngramHistory::new(16, 8).unwrap();
    let prepared = history
        .prepare(&[text(10), text(20), EngramToken::image(), text(30), text(40)])
        .unwrap();
    let expected = [
        [10, 2, 2, 2],
        [20, 10, 2, 2],
        [2, 2, 2, 2],
        [30, 2, 2, 2],
        [40, 30, 2, 2],
    ];
    for (row, expected) in prepared.rows().iter().zip(expected) {
        assert_eq!(row.tokens, expected);
    }
    for layer in [EngramLayer::Layer1, EngramLayer::Layer14] {
        assert_eq!(layer.hash(&prepared.rows()[2]).unwrap(), [-1; 24]);
    }
    assert_eq!(history.position(), 0);
}

#[test]
fn every_chunk_partition_equals_decode_through_image_boundaries() {
    let input = [
        text(10),
        text(20),
        EngramToken::image(),
        text(30),
        text(40),
        text(50),
    ];
    let mut decoded = EngramHistory::new(16, 8).unwrap();
    let mut expected = Vec::new();
    for token in input {
        let prepared = decoded.prepare(&[token]).unwrap();
        expected.extend_from_slice(prepared.rows());
        decoded.commit_prefix(prepared, 1).unwrap();
    }
    for partition in 0..32 {
        let mut history = EngramHistory::new(16, 8).unwrap();
        let mut rows = Vec::new();
        let mut start = 0;
        for end in 1..=input.len() {
            if end == input.len() || partition & (1 << (end - 1)) != 0 {
                let prepared = history.prepare(&input[start..end]).unwrap();
                rows.extend_from_slice(prepared.rows());
                history.commit_prefix(prepared, end - start).unwrap();
                start = end;
            }
        }
        assert_eq!(rows, expected);
        assert_eq!(history.position(), input.len());
    }
}

#[test]
fn every_accepted_prefix_restores_exact_ngram_history() {
    let input = [text(10), EngramToken::image(), text(20), text(30), text(40)];
    for accepted in 0..=input.len() {
        let mut history = EngramHistory::new(16, 8).unwrap();
        let prepared = history.prepare(&input).unwrap();
        history.commit_prefix(prepared, accepted).unwrap();
        let mut fresh = EngramHistory::new(16, 8).unwrap();
        if accepted > 0 {
            let prepared = fresh.prepare(&input[..accepted]).unwrap();
            fresh.commit_prefix(prepared, accepted).unwrap();
        }
        assert_eq!(
            history.prepare(&[text(70)]).unwrap().rows(),
            fresh.prepare(&[text(70)]).unwrap().rows()
        );
        assert_eq!(history.position(), accepted);
    }
}

#[test]
fn cross_request_stale_oversized_and_cancelled_transitions_cannot_mutate_history() {
    let mut history = EngramHistory::new(2, 2).unwrap();
    let other = EngramHistory::new(2, 2).unwrap();
    let prepared = other.prepare(&[text(10)]).unwrap();
    assert!(history.commit_prefix(prepared, 1).is_err());
    let stale = history.prepare(&[text(20)]).unwrap();
    let committed = history.prepare(&[text(10)]).unwrap();
    history.commit_prefix(committed, 1).unwrap();
    assert!(history.commit_prefix(stale, 1).is_err());
    assert_eq!(history.position(), 1);
    assert!(history.prepare(&[text(20), text(30)]).is_err());
    assert!(history.prepare(&[]).is_err());
    drop(history.prepare(&[text(99)]).unwrap());
    assert_eq!(history.position(), 1);
    let too_many = history.prepare(&[text(20)]).unwrap();
    assert!(history.commit_prefix(too_many, 2).is_err());
    assert_eq!(
        history.prepare(&[text(30)]).unwrap().rows()[0].tokens,
        [30, 10, 2, 2]
    );
    assert!(history.prepare(&[text(COMPRESSED_VOCABULARY)]).is_err());
    for (context, step) in [(0, 1), (1, 0), (1, 2), (MAX_CONTEXT_TOKENS + 1, 1)] {
        assert!(EngramHistory::new(context, step).is_err());
    }
}
