use super::*;
use crate::layers::xq::quant::{XqActivationFormat, XqQuantizedActivation};
use candle_core::Device;

fn packed(device: &CudaLayerDevice, coordinates: &[(usize, f32)]) -> Fp4IndexRows {
    let mut values = vec![0_f32; coordinates.len() * INDEX_HEAD_DIM];
    for (row, (col, value)) in coordinates.iter().enumerate() {
        values[row * INDEX_HEAD_DIM + col] = *value;
    }
    let input = Tensor::from_vec(
        values,
        (coordinates.len(), INDEX_HEAD_DIM),
        device.tensor_device(),
    )
    .unwrap();
    XqQuantizedActivation::encode(
        device,
        XqActivationFormat::IndexFp4,
        &input,
        &CancellationToken::new(),
    )
    .unwrap()
    .into_index_rows(&CancellationToken::new())
    .unwrap()
}

#[test]
fn xq_retains_f32_weight_and_score_differences_that_official_bf16_rounds_to_ties() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let query = packed(&device, &[(0, 1.), (1, 1.)]);
    let keys = packed(&device, &[(0, 1.), (1, 1.)]);
    let plan = SparseIndexPlan::new(device.limits(), &[0, 1], &[1], None).unwrap();
    let selector = SparseIndexer::new(device, 2, INDEX_HEAD_DIM, 1, None).unwrap();
    let weights = Tensor::new(&[[1.0001_f32, 1.0003]], &Device::Cpu).unwrap();
    let xq = selector
        .select_xq_f32_packed(&query, &keys, &weights, &plan, &CancellationToken::new())
        .unwrap();
    assert_eq!(xq.rows().to_vec2::<u32>().unwrap(), [vec![1]]);
    let official = selector
        .select_packed(
            &query,
            &keys,
            &weights.to_dtype(DType::BF16).unwrap(),
            &plan,
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(official.rows().to_vec2::<u32>().unwrap(), [vec![0]]);
    assert!(selector
        .select_xq_f32_packed(
            &query,
            &keys,
            &weights.to_dtype(DType::BF16).unwrap(),
            &plan,
            &CancellationToken::new()
        )
        .is_err());
    assert!(selector
        .select_packed(&query, &keys, &weights, &plan, &CancellationToken::new())
        .is_err());
}

#[test]
fn xq_candidate_last_block_gets_the_finite_pinned_runtime_bonus() {
    let scores = Tensor::from_vec(
        (0..16)
            .map(|i| if i == 0 { 2e30_f32 } else { 0. })
            .collect::<Vec<_>>(),
        16,
        &Device::Cpu,
    )
    .unwrap();
    let policy = CandidateBlockPolicy::new(CandidateBlockWidth::Eight, 1).unwrap();
    let cancel = CancellationToken::new();
    let xq = select_blocks_typed(&scores, policy, DType::F32, &cancel)
        .unwrap()
        .to_vec1::<u32>()
        .unwrap();
    let official = select_blocks(&scores, policy, &cancel)
        .unwrap()
        .to_vec1::<u32>()
        .unwrap();
    assert_eq!(xq, [0]);
    assert_eq!(official, [1]);
}

#[test]
fn xq_packed_selection_keeps_causal_restrictions_padding_and_cancellation() {
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let query = packed(&device, &[(0, 1.), (0, 1.)]);
    let keys = packed(&device, &[(0, 1.), (0, 2.), (0, 4.)]);
    let plan = SparseIndexPlan::new(
        device.limits(),
        &[1, 3, 5],
        &[0, 4],
        Some(&[vec![], vec![1]]),
    )
    .unwrap();
    let selector = SparseIndexer::new(device.clone(), 1, INDEX_HEAD_DIM, 2, None).unwrap();
    let weights = Tensor::ones((2, 1), DType::F32, &Device::Cpu).unwrap();
    let result = selector
        .select_xq_f32_packed(&query, &keys, &weights, &plan, &CancellationToken::new())
        .unwrap();
    assert_eq!(
        result.rows().to_vec2::<u32>().unwrap(),
        [vec![INVALID_INDEX_ROW; 2], vec![1, INVALID_INDEX_ROW]]
    );
    assert_eq!(result.valid_lengths(), [0, 1]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(selector
        .select_xq_f32_packed(&query, &keys, &weights, &plan, &cancel)
        .is_err());
    let wrong = SparseIndexer::new(
        device,
        1,
        INDEX_HEAD_DIM,
        2,
        Some(CandidateBlockPolicy::new(CandidateBlockWidth::Sixteen, 1).unwrap()),
    )
    .unwrap();
    assert!(wrong
        .select_xq_f32_packed(&query, &keys, &weights, &plan, &CancellationToken::new())
        .is_err());
}
