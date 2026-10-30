use std::io::{Cursor, Write};
use std::time::{Duration, Instant};

use lopdf::{content::Content, content::Operation, dictionary, Document, Object, Stream};
use zip::write::SimpleFileOptions;

use super::*;
use crate::ExtractedBlock;

fn policy() -> ExtractPolicy<'static> {
    ExtractPolicy {
        recognize: None,
        max_text_bytes: MAX_TEXT_BYTES,
        max_package_bytes: MAX_PACKAGE_BYTES,
        max_output: MAX_TEXT_BYTES,
        deadline: Instant::now() + Duration::from_secs(10),
    }
}

fn office(part: &str, text: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(part, SimpleFileOptions::default()).unwrap();
    zip.write_all(text.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

fn pdf(pages: &[Option<&str>]) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"
    });
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font_id } });
    let mut children = Vec::new();
    for text in pages {
        let operations = text.map_or_else(Vec::new, |text| {
            vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                Operation::new("Td", vec![50.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ]
        });
        let content = Content { operations }.encode().unwrap();
        let stream = doc.add_object(Stream::new(dictionary! {}, content));
        let id = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => resources, "Contents" => stream
        });
        children.push(Object::Reference(id));
    }
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => children, "Count" => pages.len() as i64 }.into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn parsed(value: WorkspaceDocumentRead) -> WorkspaceDocumentProjection {
    match value {
        WorkspaceDocumentRead::Parsed(source) => source,
        other => panic!("expected parsed source, got {other:?}"),
    }
}

#[test]
fn host_document_blocks_bind_parser_source_and_exact_projection_without_invented_pages() {
    let extraction = Extracted::Parsed {
        blocks: vec![ExtractedBlock {
            kind: BlockKind::Paragraph,
            level: 0,
            text: "/sheets/Budget/cells/B2\n42".into(),
        }],
        truncated: false,
    };
    let source = parsed(project_workspace_document(
        b"frozen",
        "native-office:1",
        extraction.clone(),
        &policy(),
    ));
    assert_eq!(source.passages()[0].anchor.page, None);
    assert!(source.passages()[0].text.ends_with("42"));
    let changed = parsed(project_workspace_document(
        b"frozen",
        "native-office:2",
        extraction.clone(),
        &policy(),
    ));
    assert_ne!(
        source.projection_generation(),
        changed.projection_generation()
    );
    assert!(matches!(
        project_workspace_document(b"frozen", "bad\nparser", extraction, &policy()),
        WorkspaceDocumentRead::Failed(_)
    ));
    let too_large = Extracted::Parsed {
        blocks: vec![ExtractedBlock {
            kind: BlockKind::Paragraph,
            level: 0,
            text: "x".repeat(MAX_TEXT_BYTES + 1),
        }],
        truncated: false,
    };
    assert!(matches!(
        project_workspace_document(b"frozen", "host:1", too_large, &policy()),
        WorkspaceDocumentRead::Failed(_)
    ));
}

#[test]
fn workspace_wiki_document_retains_actual_pdf_page_after_an_empty_page() {
    let bytes = pdf(&[None, Some("Evidence on the second page")]);
    let before = bytes.clone();
    let source = parsed(extract_workspace_document("source.pdf", &bytes, &policy()));
    assert!(source.passages.iter().any(|passage| {
        passage.text.contains("Evidence on the second page") && passage.anchor.page == Some(2)
    }));
    assert!(source
        .passages
        .iter()
        .all(|passage| passage.anchor.page == Some(2)));
    assert!(!source.truncated);
    assert_eq!(bytes, before);
    assert!(!format!("{source:?}").contains("Evidence on the second page"));
    assert_eq!(
        source,
        parsed(extract_workspace_document("source.pdf", &bytes, &policy()))
    );
}

#[test]
fn workspace_wiki_document_converted_blocks_never_claim_office_pages_or_cells() {
    let bytes = office(
        "word/document.xml",
        "<w:document><w:p><w:t>Source evidence</w:t></w:p></w:document>",
    );
    let source = parsed(extract_workspace_document("source.docx", &bytes, &policy()));
    assert!(source
        .passages
        .iter()
        .any(|passage| passage.text.contains("Source evidence")));
    assert!(source
        .passages
        .iter()
        .all(|passage| passage.anchor.page.is_none() && passage.anchor.block > 0));
    let next = office(
        "word/document.xml",
        "<w:document><w:p><w:t>Changed evidence</w:t></w:p></w:document>",
    );
    let changed = parsed(extract_workspace_document("source.docx", &next, &policy()));
    assert_ne!(source.source_generation, changed.source_generation);
    assert_ne!(source.projection_generation, changed.projection_generation);
}

#[test]
fn workspace_wiki_document_reports_truncation_and_binds_it_to_the_projection() {
    let bytes = office(
        "word/document.xml",
        "<w:document><w:p><w:t>Evidence requires a complete sentence</w:t></w:p></w:document>",
    );
    let full = parsed(extract_workspace_document("source.docx", &bytes, &policy()));
    let mut limited = policy();
    limited.max_output = 32;
    let short = parsed(extract_workspace_document("source.docx", &bytes, &limited));
    assert!(short.truncated);
    assert!(!full.truncated);
    assert!(
        short
            .passages
            .iter()
            .map(|passage| passage.text.len())
            .sum::<usize>()
            <= 32
    );
    assert_eq!(short.source_generation, full.source_generation);
    assert_ne!(short.projection_generation, full.projection_generation);
}

#[test]
fn workspace_wiki_document_refuses_invalid_limits_mismatched_formats_and_late_results() {
    let bytes = office("word/document.xml", "<w:p>Evidence</w:p>");
    let mut bounded = policy();
    bounded.max_package_bytes = bytes.len() - 1;
    assert_eq!(
        extract_workspace_document("source.docx", &bytes, &bounded),
        WorkspaceDocumentRead::Unparsed(SkipReason::OverSize)
    );
    bounded.max_output = MAX_TEXT_BYTES + 1;
    assert_eq!(
        extract_workspace_document("source.docx", &bytes, &bounded),
        WorkspaceDocumentRead::Failed("workspace_wiki.document.invalid_limits".into())
    );
    assert_eq!(
        extract_workspace_document("source.pdf", &bytes, &policy()),
        WorkspaceDocumentRead::Failed("workspace_wiki.document.format_mismatch".into())
    );
    assert_eq!(
        extract_workspace_document("source.exe", &bytes, &policy()),
        WorkspaceDocumentRead::Unparsed(SkipReason::Unsupported)
    );
    bounded = policy();
    bounded.deadline = Instant::now();
    assert_eq!(
        extract_workspace_document("source.docx", &bytes, &bounded),
        WorkspaceDocumentRead::Failed("timeout".into())
    );
}

#[test]
fn workspace_wiki_document_citations_reject_stale_generations_and_invented_anchors() {
    let bytes = office("word/document.xml", "<w:p>Original source evidence</w:p>");
    let source = parsed(extract_workspace_document("source.docx", &bytes, &policy()));
    let passage = &source.passages()[0];
    assert_eq!(
        source.resolve_passage(
            source.source_generation(),
            source.projection_generation(),
            &passage.anchor
        ),
        Some(passage.text.as_str())
    );
    assert_eq!(
        source.resolve_passage(
            "old-source",
            source.projection_generation(),
            &passage.anchor
        ),
        None
    );
    assert_eq!(
        source.resolve_passage(
            source.source_generation(),
            "old-conversion",
            &passage.anchor
        ),
        None
    );
    for anchor in [
        WorkspaceDocumentAnchor {
            block: 0,
            page: None,
        },
        WorkspaceDocumentAnchor {
            block: usize::MAX,
            page: None,
        },
        WorkspaceDocumentAnchor {
            block: passage.anchor.block,
            page: Some(1),
        },
    ] {
        assert_eq!(
            source.resolve_passage(
                source.source_generation(),
                source.projection_generation(),
                &anchor
            ),
            None
        );
    }
    assert!(
        serde_json::from_str::<WorkspaceDocumentAnchor>(r#"{"block":1,"page":null,"line":5}"#)
            .is_err()
    );
}

#[test]
fn workspace_wiki_document_never_invokes_an_injected_ocr_recognizer() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Probe(AtomicUsize);
    impl crate::ImageRecognizer for Probe {
        fn recognize(&self, _: &[u8]) -> Result<Vec<crate::RecognizedSpan>, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(vec![crate::RecognizedSpan {
                text: "Recognized private source".into(),
                confidence: Some(0.9),
            }])
        }
    }
    let mut document = Document::load_mem(&pdf(&[None])).unwrap();
    document.add_object(Stream::new(
        dictionary! { "Filter" => "DCTDecode" },
        vec![0xff, 0xd8, 0xff, 0xd9],
    ));
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    let probe = Probe(AtomicUsize::new(0));
    let mut bounded = policy();
    bounded.recognize = Some(&probe);
    assert!(matches!(
        extract_document("scan.pdf", &bytes, &bounded),
        Extracted::Parsed { .. }
    ));
    assert_eq!(probe.0.swap(0, Ordering::SeqCst), 1);
    assert_eq!(
        extract_workspace_document("scan.pdf", &bytes, &bounded),
        WorkspaceDocumentRead::Unparsed(SkipReason::Unparsed)
    );
    assert_eq!(probe.0.load(Ordering::SeqCst), 0);
}
