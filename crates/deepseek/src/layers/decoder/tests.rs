use super::*;
use crate::{
    engram::TABLES_PER_LAYER,
    layers::{
        attention::{
            branch::tests::plan,
            sublayer::tests::{build_layer, initial},
        },
        engram::{
            lookup::tests::{checkpoint, table},
            projection::tests::{gate, projection},
        },
    },
    model::DeepSeekModel,
    state::kv::{KvCacheKind, KvState},
};
use aster_power::{error::PowerError, inference::InferenceLimits};

fn build(id: usize, limits: InferenceLimits, with_engram: bool) -> Result<TextDecoderLayer> {
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let architecture = DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap();
    let engram = with_engram.then(|| {
        EngramProjection::new(device.clone(), projection(&device, false), gate(&device)).unwrap()
    });
    TextDecoderLayer::new(
        device,
        architecture.layer(id).unwrap(),
        build_layer(limits.clone()),
        crate::layers::sublayer::tests::sublayer(limits),
        engram,
    )
}
fn floats(t: &Tensor) -> Vec<f32> {
    t.to_dtype(DType::F32)
        .unwrap()
        .flatten_all()
        .unwrap()
        .to_vec1()
        .unwrap()
}

#[test]
fn full_attention_ffn_layer_matches_independent_residual_golden_in_prefill_and_decode() {
    let owner = build(0, InferenceLimits::default(), false).unwrap();
    let device = &owner.device;
    let cancel = CancellationToken::new();
    let prepared = owner
        .prepare(
            initial(&owner.attention, 3),
            &[0, 1, 2],
            None,
            None,
            &cancel,
        )
        .unwrap();
    let window = MainKvRows::from_records(
        device,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        prepared.attention().projected().window_rows().records(),
    )
    .unwrap();
    let output = owner
        .finish(
            prepared,
            &window,
            None,
            &plan(device, &[0, 1, 2], &[0, 1, 2]),
            &cancel,
        )
        .unwrap();
    // Independent attention outputs 512/704/768, uniform post mix, then
    // shared-first expert outputs 24+18+54=96 after its learned norm.
    let golden = [vec![608.; 128], vec![800.; 128], vec![864.; 128]].concat();
    assert_eq!(floats(output.residual()), golden);
    let mut state = KvState::new(
        device,
        KvCacheKind::Window,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        4,
        4,
    )
    .unwrap();
    let mut decoded = Vec::new();
    for position in 0..3 {
        let prepared = owner
            .prepare(
                initial(&owner.attention, 1),
                &[position],
                None,
                None,
                &cancel,
            )
            .unwrap();
        let proposal = state
            .prepare(
                position,
                1,
                &[position],
                prepared.attention().projected().window_rows().records(),
                &cancel,
            )
            .unwrap();
        let keys: Vec<_> = proposal.ready_positions().collect();
        let view = proposal.attention_rows(device).unwrap();
        let output = owner
            .finish(
                prepared,
                &view,
                None,
                &plan(device, &[position], &keys),
                &cancel,
            )
            .unwrap();
        decoded.extend(floats(output.residual()));
        state.commit_prefix(proposal, 1, &cancel).unwrap();
    }
    assert_eq!(decoded, golden);
}

#[test]
fn engram_intervenes_before_attention_pre_and_preserves_both_shifted_handoffs() {
    let owner = build(1, InferenceLimits::default(), true).unwrap();
    let device = &owner.device;
    let cancel = CancellationToken::new();
    let (_dir, checkpoint) = checkpoint(
        [vec![80; 256], vec![0; 256]].concat(),
        vec![127; 16],
        2,
        528,
    );
    let mut hash = [0; TABLES_PER_LAYER];
    hash[1] = 1;
    let embeddings = table(device, &checkpoint, 2)
        .fetch(&[hash; 3], &cancel)
        .unwrap();
    let prepared = owner
        .prepare(
            initial(&owner.attention, 3),
            &[0, 1, 2],
            Some(&embeddings),
            None,
            &cancel,
        )
        .unwrap();
    let window = MainKvRows::from_records(
        device,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        prepared.attention().projected().window_rows().records(),
    )
    .unwrap();
    let output = owner
        .finish(
            prepared,
            &window,
            None,
            &plan(device, &[0, 1, 2], &[0, 1, 2]),
            &cancel,
        )
        .unwrap();
    // Table value 8, zero keys and half gate add 4 to every stream before
    // attention. Both shifted mixes and FFN preserve that residual increment.
    assert_eq!(
        floats(output.residual()),
        [vec![612.; 128], vec![804.; 128], vec![868.; 128]].concat()
    );
}

#[test]
fn complete_layer_refuses_wrong_engram_placement_masks_foreign_halves_and_cancel() {
    assert!(build(0, InferenceLimits::default(), true).is_err());
    assert!(build(1, InferenceLimits::default(), false).is_err());
    let owner = build(0, InferenceLimits::default(), false).unwrap();
    let other = build(0, InferenceLimits::default(), false).unwrap();
    let device = &owner.device;
    let cancel = CancellationToken::new();
    let bad = Tensor::new(&[2_u8], device.tensor_device()).unwrap();
    assert!(owner
        .prepare(
            initial(&owner.attention, 1),
            &[0],
            None,
            Some(&bad),
            &cancel
        )
        .is_err());
    let prepared = owner
        .prepare(initial(&owner.attention, 1), &[0], None, None, &cancel)
        .unwrap();
    let window = MainKvRows::from_records(
        device,
        crate::layers::kv::MainKvFormat::Fp8Ue8m0,
        prepared.attention().projected().window_rows().records(),
    )
    .unwrap();
    assert!(other
        .finish(prepared, &window, None, &plan(device, &[0], &[0]), &cancel)
        .is_err());
    let prepared = owner
        .prepare(initial(&owner.attention, 1), &[0], None, None, &cancel)
        .unwrap();
    let stop = CancellationToken::new();
    stop.cancel();
    assert!(matches!(
        owner.finish(prepared, &window, None, &plan(device, &[0], &[0]), &stop),
        Err(PowerError::InferenceCancelled)
    ));
    assert!(build(
        0,
        InferenceLimits {
            max_resident_weight_bytes: owner.retained_bytes() - 1,
            ..InferenceLimits::default()
        },
        false
    )
    .is_err());
}
