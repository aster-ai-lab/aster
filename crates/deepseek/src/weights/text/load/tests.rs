use super::*;
use crate::{
    layers::{
        attention::{KvCandidate, KvSource, SparseAttentionPlan},
        kv::MainKvRows,
    },
    model::DeepSeekModel,
    weights::{tests::fixture, MatrixLayout},
};
use aster_power::inference::InferenceLimits;
use candle_core::DType;

fn architecture() -> TextArchitecture {
    DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap()
}

// Reduced geometry is private to this sibling test. Public load() rebuilds
// the complete pinned official schema and refuses this tiny checkpoint.
pub(super) fn tiny_schema(id: usize) -> LayerWeightSchema {
    let mut schema = LayerWeightSchema::new(architecture().layer(id).unwrap());
    let root = format!("layers.{id}");
    let m =
        |name: &str, layout, shape| MatrixBinding::new(&format!("{root}.{name}"), layout, shape);
    let p = |name: &str, dtype, shape: &[usize]| {
        ParameterBinding::new(&format!("{root}.{name}"), dtype, shape)
    };
    let mhc = |kind: &str| MhcBindings {
        projection: p(&format!("hc_{kind}_fn"), "f32", &[24, 128]),
        scale: p(&format!("hc_{kind}_scale"), "f32", &[3]),
        base: p(&format!("hc_{kind}_base"), "f32", &[24]),
    };
    schema.attention = super::super::AttentionBindings {
        query_a: m("attn.wq_a", MatrixLayout::Fp8Block32, [32, 32]),
        query_b: m("attn.wq_b", MatrixLayout::Fp8Block32, [1024, 32]),
        window_kv: m("attn.wkv", MatrixLayout::Fp8Block32, [512, 32]),
        output_a: m("attn.wo_a", MatrixLayout::Fp8Block32, [64, 512]),
        output_b: m("attn.wo_b", MatrixLayout::Fp8Block32, [32, 64]),
        query_norm: p("attn.q_norm.weight", "bf16", &[32]),
        kv_norm: p("attn.kv_norm.weight", "bf16", &[512]),
        norm: p("attn_norm.weight", "bf16", &[32]),
        sink: p("attn.attn_sink", "f32", &[2]),
        mhc: mhc("attn"),
    };
    let expert = |prefix: &str, layout| ExpertBindings {
        gate: m(&format!("{prefix}.w1"), layout, [32, 32]),
        up: m(&format!("{prefix}.w3"), layout, [32, 32]),
        down: m(&format!("{prefix}.w2"), layout, [32, 32]),
    };
    schema.ffn = super::super::FfnBindings {
        router: m("ffn.gate", MatrixLayout::Bf16, [6, 32]),
        router_bias: p("ffn.gate.bias", "f32", &[6]),
        image_router_bias: p("ffn.gate.bias_vl", "f32", &[6]),
        shared: expert("ffn.shared_experts", MatrixLayout::Fp8Block32),
        routed: (0..6)
            .map(|e| expert(&format!("ffn.experts.{e}"), MatrixLayout::Mxfp4Block32))
            .collect(),
        norm: p("ffn_norm.weight", "bf16", &[32]),
        mhc: mhc("ffn"),
    };
    if let Some(c) = &mut schema.compressor {
        c.value = m("attn.compressor.wkv", MatrixLayout::Bf16, [512, 32]);
        if c.gate.is_some() {
            c.gate = Some(m("attn.compressor.wgate", MatrixLayout::Bf16, [512, 32]));
        }
    }
    if let Some(i) = &mut schema.index_query {
        i.query = m("attn.indexer.wq_b", MatrixLayout::Fp8Block32, [128, 32]);
        i.head_weights = m("attn.indexer.weights_proj", MatrixLayout::Bf16, [1, 32]);
    }
    assert!(schema.engram.is_none());
    schema
}

pub(super) fn checkpoint(
    schema: &LayerWeightSchema,
    nonzero_shared: bool,
) -> (tempfile::TempDir, CheckpointWeights) {
    let a = &schema.attention;
    let mut norms = vec![
        a.norm.name(),
        a.query_norm.name(),
        a.kv_norm.name(),
        schema.ffn.norm.name(),
    ];
    if let Some(c) = &schema.compressor {
        norms.push(c.norm.name());
    }
    if let Some(i) = &schema.index_key {
        norms.push(i.norm.name());
    }
    let shared = [
        schema.ffn.shared.gate.weight_name(),
        schema.ffn.shared.up.weight_name(),
        schema.ffn.shared.down.weight_name(),
    ];
    let tensors = schema.tensors();
    let raw = tensors
        .iter()
        .map(|t| {
            let bytes = match t.dtype() {
                "bf16" if norms.contains(&t.name()) => {
                    vec![0x80, 0x3f].repeat(t.bytes() as usize / 2)
                }
                "f8_e8m0" => vec![127; t.bytes() as usize],
                "f8_e4m3" if nonzero_shared && shared.contains(&t.name()) => {
                    vec![56; t.bytes() as usize]
                }
                _ => vec![0; t.bytes() as usize],
            };
            let dtype = match t.dtype() {
                "bf16" => "BF16",
                "f32" => "F32",
                "i8" => "I8",
                "f8_e4m3" => "F8_E4M3",
                "f8_e8m0" => "F8_E8M0",
                _ => panic!("private fixture dtype is outside the exact schema"),
            };
            (t.name(), dtype, t.shape().to_vec(), bytes)
        })
        .collect::<Vec<_>>();
    // One output scale block needs 32*512+16=16400 bytes. Both query-B
    // and grouped Wv are twice this budget and exercise multi-chunk loading.
    fixture(&raw, 16400)
}

#[test]
fn verified_small_checkpoint_loads_real_decoder_and_executes_prefill_and_decode() {
    let architecture = architecture();
    let schema = tiny_schema(0);
    let (_dir, checkpoint) = checkpoint(&schema, false);
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let loaded = load_schema(
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
    assert!(loaded.decoder.retained_bytes() <= loaded.resident_upper_bytes());
    for positions in [vec![0, 1, 2], vec![3]] {
        let embedding =
            Tensor::ones((positions.len(), 32), DType::BF16, device.tensor_device()).unwrap();
        let initial = loaded.decoder.initial_continuation(&embedding).unwrap();
        let prepared = loaded
            .decoder
            .prepare(initial, &positions, None, None, &cancel)
            .unwrap();
        let window = MainKvRows::from_records(
            &device,
            MainKvFormat::Fp8Ue8m0,
            prepared.attention().projected().window_rows().records(),
        )
        .unwrap();
        let candidates = positions
            .iter()
            .map(|&p| KvCandidate::new(KvSource::Window, p))
            .collect::<Vec<_>>();
        let selection = positions
            .iter()
            .map(|&p| {
                positions
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &k)| (k <= p).then_some(i))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let plan =
            SparseAttentionPlan::new(device.limits(), 128, &candidates, &positions, &selection)
                .unwrap();
        let output = loaded
            .decoder
            .finish(prepared, &window, None, &plan, &cancel)
            .unwrap();
        // All branch matrices are exactly zero; zero mHC logits give uniform
        // residual mixing. The initially repeated unit trunk stays unit BF16.
        assert_eq!(
            output
                .residual()
                .to_dtype(DType::F32)
                .unwrap()
                .flatten_all()
                .unwrap()
                .to_vec1::<f32>()
                .unwrap(),
            vec![1.; positions.len() * 4 * 32]
        );
    }
    assert!(LoadedTextLayer::load(
        &checkpoint,
        &architecture,
        0,
        &device,
        4,
        MainKvFormat::Fp8Ue8m0,
        &cancel
    )
    .is_err());
}

#[test]
fn loaded_shared_expert_is_executed_with_independent_clipping_and_fp8_rounding_golden() {
    let architecture = architecture();
    let schema = tiny_schema(0);
    let (_dir, checkpoint) = checkpoint(&schema, true);
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    let cancel = CancellationToken::new();
    let loaded = load_schema(
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
    let embedding = Tensor::ones((1, 32), DType::BF16, device.tensor_device()).unwrap();
    let prepared = loaded
        .decoder
        .prepare(
            loaded.decoder.initial_continuation(&embedding).unwrap(),
            &[0],
            None,
            None,
            &cancel,
        )
        .unwrap();
    let window = MainKvRows::from_records(
        &device,
        MainKvFormat::Fp8Ue8m0,
        prepared.attention().projected().window_rows().records(),
    )
    .unwrap();
    let plan = SparseAttentionPlan::new(
        device.limits(),
        128,
        &[KvCandidate::new(KvSource::Window, 0)],
        &[0],
        &[vec![0]],
    )
    .unwrap();
    let output = loaded
        .decoder
        .finish(prepared, &window, None, &plan, &cancel)
        .unwrap();
    // Each shared gate/up dot is 32. Clipping to 10 gives 10*silu(10),
    // which packs to FP8 96. Down sums 32*96=3072; residual 1 rounds to
    // 3072 in BF16. Routed experts remain zero. A no-op loader cannot pass.
    assert_eq!(
        output
            .residual()
            .to_dtype(DType::F32)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap(),
        vec![3072.; 128]
    );
}

#[test]
fn source_and_consumer_loads_preserve_compressor_index_ownership_and_kind() {
    let architecture = architecture();
    let cancel = CancellationToken::new();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    for id in [2, 20, 24] {
        let schema = tiny_schema(id);
        let (_dir, checkpoint) = checkpoint(&schema, false);
        let loaded = load_schema(
            &checkpoint,
            &schema,
            architecture.layer(id).unwrap(),
            architecture.mhc_numerics(),
            &device,
            4,
            MainKvFormat::Fp8Ue8m0,
            &cancel,
        )
        .unwrap();
        assert!(loaded.index_query.is_some());
        assert!(loaded.indexer.is_some());
        assert_eq!(loaded.compressor.is_some(), id != 24);
        assert_eq!(loaded.index_key.is_some(), id != 24);
        let embedding = Tensor::ones((1, 32), DType::BF16, device.tensor_device()).unwrap();
        assert!(loaded.decoder.initial_continuation(&embedding).is_err());
        if let Some(compressor) = loaded.compressor {
            let binding = architecture.layer(id).unwrap();
            let state = binding.new_compression_state(&device, 4, 4).unwrap();
            let hidden = Tensor::ones((2, 32), DType::BF16, device.tensor_device()).unwrap();
            let proposal = compressor.prepare(&state, 0, &hidden, &cancel).unwrap();
            assert_eq!(
                proposal.emitted_positions(),
                if id == 2 { &[1][..] } else { &[0, 1][..] }
            );
        }
    }
}

#[test]
fn loading_refuses_cancellation_wrong_owner_schema_and_aggregate_residency_before_upload() {
    let architecture = architecture();
    let schema = tiny_schema(0);
    let (_dir, checkpoint) = checkpoint(&schema, false);
    let cancel = CancellationToken::new();
    let device = CudaLayerDevice::cpu_for_tests(InferenceLimits::default());
    assert!(load_schema(
        &checkpoint,
        &schema,
        architecture.layer(2).unwrap(),
        architecture.mhc_numerics(),
        &device,
        4,
        MainKvFormat::Fp8Ue8m0,
        &cancel
    )
    .is_err());
    let bound = resident_bound(&schema, 4).unwrap();
    let limited = CudaLayerDevice::cpu_for_tests(InferenceLimits {
        max_resident_weight_bytes: bound - 1,
        ..InferenceLimits::default()
    });
    assert!(load_schema(
        &checkpoint,
        &schema,
        architecture.layer(0).unwrap(),
        architecture.mhc_numerics(),
        &limited,
        4,
        MainKvFormat::Fp8Ue8m0,
        &cancel
    )
    .is_err());
    assert!(load_schema(
        &checkpoint,
        &schema,
        architecture.layer(0).unwrap(),
        architecture.mhc_numerics(),
        &device,
        0,
        MainKvFormat::Fp8Ue8m0,
        &cancel
    )
    .is_err());
    cancel.cancel();
    assert!(load_schema(
        &checkpoint,
        &schema,
        architecture.layer(0).unwrap(),
        architecture.mhc_numerics(),
        &device,
        4,
        MainKvFormat::Fp8Ue8m0,
        &cancel
    )
    .is_err());
}
