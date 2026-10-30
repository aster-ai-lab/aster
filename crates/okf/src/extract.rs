//! One document extractor for cloud upload and local OKF compile.
//! Callers pass the size policy. Recognition of pictures is injected.

use std::io::Read;
use std::time::Instant;

use lopdf::Document;
use zip::ZipArchive;

use crate::SkipReason;

pub const PARSER_GENERATION: &str = "builtin:2";
const MAX_ZIP_ENTRIES: usize = 64;
const MAX_ZIP_RATIO: u64 = 100;
const MAX_PDF_PAGES: usize = 40;
const OCR_CONFIDENCE_MIN: f32 = 0.5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockKind {
    Heading,
    Paragraph,
    Table,
    Page,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedBlock {
    pub kind: BlockKind,
    pub level: u8,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecognizedSpan {
    pub text: String,
    pub confidence: Option<f32>,
}

pub trait ImageRecognizer {
    fn recognize(&self, bytes: &[u8]) -> Result<Vec<RecognizedSpan>, String>;
}

pub struct ExtractPolicy<'a> {
    pub recognize: Option<&'a dyn ImageRecognizer>,
    pub max_text_bytes: usize,
    pub max_package_bytes: usize,
    pub max_output: usize,
    pub deadline: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Extracted {
    Parsed {
        blocks: Vec<ExtractedBlock>,
        truncated: bool,
    },
    Unparsed(SkipReason),
    Failed(String),
}

pub fn project_text(blocks: &[ExtractedBlock]) -> String {
    let mut out = String::new();
    for block in blocks {
        let piece = match block.kind {
            BlockKind::Heading => format!(
                "{} {}",
                "#".repeat(block.level.max(1) as usize),
                block.text.trim()
            ),
            BlockKind::Page => format!("## Page {}", block.level),
            BlockKind::Table | BlockKind::Paragraph => block.text.trim().to_string(),
        };
        if piece.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&piece);
    }
    out
}

/// Heading and page blocks start a passage. A table is its own passage.
pub fn passage_bodies(blocks: &[ExtractedBlock]) -> Vec<(String, String)> {
    let mut passages = Vec::new();
    let mut heading = String::new();
    let mut body = String::new();
    let flush = |passages: &mut Vec<(String, String)>, heading: &str, body: &str| {
        if body.trim().is_empty() && heading.trim().is_empty() {
            return;
        }
        passages.push((heading.trim().to_string(), body.trim().to_string()));
    };
    for block in blocks {
        match block.kind {
            BlockKind::Heading | BlockKind::Page => {
                flush(&mut passages, &heading, &body);
                body.clear();
                heading = match block.kind {
                    BlockKind::Page => format!("## Page {}", block.level),
                    _ => format!(
                        "{} {}",
                        "#".repeat(block.level.max(1) as usize),
                        block.text.trim()
                    ),
                };
            }
            BlockKind::Table => {
                flush(&mut passages, &heading, &body);
                body.clear();
                heading.clear();
                if !block.text.trim().is_empty() {
                    passages.push((String::new(), block.text.trim().to_string()));
                }
            }
            BlockKind::Paragraph => {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(block.text.trim());
            }
        }
    }
    flush(&mut passages, &heading, &body);
    passages
}

pub fn extract_document(name: &str, bytes: &[u8], policy: &ExtractPolicy<'_>) -> Extracted {
    if policy.deadline <= Instant::now() {
        return Extracted::Failed("timeout".into());
    }
    if bytes.is_empty() {
        return Extracted::Unparsed(SkipReason::Empty);
    }
    if is_raster(bytes)
        || matches!(
            extension(name).as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp"
        )
    {
        return recognize_into_blocks(bytes, policy);
    }
    if bytes.starts_with(b"%PDF-") {
        return extract_pdf(bytes, policy);
    }
    if bytes.starts_with(b"PK") {
        return extract_office(bytes, policy);
    }
    let ext = extension(name);
    if matches!(
        ext.as_str(),
        "md" | "markdown" | "txt" | "csv" | "tsv" | "json" | "html" | "htm" | "rtf"
    ) && bytes.len() > policy.max_text_bytes
    {
        return Extracted::Unparsed(SkipReason::OverCount);
    }
    let text_ext = matches!(
        ext.as_str(),
        "md" | "markdown" | "txt" | "csv" | "tsv" | "json" | "html" | "htm" | "rtf"
    );
    if bytes.contains(&0) {
        return if text_ext {
            Extracted::Unparsed(SkipReason::Unparsed)
        } else {
            Extracted::Unparsed(SkipReason::Unsupported)
        };
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return if matches!(
            ext.as_str(),
            "md" | "markdown" | "txt" | "csv" | "tsv" | "json" | "html" | "htm" | "rtf"
        ) {
            Extracted::Failed("encoding".into())
        } else {
            Extracted::Unparsed(SkipReason::Unsupported)
        };
    };
    let text = if ext == "html" || ext == "htm" || looks_like_html(text) {
        markup_text(text)
    } else if ext == "rtf" || text.trim_start().starts_with("{\\rtf") {
        rtf_text(text).unwrap_or_default()
    } else {
        text.to_string()
    };
    if let Some(delimiter) =
        matches!(ext.as_str(), "csv" | "tsv").then(|| if ext == "tsv" { '\t' } else { ',' })
    {
        if let Some((table, row_cut)) = crate::markdown_table(&text, delimiter) {
            return table_block(&table, policy.max_output, row_cut);
        }
    }
    blocks_from_plain(&text, policy.max_output)
}

fn table_block(table: &str, max_output: usize, row_cut: bool) -> Extracted {
    let truncated = row_cut || table.len() > max_output;
    let mut text = table.to_string();
    if truncated {
        let mut end = max_output.min(text.len());
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    Extracted::Parsed {
        blocks: vec![ExtractedBlock {
            kind: BlockKind::Table,
            level: 0,
            text,
        }],
        truncated,
    }
}

fn blocks_from_plain(text: &str, max_output: usize) -> Extracted {
    let mut truncated = false;
    let mut kept = text.to_string();
    if kept.len() > max_output {
        truncated = true;
        let mut end = max_output;
        while end > 0 && !kept.is_char_boundary(end) {
            end -= 1;
        }
        kept.truncate(end);
    }
    if kept.trim().is_empty() {
        return Extracted::Unparsed(SkipReason::Empty);
    }
    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    let flush = |blocks: &mut Vec<ExtractedBlock>, paragraph: &mut String| {
        if paragraph.trim().is_empty() {
            paragraph.clear();
            return;
        }
        blocks.push(ExtractedBlock {
            kind: BlockKind::Paragraph,
            level: 0,
            text: paragraph.trim().to_string(),
        });
        paragraph.clear();
    };
    let lines: Vec<&str> = kept.split_inclusive('\n').collect();
    let mut cursor = 0;
    while cursor < lines.len() {
        let line = lines[cursor];
        let trimmed = line.trim();
        if trimmed.starts_with('|') {
            let mut table = String::new();
            while cursor < lines.len() && lines[cursor].trim().starts_with('|') {
                table.push_str(lines[cursor].trim());
                table.push('\n');
                cursor += 1;
            }
            flush(&mut blocks, &mut paragraph);
            blocks.push(ExtractedBlock {
                kind: BlockKind::Table,
                level: 0,
                text: table.trim().to_string(),
            });
            continue;
        }
        if let Some(level) = markdown_heading(trimmed) {
            flush(&mut blocks, &mut paragraph);
            blocks.push(ExtractedBlock {
                kind: BlockKind::Heading,
                level,
                text: trimmed.trim_start_matches('#').trim().to_string(),
            });
            cursor += 1;
            continue;
        }
        paragraph.push_str(line);
        cursor += 1;
    }
    flush(&mut blocks, &mut paragraph);
    if blocks.is_empty() {
        return Extracted::Unparsed(SkipReason::Empty);
    }
    Extracted::Parsed { blocks, truncated }
}

fn markdown_heading(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&hashes) && line.chars().nth(hashes) == Some(' ') {
        Some(hashes as u8)
    } else {
        None
    }
}

fn recognize_into_blocks(bytes: &[u8], policy: &ExtractPolicy<'_>) -> Extracted {
    let Some(recognize) = policy.recognize else {
        return Extracted::Unparsed(SkipReason::Unparsed);
    };
    if policy.deadline <= Instant::now() {
        return Extracted::Failed("timeout".into());
    }
    match recognize.recognize(bytes) {
        Ok(spans) => spans_to_blocks(&spans),
        Err(_) => Extracted::Unparsed(SkipReason::Unparsed),
    }
}

fn spans_to_blocks(spans: &[RecognizedSpan]) -> Extracted {
    let mut blocks = Vec::new();
    for span in spans {
        let Some(confidence) = span.confidence else {
            continue;
        };
        if confidence < OCR_CONFIDENCE_MIN || span.text.trim().is_empty() {
            continue;
        }
        blocks.push(ExtractedBlock {
            kind: BlockKind::Paragraph,
            level: 0,
            text: span.text.trim().to_string(),
        });
    }
    if blocks.is_empty() {
        Extracted::Unparsed(SkipReason::Unparsed)
    } else {
        Extracted::Parsed {
            blocks,
            truncated: false,
        }
    }
}

fn extract_pdf(bytes: &[u8], policy: &ExtractPolicy<'_>) -> Extracted {
    if bytes.len() > policy.max_package_bytes {
        return Extracted::Unparsed(SkipReason::OverCount);
    }
    if let Some(blocks) = inspector_blocks(bytes, policy) {
        if !blocks.is_empty() {
            return cap_blocks(blocks, policy.max_output);
        }
    }
    let Ok(document) = Document::load_mem(bytes) else {
        return Extracted::Failed("pdf".into());
    };
    let pages: Vec<u32> = document.get_pages().keys().copied().collect();
    if pages.len() > MAX_PDF_PAGES {
        return Extracted::Unparsed(SkipReason::OverCount);
    }
    let mut blocks = Vec::new();
    for (index, number) in pages.iter().enumerate() {
        if policy.deadline <= Instant::now() {
            return Extracted::Failed("timeout".into());
        }
        let text = document.extract_text(&[*number]).unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        blocks.push(ExtractedBlock {
            kind: BlockKind::Page,
            level: (index + 1) as u8,
            text: String::new(),
        });
        blocks.push(ExtractedBlock {
            kind: BlockKind::Paragraph,
            level: 0,
            text: text.trim().to_string(),
        });
    }
    if blocks.is_empty() {
        return recognize_embedded(bytes, policy);
    }
    cap_blocks(blocks, policy.max_output)
}

fn inspector_blocks(bytes: &[u8], policy: &ExtractPolicy<'_>) -> Option<Vec<ExtractedBlock>> {
    let extracted = pdf_inspector::extract_pages_markdown_mem(bytes, None).ok()?;
    if extracted.pages.len() > MAX_PDF_PAGES {
        return None;
    }
    let mut blocks = Vec::new();
    for page in &extracted.pages {
        if policy.deadline <= Instant::now() {
            return None;
        }
        if page.needs_ocr || page.markdown.trim().is_empty() {
            continue;
        }
        blocks.push(ExtractedBlock {
            kind: BlockKind::Page,
            level: (page.page + 1) as u8,
            text: String::new(),
        });
        if let Extracted::Parsed { blocks: next, .. } =
            blocks_from_plain(&page.markdown, policy.max_output)
        {
            blocks.extend(next);
        }
    }
    (!blocks.is_empty()).then_some(blocks)
}

fn recognize_embedded(bytes: &[u8], policy: &ExtractPolicy<'_>) -> Extracted {
    let Some(recognize) = policy.recognize else {
        return Extracted::Unparsed(SkipReason::Unparsed);
    };
    let images = embedded_jpegs(bytes);
    if images.is_empty() {
        return Extracted::Unparsed(SkipReason::Unparsed);
    }
    let mut blocks = Vec::new();
    for image in images {
        if policy.deadline <= Instant::now() {
            return Extracted::Failed("timeout".into());
        }
        let Ok(spans) = recognize.recognize(&image) else {
            continue;
        };
        if let Extracted::Parsed { blocks: next, .. } = spans_to_blocks(&spans) {
            blocks.extend(next);
        }
    }
    if blocks.is_empty() {
        Extracted::Unparsed(SkipReason::Unparsed)
    } else {
        cap_blocks(blocks, policy.max_output)
    }
}

fn embedded_jpegs(bytes: &[u8]) -> Vec<Vec<u8>> {
    let Ok(document) = Document::load_mem(bytes) else {
        return Vec::new();
    };
    let mut images = Vec::new();
    for object in document.objects.values() {
        let lopdf::Object::Stream(stream) = object else {
            continue;
        };
        let jpeg = match stream.dict.get(b"Filter") {
            Ok(lopdf::Object::Name(name)) => name == b"DCTDecode",
            Ok(lopdf::Object::Array(items)) => items
                .iter()
                .any(|item| matches!(item, lopdf::Object::Name(name) if name == b"DCTDecode")),
            _ => false,
        };
        if !jpeg || !stream.content.starts_with(b"\xff\xd8\xff") {
            continue;
        }
        images.push(stream.content.clone());
        if images.len() == 4 {
            break;
        }
    }
    images
}

fn extract_office(bytes: &[u8], policy: &ExtractPolicy<'_>) -> Extracted {
    if bytes.len() > policy.max_package_bytes {
        return Extracted::Unparsed(SkipReason::OverCount);
    }
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = match ZipArchive::new(cursor) {
        Ok(archive) => archive,
        Err(_) => return Extracted::Failed("package".into()),
    };
    if archive.len() > MAX_ZIP_ENTRIES {
        return Extracted::Unparsed(SkipReason::OverCount);
    }
    for index in 0..archive.len() {
        let Ok(file) = archive.by_index(index) else {
            continue;
        };
        let name = file.name().replace('\\', "/");
        if name.split('/').any(|part| part == "..") {
            return Extracted::Unparsed(SkipReason::Unparsed);
        }
        let compressed = file.compressed_size().max(1);
        if file.size() / compressed > MAX_ZIP_RATIO {
            return Extracted::Unparsed(SkipReason::OverCount);
        }
    }
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let Ok(file) = archive.by_index(index) else {
            continue;
        };
        let name = file.name().replace('\\', "/");
        if package_part(&name) {
            names.push((part_rank(&name), name));
        }
    }
    names.sort();
    let mut blocks = Vec::new();
    let mut source_truncated = false;
    for (_, name) in names {
        if policy.deadline <= Instant::now() {
            return Extracted::Failed("timeout".into());
        }
        let Ok(mut file) = archive.by_name(&name) else {
            continue;
        };
        let declared_bytes = file.size();
        let mut captured_bytes = 0;
        let mut xml = String::new();
        let mut chunk = [0u8; 8192];
        loop {
            if xml.len() >= policy.max_output {
                break;
            }
            let Ok(read) = file.read(&mut chunk) else {
                break;
            };
            if read == 0 {
                break;
            }
            let room = policy.max_output.saturating_sub(xml.len());
            let take = read.min(room);
            captured_bytes += take;
            xml.push_str(&String::from_utf8_lossy(&chunk[..take]));
            if take < read {
                break;
            }
        }
        source_truncated |= (captured_bytes as u64) < declared_bytes;
        let text = markup_text(&xml);
        if text.trim().is_empty() {
            continue;
        }
        if let Some(label) = part_label(&name) {
            blocks.push(ExtractedBlock {
                kind: BlockKind::Heading,
                level: 2,
                text: label,
            });
        }
        if text.contains('\t') {
            if let Some((table, row_cut)) = crate::markdown_table(&text, '\t') {
                source_truncated |= row_cut;
                blocks.push(ExtractedBlock {
                    kind: BlockKind::Table,
                    level: 0,
                    text: table,
                });
                continue;
            }
        }
        blocks.push(ExtractedBlock {
            kind: BlockKind::Paragraph,
            level: 0,
            text,
        });
    }
    if blocks.is_empty() {
        Extracted::Unparsed(SkipReason::Empty)
    } else {
        match cap_blocks(blocks, policy.max_output) {
            Extracted::Parsed { blocks, truncated } => Extracted::Parsed {
                blocks,
                truncated: truncated || source_truncated,
            },
            other => other,
        }
    }
}

fn cap_blocks(blocks: Vec<ExtractedBlock>, max_output: usize) -> Extracted {
    let text = project_text(&blocks);
    if text.len() <= max_output {
        return Extracted::Parsed {
            blocks,
            truncated: false,
        };
    }
    let mut end = max_output.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    match blocks_from_plain(&text[..end], max_output) {
        Extracted::Parsed { blocks, .. } => Extracted::Parsed {
            blocks,
            truncated: true,
        },
        other => other,
    }
}

fn package_part(name: &str) -> bool {
    name == "word/document.xml"
        || name == "word/footnotes.xml"
        || name == "word/endnotes.xml"
        || name == "word/comments.xml"
        || name == "xl/sharedStrings.xml"
        || name == "content.xml"
        || (name.starts_with("word/header") && name.ends_with(".xml"))
        || (name.starts_with("word/footer") && name.ends_with(".xml"))
        || (name.starts_with("xl/worksheets/sheet") && name.ends_with(".xml"))
        || (name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
        || (name.starts_with("ppt/notesSlides/notesSlide") && name.ends_with(".xml"))
}

fn part_rank(name: &str) -> u8 {
    if name == "word/document.xml" || name == "content.xml" {
        0
    } else if name.starts_with("word/header") || name.starts_with("word/footer") {
        1
    } else if name.starts_with("word/footnotes")
        || name.starts_with("word/endnotes")
        || name.starts_with("word/comments")
    {
        2
    } else if name.starts_with("ppt/slides/") {
        3
    } else if name.starts_with("ppt/notesSlides/") {
        4
    } else if name == "xl/sharedStrings.xml" {
        5
    } else {
        6
    }
}

fn part_label(name: &str) -> Option<String> {
    if let Some(rest) = name.strip_prefix("ppt/slides/slide") {
        return Some(format!("Slide {}", rest.trim_end_matches(".xml")));
    }
    if let Some(rest) = name.strip_prefix("xl/worksheets/sheet") {
        return Some(format!("Sheet {}", rest.trim_end_matches(".xml")));
    }
    if name.starts_with("ppt/notesSlides/") {
        return Some("Notes".to_string());
    }
    if name.starts_with("word/header") {
        return Some("Header".to_string());
    }
    if name.starts_with("word/footer") {
        return Some("Footer".to_string());
    }
    if name.starts_with("word/footnotes")
        || name.starts_with("word/endnotes")
        || name.starts_with("word/comments")
    {
        return Some("Notes".to_string());
    }
    None
}

fn markup_text(xml: &str) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut inside = false;
    let mut skip = false;
    for ch in xml.chars() {
        if ch == '<' {
            inside = true;
            tag.clear();
            continue;
        }
        if inside && ch == '>' {
            inside = false;
            let closing = tag.trim_start().starts_with('/');
            let local = local_name(&tag);
            if local == "script" || local == "style" {
                skip = !closing;
            }
            if !closing {
                if let Some(level) = heading_level(&local) {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    for _ in 0..level {
                        out.push('#');
                    }
                    out.push(' ');
                }
            }
            if closing && is_block(&local) {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            } else if closing && is_cell(&local) && !out.ends_with('\t') && !out.ends_with('\n') {
                out.push('\t');
            }
            continue;
        }
        if inside {
            if tag.len() < 80 {
                tag.push(ch);
            }
            continue;
        }
        if !skip {
            out.push(ch);
        }
    }
    collapse_lines(&decode_entities(&out))
}

fn local_name(tag: &str) -> String {
    let tag = tag.trim().trim_start_matches('/').trim_start();
    let token = tag.split_whitespace().next().unwrap_or("");
    token
        .rsplit(':')
        .next()
        .unwrap_or(token)
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn heading_level(local: &str) -> Option<usize> {
    match local {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

fn is_block(local: &str) -> bool {
    matches!(
        local,
        "p" | "tr" | "br" | "li" | "div" | "si" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

fn is_cell(local: &str) -> bool {
    matches!(local, "tc" | "td" | "th")
}

fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

fn collapse_lines(text: &str) -> String {
    let mut lines = Vec::new();
    for line in text.split(['\n', '\r']) {
        let line = line.trim();
        if line.is_empty() {
            if lines.last().is_some_and(|last: &String| !last.is_empty()) {
                lines.push(String::new());
            }
            continue;
        }
        lines.push(line.to_string());
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn looks_like_html(text: &str) -> bool {
    let head: String = text
        .trim_start()
        .chars()
        .take(24)
        .collect::<String>()
        .to_ascii_lowercase();
    head.starts_with("<!doctype html") || head.starts_with("<html")
}

fn rtf_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("{\\rtf") {
        return None;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut index = 0;
    let mut out = String::new();
    while index < chars.len() {
        match chars[index] {
            '{' | '}' => index += 1,
            '\\' => {
                index += 1;
                if index >= chars.len() {
                    break;
                }
                if chars[index].is_ascii_alphabetic() {
                    let start = index;
                    while index < chars.len() && chars[index].is_ascii_alphabetic() {
                        index += 1;
                    }
                    let word: String = chars[start..index].iter().collect();
                    if word == "par" || word == "line" {
                        out.push('\n');
                    }
                    if chars.get(index) == Some(&' ') {
                        index += 1;
                    }
                } else {
                    index += 1;
                }
            }
            ch => {
                out.push(ch);
                index += 1;
            }
        }
    }
    let text = collapse_lines(&out);
    (!text.trim().is_empty()).then_some(text)
}

fn is_raster(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[cfg(test)]
#[path = "extract_tests.rs"]
mod tests;
