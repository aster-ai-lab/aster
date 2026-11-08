use super::*;
use crate::{
    engram::tests::text,
    layers::{
        kv::{MainKvFormat, MainKvRows},
        CudaLayerDevice,
    },
    model::{topology::TextArchitecture, DeepSeekModel},
    weights::text::load::{
        load_schema,
        tests::{checkpoint, tiny_schema},
    },
};
use aster_power::inference::InferenceLimits;
use candle_core::DType;

fn architecture() -> TextArchitecture {
    DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!(
            "../../../../../fixtures/official-config.json"
        ))
        .unwrap()
        .text_architecture()
        .unwrap()
}
fn state(architecture: &TextArchitecture, device: &CudaLayerDevice) -> BackboneState {
    BackboneState::new(
        architecture,
        &vec![device.clone(); 40],
        4,
        4,
        MainKvFormat::Fp8Ue8m0,
        1_000_000,
    )
    .unwrap()
}
fn hidden(device: &CudaLayerDevice, count: usize) -> Tensor {
    Tensor::ones((count, 32), DType::BF16, device.tensor_device()).unwrap()
}

#[test]
fn loaded_source_executes_actual_projection_index_and_prepared_cache_pipeline_for_odd_and_even_chunks(
) {
    let architecture = architecture();
    let cancel = CancellationToken::new();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let s0 = tiny_schema(0);
    let (_dir0, c0) = checkpoint(&s0, false);
    let s2 = tiny_schema(2);
    let (_dir2, c2) = checkpoint(&s2, false);
    let load = |schema, checkpoint, layer| {
        load_schema(
            checkpoint,
            schema,
            architecture.layer(layer).unwrap(),
            architecture.mhc_numerics(),
            &device,
            4,
            MainKvFormat::Fp8Ue8m0,
            &cancel,
        )
        .unwrap()
    };
    let first = load(&s0, &c0, 0);
    let source = load(&s2, &c2, 2);
    for count in [1, 2] {
        let mut state = state(&architecture, &device);
        let tokens = (0..count).map(|n| text(10 + n as u32)).collect::<Vec<_>>();
        let positions = (0..count).collect::<Vec<_>>();
        let mut pending = state.begin(&tokens, &cancel).unwrap();
        let mut sharing = StepIndexSharing::new(device.limits(), &positions).unwrap();
        let initial = first
            .decoder
            .initial_continuation(&hidden(&device, count))
            .unwrap();
        let continued = first
            .forward_pending(initial, &state, &mut pending, &mut sharing, &cancel)
            .unwrap();
        // Layer1's enormous official Engram table is not part of this tiny
        // fixture. Stage its zero window separately to test the source2 path;
        // this is not a claim of complete model execution.
        let zero = Tensor::zeros((count, 512), DType::BF16, device.tensor_device()).unwrap();
        let rows = MainKvRows::encode(&device, MainKvFormat::Fp8Ue8m0, &zero, &cancel).unwrap();
        let proposal = state
            .window(1)
            .unwrap()
            .prepare(0, count, &positions, rows.records(), &cancel)
            .unwrap();
        pending.stage_window(1, proposal, &cancel).unwrap();
        sharing
            .attention_plan(
                architecture.layer(1).unwrap(),
                &positions,
                None,
                None,
                &cancel,
            )
            .unwrap();
        let result = source
            .forward_pending(continued, &state, &mut pending, &mut sharing, &cancel)
            .unwrap();
        assert_eq!(
            result
                .residual()
                .to_dtype(DType::F32)
                .unwrap()
                .flatten_all()
                .unwrap()
                .to_vec1::<f32>()
                .unwrap(),
            vec![1.; count * 128]
        );
        let source = pending.compressed_source(2).unwrap();
        assert_eq!(source.main().row_count(), count / 2);
        assert_eq!(source.index().row_count(), count / 2);
        assert_eq!(
            source.compression().emitted_positions(),
            if count == 1 { &[][..] } else { &[1][..] }
        );
        assert_eq!(state.position(), 0);
        assert_eq!(state.window(0).unwrap().row_count(), 0);
        assert_eq!(state.compressed_source(2).unwrap().main().row_count(), 0);
        // Three prepared windows cannot publish any of the 53 participants.
        assert!(state.commit_prefix(pending, count, &cancel).is_err());
        assert_eq!(state.position(), 0);
        state.begin(&tokens, &cancel).unwrap();
    }
}

#[test]
fn late_layer_order_failure_poisons_staged_windows_sources_and_allows_fresh_request_after_drop() {
    let architecture = architecture();
    let cancel = CancellationToken::new();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let s0 = tiny_schema(0);
    let (_dir0, c0) = checkpoint(&s0, false);
    let s2 = tiny_schema(2);
    let (_dir2, c2) = checkpoint(&s2, false);
    let first = load_schema(
        &c0,
        &s0,
        architecture.layer(0).unwrap(),
        architecture.mhc_numerics(),
        &device,
        4,
        MainKvFormat::Fp8Ue8m0,
        &cancel,
    )
    .unwrap();
    let source = load_schema(
        &c2,
        &s2,
        architecture.layer(2).unwrap(),
        architecture.mhc_numerics(),
        &device,
        4,
        MainKvFormat::Fp8Ue8m0,
        &cancel,
    )
    .unwrap();
    let state = state(&architecture, &device);
    let tokens = [text(10), text(20)];
    let mut pending = state.begin(&tokens, &cancel).unwrap();
    let mut sharing = StepIndexSharing::new(device.limits(), &[0, 1]).unwrap();
    let input = first
        .decoder
        .initial_continuation(&hidden(&device, 2))
        .unwrap();
    let input = first
        .forward_pending(input, &state, &mut pending, &mut sharing, &cancel)
        .unwrap();
    assert!(source
        .forward_pending(input, &state, &mut pending, &mut sharing, &cancel)
        .is_err());
    assert!(pending.is_poisoned());
    assert!(pending.window(0).is_err());
    assert!(pending.compressed_source(2).is_err());
    assert!(!sharing.is_complete());
    assert_eq!(state.position(), 0);
    assert_eq!(
        state.compressed_source(2).unwrap().compression().position(),
        0
    );
    drop(pending);
    let mut fresh = state.begin(&tokens, &cancel).unwrap();
    let mut sharing = StepIndexSharing::new(device.limits(), &[0, 1]).unwrap();
    let input = first
        .decoder
        .initial_continuation(&hidden(&device, 2))
        .unwrap();
    first
        .forward_pending(input, &state, &mut fresh, &mut sharing, &cancel)
        .unwrap();
}

#[test]
fn request_range_mismatch_and_cancellation_poison_before_publication() {
    let architecture = architecture();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let schema = tiny_schema(0);
    let (_dir, checkpoint) = checkpoint(&schema, false);
    let cancel = CancellationToken::new();
    let first = load_schema(
        &checkpoint,
        &schema,
        architecture.layer(0).unwrap(),
        architecture.mhc_numerics(),
        &device,
        4,
        MainKvFormat::Fp8Ue8m0,
        &cancel,
    )
    .unwrap();
    for cancelled in [false, true] {
        let state = state(&architecture, &device);
        let mut pending = state.begin(&[text(10)], &CancellationToken::new()).unwrap();
        let mut sharing =
            StepIndexSharing::new(device.limits(), if cancelled { &[0] } else { &[1] }).unwrap();
        let input = first
            .decoder
            .initial_continuation(&hidden(&device, 1))
            .unwrap();
        let token = CancellationToken::new();
        if cancelled {
            token.cancel();
        }
        let result = first.forward_pending(input, &state, &mut pending, &mut sharing, &token);
        if cancelled {
            assert!(matches!(result, Err(PowerError::InferenceCancelled)));
        } else {
            assert!(result.is_err());
        }
        assert!(pending.is_poisoned());
        assert_eq!(state.position(), 0);
    }
}
