#![cfg(test)]

use super::*;
use a3s_code_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[path = "office_file_tests/nested_verification.rs"]
mod nested_verification;

struct Model {
    calls: AtomicUsize,
    path: String,
}

#[async_trait]
impl LlmClient for Model {
    async fn complete(
        &self,
        _messages: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let first = self.calls.fetch_add(1, Ordering::SeqCst) == 0;
        let content = if first {
            vec![ContentBlock::ToolUse {
                id: "create-file".into(),
                name: "office_file".into(),
                input: workbook(&self.path),
            }]
        } else {
            vec![ContentBlock::Text {
                text: "Created or reported the actual error.".into(),
            }]
        };
        Ok(LlmResponse {
            message: Message {
                content,
                ..Message::assistant("")
            },
            usage: TokenUsage::default(),
            stop_reason: Some(if first { "tool_use" } else { "end_turn" }.into()),
            token_logprobs: vec![],
            meta: None,
        })
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("non-streaming contract")
    }
}

async fn execute(args: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let binding = crate::plugin_workspace::WorkspaceBinding::new(&ctx.workspace)?;
    let cancel = ctx.cancellation_token();
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(anyhow::anyhow!("office_file cancelled")),
        result = create(args, ctx, || binding.validate(&ctx.workspace)) => result,
    };
    Ok(match result {
        Ok(metadata) => ToolOutput::success(metadata.to_string()).with_metadata(metadata),
        Err(error) => ToolOutput::error(format!("office_file: {error}")),
    })
}

fn context(root: &Path) -> ToolContext {
    ToolContext::new(root.to_path_buf()).with_session_id(uuid::Uuid::new_v4().to_string())
}

#[tokio::test]
async fn tool_entry_requires_a_live_turn_and_respects_real_write_and_permission_gates() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let ungoverned = OfficeFileTool::new(root.path())
        .unwrap()
        .execute(&workbook("plain.xlsx"), &ctx)
        .await
        .unwrap();
    assert!(!ungoverned.success);
    assert!(!root.path().join("plain.xlsx").exists());
    let store = tempfile::tempdir().unwrap();
    let agent = a3s_code_core::Agent::from_config(
        crate::agent::test_launch(Some(store.path().to_path_buf())).config,
    )
    .await
    .unwrap();
    let direct_root = tempfile::tempdir().unwrap();
    let options = a3s_code_core::SessionOptions::new()
        .with_session_id(uuid::Uuid::new_v4().to_string())
        .with_workspace_backend(a3s_code_core::workspace::WorkspaceServices::local(
            direct_root.path(),
        ))
        .with_permission_policy(
            a3s_code_core::permissions::PermissionPolicy::new().allow("office_file"),
        );
    let direct = agent
        .session_async(
            direct_root.path().to_string_lossy().into_owned(),
            Some(options),
        )
        .await
        .unwrap();
    direct
        .register_dynamic_tool(Arc::new(OfficeFileTool::new(direct_root.path()).unwrap()))
        .unwrap();
    let result = direct
        .governed_tool("office_file", workbook("direct.xlsx"))
        .await
        .unwrap();
    assert_ne!(
        result.exit_code, 0,
        "host-direct invocation must not manufacture a live Turn"
    );
    assert!(!direct_root.path().join("direct.xlsx").exists());
    direct.close().await;
    for (write, allow, expected) in [
        (true, true, true),
        (false, true, false),
        (true, false, false),
    ] {
        let base = a3s_code_core::workspace::WorkspaceServices::local(root.path());
        let services = if write {
            base.clone()
        } else {
            a3s_code_core::workspace::WorkspaceServicesBuilder::new(
                base.workspace_ref().clone(),
                base.fs(),
            )
            .capabilities(a3s_code_core::workspace::WorkspaceCapabilities {
                write,
                ..base.capabilities()
            })
            .build()
        };
        let policy = if allow {
            a3s_code_core::permissions::PermissionPolicy::new().allow("office_file")
        } else {
            a3s_code_core::permissions::PermissionPolicy::new().deny("office_file")
        };
        let path = format!("governed-{write}-{allow}.xlsx");
        let model = Arc::new(Model {
            calls: AtomicUsize::new(0),
            path: path.clone(),
        });
        let options = a3s_code_core::SessionOptions::new()
            .with_session_id(uuid::Uuid::new_v4().to_string())
            .with_workspace_backend(services)
            .with_permission_policy(policy)
            .with_llm_client(model.clone())
            .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
            .with_harness(
                a3s_code_core::HarnessComposeOptions::compose(
                    ["system", "tools", "budget", "compact", "infer"]
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                    None,
                    None,
                    vec![],
                )
                .unwrap(),
            );
        let session = agent
            .session_async(root.path().to_string_lossy().into_owned(), Some(options))
            .await
            .unwrap();
        session
            .register_dynamic_tool(std::sync::Arc::new(
                OfficeFileTool::new(root.path()).unwrap(),
            ))
            .unwrap();
        let sent = session
            .send("Create the workbook using office_file.", None)
            .await;
        assert!(
            sent.is_ok(),
            "write={write} allow={allow}, send={sent:?}, file_exists={}",
            root.path().join(&path).exists()
        );
        let result = sent.unwrap();
        let (success, content) = result
            .messages
            .iter()
            .flat_map(|message| message.content.iter())
            .find_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                    ..
                } if tool_use_id == "create-file" => {
                    Some((!is_error.unwrap_or(false), content.as_text()))
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!("write={write} allow={allow}: actual tool receipt missing: {result:?}")
            });
        assert_eq!(success, expected, "{content}");
        assert_eq!(
            matches!(
                result.completion,
                a3s_code_core::harness_loop::CompletionTerminal::Verified { .. }
            ),
            expected,
            "{result:?}"
        );
        assert_eq!(root.path().join(&path).is_file(), expected);
        session.close().await;
    }
}

#[tokio::test]
async fn an_admitted_tool_refuses_a_recreated_workspace_directory() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let tool = OfficeFileTool::new(&root).unwrap();
    std::fs::rename(&root, base.path().join("original")).unwrap();
    std::fs::create_dir(&root).unwrap();
    let output = tool
        .execute(&workbook("result.xlsx"), &context(&root))
        .await
        .unwrap();
    assert!(
        !output.success && output.content.contains("not admitted"),
        "{}",
        output.content
    );
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    assert_eq!(
        std::fs::read_dir(base.path().join("original"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn authority_revoked_at_publication_removes_staging_and_releases_the_write_claim() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let checks = std::sync::atomic::AtomicUsize::new(0);
    let result = create(&workbook("revoked.xlsx"), &ctx, || {
        ensure!(
            checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2,
            "authority revoked"
        );
        Ok(())
    })
    .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("authority revoked"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    assert!(!a3s_code_core::external_observation::session_owns_write(
        ctx.session_id.as_deref().unwrap(),
        &ctx.workspace,
        "revoked.xlsx"
    ));
}

fn workbook(path: &str) -> Value {
    json!({"file_path": path, "kind": "xlsx", "sheets": [
        {"name": "明细", "rows": [["编号", "值", "核对", "字面量"], ["R01", 225, true, "=1+1"], ["R02", 240, false, null]]},
        {"name": "汇总", "rows": [["合计", 465]]}
    ]})
}

#[tokio::test]
async fn artifact_verification_is_bound_to_the_created_path_and_saved_bytes() {
    let root = tempfile::tempdir().unwrap();
    let args = workbook("verified.xlsx");
    let output = execute(&args, &context(root.path())).await.unwrap();
    let tool = OfficeFileTool::new(root.path()).unwrap();
    let proof = tool.artifact_verification(&args, &output).unwrap();
    assert_eq!(proof.file_path, "verified.xlsx");
    assert_eq!(
        proof.content_digest,
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(root.path().join("verified.xlsx")).unwrap())
        )
    );
    assert!(proof.check.contains("xlsx"));
    let mut normalized = output.clone();
    normalized.metadata.as_mut().unwrap()["file_path"] = json!("verified.xlsx");
    assert!(tool.artifact_verification(&args, &normalized).is_some());
    let mut failed = output.clone();
    failed.success = false;
    assert!(tool.artifact_verification(&args, &failed).is_none());
    for (key, invalid) in [
        ("validated", json!(false)),
        ("kind", json!("pptx")),
        ("artifact_requested_path", json!("other.xlsx")),
        ("artifact_relative_path", json!("../outside.xlsx")),
        ("file_path", json!(root.path().join("other.xlsx"))),
        ("artifact_content_digest", Value::Null),
    ] {
        let mut invalid_output = output.clone();
        invalid_output.metadata.as_mut().unwrap()[key] = invalid;
        assert!(
            tool.artifact_verification(&args, &invalid_output).is_none(),
            "accepted {key}"
        );
    }
    assert!(tool
        .artifact_verification(&workbook("other.xlsx"), &output)
        .is_none());
}

#[tokio::test]
async fn native_document_preserves_ordered_paragraphs_and_table_cells() {
    let root = tempfile::tempdir().unwrap();
    let output = execute(&json!({
        "file_path": "report.docx", "kind": "docx", "blocks": [
            {"type": "paragraph", "text": "工作报告\n原文事实：11月10日完成报表系统升级。"},
            {"type": "table", "rows": [["项目", "说明"], ["性能", "2秒降至1.5秒"], ["缺失信息", "未提供"]]},
            {"type": "paragraph", "text": "自发布之日起执行。"}
        ]
    }), &context(root.path())).await.unwrap();
    assert!(output.success, "{}", output.content);
    let document = NativeOfficeDocument::open(root.path().join("report.docx"))
        .await
        .unwrap();
    assert_eq!(document.query("table").unwrap().len(), 1);
    assert_eq!(
        document
            .get("/body/tbl[1]/tr[2]/tc[2]", 0)
            .unwrap()
            .text
            .as_str(),
        "2秒降至1.5秒"
    );
    let text = document.text_view().text;
    let first = text.find("工作报告").unwrap();
    let table = text.find("缺失信息").unwrap();
    let last = text.find("自发布之日起执行").unwrap();
    assert!(first < table && table < last, "{text}");
    assert!(text.contains("11月10日"));
}

#[tokio::test]
async fn workbook_explicit_formulas_recalculate_and_formats_survive_readback() {
    let root = tempfile::tempdir().unwrap();
    let output = execute(&json!({
        "file_path": "calculated.xlsx", "kind": "xlsx", "sheets": [
            {"name": "明细", "rows": [
                [225, 240, "=SUM(A1:B1)", {"formula": "SUM(A1:B1)", "number_format": "0.00"}],
                [{"value": 0.1, "number_format": "0%"}, {"value": 45292, "number_format": "yyyy-mm-dd"}]
            ]},
            {"name": "汇总", "rows": [[{"formula": "'明细'!D1*2"}], [{"formula": "SEQUENCE(2)"}]]}
        ]
    }), &context(root.path())).await.unwrap();
    assert!(output.success, "{}", output.content);
    let document = NativeOfficeDocument::open(root.path().join("calculated.xlsx"))
        .await
        .unwrap();
    assert_eq!(
        document.get("/明细/C1", 0).unwrap().text.as_str(),
        "=SUM(A1:B1)"
    );
    assert_eq!(document.get("/明细/D1", 0).unwrap().text.as_str(), "465");
    assert_eq!(document.get("/汇总/A1", 0).unwrap().text.as_str(), "930");
    assert_eq!(document.get("/汇总/A3", 0).unwrap().text.as_str(), "2");
    let worksheet = document
        .package()
        .xml_part("xl/worksheets/sheet1.xml")
        .unwrap();
    let xml = std::str::from_utf8(worksheet.raw()).unwrap();
    assert!(xml.contains("<f>SUM(A1:B1)</f>"));
    assert!(xml.contains("<v>465</v>"));
    let formats = document.package().xml_part("xl/styles.xml").unwrap();
    let xml = std::str::from_utf8(formats.raw()).unwrap();
    assert!(xml.contains("yyyy-mm-dd"), "{xml}");
    assert_eq!(
        document.get("/明细/A2", 0).unwrap().format["numberFormat"],
        "0%"
    );
}

#[tokio::test]
async fn presentation_embeds_a_real_png_chart_below_the_title_inside_the_slide() {
    let root = tempfile::tempdir().unwrap();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="800"><path d="M10 760 L800 120 L1590 400" fill="none" stroke="blue"/></svg>"#;
    let output = execute(
        &json!({
            "file_path": "chart.pptx", "kind": "pptx", "slides": [{
                "title": "每周趋势", "body": [], "chart": {"svg": svg, "alt_text": "每周指标趋势图"}
            }]
        }),
        &context(root.path()),
    )
    .await
    .unwrap();
    assert!(output.success, "{}", output.content);
    let document = NativeOfficeDocument::open(root.path().join("chart.pptx"))
        .await
        .unwrap();
    assert_eq!(document.statistics().picture_count, 1);
    let pictures = document.query("picture").unwrap();
    let picture = &pictures[0];
    assert_eq!(picture.format["alt"], "每周指标趋势图");
    let slide = document.get("/slide[1]", 1).unwrap();
    let title = slide
        .children
        .iter()
        .find(|node| node.node_type == a3s_office::OfficeNodeType::Shape)
        .unwrap();
    let cm = |node: &a3s_office::DocumentNode, key: &str| {
        node.format[key]
            .trim_end_matches("cm")
            .parse::<f64>()
            .unwrap()
    };
    assert!(cm(picture, "y") >= cm(title, "y") + cm(title, "height"));
    assert!(cm(picture, "x") + cm(picture, "width") <= 33.867);
    assert!(cm(picture, "y") + cm(picture, "height") <= 19.05);
    assert!((cm(picture, "width") / cm(picture, "height") - 2.0).abs() < 0.01);
    let media = document
        .package()
        .part_names()
        .find(|part| part.starts_with("ppt/media/"))
        .unwrap();
    assert!(document
        .package()
        .part(media)
        .unwrap()
        .starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[tokio::test]
async fn invalid_extended_inputs_and_formula_errors_never_publish() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    for args in [
        json!({"file_path":"bad.docx","kind":"docx","blocks":[]}),
        json!({"file_path":"bad.docx","kind":"docx","blocks":[{"type":"table","rows":[["a"],["b","c"]]}]}),
        json!({"file_path":"bad.docx","kind":"docx","blocks":[{"type":"paragraph","text":"body","extra":1}]}),
        json!({"file_path":"bad.docx","kind":"docx","blocks":[{"type":"paragraph","text":"body"}],"sheets":[]}),
        json!({"file_path":"bad.xlsx","kind":"xlsx","sheets":[{"name":"x","rows":[[{"formula":"1/0"}]]}]}),
        json!({"file_path":"bad.xlsx","kind":"xlsx","sheets":[{"name":"x","rows":[[{"formula":"UNKNOWN_FUNCTION()"}]]}]}),
        json!({"file_path":"bad.xlsx","kind":"xlsx","sheets":[{"name":"x","rows":[[{"formula":"SEQUENCE(10001)"}]]}]}),
        json!({"file_path":"bad.xlsx","kind":"xlsx","sheets":[{"name":"x","rows":[[{"formula":"1+1","value":2}]]}]}),
        json!({"file_path":"bad.xlsx","kind":"xlsx","sheets":[{"name":"x","rows":[[{"value":{},"number_format":"0"}]]}]}),
        json!({"file_path":"bad.pptx","kind":"pptx","slides":[{"title":"chart","body":["overlap"],"chart":{"svg":"<svg width=\"10\" height=\"10\"/>","alt_text":"chart"}}]}),
        json!({"file_path":"bad.pptx","kind":"pptx","slides":[{"title":"chart","body":[],"chart":{"svg":"<svg width=\"10\" height=\"10\"><image href=\"file:///private/sentinel\"/></svg>","alt_text":"chart"}}]}),
        json!({"file_path":"bad.pptx","kind":"pptx","slides":[{"title":"chart","body":[],"chart":{"svg":"<svg width=\"10\" height=\"10\"/>","alt_text":""}}]}),
    ] {
        let output = execute(&args, &ctx).await.unwrap();
        assert!(
            !output.success,
            "unexpected success for {args}: {}",
            output.content
        );
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn native_workbook_saves_reopens_and_preserves_typed_cells_and_sheet_names() {
    let root = tempfile::tempdir().unwrap();
    let output = execute(&workbook("result.xlsx"), &context(root.path()))
        .await
        .unwrap();
    assert!(output.success, "{}", output.content);
    let document = NativeOfficeDocument::open(root.path().join("result.xlsx"))
        .await
        .unwrap();
    assert_eq!(document.query("worksheet").unwrap().len(), 2);
    assert_eq!(document.get("/明细/B2", 0).unwrap().text.as_str(), "225");
    assert_eq!(document.get("/明细/D2", 0).unwrap().text.as_str(), "=1+1");
    assert_eq!(document.get("/汇总/B1", 0).unwrap().text.as_str(), "465");
    let receipt: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(receipt["validated"], true);
    assert!(receipt["readback"].as_str().unwrap().contains("R01"));
    assert!(std::fs::read(root.path().join("result.xlsx"))
        .unwrap()
        .starts_with(b"PK"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn native_presentation_contains_all_requested_slides_and_saved_text() {
    let root = tempfile::tempdir().unwrap();
    let output = execute(
            &json!({
                "file_path": "result.pptx", "kind": "pptx", "slides": [
                    {"title": "季度回顾", "body": ["完成报表系统升级", "平均耗时由2秒降至1.5秒", "按期交付"]},
                    {"title": "后续工作", "body": ["建议方案 A"]}
                ]
            }),
            &context(root.path()),
        )
        .await
        .unwrap();
    assert!(output.success, "{}", output.content);
    let document = NativeOfficeDocument::open(root.path().join("result.pptx"))
        .await
        .unwrap();
    assert_eq!(document.query("slide").unwrap().len(), 2);
    let text = document.text_view().text;
    assert!(text.contains("季度回顾"));
    assert!(text.contains("平均耗时由2秒降至1.5秒"));
    assert!(text.contains("建议方案 A"));
    let slide = document.get("/slide[1]", 1).unwrap();
    let mut boxes: Vec<(i64, i64)> = slide
        .children
        .iter()
        .filter(|node| node.node_type == a3s_office::OfficeNodeType::Shape)
        .map(|node| {
            let y = (node.format["y"]
                .trim_end_matches("cm")
                .parse::<f64>()
                .unwrap()
                * 10_000.0)
                .round() as i64;
            let height = (node.format["height"]
                .trim_end_matches("cm")
                .parse::<f64>()
                .unwrap()
                * 10_000.0)
                .round() as i64;
            (y, y + height)
        })
        .collect();
    boxes.sort_unstable();
    assert_eq!(boxes.len(), 4);
    assert!(boxes
        .iter()
        .all(|&(top, bottom)| top >= 0 && bottom <= 190_500));
    assert!(boxes.windows(2).all(|pair| pair[0].1 <= pair[1].0));
}

#[tokio::test]
async fn workspace_write_capability_and_missing_session_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let base = a3s_code_core::workspace::WorkspaceServices::local(root.path());
    let services = a3s_code_core::workspace::WorkspaceServicesBuilder::new(
        base.workspace_ref().clone(),
        base.fs(),
    )
    .capabilities(a3s_code_core::workspace::WorkspaceCapabilities {
        write: false,
        ..base.capabilities()
    })
    .build();
    let ctx = context(root.path()).with_workspace_services(services);
    let output = execute(&workbook("disabled.xlsx"), &ctx).await.unwrap();
    assert!(!output.success && output.content.contains("does not grant writes"));
    let ctx = ToolContext::new(root.path().to_path_buf());
    let output = execute(&workbook("unbound.xlsx"), &ctx).await.unwrap();
    assert!(!output.success && output.content.contains("session id"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn native_png_is_a_real_image_file_and_invalid_svg_does_not_publish() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    let args = json!({"file_path": "trend.png", "kind": "png", "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"320\" height=\"180\"><path d=\"M10 160 L100 80 L200 110 L300 20\" fill=\"none\" stroke=\"blue\" stroke-width=\"3\"/></svg>"});
    let output = execute(&args, &ctx).await.unwrap();
    assert!(output.success, "{}", output.content);
    let bytes = std::fs::read(root.path().join("trend.png")).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 320);
    assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 180);
    let output = execute(&json!({"file_path": "bad.png", "kind": "png", "svg": "<svg><image href=\"file:///secret\"/></svg>"}), &ctx).await.unwrap();
    assert!(!output.success);
    assert!(!root.path().join("bad.png").exists());
}

#[tokio::test]
async fn invalid_input_no_clobber_and_cancellation_never_publish() {
    let root = tempfile::tempdir().unwrap();
    let ctx = context(root.path());
    std::fs::write(root.path().join("existing.xlsx"), b"original").unwrap();
    for args in [
        workbook("existing.xlsx"),
        workbook("../outside.xlsx"),
        workbook("result.xml"),
        json!({"file_path": "bad.xlsx", "kind": "xlsx", "sheets": [{"name": "bad/name", "rows": [[1]]}]}),
        json!({"file_path": "bad.xlsx", "kind": "xlsx", "sheets": [{"name": "x", "rows": [[{}]]}]}),
        json!({"file_path": "bad.xlsx", "kind": "xlsx", "sheets": [], "arbitrary": "rejected"}),
        json!({"file_path": "bad.xlsx", "kind": "xlsx", "sheets": [], "slides": []}),
    ] {
        let output = execute(&args, &ctx).await.unwrap();
        assert!(!output.success, "{}", output.content);
    }
    ctx.cancellation_token().cancel();
    let cancelled = execute(&workbook("cancel.xlsx"), &ctx).await.unwrap();
    assert!(!cancelled.success);
    assert!(cancelled.content.contains("cancelled"));
    assert_eq!(
        std::fs::read(root.path().join("existing.xlsx")).unwrap(),
        b"original"
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn symbolic_links_are_refused_for_both_parents_and_destinations() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("result.xlsx"),
        root.path().join("result.xlsx"),
    )
    .unwrap();
    for path in ["link/result.xlsx", "result.xlsx"] {
        let result = execute(&workbook(path), &context(root.path()))
            .await
            .unwrap();
        assert!(!result.success, "{}", result.content);
    }
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn replaced_workspace_root_is_not_followed() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let ctx = context(&root);
    let binding = crate::plugin_workspace::WorkspaceBinding::new(&root).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::rename(&root, base.path().join("old-workspace")).unwrap();
    std::os::unix::fs::symlink(outside.path(), &root).unwrap();
    assert!(create(&workbook("result.xlsx"), &ctx, || binding
        .validate(&ctx.workspace))
    .await
    .is_err());
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn columns_and_explicit_tool_denies_keep_their_contract() {
    for (index, name) in [(0, "A"), (25, "Z"), (26, "AA"), (99, "CV")] {
        assert_eq!(column_name(index), name);
    }
    let result = crate::check_admitted_tool(
        "yolo",
        "office_file",
        "office_file",
        &workbook("result.xlsx"),
        || panic!("denied tool must not execute"),
    );
    assert_eq!(result, Err("deny"));
}
