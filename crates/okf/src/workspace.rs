//! Document projections for already-admitted workspace snapshots, not a file reader.

use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::{
    content_generation, extract_document, BlockKind, ExtractPolicy, Extracted, SkipReason,
    MAX_PACKAGE_BYTES, MAX_TEXT_BYTES, PARSER_GENERATION,
};

/// A block in this parser's converted output, never an invented source paragraph or cell.
/// `page` is present only when the extractor supplied a real PDF page marker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDocumentAnchor {
    pub block: usize,
    pub page: Option<u8>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDocumentPassage {
    pub anchor: WorkspaceDocumentAnchor,
    pub text: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDocumentProjection {
    source_generation: String,
    parser_generation: String,
    /// Binds converted text, anchors and truncation as well as the original bytes.
    projection_generation: String,
    passages: Vec<WorkspaceDocumentPassage>,
    truncated: bool,
}

impl WorkspaceDocumentProjection {
    pub(crate) fn matches(&self, bytes: &[u8], parser: &str) -> bool {
        let mut size = 0usize;
        let mut block = 0;
        if self.source_generation != format!("sha256:{}", content_generation(bytes))
            || self.parser_generation != parser
            || self.passages.len() > 4096
            || self.passages.is_empty()
        {
            return false;
        }
        for passage in &self.passages {
            size = size.saturating_add(passage.text.len());
            if passage.anchor.block <= block
                || passage.anchor.page == Some(0)
                || passage.text.contains('\0')
                || size > MAX_TEXT_BYTES
            {
                return false;
            }
            block = passage.anchor.block;
        }
        let identity = serde_json::to_vec(&(
            "workspace-wiki-document-v1",
            &self.source_generation,
            parser,
            &self.passages,
            self.truncated,
        ));
        identity.is_ok_and(|bytes| {
            self.projection_generation == format!("sha256:{}", content_generation(&bytes))
        })
    }

    pub fn source_generation(&self) -> &str {
        &self.source_generation
    }
    pub fn parser_generation(&self) -> &str {
        &self.parser_generation
    }
    pub fn projection_generation(&self) -> &str {
        &self.projection_generation
    }
    pub fn passages(&self) -> &[WorkspaceDocumentPassage] {
        &self.passages
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// A source citation must bind both the admitted bytes and this exact conversion.
    pub fn resolve_passage(
        &self,
        source_generation: &str,
        projection_generation: &str,
        anchor: &WorkspaceDocumentAnchor,
    ) -> Option<&str> {
        if source_generation != self.source_generation
            || projection_generation != self.projection_generation
        {
            return None;
        }
        self.passages
            .iter()
            .find(|passage| passage.anchor == *anchor)
            .map(|passage| passage.text.as_str())
    }
}

impl std::fmt::Debug for WorkspaceDocumentProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceDocumentProjection")
            .field("source_generation", &self.source_generation)
            .field("parser_generation", &self.parser_generation)
            .field("projection_generation", &self.projection_generation)
            .field("passages", &self.passages.len())
            .field("truncated", &self.truncated)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceDocumentRead {
    Parsed(WorkspaceDocumentProjection),
    Unparsed(SkipReason),
    Failed(String),
}

/// Convert only bytes the host already admitted through its workspace source policy.
/// The original extractor supplies every block and page; no OCR, model or path I/O runs.
pub fn extract_workspace_document(
    name: &str,
    bytes: &[u8],
    policy: &ExtractPolicy<'_>,
) -> WorkspaceDocumentRead {
    use WorkspaceDocumentRead::{Failed, Unparsed};
    if policy.max_text_bytes == 0
        || policy.max_text_bytes > MAX_TEXT_BYTES
        || policy.max_package_bytes == 0
        || policy.max_package_bytes > MAX_PACKAGE_BYTES
        || policy.max_output == 0
        || policy.max_output > MAX_TEXT_BYTES
    {
        return Failed("workspace_wiki.document.invalid_limits".into());
    }
    if policy.deadline <= Instant::now() {
        return Failed("timeout".into());
    }
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let header = match extension.as_str() {
        "pdf" => b"%PDF-".as_slice(),
        "docx" | "xlsx" | "pptx" | "odt" | "ods" | "odp" => b"PK".as_slice(),
        _ => return Unparsed(SkipReason::Unsupported),
    };
    if bytes.is_empty() {
        return Unparsed(SkipReason::Empty);
    }
    if bytes.len() > policy.max_package_bytes {
        return Unparsed(SkipReason::OverSize);
    }
    if !bytes.starts_with(header) {
        return Failed("workspace_wiki.document.format_mismatch".into());
    }
    let bounded = ExtractPolicy {
        recognize: None,
        max_text_bytes: policy.max_text_bytes,
        max_package_bytes: policy.max_package_bytes,
        max_output: policy.max_output,
        deadline: policy.deadline,
    };
    let extraction = extract_document(name, bytes, &bounded);
    if policy.deadline <= Instant::now() {
        return Failed("timeout".into());
    }
    project_workspace_document(bytes, PARSER_GENERATION, extraction, policy)
}

/// Bind a host processor's bounded blocks to the exact admitted source bytes.
pub fn project_workspace_document(
    bytes: &[u8],
    parser_generation: &str,
    extraction: Extracted,
    policy: &ExtractPolicy<'_>,
) -> WorkspaceDocumentRead {
    use WorkspaceDocumentRead::{Failed, Parsed, Unparsed};
    if bytes.is_empty()
        || bytes.len() > MAX_PACKAGE_BYTES
        || parser_generation.is_empty()
        || parser_generation.len() > 256
        || parser_generation.chars().any(char::is_control)
        || policy.max_output == 0
        || policy.max_output > MAX_TEXT_BYTES
    {
        return Failed("workspace_wiki.document.invalid_projection".into());
    }
    if policy.deadline <= Instant::now() {
        return Failed("timeout".into());
    }
    let (blocks, truncated) = match extraction {
        Extracted::Parsed { blocks, truncated } => (blocks, truncated),
        Extracted::Unparsed(reason) => return Unparsed(reason),
        Extracted::Failed(reason) => return Failed(reason),
    };
    let mut page = None;
    let mut passages = Vec::new();
    let mut size = 0usize;
    if blocks.len() > 4096 {
        return Failed("workspace_wiki.document.projection_limit".into());
    }
    for (index, block) in blocks.into_iter().enumerate() {
        size = size.saturating_add(block.text.len());
        if size > policy.max_output || block.text.contains('\0') {
            return Failed("workspace_wiki.document.projection_limit".into());
        }
        if block.kind == BlockKind::Page {
            if block.level == 0 {
                return Failed("workspace_wiki.document.invalid_page".into());
            }
            page = Some(block.level);
            continue;
        }
        if !block.text.trim().is_empty() {
            passages.push(WorkspaceDocumentPassage {
                anchor: WorkspaceDocumentAnchor {
                    block: index + 1,
                    page,
                },
                text: block.text,
            });
        }
    }
    if passages.is_empty() {
        return Unparsed(SkipReason::Unparsed);
    }
    let source_generation = format!("sha256:{}", content_generation(bytes));
    let identity = match serde_json::to_vec(&(
        "workspace-wiki-document-v1",
        &source_generation,
        parser_generation,
        &passages,
        truncated,
    )) {
        Ok(bytes) => bytes,
        Err(_) => return Failed("workspace_wiki.document.invalid_projection".into()),
    };
    Parsed(WorkspaceDocumentProjection {
        source_generation,
        parser_generation: parser_generation.into(),
        projection_generation: format!("sha256:{}", content_generation(&identity)),
        passages,
        truncated,
    })
}

#[cfg(test)]
mod tests;
