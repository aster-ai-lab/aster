//! Bounded, in-process artifact creation. Sources and existing destinations stay read-only.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use a3s_code_core::tools::{Tool, ToolArtifactVerification, ToolContext, ToolOutput};
use a3s_office::{
    NativeOfficeDocument, NativeOfficeEditor, NativeOfficeImage, NativeOfficeMutation,
    NativeSpreadsheetCellFormat, SpreadsheetCellValue, SpreadsheetFormulaValue,
};
use anyhow::{bail, ensure, Result};
use async_trait::async_trait;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_CELLS: usize = 10_000;

fn render_svg_png(svg: &str) -> Result<Vec<u8>> {
    #[cfg(feature = "svg-png")]
    {
        return crate::artifact_image::render_png(svg);
    }
    #[cfg(not(feature = "svg-png"))]
    {
        let _ = svg;
        bail!("PNG rendering requires the svg-png feature");
    }
}

pub(crate) struct OfficeFileTool {
    workspace: crate::plugin_workspace::WorkspaceBinding,
}

impl OfficeFileTool {
    pub(crate) fn new(workspace: &Path) -> Result<Self> {
        Ok(Self {
            workspace: crate::plugin_workspace::WorkspaceBinding::new(workspace)?,
        })
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Docx,
    Xlsx,
    Pptx,
    Png,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Png => "png",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    file_path: String,
    kind: Kind,
    sheets: Option<Vec<Sheet>>,
    slides: Option<Vec<Slide>>,
    svg: Option<String>,
    blocks: Option<Vec<Block>>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Block {
    Paragraph { text: String },
    Table { rows: Vec<Vec<String>> },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sheet {
    name: String,
    rows: Vec<Vec<Value>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Slide {
    title: String,
    body: Vec<String>,
    chart: Option<Chart>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chart {
    svg: String,
    alt_text: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ExtendedCell {
    Formula(FormulaCell),
    Formatted(FormattedCell),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormulaCell {
    formula: String,
    number_format: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FormattedCell {
    value: Value,
    number_format: String,
}

#[async_trait]
impl Tool for OfficeFileTool {
    fn name(&self) -> &str {
        "office_file"
    }

    fn description(&self) -> &str {
        "Create and verify a real .docx document, .xlsx workbook, .pptx presentation, or .png chart in the local workspace without Bash or external programs. Existing files are never overwritten. DOCX takes ordered paragraph/table blocks. XLSX takes named sheets with JSON scalar cells (strings including '=...' stay literal), explicit {formula, number_format?} cells, or {value, number_format} cells for dates/percentages. Formulas are recalculated in process; formula errors refuse publication. Excel dates use numeric serials with a date format. PPTX takes slides with a title and up to 3 short body blocks, or a chart with SVG and alt_text and an empty body; charts are fitted below the title. PNG and embedded charts use bounded self-contained SVG with presentation attributes, numeric/px dimensions or viewBox, and no style, image, use, filter, scripts or external resources. Saves, reopens and validates Office files before publication; returns saved text readback. Read sources with the document tools first."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object", "additionalProperties": false,
            "properties": {
                "file_path": {"type": "string", "description": "New workspace path, with an extension matching kind. Parent directories must already exist."},
                "kind": {"type": "string", "enum": ["docx", "xlsx", "pptx", "png"]},
                "blocks": {"type": "array", "minItems": 1, "maxItems": 1000, "description": "Required only for docx. Ordered paragraphs and rectangular tables; at most 10000 table cells total.", "items": {"oneOf": [
                    {"type": "object", "additionalProperties": false, "properties": {"type": {"const": "paragraph"}, "text": {"type": "string"}}, "required": ["type", "text"]},
                    {"type": "object", "additionalProperties": false, "properties": {"type": {"const": "table"}, "rows": {"type": "array", "minItems": 1, "maxItems": 1000, "items": {"type": "array", "minItems": 1, "maxItems": 63, "items": {"type": "string"}}}}, "required": ["type", "rows"]}
                ]}},
                "sheets": {"type": "array", "minItems": 1, "maxItems": 32, "description": "Required only for xlsx; at most 10000 total cells.", "items": {
                    "type": "object", "additionalProperties": false,
                    "properties": {"name": {"type": "string"}, "rows": {"type": "array", "maxItems": 1000, "items": {"type": "array", "maxItems": 100, "items": {"oneOf": [
                        {"type": ["string", "number", "boolean", "null"]},
                        {"type": "object", "additionalProperties": false, "properties": {"formula": {"type": "string"}, "number_format": {"type": "string"}}, "required": ["formula"]},
                        {"type": "object", "additionalProperties": false, "properties": {"value": {"type": ["string", "number", "boolean", "null"]}, "number_format": {"type": "string"}}, "required": ["value", "number_format"]}
                    ]}}}},
                    "required": ["name", "rows"]
                }},
                "slides": {"type": "array", "minItems": 1, "maxItems": 30, "description": "Required only for pptx; split dense material over additional slides.", "items": {
                    "type": "object", "additionalProperties": false,
                    "properties": {"title": {"type": "string", "maxLength": 40}, "body": {"type": "array", "maxItems": 3, "items": {"type": "string", "maxLength": 100}}, "chart": {"type": "object", "additionalProperties": false, "description": "Optional chart, requires empty body. The same bounded SVG rules as png apply. No local image reads.", "properties": {"svg": {"type": "string"}, "alt_text": {"type": "string", "minLength": 1, "maxLength": 1000}}, "required": ["svg", "alt_text"]}},
                    "required": ["title", "body"]
                }},
                "svg": {"type": "string", "description": "Required only for png: self-contained SVG chart, at most 64 KiB. Set width/height as numbers or px, or provide viewBox. Use presentation attributes; style, image, use, filter, scripts, linked files and network resources are refused. At most 4096 pixels per axis and 8 megapixels."}
            },
            "required": ["file_path", "kind"]
        })
    }

    fn validate_execution_available(&self, _args: &Value, ctx: &ToolContext) -> Result<()> {
        self.workspace.validate(&ctx.workspace)?;
        ensure!(
            ctx.workspace_services.capabilities().write,
            "workspace does not grant writes"
        );
        ensure!(
            ctx.workspace_services.local_root().is_some(),
            "requires a local workspace"
        );
        Ok(())
    }

    fn artifact_verification(
        &self,
        args: &Value,
        output: &ToolOutput,
    ) -> Option<ToolArtifactVerification> {
        if !output.success {
            return None;
        }
        self.workspace.validate_current().ok()?;
        let metadata = output.metadata.as_ref()?;
        let requested = args.get("file_path")?.as_str()?;
        let kind: Kind = serde_json::from_value(args.get("kind")?.clone()).ok()?;
        if metadata.get("validated")?.as_bool()? != true
            || metadata.get("artifact_requested_path")?.as_str()? != requested
            || metadata.get("kind")?.as_str()? != kind.as_str()
        {
            return None;
        }
        let relative = metadata.get("artifact_relative_path")?.as_str()?;
        if relative.is_empty()
            || !Path::new(relative)
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return None;
        }
        let saved = self.workspace.root().join(relative);
        let observed = Path::new(metadata.get("file_path")?.as_str()?);
        if observed != saved && observed != Path::new(relative) {
            return None;
        }
        Some(ToolArtifactVerification {
            file_path: relative.to_owned(),
            content_digest: metadata
                .get("artifact_content_digest")?
                .as_str()?
                .to_owned(),
            check: format!(
                "Native {kind} structure and persisted bytes were validated",
                kind = kind.as_str()
            ),
        })
    }

    async fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let outcome = async {
            self.validate_execution_available(args, ctx)?;
            let access =
                ctx.workspace_operation(a3s_code_core::capability::WorkspaceOperation::Write)?;
            ctx.register_capability_effect(self.workspace.clone())?;
            let cancel = access.cancellation()?;
            let validate_access = || {
                self.workspace.validate(&ctx.workspace)?;
                access.check()?;
                Ok(())
            };
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(anyhow::anyhow!("office_file cancelled")),
                result = create(args, ctx, validate_access) => result,
            }
        }
        .await;
        Ok(match outcome {
            Ok(metadata) => ToolOutput::success(metadata.to_string()).with_metadata(metadata),
            Err(error) => ToolOutput::error(format!("office_file: {error}")),
        })
    }
}

async fn create(
    args: &Value,
    ctx: &ToolContext,
    validate_access: impl Fn() -> Result<()>,
) -> Result<Value> {
    validate_access()?;
    ensure!(
        serde_json::to_vec(args)?.len() <= MAX_INPUT_BYTES,
        "input exceeds 1 MiB"
    );
    let input: Input = serde_json::from_value(args.clone())?;
    ensure!(
        ctx.workspace_services.capabilities().write,
        "workspace does not grant writes"
    );
    let root = ctx
        .workspace_services
        .local_root()
        .ok_or_else(|| anyhow::anyhow!("requires a local workspace"))?;
    let path = ctx.resolve_workspace_path(&input.file_path)?;
    ensure!(
        Path::new(path.as_str())
            .extension()
            .and_then(|v| v.to_str())
            == Some(input.kind.as_str()),
        "file extension must match kind"
    );
    ctx.workspace_services.refuse_direct_write(&path)?;
    let (parent, name) = parent_dir(root, path.as_str())?;
    ensure!(
        parent
            .symlink_metadata(&name)
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "destination already exists or is inaccessible"
    );
    ensure!(
        ctx.session_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty()),
        "write requires a session id"
    );

    let (bytes, readback) = match input.kind {
        Kind::Png => {
            ensure!(
                input.sheets.is_none() && input.slides.is_none() && input.blocks.is_none(),
                "png accepts svg only"
            );
            let svg = input
                .svg
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("png requires svg"))?;
            let bytes = render_svg_png(svg)?;
            NativeOfficeImage::inspect_bytes(&bytes)?;
            (bytes, String::new())
        }
        Kind::Docx | Kind::Xlsx | Kind::Pptx => build_office(&input, ctx).await?,
    };
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "artifact exceeds 20 MiB"
    );
    ensure!(!ctx.is_cancelled(), "office_file cancelled");
    validate_access()?;
    ctx.workspace_services.refuse_direct_write(&path)?;
    // Reopen parents after generation so publication uses the current authorized path.
    let (parent, name) = parent_dir(root, path.as_str())?;
    let session = ctx
        .session_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("write requires a session id"))?;
    let acquired = !a3s_code_core::external_observation::session_owns_write(
        session,
        &ctx.workspace,
        path.as_str(),
    );
    a3s_code_core::external_observation::claim_bound_write(
        Some(session),
        &ctx.workspace,
        path.as_str(),
    )
    .map_err(anyhow::Error::msg)?;
    if let Err(error) = publish(&parent, &name, &bytes, ctx, validate_access) {
        if acquired {
            a3s_code_core::external_observation::release_write_claim(
                session,
                &ctx.workspace,
                path.as_str(),
            );
        }
        return Err(error);
    }
    let display = ctx.workspace_services.display_path(&path);
    Ok(json!({
        "file_path": display, "files": [display], "kind": input.kind.as_str(), "bytes": bytes.len(),
        "artifact_relative_path": path.as_str(), "artifact_requested_path": input.file_path,
        "artifact_content_digest": format!("{:x}", Sha256::digest(&bytes)),
        "validated": true, "readback": readback.chars().take(8000).collect::<String>(),
        "readback_truncated": readback.chars().count() > 8000
    }))
}

async fn build_office(input: &Input, ctx: &ToolContext) -> Result<(Vec<u8>, String)> {
    ensure!(input.svg.is_none(), "Office formats do not accept svg");
    let temp = tempfile::tempdir()?;
    let file = temp
        .path()
        .join(format!("artifact.{}", input.kind.as_str()));
    let mut editor = NativeOfficeEditor::create(&file).await?;
    match input.kind {
        Kind::Docx => {
            ensure!(
                input.sheets.is_none() && input.slides.is_none(),
                "docx accepts blocks only"
            );
            let blocks = input
                .blocks
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("docx requires blocks"))?;
            ensure!(
                (1..=1000).contains(&blocks.len()),
                "docx requires 1 to 1000 blocks"
            );
            let mut total = 0usize;
            for block in blocks {
                ensure!(!ctx.is_cancelled(), "office_file cancelled");
                match block {
                    Block::Paragraph { text } => {
                        editor.add_paragraph("/body", text)?;
                    }
                    Block::Table { rows } => {
                        let columns = rows.first().map_or(0, Vec::len);
                        ensure!(
                            (1..=1000).contains(&rows.len())
                                && (1..=63).contains(&columns)
                                && rows.iter().all(|row| row.len() == columns),
                            "docx tables require 1 to 1000 rectangular rows of 1 to 63 cells"
                        );
                        total += rows.len() * columns;
                        ensure!(total <= MAX_CELLS, "document exceeds 10000 table cells");
                        let path = editor.add_table("/body", rows.len(), columns)?;
                        let cells: Vec<_> = rows
                            .iter()
                            .enumerate()
                            .flat_map(|(row, values)| {
                                let path = &path;
                                values.iter().enumerate().map(move |(column, text)| {
                                    NativeOfficeMutation::SetText {
                                        path: format!("{path}/tr[{}]/tc[{}]", row + 1, column + 1),
                                        text: text.clone(),
                                    }
                                })
                            })
                            .collect();
                        editor.apply_batch(&cells)?;
                    }
                }
            }
        }
        Kind::Xlsx => {
            ensure!(
                input.slides.is_none() && input.blocks.is_none(),
                "xlsx accepts sheets only"
            );
            let sheets = input
                .sheets
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("xlsx requires sheets"))?;
            ensure!(
                (1..=32).contains(&sheets.len()),
                "xlsx requires 1 to 32 sheets"
            );
            let mut total = 0usize;
            let mut formula_count = 0usize;
            for (index, sheet) in sheets.iter().enumerate() {
                ensure!(!ctx.is_cancelled(), "office_file cancelled");
                ensure!(sheet.rows.len() <= 1000, "sheet exceeds 1000 rows");
                let sheet_path = if index == 0 {
                    editor.rename_worksheet("/Sheet1", &sheet.name)?
                } else {
                    editor.add_worksheet(&sheet.name)?
                };
                let mut cells = Vec::new();
                for (row, values) in sheet.rows.iter().enumerate() {
                    ensure!(values.len() <= 100, "row exceeds 100 columns");
                    total += values.len();
                    ensure!(total <= MAX_CELLS, "workbook exceeds 10000 cells");
                    for (column, value) in values.iter().enumerate() {
                        let path = format!("{sheet_path}/{}{}", column_name(column), row + 1);
                        let (value, number_format) = if value.is_object() {
                            match serde_json::from_value::<ExtendedCell>(value.clone())? {
                                ExtendedCell::Formula(cell) => {
                                    formula_count += 1;
                                    (
                                        Some(SpreadsheetCellValue::Formula {
                                            expression: cell.formula,
                                        }),
                                        cell.number_format,
                                    )
                                }
                                ExtendedCell::Formatted(cell) => {
                                    (scalar_value(&cell.value)?, Some(cell.number_format))
                                }
                            }
                        } else {
                            (scalar_value(value)?, None)
                        };
                        if let Some(value) = value {
                            cells.push(NativeOfficeMutation::SetCellValue {
                                path: path.clone(),
                                value,
                            });
                        }
                        if let Some(number_format) = number_format {
                            cells.push(NativeOfficeMutation::SetCellFormat {
                                path,
                                format: NativeSpreadsheetCellFormat {
                                    number_format: Some(number_format),
                                    ..Default::default()
                                },
                            });
                        }
                    }
                }
                editor.apply_batch(&cells)?;
            }
            if formula_count > 0 {
                let calculation = editor.recalculate_spreadsheet_formulas()?;
                ensure!(
                    calculation.formula_count == formula_count,
                    "workbook formula count changed during calculation"
                );
                if let Some(cell) = calculation
                    .cells
                    .iter()
                    .find(|cell| formula_error(&cell.value))
                {
                    bail!(
                        "workbook formula calculation returned an error at {}: {:?}",
                        cell.cell.path(),
                        cell.value
                    );
                }
                ensure!(
                    total + calculation.spill_cell_count <= MAX_CELLS,
                    "workbook exceeds 10000 cells after formula spill"
                );
            }
        }
        Kind::Pptx => {
            ensure!(
                input.sheets.is_none() && input.blocks.is_none(),
                "pptx accepts slides only"
            );
            let slides = input
                .slides
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("pptx requires slides"))?;
            ensure!(
                (1..=30).contains(&slides.len()),
                "pptx requires 1 to 30 slides"
            );
            for slide in slides {
                ensure!(!ctx.is_cancelled(), "office_file cancelled");
                ensure!(
                    !slide.title.trim().is_empty() && slide.title.chars().count() <= 40,
                    "slide title must contain 1 to 40 characters"
                );
                ensure!(
                    slide.body.len() <= 3
                        && slide.body.iter().all(|text| text.chars().count() <= 100),
                    "slide body supports up to 3 blocks of 100 characters"
                );
                let path = editor.add_slide("/", &slide.title)?;
                if let Some(chart) = &slide.chart {
                    ensure!(
                        slide.body.is_empty(),
                        "chart slides require an empty body; put findings on another slide"
                    );
                    ensure!(
                        !chart.alt_text.trim().is_empty() && chart.alt_text.chars().count() <= 1000,
                        "chart alt_text must contain 1 to 1000 characters"
                    );
                    let png = render_svg_png(&chart.svg)?;
                    let size = NativeOfficeImage::inspect_bytes(&png)?;
                    let scale = (1000.0 / f64::from(size.width_px))
                        .min(460.0 / f64::from(size.height_px))
                        .min(1.0);
                    let image = NativeOfficeImage::from_bytes(&png)?
                        .with_alt_text(&chart.alt_text)
                        .with_width_px((f64::from(size.width_px) * scale).round().max(1.0) as u32)
                        .with_height_px((f64::from(size.height_px) * scale).round().max(1.0) as u32)
                        .with_presentation_position_emu(914_400, 1_828_800);
                    editor.add_image(&path, image)?;
                }
                for text in &slide.body {
                    editor.add_shape(&path, text)?;
                }
            }
        }
        Kind::Png => bail!("png is not an Office package"),
    }
    editor.save().await?;
    let document = NativeOfficeDocument::open(&file).await?;
    let readback = document.text_view().text;
    let mut bytes = Vec::new();
    std::fs::File::open(&file)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "artifact exceeds 20 MiB"
    );
    Ok((bytes, readback))
}

fn scalar_value(value: &Value) -> Result<Option<SpreadsheetCellValue>> {
    Ok(match value {
        Value::Null => None,
        Value::String(value) => Some(SpreadsheetCellValue::Text {
            value: value.clone(),
        }),
        Value::Number(value) => Some(SpreadsheetCellValue::Number {
            value: value.to_string(),
        }),
        Value::Bool(value) => Some(SpreadsheetCellValue::Boolean { value: *value }),
        _ => bail!("cell values must be string, number, boolean, or null"),
    })
}

fn formula_error(value: &SpreadsheetFormulaValue) -> bool {
    match value {
        SpreadsheetFormulaValue::Error { .. } => true,
        SpreadsheetFormulaValue::Array { rows } => rows.iter().flatten().any(formula_error),
        _ => false,
    }
}

fn column_name(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            return name;
        }
        index = index / 26 - 1;
    }
}

fn parent_dir(root: &Path, relative: &str) -> Result<(Dir, std::ffi::OsString)> {
    let path = Path::new(relative);
    ensure!(
        !path.as_os_str().is_empty()
            && path.components().all(|c| matches!(c, Component::Normal(_))),
        "file path must stay inside the workspace"
    );
    ensure!(root.is_absolute(), "local workspace root must be absolute");
    let mut anchor = PathBuf::new();
    let mut components = root.components().peekable();
    while components
        .peek()
        .is_some_and(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
    {
        if let Some(component) = components.next() {
            anchor.push(component.as_os_str());
        }
    }
    let mut dir = Dir::open_ambient_dir(anchor, cap_std::ambient_authority())?;
    for component in components {
        ensure!(
            matches!(component, Component::Normal(_)),
            "local workspace root is not normalized"
        );
        dir = dir.open_dir_nofollow(component.as_os_str())?;
    }
    if let Some(parent) = path.parent() {
        for part in parent.components() {
            dir = dir.open_dir_nofollow(part.as_os_str())?;
        }
    }
    Ok((
        dir,
        path.file_name()
            .ok_or_else(|| anyhow::anyhow!("file name is missing"))?
            .to_os_string(),
    ))
}

fn publish(
    parent: &Dir,
    name: &std::ffi::OsStr,
    bytes: &[u8],
    ctx: &ToolContext,
    validate_access: impl Fn() -> Result<()>,
) -> Result<()> {
    let temporary = format!(".aster-artifact-{}.tmp", uuid::Uuid::new_v4());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = parent.open_with(&temporary, &options)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        ensure!(!ctx.is_cancelled(), "office_file cancelled");
        validate_access()?;
        parent.hard_link(&temporary, parent, name)?;
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No).nonblock(true);
        let saved = parent.open_with(name, &options)?;
        ensure!(
            saved.metadata()?.is_file(),
            "saved artifact is not a regular file"
        );
        let mut readback = Vec::new();
        saved.take(MAX_FILE_BYTES + 1).read_to_end(&mut readback)?;
        ensure!(
            readback == bytes,
            "saved artifact changed during publication"
        );
        Ok(())
    })();
    drop(file);
    let _ = parent.remove_file(&temporary);
    result
}

#[cfg(test)]
#[path = "office_file_tests.rs"]
mod tests;
