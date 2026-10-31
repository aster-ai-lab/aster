//! OKF v0.2 compiler. One source file becomes one concept page.
//! A model does not write the prose. Callers pass the passages they already extracted,
//! or `compile_workspace` reads a local workspace and writes `wiki/`.
//! `publish_note_bundle` writes the bundle inside the directory it is given.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

mod extract;
mod personal;
mod workspace;

pub use personal::{
    abandon_workspace_knowledge_job, abandon_workspace_knowledge_maintenance,
    begin_workspace_knowledge_maintenance, cancel_workspace_knowledge_job,
    cancel_workspace_knowledge_maintenance, fail_workspace_knowledge_job,
    fail_workspace_knowledge_maintenance, finish_workspace_knowledge_maintenance,
    retained_workspace_document_projection, retire_workspace_knowledge,
    reuse_workspace_knowledge_maintenance, review_workspace_knowledge_change,
    workspace_knowledge_change, workspace_knowledge_citation,
    workspace_knowledge_compile_workspace, workspace_knowledge_maintenance_enabled,
    workspace_knowledge_maintenance_job, workspace_knowledge_maintenance_plan,
    workspace_knowledge_output_root, workspace_knowledge_page, workspace_knowledge_summary,
    WorkspaceKnowledgeCitation, WorkspaceKnowledgeInput, WorkspaceKnowledgePage,
    WorkspaceKnowledgeSource, WorkspaceKnowledgeStatus, WorkspaceKnowledgeSummary,
};
pub use workspace::{
    extract_workspace_document, project_workspace_document, WorkspaceDocumentAnchor,
    WorkspaceDocumentPassage, WorkspaceDocumentProjection, WorkspaceDocumentRead,
};

pub use extract::{
    extract_document, passage_bodies, project_text, BlockKind, ExtractPolicy, Extracted,
    ExtractedBlock, ImageRecognizer, RecognizedSpan, PARSER_GENERATION,
};
pub use personal::{
    abandon_knowledge_maintenance, accept_personal_bundle, begin_knowledge_maintenance,
    cancel_knowledge_maintenance, fail_knowledge_maintenance, finish_knowledge_maintenance,
    is_personal_vault, knowledge_change, knowledge_compile_workspace, knowledge_maintenance_job,
    knowledge_maintenance_plan, knowledge_page_path, pin_personal_compile, plan_personal_compile,
    publish_note_bundle, publish_note_bundle_with, release_personal_compile,
    remember_personal_generations, review_knowledge_change, try_workspace_knowledge_read,
    with_knowledge_source_mutation, workspace_knowledge_read, KnowledgeChangeDetail, KnowledgeJob,
    KnowledgePlan, KnowledgeSourceMutationBlocked, MaintenanceOperation, WorkspaceKnowledgeRead,
};

pub const PASSAGE_CAP: usize = 32;
pub const BODY_CAP: usize = 100_000;
pub const MAX_PAGES: usize = 48;
pub const MAX_DEPTH: usize = 4;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_PACKAGE_BYTES: usize = 8 * 1024 * 1024;
pub const OKF_DIR: &str = "wiki";
pub const OKF_INDEX: &str = "index.md";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkipReason {
    Unsupported,
    Unparsed,
    Empty,
    OverDepth,
    OverCount,
    OverSize,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Unparsed => "unparsed",
            Self::Empty => "empty",
            Self::OverDepth => "over_depth",
            Self::OverCount => "over_count",
            Self::OverSize => "over_size",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextPassage<'a> {
    pub heading: &'a str,
    pub body: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConceptRequest<'a> {
    pub title: &'a str,
    pub file_name: &'a str,
    pub content_type: &'a str,
    pub resource: &'a str,
    pub source_generation: &'a str,
    pub passages: &'a [TextPassage<'a>],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConceptPage {
    pub markdown: String,
    pub kept: usize,
    pub dropped: usize,
    pub truncated: bool,
}

/// One source has one result. Skip reasons stay on `Skipped`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileOutcome {
    Compiled,
    Unchanged,
    Skipped(SkipReason),
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceReport {
    pub compiled: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub failed: usize,
    pub outcomes: Vec<(String, CompileOutcome)>,
    /// Path and reason for `CompileOutcome::Failed`. Empty when the failure has no closed reason.
    pub failure_reasons: Vec<(String, String)>,
}

/// Compile one concept. Empty prose yields nothing. Truncation is written into the body.
pub fn compile_concept(request: &ConceptRequest<'_>) -> Option<ConceptPage> {
    if request.title.trim().is_empty() || request.resource.is_empty() {
        return None;
    }
    let prose = build_prose(request)?;
    if prose.text.trim().is_empty() {
        return None;
    }
    let dropped = request.passages.len().saturating_sub(prose.kept);
    let truncated = dropped > 0 || prose.body_cut;
    let markdown = render_concept(request, &prose.text, prose.kept, dropped, truncated);
    Some(ConceptPage {
        markdown,
        kept: prose.kept,
        dropped,
        truncated,
    })
}

pub fn content_generation(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn published_generation(markdown: &str) -> Option<&str> {
    frontmatter_value(markdown, "source_generation:")
}

pub fn published_parser(markdown: &str) -> Option<&str> {
    frontmatter_value(markdown, "parser_generation:")
}

fn frontmatter_value<'a>(markdown: &'a str, key: &str) -> Option<&'a str> {
    let inner = frontmatter_inner(markdown)?;
    for line in inner.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let value = rest.trim().trim_matches('"');
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

/// The closed frontmatter block. A value in the body is not a field.
fn frontmatter_inner(markdown: &str) -> Option<&str> {
    let mut lines = markdown.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return None;
    }
    let start = first.len();
    let mut offset = start;
    for line in lines {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some(&markdown[start..offset]);
        }
        offset += line.len();
    }
    None
}

/// Rewrite `wiki/` from workspace sources. A failed source keeps its previous concept file.
pub fn compile_workspace(root: &Path) -> io::Result<WorkspaceReport> {
    compile_workspace_with(root, None)
}

/// Same compile, with the caller's image recognizer. `None` leaves scans unparsed.
pub fn compile_workspace_with(
    root: &Path,
    recognize: Option<&dyn ImageRecognizer>,
) -> io::Result<WorkspaceReport> {
    if !root.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "workspace missing"));
    }
    let wiki = root.join(OKF_DIR);
    fs::create_dir_all(&wiki)?;
    let mut sources = Vec::new();
    collect_sources(root, root, 0, &mut sources)?;
    sources.sort_by(|left, right| left.relative.cmp(&right.relative));
    let mut compiled = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();
    let mut keep = Vec::new();
    let mut compiled_count = 0;
    let mut unchanged = 0;
    let mut outcomes = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        if index >= MAX_PAGES {
            skipped.push((source.relative.clone(), SkipReason::OverCount));
            outcomes.push((
                source.relative.clone(),
                CompileOutcome::Skipped(SkipReason::OverCount),
            ));
            continue;
        }
        if source.depth > MAX_DEPTH {
            skipped.push((source.relative.clone(), SkipReason::OverDepth));
            outcomes.push((
                source.relative.clone(),
                CompileOutcome::Skipped(SkipReason::OverDepth),
            ));
            continue;
        }
        let concept_name = concept_file_name(&source.relative);
        let concept_path = wiki.join(&concept_name);
        let generation = content_generation(&source.bytes);
        if let Some(existing) = read_text_file(&concept_path) {
            if published_generation(&existing) == Some(generation.as_str())
                && published_parser(&existing) == Some(PARSER_GENERATION)
                && existing.contains("type: Document")
            {
                keep.push(concept_name);
                compiled.push((source.title.clone(), concept_file_name(&source.relative)));
                outcomes.push((source.relative.clone(), CompileOutcome::Unchanged));
                unchanged += 1;
                continue;
            }
        }
        let policy = ExtractPolicy {
            recognize,
            max_text_bytes: MAX_TEXT_BYTES,
            max_package_bytes: MAX_PACKAGE_BYTES,
            max_output: BODY_CAP,
            deadline: Instant::now() + Duration::from_secs(5),
        };
        match extract_document(&source.name, &source.bytes, &policy) {
            Extracted::Parsed { blocks, .. } => {
                let owned = passage_bodies(&blocks);
                let passages = owned
                    .iter()
                    .map(|(heading, body)| TextPassage { heading, body })
                    .collect::<Vec<_>>();
                let resource = format!("workspace:{}", source.relative);
                let request = ConceptRequest {
                    title: &source.title,
                    file_name: &source.name,
                    content_type: "",
                    resource: &resource,
                    source_generation: &generation,
                    passages: &passages,
                };
                match compile_concept(&request) {
                    Some(page) => {
                        write_atomic(&concept_path, page.markdown.as_bytes())?;
                        keep.push(concept_name.clone());
                        compiled.push((source.title.clone(), concept_name));
                        outcomes.push((source.relative.clone(), CompileOutcome::Compiled));
                        compiled_count += 1;
                    }
                    None => {
                        let _ = fs::remove_file(&concept_path);
                        skipped.push((source.relative.clone(), SkipReason::Empty));
                        outcomes.push((
                            source.relative.clone(),
                            CompileOutcome::Skipped(SkipReason::Empty),
                        ));
                    }
                }
            }
            Extracted::Unparsed(reason) => {
                let _ = fs::remove_file(&concept_path);
                skipped.push((source.relative.clone(), reason));
                outcomes.push((source.relative.clone(), CompileOutcome::Skipped(reason)));
            }
            Extracted::Failed(message) => {
                if concept_path.is_file() {
                    keep.push(concept_name.clone());
                    compiled.push((source.title.clone(), concept_name));
                }
                failed.push((source.relative.clone(), message));
                outcomes.push((source.relative.clone(), CompileOutcome::Failed));
            }
        }
    }
    let index = render_index(&compiled, &skipped, &failed);
    write_atomic(&wiki.join(OKF_INDEX), index.as_bytes())?;
    if let Ok(entries) = fs::read_dir(&wiki) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == OKF_INDEX || keep.iter().any(|kept| kept == &name) {
                continue;
            }
            if name.ends_with(".md") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(WorkspaceReport {
        compiled: compiled_count,
        unchanged,
        skipped: skipped.len(),
        failed: failed.len(),
        outcomes,
        failure_reasons: failed,
    })
}

struct BuiltProse {
    text: String,
    kept: usize,
    body_cut: bool,
}

fn build_prose(request: &ConceptRequest<'_>) -> Option<BuiltProse> {
    let delimiter = table_delimiter(request.file_name, request.content_type);
    if let Some(delimiter) = delimiter {
        let raw = request
            .passages
            .iter()
            .map(|passage| passage.body.trim())
            .filter(|chunk| !chunk.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let already_table = raw.lines().any(|line| line.trim_start().starts_with('|'));
        if !already_table {
            if let Some((table, cut)) = markdown_table(&raw, delimiter) {
                return Some(BuiltProse {
                    text: table,
                    kept: 1,
                    body_cut: cut,
                });
            }
        }
    }
    let mut text = String::new();
    let mut kept = 0;
    let mut body_cut = false;
    for passage in request.passages.iter().take(PASSAGE_CAP) {
        let chunk = passage.body.trim();
        if chunk.is_empty() {
            continue;
        }
        if !text.is_empty() {
            if text.len() + 2 >= BODY_CAP {
                body_cut = true;
                break;
            }
            text.push_str("\n\n");
        }
        let room = BODY_CAP.saturating_sub(text.len());
        if room == 0 {
            body_cut = true;
            break;
        }
        let mut end = room.min(chunk.len());
        while end > 0 && !chunk.is_char_boundary(end) {
            end -= 1;
        }
        text.push_str(&chunk[..end]);
        kept += 1;
        if end < chunk.len() {
            body_cut = true;
            break;
        }
    }
    if request.passages.len() > PASSAGE_CAP {
        body_cut = true;
    }
    if kept == 0 {
        return None;
    }
    Some(BuiltProse {
        text,
        kept,
        body_cut,
    })
}

fn render_concept(
    request: &ConceptRequest<'_>,
    prose: &str,
    kept: usize,
    dropped: usize,
    truncated: bool,
) -> String {
    let title = yaml_quote(request.title);
    let resource = yaml_quote(request.resource);
    let source = yaml_quote(request.file_name);
    let generation = yaml_quote(request.source_generation);
    let parser = yaml_quote(PARSER_GENERATION);
    let mut body = format!(
        "---\ntype: Document\ntitle: {title}\nresource: {resource}\nsource_generation: {generation}\nparser_generation: {parser}\ngenerated:\n  by: aster-okf-compiler/0.2\nsources:\n  - id: source\n    resource: {resource}\n    title: {source}\n---\n\n{prose}\n"
    );
    if truncated {
        body.push_str(&format!(
            "\n正文已截断，保留 {kept} 段，略去 {dropped} 段。\n"
        ));
    }
    body.push_str("\n## Sources\n\n");
    let label = link_label(request.title);
    body.push_str(&format!("- [{label}]({})\n", request.resource));
    let mut ordinal = 0;
    for passage in request.passages.iter().take(kept) {
        let heading = passage.heading.trim().trim_start_matches('#').trim();
        let label = if heading.is_empty() {
            format!("段落 {}", ordinal + 1)
        } else {
            link_label(heading)
        };
        body.push_str(&format!(
            "- [{label}]({}#passage-{ordinal})\n",
            request.resource
        ));
        ordinal += 1;
    }
    body
}

pub(crate) fn render_index(
    concepts: &[(String, String)],
    skipped: &[(String, SkipReason)],
    failed: &[(String, String)],
) -> String {
    let mut body = String::from("---\nokf_version: \"0.2\"\n---\n\n");
    for (title, file) in concepts {
        let href = file
            .chars()
            .map(|ch| {
                if ch.is_ascii() && !ch.is_ascii_alphanumeric() && !"/-_.~".contains(ch) {
                    format!("%{:02X}", ch as u8)
                } else {
                    ch.to_string()
                }
            })
            .collect::<String>();
        body.push_str(&format!("- [{}](/{href})\n", link_label(title)));
    }
    if !skipped.is_empty() {
        body.push_str("\n## Skipped\n\n");
        for (path, reason) in skipped {
            body.push_str(&format!("- `{path}` — {}\n", reason.as_str()));
        }
    }
    if !failed.is_empty() {
        body.push_str("\n## Failed\n\n");
        for (path, reason) in failed {
            body.push_str(&format!("- `{path}` — {reason}\n"));
        }
    }
    body
}

struct LocalSource {
    relative: String,
    name: String,
    title: String,
    bytes: Vec<u8>,
    depth: usize,
}

fn collect_sources(
    root: &Path,
    directory: &Path,
    depth: usize,
    out: &mut Vec<LocalSource>,
) -> io::Result<()> {
    let entries = fs::read_dir(directory)?;
    let mut names = Vec::new();
    for entry in entries.flatten() {
        names.push(entry);
    }
    names.sort_by_key(|entry| entry.file_name());
    for entry in names {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.symlink_metadata()?.file_type().is_symlink() {
            continue;
        }
        let relative = match path.strip_prefix(root) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if depth == 0 && name == OKF_DIR {
            continue;
        }
        if path.is_dir() {
            if depth + 1 > MAX_DEPTH {
                out.push(LocalSource {
                    relative,
                    name,
                    title: String::new(),
                    bytes: Vec::new(),
                    depth: depth + 1,
                });
                continue;
            }
            collect_sources(root, &path, depth + 1, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).unwrap_or_default();
        let title = source_title(&name);
        out.push(LocalSource {
            relative,
            name,
            title,
            bytes,
            depth,
        });
    }
    Ok(())
}

pub(crate) fn source_title(name: &str) -> String {
    let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name);
    stem.chars().take(200).collect()
}

pub fn concept_file_name(relative: &str) -> String {
    let stem = strip_extension(relative);
    format!("{}.md", stem.replace('/', "__"))
}

fn strip_extension(relative: &str) -> String {
    let (dir, name) = match relative.rsplit_once('/') {
        Some((dir, name)) => (Some(dir), name),
        None => (None, relative),
    };
    let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name);
    match dir {
        Some(dir) => format!("{dir}/{stem}"),
        None => stem.to_string(),
    }
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn table_delimiter(name: &str, content_type: &str) -> Option<char> {
    let ext = extension(name);
    if ext == "tsv" || content_type == "text/tab-separated-values" {
        return Some('\t');
    }
    if ext == "csv" || content_type == "text/csv" {
        return Some(',');
    }
    None
}

pub(crate) fn markdown_table(text: &str, delimiter: char) -> Option<(String, bool)> {
    let mut rows = Vec::new();
    let mut cut = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if rows.len() == 200 {
            cut = true;
            break;
        }
        let cells = split_row(line, delimiter);
        if cells.iter().all(|cell| cell.is_empty()) {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return None;
    }
    let width = rows.iter().map(Vec::len).max()?;
    if width == 0 || width > 32 {
        return None;
    }
    let mut out = String::new();
    for (index, row) in rows.iter().enumerate() {
        write_row(&mut out, row, width);
        if index == 0 {
            out.push('|');
            for _ in 0..width {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    if cut {
        out.push_str("\n正文已截断，保留 200 行。\n");
    }
    Some((out, cut))
}

fn write_row(out: &mut String, row: &[String], width: usize) {
    out.push('|');
    for index in 0..width {
        let value = row.get(index).map(String::as_str).unwrap_or("");
        out.push(' ');
        out.push_str(&value.replace('|', "\\|"));
        out.push_str(" |");
    }
    out.push('\n');
}

fn split_row(line: &str, delimiter: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(ch);
            }
            continue;
        }
        if ch == '"' && cell.is_empty() {
            quoted = true;
            continue;
        }
        if ch == delimiter {
            cells.push(cell.trim().to_string());
            cell.clear();
            continue;
        }
        cell.push(ch);
    }
    cells.push(cell.trim().to_string());
    cells
}

pub(crate) fn yaml_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars().take(200) {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn link_label(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '[' | ']' | '(' | ')' | '\n' | '\r'))
        .take(200)
        .collect()
}

fn read_text_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    String::from_utf8(bytes).ok()
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut temp = parent.to_path_buf();
    temp.push(format!(
        ".{}.partial",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("page")
    ));
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("aster-okf-{nanos}-{n}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_kept_sentence_is_compiled_and_the_tail_is_marked_absent() {
        let mut passages = Vec::new();
        let mut owned = Vec::new();
        for index in 0..33 {
            owned.push(format!("句子 {index} 在这一段"));
        }
        for body in &owned {
            passages.push(TextPassage { heading: "", body });
        }
        let page = compile_concept(&ConceptRequest {
            title: "规范",
            file_name: "规范.md",
            content_type: "text/markdown",
            resource: "workspace:规范.md",
            source_generation: "gen-1",
            passages: &passages,
        })
        .expect("page");
        assert!(page.markdown.contains("句子 0 在这一段"));
        assert!(!page.markdown.contains("句子 32 在这一段"));
        assert!(page
            .markdown
            .contains("正文已截断，保留 32 段，略去 1 段。"));
        assert!(page.markdown.contains("](workspace:规范.md#passage-0)"));
        assert!(page.markdown.contains("source_generation: \"gen-1\""));
        assert_eq!(published_parser(&page.markdown), Some(PARSER_GENERATION));
        assert!(page.markdown.starts_with("---\n"));
        assert!(!page.markdown.contains("[["));
        let index = render_index(&[], &[("scan.png".into(), SkipReason::Unsupported)], &[]);
        assert!(index.starts_with("---\nokf_version: \"0.2\"\n---\n"));
        assert!(index.contains("`scan.png` — unsupported"));
        assert!(!index.contains("okf_version: \"0.2\"\nscan"));
    }

    #[test]
    fn a_source_is_compiled_skipped_or_failed() {
        let root = scratch();
        fs::write(root.join("规范.md"), "句子 0 在这一段\n").unwrap();
        fs::write(root.join("scan.png"), b"\x89PNG").unwrap();
        let report = compile_workspace(&root).unwrap();
        assert!(report.outcomes.iter().any(|(name, outcome)| {
            name == "规范.md" && *outcome == CompileOutcome::Compiled
        }));
        assert!(report.outcomes.iter().any(|(name, outcome)| {
            name == "scan.png" && *outcome == CompileOutcome::Skipped(SkipReason::Unparsed)
        }));
        let index = fs::read_to_string(root.join("wiki").join("index.md")).unwrap();
        assert!(index.contains("`scan.png` — unparsed"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_csv_passage_becomes_a_markdown_table() {
        let body = "a,b\n1,\"2,3\"\n";
        let page = compile_concept(&ConceptRequest {
            title: "rows",
            file_name: "rows.csv",
            content_type: "text/csv",
            resource: "drive:file",
            source_generation: "1",
            passages: &[TextPassage { heading: "", body }],
        })
        .expect("table");
        assert!(page.markdown.contains("| a | b |"));
        assert!(page.markdown.contains("| 1 | 2,3 |"));
    }

    #[test]
    fn a_failed_rewrite_keeps_the_previous_concept() {
        let root = scratch();
        fs::write(root.join("规范.md"), "差旅标准写在这里").unwrap();
        let first = compile_workspace(&root).unwrap();
        assert_eq!(first.compiled, 1);
        let concept = root.join("wiki").join("规范.md");
        let previous = fs::read_to_string(&concept).unwrap();
        assert!(previous.contains("差旅标准写在这里"));
        fs::write(root.join("规范.md"), [0xff, 0xfe]).unwrap();
        let second = compile_workspace(&root).unwrap();
        assert_eq!(second.failed, 1);
        let kept = fs::read_to_string(&concept).unwrap();
        assert!(kept.contains("差旅标准写在这里"));
        assert!(kept.contains("type: Document"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unchanged_source_is_not_rewritten() {
        let root = scratch();
        fs::write(root.join("规范.md"), "同一句").unwrap();
        compile_workspace(&root).unwrap();
        let concept = root.join("wiki").join("规范.md");
        let stamp = fs::metadata(&concept).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let report = compile_workspace(&root).unwrap();
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.compiled, 0);
        assert_eq!(
            concept_file_name("制度/报销.docx"),
            "制度/报销.md".replace('/', "__")
        );
        assert_eq!(fs::metadata(&concept).unwrap().modified().unwrap(), stamp);
        let index = fs::read_to_string(root.join("wiki").join("index.md")).unwrap();
        assert!(index.contains("okf_version: \"0.2\""));
        assert!(!index.contains("source_generation:"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn html_headings_survive_extraction() {
        let policy = ExtractPolicy {
            recognize: None,
            max_text_bytes: MAX_TEXT_BYTES,
            max_package_bytes: MAX_PACKAGE_BYTES,
            max_output: BODY_CAP,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        };
        let extracted = extract_document(
            "page.html",
            "<h1>标题</h1><p>页面句子</p><script>隐藏脚本</script>".as_bytes(),
            &policy,
        );
        let Extracted::Parsed { blocks, .. } = extracted else {
            panic!("html parsed");
        };
        let text = project_text(&blocks);
        assert!(text.contains("# 标题"));
        assert!(text.contains("页面句子"));
        assert!(!text.contains("隐藏脚本"));
    }

    struct FixedOcr;
    impl ImageRecognizer for FixedOcr {
        fn recognize(&self, _bytes: &[u8]) -> Result<Vec<RecognizedSpan>, String> {
            Ok(vec![RecognizedSpan {
                text: "界面验收句".into(),
                confidence: Some(0.9),
            }])
        }
    }

    #[test]
    fn the_same_bytes_project_the_same_text_and_a_scan_needs_ocr() {
        let policy = ExtractPolicy {
            recognize: None,
            max_text_bytes: MAX_TEXT_BYTES,
            max_package_bytes: MAX_PACKAGE_BYTES,
            max_output: BODY_CAP,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        };
        let bytes = "句子 0 在这一段\n".as_bytes();
        let once = extract_document("规范.md", bytes, &policy);
        let twice = extract_document("规范.md", bytes, &policy);
        let Extracted::Parsed { blocks: left, .. } = once else {
            panic!("parsed")
        };
        let Extracted::Parsed { blocks: right, .. } = twice else {
            panic!("parsed")
        };
        assert_eq!(project_text(&left), project_text(&right));
        assert!(project_text(&left).contains("句子 0 在这一段"));
        let scan = extract_document("scan.png", b"\x89PNG\r\n\x1a\n", &policy);
        assert_eq!(scan, Extracted::Unparsed(SkipReason::Unparsed));
        let ocr = FixedOcr;
        let recognized = extract_document(
            "scan.png",
            b"\x89PNG\r\n\x1a\n",
            &ExtractPolicy {
                recognize: Some(&ocr),
                max_text_bytes: MAX_TEXT_BYTES,
                max_package_bytes: MAX_PACKAGE_BYTES,
                max_output: BODY_CAP,
                deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
            },
        );
        let Extracted::Parsed { blocks, .. } = recognized else {
            panic!("ocr text");
        };
        assert!(project_text(&blocks).contains("界面验收句"));
    }

    #[test]
    fn a_stale_parser_generation_is_rewritten() {
        let root = scratch();
        fs::write(root.join("规范.md"), "差旅标准写在这里").unwrap();
        compile_workspace(&root).unwrap();
        let concept = root.join("wiki").join("规范.md");
        let stale = fs::read_to_string(&concept)
            .unwrap()
            .replace(PARSER_GENERATION, "builtin:0");
        fs::write(&concept, stale).unwrap();
        let report = compile_workspace(&root).unwrap();
        assert_eq!(report.unchanged, 0);
        assert_eq!(report.compiled, 1);
        let body = fs::read_to_string(&concept).unwrap();
        assert_eq!(published_parser(&body), Some(PARSER_GENERATION));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_oversized_zip_is_refused_before_blocks() {
        let policy = ExtractPolicy {
            recognize: None,
            max_text_bytes: MAX_TEXT_BYTES,
            max_package_bytes: 8,
            max_output: BODY_CAP,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        };
        let extracted = extract_document("note.docx", b"PK\x03\x04oversized-package", &policy);
        assert_eq!(extracted, Extracted::Unparsed(SkipReason::OverCount));
    }

    #[test]
    fn a_csv_source_is_a_table_block() {
        let policy = ExtractPolicy {
            recognize: None,
            max_text_bytes: MAX_TEXT_BYTES,
            max_package_bytes: MAX_PACKAGE_BYTES,
            max_output: BODY_CAP,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        };
        let extracted = extract_document("rows.csv", b"a,b\n1,2\n", &policy);
        let Extracted::Parsed { blocks, .. } = extracted else {
            panic!("table");
        };
        assert_eq!(blocks[0].kind, BlockKind::Table);
        assert!(blocks[0].text.contains("| a | b |"));
    }
}
