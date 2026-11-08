use super::*;
use crate::layers::kv::MainKvFormat;
use crate::model::DeepSeekModel;
use crate::weights::xq::{storage::tests::fixture_with_metadata, XqTensorType};
use aster_power::inference::InferenceLimits;

pub(crate) fn architecture() -> TextArchitecture {
    DeepSeekModel::V41Flash1017Xq
        .verify_config(include_bytes!("../../../../fixtures/xq-model-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap()
}

pub(crate) fn layer(device: &CudaLayerDevice, id: usize) -> LoadedXqLayer<'static> {
    let architecture = architecture();
    let binding = architecture.layer(id).unwrap().clone();
    assert!(
        binding.engram().is_none(),
        "this small fixture does not replace giant Engram tables"
    );
    let attention = crate::layers::xq::attention::sublayer::tests::sublayer(device, 256, 8);
    let ffn = crate::layers::xq::moe::tests::sublayer_for_pipeline(device, id);
    let compressor = binding
        .is_kv_source()
        .then(|| crate::layers::xq::compressor::tests::for_pipeline(device, id));
    let (index_query, index_key, indexer) = if binding.is_index_topk_source() {
        let (q, k) = crate::layers::xq::indexer::tests::for_pipeline(device, id);
        let policy = binding
            .is_candidate_source()
            .then(|| CandidateBlockPolicy::new(CandidateBlockWidth::Eight, 2048).unwrap());
        (
            Some(q),
            k,
            Some(SparseIndexer::new(device.clone(), 2, 128, 512, policy).unwrap()),
        )
    } else {
        (None, None, None)
    };
    let resident_upper_bytes = attention.retained_bytes()
        + ffn.retained_bytes()
        + compressor
            .as_ref()
            .map_or(0, XqKvCompressor::retained_bytes)
        + index_query
            .as_ref()
            .map_or(0, XqIndexQueryProjection::retained_bytes)
        + index_key
            .as_ref()
            .map_or(0, XqIndexKeyProjection::retained_bytes)
        + 16384;
    LoadedXqLayer {
        device: device.clone(),
        binding,
        attention,
        ffn,
        engram: None,
        compressor,
        index_query,
        index_key,
        indexer,
        resident_upper_bytes,
    }
}

pub(crate) fn state(device: &CudaLayerDevice) -> BackboneState {
    BackboneState::new(
        &architecture(),
        &vec![device.clone(); 40],
        8,
        4,
        MainKvFormat::Fp4E4m3,
        2_000_000,
    )
    .unwrap()
}

#[test]
fn loaded_xq_forward_refuses_wrong_state_order_cpu_fallback_and_cancel_without_publication() {
    use crate::engram::tests::text;
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let model = layer(&device, 0);
    let state = state(&device);
    let trunk = || {
        XqMhcContinuation::from_embedding(
            &device,
            &Tensor::ones((1, 256), DType::F32, device.tensor_device()).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap()
    };
    let cancel = CancellationToken::new();
    let mut pending = state.begin(&[text(7)], &cancel).unwrap();
    let mut sharing = StepIndexSharing::new(device.limits(), &[0]).unwrap();
    assert!(model
        .forward_pending(trunk(), &state, &mut pending, &mut sharing, &cancel)
        .is_err()); // no CPU projection fallback
    assert!(pending.is_poisoned());
    assert_eq!(state.position(), 0);
    drop(pending);
    let next = layer(&device, 2);
    let mut pending = state.begin(&[text(7)], &cancel).unwrap();
    let mut sharing = StepIndexSharing::new(device.limits(), &[0]).unwrap();
    assert!(next
        .forward_pending(trunk(), &state, &mut pending, &mut sharing, &cancel)
        .is_err());
    assert!(pending.is_poisoned());
    assert_eq!(state.window(2).unwrap().position(), 0);
    drop(pending);
    let mut pending = state.begin(&[text(7)], &cancel).unwrap();
    let mut sharing = StepIndexSharing::new(device.limits(), &[0]).unwrap();
    cancel.cancel();
    assert!(model
        .forward_pending(trunk(), &state, &mut pending, &mut sharing, &cancel)
        .is_err());
    assert!(pending.is_poisoned());
    assert_eq!(state.position(), 0);
    drop(pending);
    let official = DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap();
    let official = BackboneState::new(
        &official,
        &vec![device.clone(); 40],
        8,
        4,
        MainKvFormat::Fp4E4m3,
        2_000_000,
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let mut pending = official.begin(&[text(7)], &cancel).unwrap();
    let mut sharing = StepIndexSharing::new(device.limits(), &[0]).unwrap();
    assert!(model
        .forward_pending(trunk(), &official, &mut pending, &mut sharing, &cancel)
        .is_err());
    assert!(pending.is_poisoned());
    assert_eq!(official.position(), 0);
}

#[cfg(feature = "cuda")]
mod gpu;

#[test]
fn aggregate_decoder_admission_refuses_before_any_kernel_when_components_fit() {
    use crate::engram::tests::text;
    let baseline_device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let baseline = layer(&baseline_device, 0);
    let baseline_state = state(&baseline_device);
    let bytes = baseline
        .prepare_live_tensor_bytes(&baseline_state, 4)
        .unwrap();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: bytes - 1,
        ..InferenceLimits::default()
    });
    let model = layer(&device, 0);
    let state = state(&device);
    let cancel = CancellationToken::new();
    let mut pending = state
        .begin(&[text(1), text(2), text(3), text(4)], &cancel)
        .unwrap();
    let mut sharing = StepIndexSharing::new(device.limits(), &[0, 1, 2, 3]).unwrap();
    let input = Tensor::ones((4, 256), DType::F32, device.tensor_device()).unwrap();
    let input = XqMhcContinuation::from_embedding(&device, &input, &cancel).unwrap();
    let error = model
        .forward_pending(input, &state, &mut pending, &mut sharing, &cancel)
        .err()
        .unwrap();
    assert!(error.to_string().contains("complete decoder prepare live"));
    assert!(pending.is_poisoned());
    assert_eq!(state.position(), 0);
}

#[test]
fn official_loader_refuses_xq_architecture_before_weight_reads() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let (_dir, weights) =
        crate::weights::tests::fixture(&[("dummy", "F32", vec![1], vec![0; 4])], 8192);
    let refused = crate::weights::text::load::LoadedTextLayer::load(
        &weights,
        &architecture(),
        0,
        &device,
        8,
        MainKvFormat::Fp4E4m3,
        &CancellationToken::new(),
    );
    assert!(refused
        .err()
        .unwrap()
        .to_string()
        .contains("official BF16 architecture"));
}

#[test]
fn exact_xq_loader_refuses_official_architecture_unknown_layer_and_incomplete_weights() {
    // An empty metadata handle is not forged: use the complete pinned map and
    // parameter fixture already validated by the Engram metadata sibling.
    let (_root, weights, metadata) =
        crate::layers::xq::engram::metadata::tests::fixture_for_components();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let official = DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap();
    assert!(LoadedXqLayer::load(
        &weights,
        None,
        &metadata,
        &official,
        0,
        &device,
        8,
        &CancellationToken::new()
    )
    .is_err());
    assert!(LoadedXqLayer::load(
        &weights,
        None,
        &metadata,
        &architecture(),
        40,
        &device,
        8,
        &CancellationToken::new()
    )
    .is_err());
    assert!(LoadedXqLayer::load(
        &weights,
        None,
        &metadata,
        &architecture(),
        0,
        &device,
        8,
        &CancellationToken::new()
    )
    .is_err());
    let (_dir, weights) = fixture_with_metadata(
        &[("blk.1.dummy", vec![1], XqTensorType::F32, vec![0; 4])],
        &[],
        8192,
    );
    assert!(LoadedXqLayer::load(
        &weights,
        None,
        &metadata,
        &architecture(),
        1,
        &device,
        8,
        &CancellationToken::new()
    )
    .is_err());
}
