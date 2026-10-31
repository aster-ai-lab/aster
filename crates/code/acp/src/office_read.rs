//! Bounded source snapshots through the native Office semantic model.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use aster_core::capability::WorkspaceOperation;
use aster_core::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use aster_office::{NativeOfficeDocument, NativeOfficePackage, OfficeNodeType, PackageLimits};
use async_trait::async_trait;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::OpenOptions;
use serde::Deserialize;
use serde_json::{json, Value};

const FILE_LIMIT: u64 = 20 * 1024 * 1024;
const OUTPUT_LIMIT: usize = 64 * 1024;

pub(crate) struct OfficeReadTool {
    workspace: crate::plugin_workspace::WorkspaceBinding,
    host: Option<(Arc<crate::document_read::DocumentBridge>, String)>,
}

impl OfficeReadTool {
    pub(crate) fn new(workspace: &Path) -> Result<Self> {
        Ok(Self {
            workspace: crate::plugin_workspace::WorkspaceBinding::new(workspace)?,
            host: None,
        })
    }

    pub(crate) fn with_host(
        mut self,
        bridge: Option<Arc<crate::document_read::DocumentBridge>>,
        session: String,
    ) -> Self {
        self.host = bridge.map(|bridge| (bridge, session));
        self
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    file_path: String,
    node_path: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "first_page")]
    start_page: usize,
    #[serde(default = "page_count")]
    page_count: usize,
}

fn first_page() -> usize {
    1
}
fn page_count() -> usize {
    8
}

fn default_limit() -> usize {
    200
}

#[derive(Clone, Copy)]
enum Kind {
    Docx,
    Xlsx,
    Pptx,
    Pdf,
    Image,
}

impl Kind {
    fn parse(extension: &str) -> Result<Self> {
        match extension {
            "docx" => Ok(Self::Docx),
            "xlsx" => Ok(Self::Xlsx),
            "pptx" => Ok(Self::Pptx),
            "pdf" => Ok(Self::Pdf),
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" => Ok(Self::Image),
            _ => bail!(
                "office_read source extension must be docx, xlsx, pptx, pdf or a supported image"
            ),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Pdf => "pdf",
            Self::Image => "image",
        }
    }
}

#[async_trait]
impl Tool for OfficeReadTool {
    fn name(&self) -> &str {
        "office_read"
    }
    fn description(&self) -> &str {
        if self.host.is_some() {
            "Read persisted DOCX/XLSX/PPTX, PDFs and images without Bash or installing packages. Studio provides local PP-OCRv6: PDF text layers are read directly; scanned pages and images use OCR. Source SHA-256 and page numbers are retained; OCR is untrusted extracted evidence. PDF/image reads use start_page (default 1) and page_count (default 8, max 16); follow next_page until null before claiming full coverage. Repeated reads reuse cached results. Office reads return stable native nodes and typed spreadsheet cells; limit is 1..1000. Input max 20 MiB, output max 64 KiB."
        } else {
            "Read persisted DOCX/XLSX/PPTX sources with stable native nodes, typed cells and source SHA-256. Input max 20 MiB; output max 64 KiB; limit 1..1000. PDF and image OCR require an admitted Studio document host and are unavailable here."
        }
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"required":["file_path"],"properties":{
            "file_path":{"type":"string","description":"Persisted Office source path inside the current workspace."},
            "node_path":{"type":"string","description":"Optional exact native semantic path returned by a previous read; no wildcard or guessed business label."},
            "limit":{"type":"integer","minimum":1,"maximum":1000,"description":"Annotated entry bound; default 200. Exact node reads return that node without children."}
            ,"start_page":{"type":"integer","minimum":1,"maximum":500,"description":"First PDF page, numbered from 1; default 1."}
            ,"page_count":{"type":"integer","minimum":1,"maximum":16,"description":"PDF page count; default 8. Follow next_page for remaining pages."}
        }})
    }
    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        ToolCapabilities {
            read_only: true,
            idempotent: true,
            ..ToolCapabilities::conservative()
        }
    }
    fn validate_execution_available(&self, _: &Value, ctx: &ToolContext) -> Result<()> {
        self.workspace.validate(&ctx.workspace)?;
        ensure!(
            ctx.workspace_services.capabilities().read,
            "workspace does not grant reads"
        );
        ensure!(
            ctx.workspace_services.local_root().is_some(),
            "office_read requires a local workspace"
        );
        Ok(())
    }
    async fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let result = async {
            self.validate_execution_available(args, ctx)?;
            let access = ctx.workspace_operation(WorkspaceOperation::Read)?;
            ctx.register_capability_effect(self.workspace.clone())?;
            read_source(args, ctx, self.host.as_ref(), || {
                self.workspace.validate(&ctx.workspace)?;
                access.check()?;
                Ok(())
            })
            .await
        }
        .await;
        Ok(match result {
            Ok(value) => ToolOutput::success(value.to_string()),
            Err(error) => ToolOutput::error(format!("office_read: {error}")),
        })
    }
}

async fn read_source(
    args: &Value,
    ctx: &ToolContext,
    host: Option<&(Arc<crate::document_read::DocumentBridge>, String)>,
    validate: impl Fn() -> Result<()>,
) -> Result<Value> {
    validate()?;
    ensure!(
        !ctx.cancellation_token().is_cancelled(),
        "office_read cancelled"
    );
    ensure!(
        serde_json::to_vec(args)?.len() <= 16 * 1024,
        "office_read arguments exceed 16 KiB"
    );
    let input: Input = serde_json::from_value(args.clone())?;
    ensure!(
        (1..=1000).contains(&input.limit),
        "office_read limit must be 1..1000"
    );
    ensure!(
        ctx.workspace_services.capabilities().read,
        "workspace does not grant reads"
    );
    let root = ctx
        .workspace_services
        .local_root()
        .ok_or_else(|| anyhow::anyhow!("requires a local workspace"))?;
    let path = ctx.resolve_workspace_path(&input.file_path)?;
    let extension = Path::new(path.as_str())
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = Kind::parse(&extension)?;
    if matches!(kind, Kind::Pdf | Kind::Image) && host.is_none() {
        bail!("native PDF text extraction, PDF page images and OCR are unavailable without an admitted Studio document host; no source text was read");
    }
    let (parent, name) =
        crate::office_file::parent_dir(root, path.as_str()).context("Office source not found")?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = parent
        .open_with(name, &options)
        .context("Office source not found")?
        .into_std();
    ctx.workspace_services
        .refuse_direct_read(&path, &file)
        .context("Office source not found")?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= FILE_LIMIT,
        "source must be a regular file of at most 20 MiB"
    );
    let mut bytes = Vec::new();
    (&file).take(FILE_LIMIT + 1).read_to_end(&mut bytes)?;
    ctx.workspace_services
        .refuse_direct_read(&path, &file)
        .context("Office source not found")?;
    ensure!(bytes.len() as u64 <= FILE_LIMIT, "source exceeds 20 MiB");
    validate()?;
    if matches!(kind, Kind::Pdf | Kind::Image) {
        ensure!(
            input.node_path.is_none(),
            "PDF/image reads use start_page and page_count instead of node_path"
        );
        ensure!(
            (1..=500).contains(&input.start_page) && (1..=16).contains(&input.page_count),
            "invalid PDF/image page range"
        );
        let (bridge, session) = host.context("document host unavailable")?;
        use sha2::Digest;
        let digest = format!("{:x}", sha2::Sha256::digest(&bytes));
        let mut value = bridge
            .process_source(
                ctx.workspace_operation(WorkspaceOperation::Read)?,
                session,
                &root.join(path.as_str()),
                &digest,
                input.start_page,
                input.page_count,
            )
            .await?;
        validate()?;
        value["file_path"] = json!(path.as_str());
        return Ok(value);
    }
    // The parser receives a private bounded snapshot, never a path that can be retargeted outside the workspace.
    let mut snapshot = tempfile::Builder::new()
        .prefix("aster-office-read-")
        .suffix(&format!(".{}", kind.as_str()))
        .tempfile()?;
    snapshot.write_all(&bytes)?;
    let package = NativeOfficePackage::open_with_limits(
        snapshot.path(),
        PackageLimits {
            max_archive_bytes: FILE_LIMIT,
            max_entries: 4096,
            max_part_bytes: 8 * 1024 * 1024,
            max_uncompressed_bytes: 64 * 1024 * 1024,
            max_compression_ratio: 250,
        },
    )
    .await?;
    let digest = package.source_revision().sha256.clone();
    let document = NativeOfficeDocument::from_package(package)?;
    let mut result = if let Some(node_path) = input.node_path {
        let node = document.get(&node_path, 0)?;
        let mut value = serde_json::to_value(&node)?;
        if let Some(typed) = cell_value(node.node_type, &node.text, &node.format)? {
            value["value"] = typed;
        }
        json!({"node":value})
    } else {
        let view = document
            .annotated(aster_office::NativeOfficeAnnotatedOptions { limit: input.limit })?;
        let mut entries = Vec::with_capacity(view.entries.len());
        for entry in view.entries {
            let mut value = serde_json::to_value(&entry)?;
            if entry.node_type == OfficeNodeType::Cell {
                ensure!(
                    !entry.text_truncated,
                    "cell value exceeds annotated bound; read its exact node_path"
                );
                let cell = document.get(&entry.path, 0)?;
                value["format"] = serde_json::to_value(&cell.format)?;
                if let Some(typed) = cell_value(cell.node_type, &cell.text, &cell.format)? {
                    value["value"] = typed;
                }
            }
            entries.push(value);
        }
        let mut value = json!({"entries":entries,"total":view.total,"returned":view.returned,"truncated":view.truncated});
        if matches!(kind, Kind::Xlsx) {
            let sheets: Vec<_> = document
                .root()
                .children
                .iter()
                .filter(|node| node.node_type == OfficeNodeType::Worksheet)
                .collect();
            // Keep discovery before detailed cells when a long JSON result is projected or paged.
            value["document_outline"] = json!({
                "paths": sheets.iter().take(input.limit).map(|node| node.path.as_str()).collect::<Vec<_>>(),
                "total": sheets.len(), "truncated": sheets.len() > input.limit
            });
        }
        value
    };
    result["file_path"] = json!(path.as_str());
    result["kind"] = json!(kind.as_str());
    result["source_sha256"] = json!(digest);
    result["capabilities"] =
        json!({"ooxml":true,"pdf_text":false,"pdf_page_image":false,"ocr":false});
    // Keep the outline ahead of entries even when serde_json enables preserve_order.
    result
        .as_object_mut()
        .context("Office source response must be an object")?
        .sort_keys();
    ensure!(
        serde_json::to_vec(&result)?.len() <= OUTPUT_LIMIT,
        "office_read output exceeds 64 KiB; lower limit or select a narrower node_path"
    );
    validate()?;
    ensure!(
        !ctx.cancellation_token().is_cancelled(),
        "office_read cancelled"
    );
    Ok(result)
}

fn cell_value(
    kind: OfficeNodeType,
    text: &str,
    format: &BTreeMap<String, String>,
) -> Result<Option<Value>> {
    if kind != OfficeNodeType::Cell {
        return Ok(None);
    }
    if format
        .get("valuePresent")
        .is_some_and(|value| value == "false")
    {
        return Ok(Some(Value::Null));
    }
    Ok(Some(match format.get("valueType").map(String::as_str) {
        Some("Number") => Value::Number(text.parse()?),
        Some("Boolean") => match text {
            "true" => json!(true),
            "false" => json!(false),
            _ => bail!("invalid source boolean value"),
        },
        Some("String" | "Date" | "Error") => json!(text),
        _ => bail!("unknown native source cell value type"),
    }))
}

#[cfg(test)]
#[path = "office_read_tests.rs"]
mod tests;
