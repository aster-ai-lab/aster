use super::*;

#[test]
fn xq_acl_roundtrip_keeps_exact_model_revision_and_native_execution_identity() {
    let text = valid()
        .replace(
            DeepSeekModel::V41Flash1017Official.as_str(),
            DeepSeekModel::V41Flash1017Xq.as_str(),
        )
        .replace(
            DeepSeekModel::V41Flash1017Official.revision(),
            DeepSeekModel::V41Flash1017Xq.revision(),
        );
    let config = NativeModelConfig::from_acl(&text).unwrap();
    assert_eq!(config.model(), DeepSeekModel::V41Flash1017Xq);
    assert_eq!(config.execution(), NativeExecution::Cuda);
    assert_eq!(
        NativeModelConfig::from_acl(&config.to_acl()).unwrap(),
        config
    );
    assert!(NativeModelConfig::from_acl(&text.replace(
        DeepSeekModel::V41Flash1017Xq.revision(),
        DeepSeekModel::V41Flash1017Official.revision()
    ))
    .is_err());
}

fn valid() -> String {
    format!(
        r#"schema = "{CONFIG_SCHEMA}"
model = "{}"
execution = "cuda_native"
checkpoint_dir = "/weights/official"
revision = "{}"
devices = [0, 1, 2, 3, 4, 5, 6, 7]
context_tokens = 8192
"#,
        DeepSeekModel::V41Flash1017Official.as_str(),
        DeepSeekModel::V41Flash1017Official.revision()
    )
}

#[test]
fn native_acl_roundtrip_preserves_exact_identity_and_rank_order() {
    let text = valid().replace("[0, 1, 2, 3, 4, 5, 6, 7]", "[7, 6, 5, 4, 3, 2, 1, 0]");
    let config = NativeModelConfig::from_acl(&text).unwrap();
    assert_eq!(config.model(), DeepSeekModel::V41Flash1017Official);
    assert_eq!(config.execution(), NativeExecution::Cuda);
    assert_eq!(config.checkpoint_dir(), Path::new("/weights/official"));
    assert_eq!(config.devices(), &[7, 6, 5, 4, 3, 2, 1, 0]);
    assert_eq!(config.context_tokens(), 8192);
    assert_eq!(
        NativeModelConfig::from_acl(&config.to_acl()).unwrap(),
        config
    );
}

#[test]
fn acl_roundtrip_escapes_filesystem_text() {
    let text = valid().replace("/weights/official", r#"D:\\权重\\quoted\"name"#);
    let config = NativeModelConfig::from_acl(&text).unwrap();
    assert_eq!(config.checkpoint_dir(), Path::new("D:\\权重\\quoted\"name"));
    assert_eq!(
        NativeModelConfig::from_acl(&config.to_acl()).unwrap(),
        config
    );
}

#[test]
fn schema_model_revision_and_execution_are_required_closed_sets() {
    for (from, to) in [
        (CONFIG_SCHEMA, "aster.deepseek.native.v2"),
        (
            DeepSeekModel::V41Flash1017Official.as_str(),
            "unknown/model",
        ),
        (DeepSeekModel::V41Flash1017Official.revision(), "main"),
        ("cuda_native", "auto"),
        ("cuda_native", "cpu"),
        ("cuda_native", "http"),
        ("cuda_native", "proxy"),
    ] {
        assert!(
            NativeModelConfig::from_acl(&valid().replace(from, to)).is_err(),
            "{to}"
        );
    }
}

#[test]
fn missing_unknown_duplicate_and_block_fields_refuse() {
    for line in valid().lines() {
        let name = line.split_once('=').unwrap().0.trim();
        let missing = valid()
            .lines()
            .filter(|item| *item != line)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(NativeModelConfig::from_acl(&missing).is_err(), "{name}");
        let duplicate = format!("{}\n{line}\n", valid());
        assert!(NativeModelConfig::from_acl(&duplicate).is_err(), "{name}");
    }
    for suffix in [
        "fallback = true",
        "unknown = 1",
        "native { execution = \"cpu\" }",
    ] {
        assert!(NativeModelConfig::from_acl(&format!("{}\n{suffix}\n", valid())).is_err());
    }
}

#[test]
fn device_lists_refuse_duplicates_missing_fractional_and_non_numbers() {
    for invalid in [
        "[]",
        "[0, 1, 2, 3, 4, 5, 6]",
        "[0, 1, 2, 3, 4, 5, 6, 6]",
        "[0, 1, 2, 3, 4, 5, 6, 8]",
        "[0, 1, 2, 3, 4, 5, 6, -1]",
        "[0, 1, 2, 3, 4, 5, 6, 7.5]",
        "[0, 1, 2, 3, 4, 5, 6, true]",
        "true",
        "\"0,1,2,3,4,5,6,7\"",
    ] {
        assert!(
            NativeModelConfig::from_acl(&valid().replace("[0, 1, 2, 3, 4, 5, 6, 7]", invalid))
                .is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn context_and_path_bounds_refuse_invalid_inputs() {
    for invalid in ["0", "-1", "1.5", "1048577", "1e100", "true", "\"8192\""] {
        assert!(
            NativeModelConfig::from_acl(&valid().replace(
                "context_tokens = 8192",
                &format!("context_tokens = {invalid}")
            ))
            .is_err(),
            "{invalid}"
        );
    }
    for accepted in [1, MAX_CONTEXT_TOKENS] {
        let config = NativeModelConfig::from_acl(&valid().replace(
            "context_tokens = 8192",
            &format!("context_tokens = {accepted}"),
        ))
        .unwrap();
        assert_eq!(config.context_tokens(), accepted);
    }
    assert!(NativeModelConfig::from_acl(&valid().replace("/weights/official", "")).is_err());
}

#[test]
fn document_and_collection_parse_limits_are_enforced() {
    assert!(NativeModelConfig::from_acl(&" ".repeat(ACL_LIMITS.max_document_bytes + 1)).is_err());
    let list = format!(
        "[{}]",
        vec!["0"; ACL_LIMITS.max_collection_items + 1].join(",")
    );
    assert!(
        NativeModelConfig::from_acl(&valid().replace("[0, 1, 2, 3, 4, 5, 6, 7]", &list)).is_err()
    );
    assert!(NativeModelConfig::from_acl(&valid().replace(
        "/weights/official",
        &"x".repeat(ACL_LIMITS.max_token_bytes + 1)
    ))
    .is_err());
    assert!(NativeModelConfig::from_acl("{").is_err());
}
