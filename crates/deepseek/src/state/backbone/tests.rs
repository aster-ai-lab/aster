use super::*;
use crate::{
    engram::tests::text,
    layers::{kv::MainKvRows, quant::Fp4IndexRows},
    model::DeepSeekModel,
    state::CompressionKind,
};
use aster_power::{error::PowerError, inference::InferenceLimits};
use candle_core::{DType, Tensor};

fn device() -> CudaLayerDevice {
    CudaLayerDevice::cpu_for_tests(InferenceLimits::default())
}
fn state(budget: u64) -> BackboneState {
    let architecture = DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap();
    BackboneState::new(
        &architecture,
        &vec![device(); 40],
        8,
        4,
        MainKvFormat::Fp4E4m3,
        budget,
    )
    .unwrap()
}
fn tokens() -> [EngramToken; 4] {
    [text(10), text(20), EngramToken::image(), text(30)]
}
fn window(owner: &BackboneState, layer: usize, tokens: usize) -> PreparedKv {
    let device = device();
    window_on(owner, layer, tokens, &device)
}
fn window_on(
    owner: &BackboneState,
    layer: usize,
    tokens: usize,
    device: &CudaLayerDevice,
) -> PreparedKv {
    let start = owner.position();
    let cancel = CancellationToken::new();
    let values: Vec<_> = (start..start + tokens)
        .flat_map(|p| vec![(layer + p) as f32 / 8. + 0.25; 512])
        .collect();
    let values = Tensor::from_vec(values, (tokens, 512), device.tensor_device())
        .unwrap()
        .to_dtype(DType::BF16)
        .unwrap();
    let rows = MainKvRows::encode(&device, MainKvFormat::Fp8Ue8m0, &values, &cancel).unwrap();
    owner
        .window(layer)
        .unwrap()
        .prepare(
            start,
            tokens,
            &(start..start + tokens).collect::<Vec<_>>(),
            rows.records(),
            &cancel,
        )
        .unwrap()
}
fn source(owner: &BackboneState, layer: usize, tokens: usize) -> PreparedSourceState {
    let device = device();
    source_on(owner, layer, tokens, &device)
}
fn source_on(
    owner: &BackboneState,
    layer: usize,
    tokens: usize,
    device: &CudaLayerDevice,
) -> PreparedSourceState {
    let start = owner.position();
    let cancel = CancellationToken::new();
    let source = owner.compressed_source(layer).unwrap();
    let values: Vec<_> = (start..start + tokens)
        .flat_map(|p| (0..512).map(move |c| (layer + p + c % 3) as f32 / 8. + 0.25))
        .collect();
    let values = Tensor::from_vec(values, (tokens, 512), device.tensor_device())
        .unwrap()
        .to_dtype(
            if source.compression.kind() == CompressionKind::C1
                && source.compression.numerics() == CompressionNumerics::OfficialBf16
            {
                DType::BF16
            } else {
                DType::F32
            },
        )
        .unwrap();
    let scores = Tensor::zeros((tokens, 512), DType::F32, device.tensor_device()).unwrap();
    let compression = source
        .compression
        .prepare(
            start,
            (source.compression.kind() == CompressionKind::C2).then_some(&scores),
            &values,
            &cancel,
        )
        .unwrap();
    let ready = compression.emitted_positions();
    let (records, data, scales) = if let Some(emitted) = compression.emissions() {
        let (main, index) = if source.compression.numerics() == CompressionNumerics::XqF32 {
            use crate::layers::xq::quant::{XqActivationFormat, XqQuantizedActivation};
            let main = XqQuantizedActivation::encode(
                &device,
                XqActivationFormat::CompressedFp4,
                emitted,
                &cancel,
            )
            .unwrap()
            .into_main_rows(&cancel)
            .unwrap();
            let index = XqQuantizedActivation::encode(
                &device,
                XqActivationFormat::IndexFp4,
                &emitted.narrow(1, 0, 128).unwrap(),
                &cancel,
            )
            .unwrap()
            .into_index_rows(&cancel)
            .unwrap();
            (main, index)
        } else {
            (
                MainKvRows::encode(&device, MainKvFormat::Fp4E4m3, emitted, &cancel).unwrap(),
                Fp4IndexRows::encode(&device, &emitted.narrow(1, 0, 128).unwrap(), &cancel)
                    .unwrap(),
            )
        };
        (
            main.records().clone(),
            index.data_bytes().clone(),
            index.scale_bytes().clone(),
        )
    } else {
        (
            Tensor::zeros((0, 288), DType::U8, device.tensor_device()).unwrap(),
            Tensor::zeros((0, 64), DType::U8, device.tensor_device()).unwrap(),
            Tensor::zeros((0, 4), DType::U8, device.tensor_device()).unwrap(),
        )
    };
    let main = source
        .main
        .prepare(start, tokens, ready, &records, &cancel)
        .unwrap();
    let index = source
        .index
        .prepare(start, tokens, ready, &data, &scales, &cancel)
        .unwrap();
    PreparedSourceState {
        compression,
        main,
        index,
    }
}
fn prepare_except(
    owner: &BackboneState,
    tokens: &[EngramToken],
    omit_window: Option<usize>,
    omit_source: Option<usize>,
) -> PendingBackboneStep {
    let cancel = CancellationToken::new();
    let mut pending = owner.begin(tokens, &cancel).unwrap();
    assert_eq!(pending.ngrams().len(), tokens.len());
    for layer in 0..40 {
        if Some(layer) != omit_window {
            pending
                .stage_window(layer, window(owner, layer, tokens.len()), &cancel)
                .unwrap();
        }
    }
    for layer in SOURCE_LAYERS {
        if Some(layer) != omit_source {
            let p = source(owner, layer, tokens.len());
            pending
                .stage_source(layer, p.compression, p.main, p.index, &cancel)
                .unwrap();
        }
    }
    pending
}
fn prepare(owner: &BackboneState, tokens: &[EngramToken]) -> PendingBackboneStep {
    prepare_except(owner, tokens, None, None)
}

/// Synthetic state for layers outside the explicitly executed GPU fixture.
/// This is never presented as a complete-model computation or quality proof.
#[cfg(feature = "cuda")]
pub(crate) fn stage_synthetic_rest(
    owner: &BackboneState,
    pending: &mut PendingBackboneStep,
    device: &CudaLayerDevice,
    actual_layers: &[usize],
    actual_source: usize,
) {
    let cancel = CancellationToken::new();
    let tokens = pending.token_count();
    for layer in 0..40 {
        if !actual_layers.contains(&layer) {
            pending
                .stage_window(layer, window_on(owner, layer, tokens, device), &cancel)
                .unwrap();
        }
    }
    for layer in SOURCE_LAYERS {
        if layer != actual_source {
            let s = source_on(owner, layer, tokens, device);
            pending
                .stage_source(layer, s.compression, s.main, s.index, &cancel)
                .unwrap();
        }
    }
}

fn xq_state() -> BackboneState {
    let architecture = DeepSeekModel::V41Flash1017Xq
        .verify_config(include_bytes!("../../../fixtures/xq-model-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap();
    assert!(BackboneState::new(
        &architecture,
        &vec![device(); 40],
        8,
        4,
        MainKvFormat::Fp8Ue8m0,
        2_000_000
    )
    .is_err());
    BackboneState::new(
        &architecture,
        &vec![device(); 40],
        8,
        4,
        MainKvFormat::Fp4E4m3,
        2_000_000,
    )
    .unwrap()
}

#[test]
fn pinned_xq_backbone_commits_every_f32_source_and_accepted_prefix_atomically() {
    let cancel = CancellationToken::new();
    for accepted in 0..=4 {
        let mut actual = xq_state();
        let mut expected = xq_state();
        assert_eq!(actual.compression_numerics(), CompressionNumerics::XqF32);
        assert_eq!(actual.participant_count(), 53);
        for source in &actual.sources {
            assert_eq!(source.compression().numerics(), CompressionNumerics::XqF32);
        }
        let proposal = prepare(&actual, &tokens());
        let committed = actual.commit_prefix(proposal, accepted, &cancel).unwrap();
        for compression in committed {
            if let Some(emissions) = compression.emissions() {
                assert_eq!(emissions.dtype(), DType::F32);
            }
        }
        if accepted > 0 {
            let proposal = prepare(&expected, &tokens()[..accepted]);
            expected.commit_prefix(proposal, accepted, &cancel).unwrap();
        }
        assert_eq!(snapshot(&actual), snapshot(&expected));
        let next = [text(41), text(42)];
        let proposal = prepare(&actual, &next);
        actual.commit_prefix(proposal, 2, &cancel).unwrap();
        let proposal = prepare(&expected, &next);
        expected.commit_prefix(proposal, 2, &cancel).unwrap();
        assert_eq!(snapshot(&actual), snapshot(&expected));
    }
}

#[test]
fn cancelled_xq_53_participant_commit_and_foreign_official_source_publish_nothing() {
    let mut actual = xq_state();
    let before = snapshot(&actual);
    let proposal = prepare(&actual, &tokens());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(actual.commit_prefix(proposal, 3, &cancel).is_err());
    assert_eq!(snapshot(&actual), before);
    let official = state(2_000_000);
    let mut proposal = prepare_except(&actual, &tokens(), None, Some(20));
    let foreign = source(&official, 20, 4);
    let cancel = CancellationToken::new();
    proposal
        .stage_source(
            20,
            foreign.compression,
            foreign.main,
            foreign.index,
            &cancel,
        )
        .unwrap();
    assert!(actual.commit_prefix(proposal, 3, &cancel).is_err());
    assert_eq!(snapshot(&actual), before);
}
fn append(out: &mut Vec<u8>, tensor: &Tensor) {
    out.extend(tensor.flatten_all().unwrap().to_vec1::<u8>().unwrap());
}
fn snapshot(owner: &BackboneState) -> Vec<u8> {
    let device = device();
    let mut out = Vec::new();
    out.extend((owner.position() as u64).to_le_bytes());
    for window in &owner.windows {
        out.extend((window.position() as u64).to_le_bytes());
        out.extend((window.row_count() as u64).to_le_bytes());
        append(&mut out, window.attention_rows(&device).unwrap().records());
    }
    for source in &owner.sources {
        for p in [
            source.compression.position(),
            source.main.position(),
            source.index.position(),
            source.compression.pending_bytes() as usize,
        ] {
            out.extend((p as u64).to_le_bytes());
        }
        append(
            &mut out,
            source.main.attention_rows(&device).unwrap().records(),
        );
        let index = source.index.index_rows(&device).unwrap();
        append(&mut out, index.data_bytes());
        append(&mut out, index.scale_bytes());
    }
    let ngram = owner.history.prepare(&[text(77)]).unwrap();
    for id in ngram.rows()[0].tokens() {
        out.extend(id.to_le_bytes());
    }
    out
}

#[test]
fn complete_base_state_commits_all_53_participants_and_exposes_prepared_shared_views() {
    let mut owner = state(2_000_000);
    let cancel = CancellationToken::new();
    let pending = prepare(&owner, &tokens());
    assert_eq!(owner.participant_count(), 53);
    assert_eq!(pending.position(), 0);
    assert_eq!(pending.token_count(), 4);
    assert_eq!(owner.position(), 0);
    assert_eq!(
        pending
            .window(39)
            .unwrap()
            .ready_positions()
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(
        pending
            .compressed_source(20)
            .unwrap()
            .main()
            .ready_positions()
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(
        pending
            .compressed_source(14)
            .unwrap()
            .index()
            .ready_positions()
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    let committed = owner.commit_prefix(pending, 3, &cancel).unwrap();
    assert_eq!(committed.len(), 4);
    assert_eq!(owner.position(), 3);
    for layer in 0..40 {
        let state = owner.window(layer).unwrap();
        assert_eq!(state.position(), 3);
        assert_eq!(state.row_count(), 3);
    }
    for layer in SOURCE_LAYERS {
        let s = owner.compressed_source(layer).unwrap();
        assert_eq!(s.layer(), layer);
        assert_eq!(
            (
                s.compression().position(),
                s.main().position(),
                s.index().position()
            ),
            (3, 3, 3)
        );
        assert_eq!(s.main().row_count(), if layer == 20 { 3 } else { 1 });
        assert_eq!(s.index().row_count(), s.main().row_count());
        assert_eq!(
            s.compression().pending_bytes(),
            if layer == 20 { 0 } else { 4096 }
        );
    }
}

#[test]
fn every_accepted_prefix_and_following_odd_pair_matches_fresh_complete_state() {
    let cancel = CancellationToken::new();
    for accepted in 0..=4 {
        let mut actual = state(2_000_000);
        let mut expected = state(2_000_000);
        let proposal = prepare(&actual, &tokens());
        actual.commit_prefix(proposal, accepted, &cancel).unwrap();
        if accepted > 0 {
            let proposal = prepare(&expected, &tokens()[..accepted]);
            expected.commit_prefix(proposal, accepted, &cancel).unwrap();
        }
        assert_eq!(
            snapshot(&actual),
            snapshot(&expected),
            "accepted {accepted}"
        );
        for target in [&mut actual, &mut expected] {
            let proposal = prepare(target, &[text(88)]);
            target.commit_prefix(proposal, 1, &cancel).unwrap();
        }
        assert_eq!(
            snapshot(&actual),
            snapshot(&expected),
            "continued {accepted}"
        );
    }
}

#[test]
fn missing_window_or_source_cannot_commit_any_other_participant() {
    let mut owner = state(2_000_000);
    let cancel = CancellationToken::new();
    let before = snapshot(&owner);
    for (window, source) in [
        (Some(0), None),
        (Some(39), None),
        (None, Some(2)),
        (None, Some(20)),
    ] {
        let proposal = prepare_except(&owner, &tokens(), window, source);
        assert!(owner.commit_prefix(proposal, 4, &cancel).is_err());
        assert_eq!(snapshot(&owner), before);
        assert!(owner.begin(&[text(1)], &cancel).is_ok());
    }
}

#[test]
fn last_foreign_index_rejects_after_52_staged_participants_without_partial_publication() {
    let mut owner = state(2_000_000);
    let other = state(2_000_000);
    let cancel = CancellationToken::new();
    let before = snapshot(&owner);
    let mut proposal = prepare_except(&owner, &tokens(), None, Some(20));
    let own = source(&owner, 20, 4);
    let foreign = source(&other, 20, 4);
    drop(own.index);
    proposal
        .stage_source(20, own.compression, own.main, foreign.index, &cancel)
        .unwrap();
    assert!(owner.commit_prefix(proposal, 4, &cancel).is_err());
    assert_eq!(snapshot(&owner), before);
    let proposal = prepare(&owner, &tokens());
    owner.commit_prefix(proposal, 4, &cancel).unwrap();
    assert_eq!(owner.position(), 4);
}

#[test]
fn cancellation_after_all_staging_and_dropped_guard_preserve_every_old_state_and_release_leases() {
    let mut owner = state(2_000_000);
    let cancel = CancellationToken::new();
    let before = snapshot(&owner);
    let pending = prepare(&owner, &tokens());
    let guard = owner.stage_prefix(pending, 3, &cancel).unwrap();
    assert!(guard.staged_bytes() > 0);
    let stop = CancellationToken::new();
    stop.cancel();
    assert!(matches!(
        guard.commit(&stop),
        Err(PowerError::InferenceCancelled)
    ));
    assert_eq!(snapshot(&owner), before);
    let pending = prepare(&owner, &tokens());
    let guard = owner.stage_prefix(pending, 3, &cancel).unwrap();
    drop(guard);
    assert_eq!(snapshot(&owner), before);
    let pending = prepare(&owner, &tokens());
    owner.commit_prefix(pending, 3, &cancel).unwrap();
    assert_eq!(owner.position(), 3);
}

#[test]
fn ignored_duplicate_failure_poisoning_and_late_staged_budget_refusal_cannot_publish() {
    let mut owner = state(2_000_000);
    let other = state(2_000_000);
    let cancel = CancellationToken::new();
    let before = snapshot(&owner);
    let mut proposal = prepare(&owner, &tokens());
    assert!(proposal
        .stage_window(39, window(&other, 39, 4), &cancel)
        .is_err());
    assert!(proposal.window(0).is_err());
    assert!(proposal.compressed_source(2).is_err());
    assert!(owner.commit_prefix(proposal, 4, &cancel).is_err());
    assert_eq!(snapshot(&owner), before);
    let mut small = state(8192);
    let before = snapshot(&small);
    let proposal = prepare(&small, &tokens());
    assert!(small.commit_prefix(proposal, 4, &cancel).is_err());
    assert_eq!(snapshot(&small), before);
}

#[test]
fn placement_layer_source_bounds_and_one_outstanding_model_proposal_refuse() {
    let mut owner = state(2_000_000);
    let cancel = CancellationToken::new();
    assert!(owner.window(40).is_err());
    assert!(owner.compressed_source(3).is_err());
    let pending = owner.begin(&tokens(), &cancel).unwrap();
    assert!(owner.begin(&tokens(), &cancel).is_err());
    assert!(pending.window(0).is_err());
    assert!(pending.compressed_source(2).is_err());
    drop(pending);
    let pending = prepare(&owner, &tokens());
    assert!(owner.commit_prefix(pending, 5, &cancel).is_err());
    assert_eq!(owner.position(), 0);
    let architecture = DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap();
    assert!(BackboneState::new(
        &architecture,
        &vec![device(); 39],
        8,
        4,
        MainKvFormat::Fp8Ue8m0,
        1000
    )
    .is_err());
    assert!(BackboneState::new(
        &architecture,
        &vec![device(); 40],
        8,
        4,
        MainKvFormat::Fp8Ue8m0,
        0
    )
    .is_err());
}
