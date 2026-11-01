#![cfg(test)]

use super::*;
use aster_office::{NativeOfficeEditor, NativeSpreadsheetCellFormat, SpreadsheetCellValue};

#[path = "office_read_governed_tests.rs"]
mod governed;

#[path = "office_read_discovery_tests.rs"]
mod discovery;

async fn workbook(root: &Path) {
    let mut editor = NativeOfficeEditor::create(root.join("source.xlsx"))
        .await
        .unwrap();
    editor.rename_worksheet("/Sheet1", "数据").unwrap();
    for (cell, value) in [
        (
            "A1",
            SpreadsheetCellValue::Number {
                value: "12000".into(),
            },
        ),
        ("B1", SpreadsheetCellValue::Number { value: "0".into() }),
        ("C1", SpreadsheetCellValue::Boolean { value: true }),
        (
            "D1",
            SpreadsheetCellValue::Text {
                value: "【缺失】".into(),
            },
        ),
        (
            "E1",
            SpreadsheetCellValue::Formula {
                expression: "A1+B1".into(),
            },
        ),
        (
            "F1",
            SpreadsheetCellValue::Number {
                value: "46030".into(),
            },
        ),
    ] {
        editor
            .set_cell_value(format!("/数据/{cell}"), value)
            .unwrap();
    }
    editor
        .set_cell_format(
            "/数据/F1",
            NativeSpreadsheetCellFormat {
                number_format: Some("yyyy-mm-dd".into()),
                ..Default::default()
            },
        )
        .unwrap();
    editor
        .set_cell_value(
            "/数据/G1",
            SpreadsheetCellValue::Formula {
                expression: format!("SUM({})", vec!["A1"; 180].join(",")),
            },
        )
        .unwrap();
    editor.recalculate_spreadsheet_formulas().unwrap();
    editor.save().await.unwrap();
}

async fn read(root: &Path, args: Value) -> Result<Value> {
    let binding = crate::plugin_workspace::WorkspaceBinding::new(root)?;
    let ctx = ToolContext::new(root.to_path_buf());
    read_source(&args, &ctx, None, || binding.validate(root)).await
}

#[tokio::test]
async fn native_reads_preserve_cells_types_formulas_formats_positions_and_source_bytes() {
    let root = tempfile::tempdir().unwrap();
    workbook(root.path()).await;
    let before = std::fs::read(root.path().join("source.xlsx")).unwrap();
    let result = read(root.path(), json!({"file_path":"source.xlsx"}))
        .await
        .unwrap();
    let entries = result["entries"].as_array().unwrap();
    let cell = |address: &str| {
        entries
            .iter()
            .find(|entry| entry["path"] == format!("/数据/{address}"))
            .unwrap()
    };
    assert_eq!(cell("A1")["value"], 12000);
    assert_eq!(cell("B1")["value"], 0);
    assert_eq!(cell("C1")["value"], true);
    assert_eq!(cell("D1")["value"], "【缺失】");
    assert_eq!(cell("E1")["value"], 12000);
    assert_eq!(cell("E1")["format"]["formula"], "A1+B1");
    assert_eq!(cell("F1")["value"], 46030);
    assert_eq!(cell("F1")["format"]["numberFormat"], "yyyy-mm-dd");
    assert_eq!(
        cell("G1")["format"]["formula"],
        format!("SUM({})", vec!["A1"; 180].join(","))
    );
    assert_eq!(result["source_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(result["capabilities"]["ocr"], false);
    let node = read(
        root.path(),
        json!({"file_path":"source.xlsx","node_path":"/数据/A1"}),
    )
    .await
    .unwrap();
    assert_eq!(node["node"]["value"], 12000);
    assert_eq!(
        std::fs::read(root.path().join("source.xlsx")).unwrap(),
        before
    );
    let limited = read(root.path(), json!({"file_path":"source.xlsx","limit":1}))
        .await
        .unwrap();
    assert_eq!(limited["truncated"], true);
    assert_eq!(limited["returned"], 1);
}

#[tokio::test]
async fn native_docx_reads_preserve_paragraph_and_table_locations() {
    let root = tempfile::tempdir().unwrap();
    let mut editor = NativeOfficeEditor::create(root.path().join("source.docx"))
        .await
        .unwrap();
    let paragraph = editor
        .add_paragraph("/body", "本期55，上期56。不得写成一致。")
        .unwrap();
    let table = editor.add_table("/body", 1, 2).unwrap();
    editor
        .set_text(format!("{table}/tr[1]/tc[1]"), "原始单元格")
        .unwrap();
    editor.save().await.unwrap();
    let result = read(root.path(), json!({"file_path":"source.docx"}))
        .await
        .unwrap();
    let entries = result["entries"].as_array().unwrap();
    assert!(entries.iter().any(
        |entry| entry["path"] == paragraph && entry["text"] == "本期55，上期56。不得写成一致。"
    ));
    assert!(entries
        .iter()
        .any(|entry| entry["path"] == table
            && entry["text"].as_str().unwrap().contains("原始单元格")));
    let cell = read(
        root.path(),
        json!({"file_path":"source.docx","node_path":format!("{table}/tr[1]/tc[1]")}),
    )
    .await
    .unwrap();
    assert!(cell["node"]["text"]
        .as_str()
        .unwrap()
        .contains("原始单元格"));
}

#[tokio::test]
async fn native_pptx_reads_preserve_slide_and_shape_locations() {
    let root = tempfile::tempdir().unwrap();
    let mut editor = NativeOfficeEditor::create(root.path().join("source.pptx"))
        .await
        .unwrap();
    let slide = editor.add_slide("/", "进展").unwrap();
    let shape = editor.add_shape(&slide, "完成两项，剩余一项。").unwrap();
    editor.save().await.unwrap();
    let before = std::fs::read(root.path().join("source.pptx")).unwrap();
    let result = read(root.path(), json!({"file_path":"source.pptx"}))
        .await
        .unwrap();
    assert_eq!(result["kind"], "pptx");
    assert!(result["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["path"] == shape));
    let exact = read(
        root.path(),
        json!({"file_path":"source.pptx","node_path":shape}),
    )
    .await
    .unwrap();
    assert!(exact["node"]["text"]
        .as_str()
        .unwrap()
        .contains("完成两项，剩余一项。"));
    assert_eq!(
        std::fs::read(root.path().join("source.pptx")).unwrap(),
        before
    );
}

#[tokio::test]
async fn source_reads_refuse_malformed_oversized_unknown_and_unavailable_inputs() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("bad.xlsx"), b"not an Office archive").unwrap();
    std::fs::File::create(root.path().join("large.xlsx"))
        .unwrap()
        .set_len(FILE_LIMIT + 1)
        .unwrap();
    for args in [
        json!({"file_path":"bad.xlsx"}),
        json!({"file_path":"large.xlsx"}),
        json!({"file_path":"missing.xlsx"}),
        json!({"file_path":"../outside.xlsx"}),
        json!({"file_path":"bad.xlsx","unknown":true}),
        json!({"file_path":"bad.xlsx","limit":0}),
        json!({"file_path":"bad.xlsx","limit":1001}),
        json!({"file_path":"bad.exe"}),
    ] {
        assert!(read(root.path(), args.clone()).await.is_err(), "{args}");
    }
    let error = read(root.path(), json!({"file_path":"source.pdf"}))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("PDF text extraction, PDF page images and OCR are unavailable"));
}

#[tokio::test]
async fn output_and_argument_bounds_refuse_unbounded_reads() {
    let root = tempfile::tempdir().unwrap();
    let mut editor = NativeOfficeEditor::create(root.path().join("long.docx"))
        .await
        .unwrap();
    let mut seed = 1u32;
    for _ in 0..40 {
        let text: String = (0..1800)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                char::from(b'a' + ((seed >> 16) % 26) as u8)
            })
            .collect();
        editor.add_paragraph("/body", text).unwrap();
    }
    editor.save().await.unwrap();
    let partial = read(root.path(), json!({"file_path":"long.docx"}))
        .await
        .unwrap();
    assert_eq!(partial["truncated"], true);
    assert!(partial["returned"].as_u64().unwrap() > 0);
    assert!(partial["returned"].as_u64().unwrap() < partial["total"].as_u64().unwrap());
    assert!(
        partial.get("next_node_path").and_then(|value| value.as_str()).is_some(),
        "{partial}"
    );
    assert!(
        serde_json::to_vec(&partial).unwrap().len() <= 64 * 1024,
        "partial result must stay inside the output budget"
    );
    assert_eq!(
        read(root.path(), json!({"file_path":"long.docx","limit":1}))
            .await
            .unwrap()["truncated"],
        true
    );
    let error = read(root.path(), json!({"file_path":"x".repeat(16384)}))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("arguments exceed 16 KiB"));
}

#[cfg(unix)]
#[tokio::test]
async fn source_reads_refuse_leaf_and_parent_symlink_escapes() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    workbook(outside.path()).await;
    std::os::unix::fs::symlink(
        outside.path().join("source.xlsx"),
        root.path().join("link.xlsx"),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("linked")).unwrap();
    for name in ["link.xlsx", "linked/source.xlsx"] {
        assert!(read(root.path(), json!({"file_path":name})).await.is_err());
    }
}

#[tokio::test]
async fn native_reads_keep_the_selected_credential_and_hardlink_boundary() {
    use aster_core::workspace::{
        LocalWorkspaceAccessPolicy, ManifestWorkspaceBackend, WorkspaceServices,
    };
    let root = tempfile::tempdir().unwrap();
    workbook(root.path()).await;
    std::fs::copy(
        root.path().join("source.xlsx"),
        root.path().join(".env.xlsx"),
    )
    .unwrap();
    std::fs::hard_link(
        root.path().join("source.xlsx"),
        root.path().join("alias.xlsx"),
    )
    .unwrap();
    let services = WorkspaceServices::local_with_manifest_backend(
        ManifestWorkspaceBackend::new_with_access_policy(
            root.path(),
            LocalWorkspaceAccessPolicy::CredentialBoundary,
        ),
    );
    let ctx = ToolContext::new(root.path().to_path_buf()).with_workspace_services(services);
    let binding = crate::plugin_workspace::WorkspaceBinding::new(root.path()).unwrap();
    for name in [".env.xlsx", "alias.xlsx", "missing.xlsx"] {
        let error = read_source(&json!({"file_path":name}), &ctx, None, || {
            binding.validate(root.path())
        })
        .await
        .unwrap_err()
        .to_string();
        assert_eq!(error, "Office source not found", "{name}");
    }
}

#[tokio::test]
async fn tool_entry_requires_a_live_read_scope() {
    let root = tempfile::tempdir().unwrap();
    let output = OfficeReadTool::new(root.path())
        .unwrap()
        .execute(
            &json!({"file_path":"source.xlsx"}),
            &ToolContext::new(root.path().to_path_buf()),
        )
        .await
        .unwrap();
    assert!(!output.success);
}

#[test]
fn cell_conversion_refuses_invalid_values_and_keeps_missing_distinct_from_zero() {
    let mut format = BTreeMap::from([("valueType".into(), "Number".into())]);
    assert_eq!(
        cell_value(OfficeNodeType::Cell, "0", &format).unwrap(),
        Some(json!(0))
    );
    assert!(cell_value(OfficeNodeType::Cell, "NaN", &format).is_err());
    assert!(cell_value(OfficeNodeType::Cell, "", &format).is_err());
    format.insert("valuePresent".into(), "false".into());
    assert_eq!(
        cell_value(OfficeNodeType::Cell, "", &format).unwrap(),
        Some(Value::Null)
    );
}
