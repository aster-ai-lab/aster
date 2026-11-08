use super::*;
use crate::{
    layers::{
        attention::SparseMqaAttention,
        indexer::{CandidateBlockPolicy, CandidateBlockWidth, SparseIndexer},
        quant::Fp4IndexRows,
        CudaLayerDevice,
    },
    model::{topology::TextArchitecture, DeepSeekModel},
};
use candle_core::Tensor;

fn architecture() -> TextArchitecture {
    DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap()
}
fn compute(
    sharing: &StepIndexSharing,
    binding: &LayerBinding,
    ready: &[usize],
    device: &CudaLayerDevice,
) -> SparseIndexSelection {
    let policy = if binding.is_candidate_source() {
        Some(CandidateBlockPolicy::new(CandidateBlockWidth::Eight, 1).unwrap())
    } else {
        None
    };
    let indexer = SparseIndexer::new(device.clone(), 1, 128, 512, policy).unwrap();
    let cancel = CancellationToken::new();
    let queries = Fp4IndexRows::encode(
        device,
        &Tensor::zeros((1, 128), DType::BF16, device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    let keys = Fp4IndexRows::encode(
        device,
        &Tensor::zeros((ready.len(), 128), DType::BF16, device.tensor_device()).unwrap(),
        &cancel,
    )
    .unwrap();
    indexer
        .select_packed(
            &queries,
            &keys,
            &Tensor::ones((1, 1), DType::BF16, device.tensor_device()).unwrap(),
            &sharing.index_plan(binding, ready).unwrap(),
            &cancel,
        )
        .unwrap()
}
fn ready(binding: &LayerBinding) -> Option<Vec<usize>> {
    binding.compression().map(|c| {
        (0..24 / c.ratio())
            .map(|row| (row + 1) * c.ratio() - 1)
            .collect()
    })
}

#[test]
fn forty_layer_sharing_reuses_nearest_topk_and_layer20_candidates_with_real_attention() {
    let architecture = architecture();
    let limits = InferenceLimits::default();
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let cancel = CancellationToken::new();
    let mut sharing = StepIndexSharing::new(&limits, &[23]).unwrap();
    let attention = SparseMqaAttention::new(
        device.clone(),
        1,
        512,
        Tensor::zeros(1, DType::F32, device.tensor_device()).unwrap(),
    )
    .unwrap();
    let query = Tensor::zeros((1, 1, 512), DType::BF16, device.tensor_device()).unwrap();
    for binding in architecture.layers() {
        let main = ready(binding);
        let selection = binding
            .is_index_topk_source()
            .then(|| compute(&sharing, binding, main.as_ref().unwrap(), &device));
        let plan = sharing
            .attention_plan(binding, &[23], main.as_deref(), selection.as_ref(), &cancel)
            .unwrap();
        let count = main.as_ref().map_or(0, Vec::len);
        let mut values = vec![0_f32; 512];
        for row in 0..count {
            values.extend(vec![
                if binding.layer() >= 20 && row >= 16 {
                    1.
                } else {
                    0.
                };
                512
            ]);
        }
        let kv = Tensor::from_vec(values, (1 + count, 512), device.tensor_device())
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap();
        let output = attention
            .forward_decoded(&query, &kv, &plan, &cancel)
            .unwrap()
            .to_dtype(DType::F32)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        // Layer20..23 attends the 24 shared rows, eight carrying value1:
        // sum8/(24 main+1 window+1 sink). From24 on, layer20's forced most
        // recent eight-row candidate block restricts all later index sources.
        let expected = if binding.layer() < 20 {
            0.
        } else if binding.layer() < 24 {
            0.30859375
        } else {
            0.80078125
        };
        assert_eq!(output, vec![expected; 512], "layer {}", binding.layer());
    }
    assert!(sharing.is_complete());
    assert!(sharing
        .attention_plan(
            architecture.layer(39).unwrap(),
            &[23],
            Some(&(0..24).collect::<Vec<_>>()),
            None,
            &cancel
        )
        .is_err());
}

#[test]
fn mismatched_prepared_rows_or_missing_source_never_produce_a_reusable_plan() {
    let architecture = architecture();
    let limits = InferenceLimits::default();
    let device = CudaLayerDevice::cpu_for_tests(limits.clone());
    let cancel = CancellationToken::new();
    let mut sharing = StepIndexSharing::new(&limits, &[23]).unwrap();
    for id in 0..2 {
        sharing
            .attention_plan(architecture.layer(id).unwrap(), &[23], None, None, &cancel)
            .unwrap();
    }
    let binding = architecture.layer(2).unwrap();
    let main = ready(binding).unwrap();
    assert!(sharing
        .attention_plan(binding, &[23], Some(&main), None, &cancel)
        .is_err());
    assert!(sharing.index_plan(binding, &main).is_err());
    assert!(!sharing.is_complete());
    let mut sharing = StepIndexSharing::new(&limits, &[23]).unwrap();
    for id in 0..3 {
        let binding = architecture.layer(id).unwrap();
        let main = ready(binding);
        let selected = binding
            .is_index_topk_source()
            .then(|| compute(&sharing, binding, main.as_ref().unwrap(), &device));
        sharing
            .attention_plan(binding, &[23], main.as_deref(), selected.as_ref(), &cancel)
            .unwrap();
    }
    assert!(sharing
        .attention_plan(
            architecture.layer(3).unwrap(),
            &[23],
            Some(&main[..11]),
            None,
            &cancel
        )
        .is_err());
    assert!(!sharing.is_complete());
}

#[test]
fn ignored_sequence_and_cancellation_errors_poison_only_the_current_step() {
    let architecture = architecture();
    let limits = InferenceLimits::default();
    let cancel = CancellationToken::new();
    let mut sharing = StepIndexSharing::new(&limits, &[23]).unwrap();
    assert!(sharing
        .attention_plan(architecture.layer(1).unwrap(), &[23], None, None, &cancel)
        .is_err());
    assert!(sharing
        .attention_plan(architecture.layer(0).unwrap(), &[23], None, None, &cancel)
        .is_err());
    let mut sharing = StepIndexSharing::new(&limits, &[23]).unwrap();
    cancel.cancel();
    assert!(matches!(
        sharing.attention_plan(architecture.layer(0).unwrap(), &[23], None, None, &cancel),
        Err(PowerError::InferenceCancelled)
    ));
    assert!(sharing
        .attention_plan(
            architecture.layer(0).unwrap(),
            &[23],
            None,
            None,
            &CancellationToken::new()
        )
        .is_err());
    let mut fresh = StepIndexSharing::new(&limits, &[23]).unwrap();
    fresh
        .attention_plan(
            architecture.layer(0).unwrap(),
            &[23],
            None,
            None,
            &CancellationToken::new(),
        )
        .unwrap();
}

#[test]
fn metadata_and_position_refusals_happen_before_device_readback_or_expansion() {
    let limits = InferenceLimits::default();
    for positions in [
        vec![],
        vec![0, 2],
        vec![2, 1],
        vec![limits.max_context_tokens],
    ] {
        assert!(StepIndexSharing::new(&limits, &positions).is_err());
    }
    let limited = InferenceLimits {
        max_graph_plan_bytes: 1024,
        ..limits.clone()
    };
    assert!(StepIndexSharing::new(&limited, &[23]).is_err());
    let architecture = architecture();
    let mut sharing = StepIndexSharing::new(&limits, &[23]).unwrap();
    assert!(sharing
        .attention_plan(
            architecture.layer(0).unwrap(),
            &[23, 22],
            None,
            None,
            &CancellationToken::new()
        )
        .is_err());
}
