use super::*;
use crate::{
    engram::{EngramHistory, EngramToken},
    state::transaction::StateCommitBatch,
    tokenizer::COMPRESSED_MAP_SHA256,
    weights::xq::{storage::tests::fixture_with_metadata, XqTensorType},
};
use sha2::{Digest, Sha256};

const MAP: &[u8] = include_bytes!("../../../../../fixtures/xq-engram-token-map.u32le");
type Metadata = Vec<(&'static str, u32, Vec<u8>)>;

fn array32(values: &[u32]) -> Vec<u8> {
    let mut bytes = 4_u32.to_le_bytes().to_vec();
    bytes.extend((values.len() as u64).to_le_bytes());
    bytes.extend(values.iter().flat_map(|x| x.to_le_bytes()));
    bytes
}
fn metadata() -> Metadata {
    let mut encoding = ("e4m3_e8m0_32_row264".len() as u64).to_le_bytes().to_vec();
    encoding.extend(b"e4m3_e8m0_32_row264");
    let mut map = 4_u32.to_le_bytes().to_vec();
    map.extend((RAW_VOCABULARY as u64).to_le_bytes());
    map.extend(MAP);
    let mut multipliers = 10_u32.to_le_bytes().to_vec();
    multipliers.extend(8_u64.to_le_bytes());
    let mut primes = Vec::new();
    for layer in [EngramLayer::Layer1, EngramLayer::Layer14] {
        primes.extend_from_slice(layer.sizes());
        multipliers.extend(layer.multipliers().iter().flat_map(|x| x.to_le_bytes()));
    }
    vec![
        ("deepseek41.engram.encoding", 8, encoding),
        (
            "deepseek41.engram.compressed_vocab_size",
            4,
            COMPRESSED_VOCABULARY.to_le_bytes().to_vec(),
        ),
        (
            "deepseek41.engram.pad_id",
            4,
            COMPRESSED_PAD.to_le_bytes().to_vec(),
        ),
        ("deepseek41.engram.layer_ids", 9, array32(&[1, 14])),
        (
            "deepseek41.engram.rows",
            9,
            array32(&[
                EngramLayer::Layer1.table_rows(),
                EngramLayer::Layer14.table_rows(),
            ]),
        ),
        ("deepseek41.engram.primes", 9, array32(&primes)),
        ("deepseek41.engram.multipliers", 9, multipliers),
        ("deepseek41.engram.token_map", 9, map),
    ]
}
fn fixture(meta: &Metadata, budget: u64) -> (tempfile::TempDir, XqWeights) {
    fixture_with_metadata(
        &[("dummy", vec![1], XqTensorType::F32, vec![0; 4])],
        meta,
        budget,
    )
}

pub(crate) fn fixture_for_components() -> (tempfile::TempDir, XqWeights, XqEngramMetadata) {
    let (dir, weights) = fixture(&metadata(), 1024 * 1024);
    let metadata = XqEngramMetadata::load(
        &weights,
        &InferenceLimits::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    (dir, weights, metadata)
}

#[test]
fn authenticated_xq_map_matches_the_complete_official_compressed_vocabulary() {
    assert_eq!(MAP.len(), RAW_VOCABULARY * 4);
    assert_eq!(format!("{:x}", Sha256::digest(MAP)), COMPRESSED_MAP_SHA256);
    let (_dir, weights) = fixture(&metadata(), 1024 * 1024);
    let bound = XqEngramMetadata::load(
        &weights,
        &InferenceLimits::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    let vocabulary = bound.vocabulary();
    assert_eq!(vocabulary.raw_vocabulary_size(), RAW_VOCABULARY);
    assert_eq!(vocabulary.mapping_sha256(), COMPRESSED_MAP_SHA256);
    for (id, bytes) in MAP.chunks_exact(4).enumerate() {
        assert_eq!(
            vocabulary.compress(id as u32).unwrap(),
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        );
    }
    assert!(vocabulary.compress(RAW_VOCABULARY as u32).is_err());
}

#[test]
fn changed_encoding_extent_layer_order_prime_multiplier_or_token_map_is_refused() {
    for index in 0..8 {
        let mut meta = metadata();
        let last = meta[index].2.len() - 1;
        meta[index].2[last] ^= 1;
        let (_dir, weights) = fixture(&meta, 1024 * 1024);
        assert!(
            XqEngramMetadata::load(
                &weights,
                &InferenceLimits::default(),
                &CancellationToken::new()
            )
            .is_err(),
            "changed field {index}"
        );
    }
    let mut meta = metadata();
    meta[3].2 = array32(&[14, 1]);
    let (_dir, weights) = fixture(&meta, 1024 * 1024);
    assert!(XqEngramMetadata::load(
        &weights,
        &InferenceLimits::default(),
        &CancellationToken::new()
    )
    .is_err());
    let mut meta = metadata();
    meta.pop();
    let (_dir, weights) = fixture(&meta, 1024 * 1024);
    assert!(XqEngramMetadata::load(
        &weights,
        &InferenceLimits::default(),
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn integer_array_type_length_read_budget_metadata_budget_and_cancel_fail_closed() {
    let (_dir, weights) = fixture(&metadata(), 1024 * 1024);
    for (kind, len) in [(4, 7), (10, 7), (6, 8), (4, usize::MAX)] {
        assert!(weights
            .read_metadata_array(
                "deepseek41.engram.multipliers",
                kind,
                len,
                &CancellationToken::new()
            )
            .is_err());
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(XqEngramMetadata::load(&weights, &InferenceLimits::default(), &cancel).is_err());
    for limits in [
        InferenceLimits {
            max_input_bytes: 1024,
            ..InferenceLimits::default()
        },
        InferenceLimits {
            max_state_bytes: 1024,
            ..InferenceLimits::default()
        },
    ] {
        assert!(XqEngramMetadata::load(&weights, &limits, &CancellationToken::new()).is_err());
    }
    let (_dir, weights) = fixture(&metadata(), 256);
    assert!(XqEngramMetadata::load(
        &weights,
        &InferenceLimits::default(),
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn xq_mapped_tokens_history_and_atomic_prefix_keep_images_and_rejected_future_out() {
    let (_dir, weights) = fixture(&metadata(), 1024 * 1024);
    let metadata = XqEngramMetadata::load(
        &weights,
        &InferenceLimits::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    let vocabulary = metadata.vocabulary();
    let token = |id| EngramToken::from_raw(id, vocabulary).unwrap();
    let tokens = [
        token(129279),
        token(5421),
        EngramToken::image(),
        token(16000),
        token(28942),
    ];
    for accepted in 0..=tokens.len() {
        let mut history = EngramHistory::new(32, 8).unwrap();
        let mut fresh = EngramHistory::new(32, 8).unwrap();
        let proposal = history.prepare(&tokens).unwrap();
        if accepted > 0 {
            fresh
                .commit_prefix(fresh.prepare(&tokens[..accepted]).unwrap(), accepted)
                .unwrap();
        }
        let cancel = CancellationToken::new();
        let mut transaction = StateCommitBatch::new(0, tokens.len(), accepted, 4096).unwrap();
        transaction
            .add_engram_history(&mut history, proposal, &cancel)
            .unwrap();
        transaction.commit(&cancel).unwrap();
        assert_eq!(history.position(), fresh.position());
        let next = [token(11230)];
        assert_eq!(
            history.prepare(&next).unwrap().rows(),
            fresh.prepare(&next).unwrap().rows()
        );
    }
    let mut history = EngramHistory::new(32, 8).unwrap();
    let proposal = history.prepare(&tokens).unwrap();
    assert!(proposal.rows()[2].is_image());
    assert_eq!(
        proposal.rows()[3].tokens(),
        &[
            vocabulary.compress(16000).unwrap(),
            COMPRESSED_PAD,
            COMPRESSED_PAD,
            COMPRESSED_PAD
        ]
    );
    let cancel = CancellationToken::new();
    let mut transaction = StateCommitBatch::new(0, tokens.len(), 2, 4096).unwrap();
    transaction
        .add_engram_history(&mut history, proposal, &cancel)
        .unwrap();
    cancel.cancel();
    assert!(transaction.commit(&cancel).is_err());
    assert_eq!(history.position(), 0);
}
