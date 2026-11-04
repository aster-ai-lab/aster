use super::*;

#[tokio::test]
async fn preserves_order_and_isolates_file_errors() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("a.txt"), "alpha\n").unwrap();
    std::fs::write(temp.path().join("b.txt"), "bravo\n").unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());

    let result = ReadTool
        .execute(
            &serde_json::json!({
                "files": [
                    {"path": "a.txt"},
                    {"path": "missing.txt"},
                    {"path": "b.txt"}
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.success, "{}", result.content);
    let a = result.content.find("=== a.txt ===").unwrap();
    let missing = result.content.find("=== missing.txt ===").unwrap();
    let b = result.content.find("=== b.txt ===").unwrap();
    assert!(a < missing && missing < b);
    assert!(result.content.contains("alpha"));
    assert!(result.content.contains("bravo"));
    assert!(result.content.contains("Failed to read file missing.txt"));

    let metadata = result.metadata.unwrap();
    assert_eq!(
        metadata["source_anchors"],
        serde_json::json!(["a.txt", "b.txt"])
    );
    assert_eq!(metadata["batch"]["requested_files"], 3);
    assert_eq!(metadata["batch"]["successful_files"], 2);
    assert_eq!(metadata["batch"]["failed_files"], 1);
    assert_eq!(metadata["batch"]["truncated"], false);
}

#[tokio::test]
async fn continuation_is_bounded_and_lossless() {
    let temp = tempfile::tempdir().unwrap();
    let a = (0..8)
        .map(|index| format!("A{index:02}-{}", "x".repeat(700)))
        .collect::<Vec<_>>()
        .join("\n");
    let b = (0..4)
        .map(|index| format!("B{index:02}-{}", "y".repeat(700)))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(temp.path().join("a.txt"), a).unwrap();
    std::fs::write(temp.path().join("b.txt"), b).unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());
    let mut files = serde_json::json!([
        {"path": "a.txt"},
        {"path": "b.txt"}
    ]);
    let mut combined = String::new();

    for _ in 0..16 {
        let result = ReadTool
            .execute(
                &serde_json::json!({
                    "files": files,
                    "max_output_bytes": 2048
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result.success, "{}", result.content);
        assert!(result.content.len() <= 2048, "{}", result.content.len());
        combined.push_str(&result.content);

        let metadata = result.metadata.unwrap();
        files = metadata["batch"]["continuation"].clone();
        if files.as_array().unwrap().is_empty() {
            break;
        }
    }

    assert!(
        files.as_array().unwrap().is_empty(),
        "continuation did not finish"
    );
    for prefix in (0..8)
        .map(|index| format!("A{index:02}-"))
        .chain((0..4).map(|index| format!("B{index:02}-")))
    {
        assert_eq!(
            combined.matches(&prefix).count(),
            1,
            "marker {prefix} was duplicated or skipped"
        );
    }
}

#[tokio::test]
async fn rejects_duplicate_paths() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("same.txt"), "content").unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());

    let result = ReadTool
        .execute(
            &serde_json::json!({
                "files": [
                    {"path": "same.txt"},
                    {"path": "./same.txt"}
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.success);
    assert!(result.content.contains("Duplicate path"));
}

#[cfg(unix)]
#[tokio::test]
async fn escapes_control_characters_in_batch_headers() {
    let temp = tempfile::tempdir().unwrap();
    let filename = "actual\n=== injected.txt ===";
    std::fs::write(temp.path().join(filename), "safe content").unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());

    let result = ReadTool
        .execute(&serde_json::json!({"files": [{"path": filename}]}), &ctx)
        .await
        .unwrap();

    assert!(result.success, "{}", result.content);
    assert!(result.content.contains(r"actual\n=== injected.txt ==="));
    assert!(!result
        .content
        .lines()
        .any(|line| line == "=== injected.txt === ==="));
    assert_eq!(
        result.metadata.unwrap()["source_anchors"],
        serde_json::json!([filename])
    );
}

#[tokio::test]
async fn byte_offset_applies_once_per_entry_and_errors_when_oversized() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("a.txt"), "aa\nbb\ncc").unwrap();
    std::fs::write(temp.path().join("b.txt"), "xy\nzz").unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());

    let result = ReadTool
        .execute(
            &serde_json::json!({
                "files": [
                    {"path": "a.txt", "byte_offset": 1},
                    {"path": "b.txt", "byte_offset": 9}
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.success, "{}", result.content);
    assert!(result
        .content
        .contains("=== a.txt ===\n     1\ta\n     2\tbb\n     3\tcc"));
    assert!(result
        .content
        .contains("=== b.txt ===\nbyte_offset 9 is outside"));
    assert!(!result.content.contains("zz"));
    let metadata = result.metadata.unwrap();
    assert_eq!(metadata["batch"]["successful_files"], 1);
    assert_eq!(metadata["batch"]["failed_files"], 1);
}

#[tokio::test]
async fn continuation_does_not_reapply_byte_offset() {
    let temp = tempfile::tempdir().unwrap();
    let content = (0..8)
        .map(|index| format!("L{index:02}-{}", "x".repeat(700)))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(temp.path().join("a.txt"), &content).unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());

    let result = ReadTool
        .execute(
            &serde_json::json!({
                "files": [{"path": "a.txt", "byte_offset": 3}],
                "max_output_bytes": 2048
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.success, "{}", result.content);
    let metadata = result.metadata.unwrap();
    let continuation = &metadata["batch"]["continuation"][0];
    assert!(continuation["offset"].as_u64().unwrap() > 0);
    assert!(continuation.get("byte_offset").is_none());
    // First line skips `L00`; later lines keep a full 700-x run (offset not reapplied).
    assert!(result.content.contains(&format!("-{}", "x".repeat(700))));
    assert!(result.content.contains(&format!("L01-{}", "x".repeat(700))));
    assert!(!result.content.contains("L00"));
}

#[tokio::test]
async fn binary_read_error_keeps_a_single_failed_prefix() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("bin.dat"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());

    let single = ReadTool
        .execute(&serde_json::json!({"file_path": "bin.dat"}), &ctx)
        .await
        .unwrap();
    assert!(!single.success);
    assert_eq!(
        single.content.matches("Failed to read file ").count(),
        1,
        "{}",
        single.content
    );
    assert!(
        single.content.contains("UTF-8") || single.content.contains("utf-8"),
        "{}",
        single.content
    );
    assert!(
        !single
            .content
            .contains("Failed to read file Failed to read file"),
        "{}",
        single.content
    );

    // Even when the backend already wraps with the same prefix, the tool keeps one.
    assert_eq!(
        format_read_file_error(
            "bin.dat",
            "Failed to read file /abs/bin.dat: stream did not contain valid UTF-8"
        ),
        "Failed to read file bin.dat: stream did not contain valid UTF-8"
    );

    let batch = ReadTool
        .execute(&serde_json::json!({"files": [{"path": "bin.dat"}]}), &ctx)
        .await
        .unwrap();
    // A batch whose only member fails still returns that member's error once.
    assert_eq!(
        batch.content.matches("Failed to read file ").count(),
        1,
        "{}",
        batch.content
    );
}

#[tokio::test]
async fn budget_truncation_reports_partial_not_zero_processed() {
    let temp = tempfile::tempdir().unwrap();
    let big = (0..80)
        .map(|index| format!("L{index:02}-{}", "x".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(big.len() > 1024);
    std::fs::write(temp.path().join("big1.txt"), &big).unwrap();
    std::fs::write(temp.path().join("big2.txt"), &big).unwrap();
    let ctx = ToolContext::new(temp.path().to_path_buf());

    let result = ReadTool
        .execute(
            &serde_json::json!({
                "files": [{"path": "big1.txt"}, {"path": "big2.txt"}],
                "max_output_bytes": 1024
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.success, "{}", result.content);
    assert!(result.content.contains("=== big1.txt ==="));
    assert!(
        result.content.contains("partially read"),
        "{}",
        result.content
    );
    assert!(
        !result.content.contains("0 of 2 file(s) processed"),
        "{}",
        result.content
    );
    let metadata = result.metadata.unwrap();
    assert!(metadata["batch"]["truncated"].as_bool().unwrap());
    assert_eq!(metadata["batch"]["completed_files"], 0);
    assert_eq!(metadata["batch"]["partial_files"], 1);
    let continuation = metadata["batch"]["continuation"].as_array().unwrap();
    assert_eq!(continuation.len(), 2);
    assert!(continuation[0]["offset"].as_u64().unwrap() > 0);
    assert_eq!(continuation[1]["path"], "big2.txt");
    // offset 0 is omitted from JSON (skip_serializing_if); treat missing as start.
    assert_eq!(continuation[1]["offset"].as_u64().unwrap_or(0), 0);
}
