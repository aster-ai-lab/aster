use super::*;
use aster_power::inference::DevicePreference;

const CONFIG: &[u8] = include_bytes!("../../fixtures/official-config.json");
const XQ_CONFIG: &[u8] = include_bytes!("../../fixtures/xq-model-config.json");

#[test]
fn xq_identity_and_metadata_are_pinned_separately_despite_identical_topology() {
    let model = DeepSeekModel::V41Flash1017Xq;
    assert_eq!(DeepSeekModel::parse(model.as_str()).unwrap(), model);
    assert_eq!(model.as_str(), XQ_REPOSITORY);
    assert_eq!(model.revision(), XQ_REVISION);
    assert_eq!(model.served_alias(), "deepseek-v4.1-flash-1017-xq");
    let verified = model.verify_config(XQ_CONFIG).unwrap();
    assert_eq!(verified.model(), model);
    assert_eq!(verified.sha256(), XQ_CONFIG_SHA256);
    assert_ne!(verified.sha256(), OFFICIAL_CONFIG_SHA256);
    let official = DeepSeekModel::V41Flash1017Official
        .verify_config(CONFIG)
        .unwrap();
    assert_eq!(verified.metadata(), official.metadata());
    assert!(model.verify_config(CONFIG).is_err());
    assert!(DeepSeekModel::V41Flash1017Official
        .verify_config(XQ_CONFIG)
        .is_err());
    let architecture = verified.text_architecture().unwrap();
    assert_eq!(
        architecture.compression_numerics(),
        crate::state::CompressionNumerics::XqF32
    );
    assert!(architecture
        .layers()
        .iter()
        .all(|l| l.compression_numerics() == architecture.compression_numerics()));
    assert_eq!(architecture.layers().len(), 40);
    let mut changed: serde_json::Value = serde_json::from_slice(XQ_CONFIG).unwrap();
    changed["text_config"]["hidden_size"] = serde_json::json!(4096);
    assert!(model
        .verify_config(&serde_json::to_vec(&changed).unwrap())
        .is_err());
}

#[test]
fn xq_kernel_requirements_refuse_official_only_inventory_and_every_missing_group() {
    let model = DeepSeekModel::V41Flash1017Xq;
    let ranks: Vec<_> = (0..8).collect();
    let capabilities = model.required_capabilities();
    for capability in capabilities {
        assert_eq!(
            NativeCapability::parse(capability.as_str()).unwrap(),
            *capability
        );
    }
    assert!(
        validate_runtime_requirements_for_model(true, model, &ranks, &devices(), capabilities)
            .is_ok()
    );
    assert!(validate_runtime_requirements_for_model(
        false,
        model,
        &ranks,
        &devices(),
        capabilities
    )
    .is_err());
    assert!(validate_runtime_requirements_for_model(
        true,
        model,
        &ranks,
        &devices(),
        &NativeCapability::BASE_TEXT
    )
    .is_err());
    assert!(validate_runtime_requirements(true, &ranks, &devices(), capabilities).is_err());
    for omitted in 0..capabilities.len() {
        let partial: Vec<_> = capabilities
            .iter()
            .enumerate()
            .filter_map(|(i, x)| (i != omitted).then_some(*x))
            .collect();
        assert!(
            validate_runtime_requirements_for_model(true, model, &ranks, &devices(), &partial)
                .is_err()
        );
    }
}

fn devices() -> Vec<RuntimeDeviceIdentity> {
    (0..8)
        .map(|ordinal| RuntimeDeviceIdentity {
            kind: RuntimeDeviceKind::Cuda,
            ordinal: Some(ordinal),
        })
        .collect()
}

#[test]
fn closed_identities_roundtrip_and_refuse_aliases() {
    let model = DeepSeekModel::V41Flash1017Official;
    assert_eq!(DeepSeekModel::parse(model.as_str()).unwrap(), model);
    assert_eq!(model.revision(), OFFICIAL_REVISION);
    for name in [
        "",
        "deepseek-v4.1-flash-1017",
        "deepseek_v41",
        "other/model",
    ] {
        assert!(DeepSeekModel::parse(name).is_err(), "{name}");
    }
    assert_eq!(
        NativeExecution::parse(NativeExecution::Cuda.as_str()).unwrap(),
        NativeExecution::Cuda
    );
    for name in ["", "cpu", "auto", "cuda", "proxy", "http", "sglang"] {
        assert!(NativeExecution::parse(name).is_err(), "{name}");
    }
    for capability in NativeCapability::BASE_TEXT {
        assert_eq!(
            NativeCapability::parse(capability.as_str()).unwrap(),
            capability
        );
    }
    assert!(NativeCapability::parse("dspark").is_err());
    assert!(NativeCapability::parse("vision").is_err());
}

#[test]
fn pinned_config_proves_only_exact_artifact_metadata() {
    let verified = DeepSeekModel::V41Flash1017Official
        .verify_config(CONFIG)
        .unwrap();
    assert_eq!(verified.model(), DeepSeekModel::V41Flash1017Official);
    assert_eq!(verified.sha256(), OFFICIAL_CONFIG_SHA256);
    assert_eq!(verified.metadata()["text_config"]["hidden_size"], 5120);
    assert_eq!(verified.metadata()["text_config"]["num_hidden_layers"], 40);
    assert_eq!(
        verified.metadata()["quantization_config"]["weight_block_size"],
        serde_json::json!([32, 32])
    );
    assert_eq!(
        verified.metadata()["quantization_config"]["expert_dtype"],
        "fp4"
    );
}

#[test]
fn unknown_schema_quantization_and_changed_bytes_refuse() {
    let model = DeepSeekModel::V41Flash1017Official;
    for (pointer, replacement) in [
        ("/model_type", serde_json::json!("deepseek_v3")),
        ("/architectures", serde_json::json!(["OtherModel"])),
        ("/text_config/model_type", serde_json::json!("unknown")),
        ("/text_config/num_hidden_layers", serde_json::json!(41)),
        (
            "/quantization_config/expert_dtype",
            serde_json::json!("int8"),
        ),
    ] {
        let mut metadata: serde_json::Value = serde_json::from_slice(CONFIG).unwrap();
        *metadata.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            model
                .verify_config(&serde_json::to_vec(&metadata).unwrap())
                .is_err(),
            "{pointer}"
        );
    }
    let mut changed = CONFIG.to_vec();
    changed.push(b' ');
    assert!(model.verify_config(&changed).is_err());
}

#[test]
fn malformed_truncated_and_oversized_metadata_refuse() {
    let model = DeepSeekModel::V41Flash1017Official;
    for bytes in [
        b"".as_slice(),
        b"[]",
        b"null",
        b"not JSON",
        &CONFIG[..CONFIG.len() - 1],
    ] {
        // A final newline may be removed without truncating JSON; digest still refuses.
        assert!(model.verify_config(bytes).is_err());
    }
    assert!(model
        .verify_config(&vec![b' '; MAX_CHECKPOINT_CONFIG_BYTES + 1])
        .is_err());
}

#[test]
fn complete_requirements_need_cuda_and_all_kernel_groups() {
    let ordinals: Vec<_> = (0..8).collect();
    let caps = NativeCapability::BASE_TEXT;
    assert!(validate_runtime_requirements(true, &ordinals, &devices(), &caps).is_ok());
    assert!(validate_runtime_requirements(false, &ordinals, &devices(), &caps).is_err());
    for omitted in 0..caps.len() {
        let partial: Vec<_> = caps
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, item)| (index != omitted).then_some(item))
            .collect();
        assert!(validate_runtime_requirements(true, &ordinals, &devices(), &partial).is_err());
    }
    let mut duplicate = caps.to_vec();
    duplicate.push(caps[0]);
    assert!(validate_runtime_requirements(true, &ordinals, &devices(), &duplicate).is_err());
}

#[test]
fn wrong_device_kind_count_rank_or_duplicate_refuses() {
    let ordinals: Vec<_> = (0..8).collect();
    let caps = NativeCapability::BASE_TEXT;
    for kind in [RuntimeDeviceKind::Cpu, RuntimeDeviceKind::Metal] {
        let mut resolved = devices();
        resolved[3].kind = kind;
        assert!(validate_runtime_requirements(true, &ordinals, &resolved, &caps).is_err());
    }
    for ordinal in [None, Some(0), Some(8)] {
        let mut resolved = devices();
        resolved[3].ordinal = ordinal;
        assert!(validate_runtime_requirements(true, &ordinals, &resolved, &caps).is_err());
    }
    for expected in [
        vec![0, 1, 2, 3, 4, 5, 6],
        vec![0, 0, 2, 3, 4, 5, 6, 7],
        vec![0, 1, 2, 3, 4, 5, 6, 8],
    ] {
        assert!(validate_runtime_requirements(true, &expected, &devices(), &caps).is_err());
    }
    assert!(validate_runtime_requirements(true, &ordinals, &devices()[..7], &caps).is_err());
    let mut resolved = devices();
    resolved.swap(0, 1);
    assert!(validate_runtime_requirements(true, &ordinals, &resolved, &caps).is_err());
}

#[test]
fn actual_cpu_handles_cannot_activate_native_execution() {
    let resolved: Vec<_> = (0..8)
        .map(|_| RuntimeDevice::resolve(DevicePreference::Cpu).unwrap())
        .collect();
    let error = NativeExecution::Cuda
        .validate_runtime(
            &(0..8).collect::<Vec<_>>(),
            &resolved,
            &NativeCapability::BASE_TEXT,
        )
        .unwrap_err();
    assert!(matches!(error, PowerError::BackendNotAvailable(_)));
}
