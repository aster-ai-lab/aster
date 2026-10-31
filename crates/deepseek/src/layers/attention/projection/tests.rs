use super::*;
use crate::layers::{attention::test_support::*, indexer::projection::IndexQueryProjection};
use aster_power::inference::InferenceLimits;

fn weights(rows: usize, cols: usize) -> Vec<u8> {
    let mut data = vec![0_u8; rows * cols];
    for r in 0..rows {
        data[r * cols + r % cols] = if r / 512 == 1 { 184 } else { 56 };
    }
    data
}
fn build(device: &CudaLayerDevice) -> AttentionInputProjection {
    AttentionInputProjection::new(
        device.clone(),
        matrix(device, 32, 32, &weights(32, 32), &[127]),
        matrix(device, 1024, 32, &weights(1024, 32), &[127; 32]),
        matrix(device, 512, 32, &weights(512, 32), &[128; 16]),
        norm(device, 32),
        norm(device, 512),
        cache(),
    )
    .unwrap()
}
fn hidden() -> Tensor {
    let mut v: Vec<_> = (0..96)
        .map(|i| ((i * 7 + 5) % 41) as f32 / 16. - 1.)
        .collect();
    v[0] = 0.2578125;
    v[1] = 6.;
    bf16(&v, &[3, 32])
}

#[test]
fn main_query_and_current_hidden_swa_match_independent_scalar_pipeline() {
    let device = device();
    let owner = build(&device);
    let input = hidden();
    let before = floats(&input);
    let cancel = CancellationToken::new();
    let result = owner.forward(&input, &[0, 2, 3], &cancel).unwrap();
    let mut expected_q = Vec::new();
    let mut expected_kv = Vec::new();
    let mut latent_data = Vec::new();
    let mut latent_sf = Vec::new();
    for (token, position) in [0, 2, 3].into_iter().enumerate() {
        let input = &before[token * 32..(token + 1) * 32];
        let cast = scalar_fp8(input).2;
        let qa = scalar_linear(&cast, &weights(32, 32), &[127], 32, 32);
        let normalized = scalar_norm(&qa, &norm_weights(32));
        let (data, sf, normalized) = scalar_fp8(&normalized);
        latent_data.extend(data);
        latent_sf.extend(sf);
        let q = scalar_linear(&normalized, &weights(1024, 32), &[127; 32], 1024, 32);
        for head in q.chunks_exact(512) {
            expected_q.extend(scalar_rope(head, position, false));
        }
        let kv = scalar_linear(&cast, &weights(512, 32), &[128; 16], 512, 32);
        let kv = scalar_norm(&kv, &norm_weights(512));
        expected_kv.extend(scalar_rope(&kv, position, false));
    }
    assert_eq!(floats(result.query()), expected_q);
    assert_eq!(bytes(result.normalized_query().data_bytes()), latent_data);
    assert_eq!(bytes(result.normalized_query().scale_bytes()), latent_sf);
    let expected_kv = MainKvRows::encode(
        &device,
        MainKvFormat::Fp8Ue8m0,
        &bf16(&expected_kv, &[3, 512]),
        &cancel,
    )
    .unwrap();
    assert_eq!(
        bytes(result.window_rows().records()),
        bytes(expected_kv.records())
    );
    assert_eq!(result.positions(), [0, 2, 3]);
    assert_eq!(owner.heads(), 2);
    assert_eq!(owner.hidden_size(), 32);
    assert_eq!(floats(&input), before);
    assert_ne!(
        expected_q[1], 6.,
        "the learned latent norm must actually run"
    );
}

#[test]
fn token_chunks_preserve_main_and_index_queries_using_the_same_packed_latent() {
    let device = device();
    let owner = build(&device);
    let input = hidden();
    let cancel = CancellationToken::new();
    let all = owner.forward(&input, &[0, 2, 3], &cancel).unwrap();
    let hw = DeviceMatrix::from_tensors_for_tests(
        &device,
        MatrixLayout::Bf16,
        [2, 32],
        &bf16(&[1.; 64], &[2, 32]),
        None,
    )
    .unwrap();
    let index = IndexQueryProjection::new(
        device.clone(),
        matrix(&device, 256, 32, &weights(256, 32), &[127; 8]),
        hw,
        cache(),
    )
    .unwrap();
    let indexed = index
        .forward_packed(&input, all.normalized_query(), all.positions(), &cancel)
        .unwrap();
    let mut q = Vec::new();
    let mut kv = Vec::new();
    let mut iq = Vec::new();
    let mut isf = Vec::new();
    let mut iw = Vec::new();
    for (row, pos) in [0, 2, 3].into_iter().enumerate() {
        let hidden = input.narrow(0, row, 1).unwrap();
        let one = owner.forward(&hidden, &[pos], &cancel).unwrap();
        q.extend(floats(one.query()));
        kv.extend(bytes(one.window_rows().records()));
        let one_index = index
            .forward_packed(&hidden, one.normalized_query(), &[pos], &cancel)
            .unwrap();
        iq.extend(bytes(one_index.rows().data_bytes()));
        isf.extend(bytes(one_index.rows().scale_bytes()));
        iw.extend(floats(one_index.head_weights()));
    }
    assert_eq!(q, floats(all.query()));
    assert_eq!(kv, bytes(all.window_rows().records()));
    assert_eq!(iq, bytes(indexed.rows().data_bytes()));
    assert_eq!(isf, bytes(indexed.rows().scale_bytes()));
    assert_eq!(iw, floats(indexed.head_weights()));
    assert!(index
        .forward_packed(
            &input.narrow(0, 0, 1).unwrap(),
            all.normalized_query(),
            &[0],
            &cancel
        )
        .is_err());
}

#[test]
fn missing_norm_wrong_layout_width_and_foreign_limits_refuse() {
    let device = device();
    let make = |qnorm| {
        AttentionInputProjection::new(
            device.clone(),
            matrix(&device, 32, 32, &weights(32, 32), &[127]),
            matrix(&device, 1024, 32, &weights(1024, 32), &[127; 32]),
            matrix(&device, 512, 32, &weights(512, 32), &[127; 16]),
            qnorm,
            norm(&device, 512),
            cache(),
        )
    };
    assert!(make(EffectiveRmsNorm::new(device.clone(), 32, 1e-20, None).unwrap()).is_err());
    assert!(make(norm(&device, 64)).is_err());
    let other = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_context_tokens: 32,
        ..InferenceLimits::default()
    });
    assert!(make(norm(&other, 32)).is_err());
    let wrong = DeviceMatrix::from_tensors_for_tests(
        &device,
        MatrixLayout::Bf16,
        [32, 32],
        &bf16(&[1.; 1024], &[32, 32]),
        None,
    )
    .unwrap();
    assert!(AttentionInputProjection::new(
        device.clone(),
        wrong,
        matrix(&device, 1024, 32, &weights(1024, 32), &[127; 32]),
        matrix(&device, 512, 32, &weights(512, 32), &[127; 16]),
        norm(&device, 32),
        norm(&device, 512),
        cache()
    )
    .is_err());
}

#[test]
fn runtime_dtype_position_finite_cancel_and_aggregate_budgets_refuse() {
    let device = device();
    let owner = build(&device);
    let cancel = CancellationToken::new();
    assert!(owner.forward(&hidden(), &[0, 1, 4], &cancel).is_err());
    assert!(owner.forward(&hidden(), &[0, 1], &cancel).is_err());
    assert!(owner
        .forward(&hidden().to_dtype(DType::F32).unwrap(), &[0, 1, 2], &cancel)
        .is_err());
    assert!(owner
        .forward(&bf16(&[f32::NAN; 96], &[3, 32]), &[0, 1, 2], &cancel)
        .is_err());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        owner.forward(&hidden(), &[0, 1, 2], &cancelled),
        Err(PowerError::InferenceCancelled)
    ));
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: 45_000,
        ..InferenceLimits::default()
    });
    assert!(AttentionInputProjection::new(
        low.clone(),
        matrix(&low, 32, 32, &weights(32, 32), &[127]),
        matrix(&low, 1024, 32, &weights(1024, 32), &[127; 32]),
        matrix(&low, 512, 32, &weights(512, 32), &[128; 16]),
        norm(&low, 32),
        norm(&low, 512),
        cache()
    )
    .is_err());
    let low = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_state_bytes: 900_000,
        ..InferenceLimits::default()
    });
    let owner = build(&low);
    let error = owner.forward(&hidden(), &[0, 1, 2], &cancel).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("attention input composite live tensors"),
        "{error}"
    );
}
