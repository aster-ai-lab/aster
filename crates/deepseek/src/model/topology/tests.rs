use super::*;
use crate::model::DeepSeekModel;

fn architecture() -> TextArchitecture {
    DeepSeekModel::V41Flash1017Official
        .verify_config(include_bytes!("../../../fixtures/official-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap()
}

fn metadata() -> Value {
    serde_json::from_slice(include_bytes!("../../../fixtures/official-config.json")).unwrap()
}

#[test]
fn all_backbone_layers_bind_exact_source_segments_and_only_four_compressors() {
    let model = architecture();
    assert_eq!(model.layers().len(), 40);
    let mut owners = Vec::new();
    let mut indexers = Vec::new();
    for (start, end, kv) in [
        (2, 8, 2),
        (8, 14, 8),
        (14, 20, 14),
        (20, 24, 20),
        (24, 28, 20),
        (28, 32, 20),
        (32, 36, 20),
        (36, 40, 20),
    ] {
        for id in start..end {
            let layer = model.layer(id).unwrap();
            let sources = layer.sources().unwrap();
            assert_eq!(sources.kv(), kv);
            assert_eq!(sources.index_k(), kv);
            assert_eq!(sources.index_topk(), start);
            assert_eq!(sources.candidate(), if id >= 20 { Some(20) } else { None });
            assert_eq!(layer.window_hidden_source(), id);
            assert_eq!(
                layer.compression(),
                Some(if id < 20 {
                    CompressionKind::C2
                } else {
                    CompressionKind::C1
                })
            );
            if layer.is_kv_source() {
                owners.push(id);
            }
            assert_eq!(layer.is_index_k_source(), layer.is_kv_source());
            if layer.is_index_topk_source() {
                indexers.push(id);
            }
            assert_eq!(layer.is_candidate_source(), id == 20);
        }
    }
    assert_eq!(owners, [2, 8, 14, 20]);
    assert_eq!(indexers, [2, 8, 14, 20, 24, 28, 32, 36]);
    for id in 0..2 {
        let layer = model.layer(id).unwrap();
        assert_eq!(layer.sources(), None);
        assert_eq!(layer.compression(), None);
        assert!(!layer.is_kv_source());
    }
}

#[test]
fn hf_defaults_keep_each_swa_hidden_and_disable_extra_query_norm_and_exp_weights() {
    let model = architecture();
    assert!(!model.shared_window_hidden());
    assert!(!model.use_query_head_norm());
    assert!(!model.use_exponential_norm_weights());
    assert!(model.shifted_mhc());
    for layer in model.layers() {
        assert_eq!(layer.window_hidden_source(), layer.layer());
        assert_eq!(
            layer.rope_profile(),
            if layer.layer() < 2 {
                AttentionRopeProfile::WindowBase
            } else {
                AttentionRopeProfile::CompressedYarn
            }
        );
    }
    let numerics = model.mhc_numerics();
    assert_eq!(numerics.norm_epsilon, 1e-20);
    assert_eq!(numerics.pre_epsilon, 1e-6);
    assert_eq!(numerics.sinkhorn_epsilon, 1e-6);
    assert_eq!(numerics.sinkhorn_iterations, 20);
    assert_eq!(numerics.post_multiplier, 2.0);
}

#[test]
fn engram_tables_bind_separately_from_main_and_draft_layer_numbers() {
    let model = architecture();
    let tables: Vec<_> = model
        .layers()
        .iter()
        .filter_map(|layer| {
            layer
                .engram()
                .map(|table| (layer.layer(), table.table(), table.rows()))
        })
        .collect();
    assert_eq!(tables, [(1, 0, 384_006_168), (14, 1, 384_016_682)]);
    for id in [40, 41, 42, 43, usize::MAX] {
        assert!(model.layer(id).is_err());
    }
}

#[test]
fn compressed_state_cannot_be_allocated_for_window_or_consumer_layers() {
    let model = architecture();
    for layer in model.layers() {
        match layer.layer() {
            2 | 8 | 14 => assert_eq!(layer.owned_compression().unwrap(), CompressionKind::C2),
            20 => assert_eq!(layer.owned_compression().unwrap(), CompressionKind::C1),
            _ => assert!(layer.owned_compression().is_err()),
        }
    }
}

#[test]
fn public_admission_still_rejects_modified_artifacts_before_topology_binding() {
    for (name, value) in [
        ("hc_shifted", Value::Bool(false)),
        ("kv_source_layer_ids", serde_json::json!([2, 8, 14])),
        ("compress_ratios", serde_json::json!([0, 0, 2, 1])),
        ("yoco_swa_share_hidden", Value::Bool(true)),
    ] {
        let mut artifact = metadata();
        artifact["text_config"][name] = value;
        assert!(DeepSeekModel::V41Flash1017Official
            .verify_config(&serde_json::to_vec(&artifact).unwrap())
            .is_err());
    }
}

#[test]
fn malformed_source_schedules_and_cross_ratio_sharing_refuse() {
    for invalid_ids in [
        serde_json::json!([]),
        serde_json::json!([0, 2]),
        serde_json::json!([2, 2]),
        serde_json::json!([2, 1]),
        serde_json::json!([2, 40]),
        serde_json::json!([2, -1]),
        serde_json::json!([2, 8.5]),
    ] {
        assert!(sources(Some(&invalid_ids)).is_err());
    }
    let ratios = integers(metadata()["text_config"].get("compress_ratios")).unwrap();
    assert!(validate_refinement(
        &[2, 8, 14, 20],
        &[2, 8, 14, 24],
        &[2, 8, 14, 20, 24],
        &ratios
    )
    .is_err());
    assert!(
        validate_refinement(&[2, 8, 14, 20], &[2, 8, 14, 20], &[2, 8, 14, 22], &ratios).is_err()
    );
    assert!(validate_refinement(&[2, 8, 14], &[2, 8, 14, 20], &[2, 8, 14, 20], &ratios).is_err());
    for (name, value) in [
        ("compress_ratios", serde_json::json!([0, 0, 2])),
        ("index_source_layer_ids", serde_json::json!([2, 8, 14, 20])),
        ("engram_num_embeddings", serde_json::json!([384006168])),
        ("hc_shifted", Value::Bool(false)),
    ] {
        let mut data = metadata();
        data["text_config"][name] = value;
        assert!(TextArchitecture::from_metadata(&data).is_err());
    }
}

#[test]
fn rope_profiles_are_closed_and_keep_distinct_bases() {
    for profile in [
        AttentionRopeProfile::WindowBase,
        AttentionRopeProfile::CompressedYarn,
    ] {
        assert_eq!(
            AttentionRopeProfile::parse(profile.as_str()).unwrap(),
            profile
        );
    }
    assert_eq!(AttentionRopeProfile::WindowBase.theta(), 10000);
    assert_eq!(AttentionRopeProfile::CompressedYarn.theta(), 160000);
    assert!(AttentionRopeProfile::parse("base").is_err());
}

#[test]
fn index_cache_factories_allocate_only_explicit_source_schedules() {
    let model = architecture();
    let device = CudaLayerDevice::cpu_for_tests(aster_power::inference::InferenceLimits::default());
    let mut owners = Vec::new();
    for layer in model.layers() {
        match layer.new_index_kv_state(&device, 16, 8) {
            Ok(state) => {
                owners.push(layer.layer());
                assert_eq!(state.kind(), layer.compression().unwrap());
                assert_eq!(state.position(), 0);
            }
            Err(_) => assert!(!layer.is_index_k_source()),
        }
    }
    assert_eq!(owners, [2, 8, 14, 20]);
}
