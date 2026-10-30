use super::*;

#[test]
fn csv_row_limits_reach_extraction_and_concept_publication_metadata() {
    let source = "Name,Value\n".to_owned()
        + &(1..=201)
            .map(|row| format!("row-{row},7350\n"))
            .collect::<String>();
    let policy = ExtractPolicy {
        recognize: None,
        max_text_bytes: crate::MAX_TEXT_BYTES,
        max_package_bytes: crate::MAX_PACKAGE_BYTES,
        max_output: crate::MAX_TEXT_BYTES,
        deadline: Instant::now() + std::time::Duration::from_secs(1),
    };
    let Extracted::Parsed { blocks, truncated } =
        extract_document("source.csv", source.as_bytes(), &policy)
    else {
        panic!("expected converted table")
    };
    assert!(truncated);
    assert!(blocks[0].text.contains("row-199"));
    assert!(!blocks[0].text.contains("row-200"));
    let passages = [crate::TextPassage {
        heading: "Original",
        body: &source,
    }];
    let page = crate::compile_concept(&crate::ConceptRequest {
        title: "Original CSV",
        resource: "source.csv",
        file_name: "source.csv",
        content_type: "text/csv",
        source_generation: "fixture",
        passages: &passages,
    })
    .unwrap();
    assert!(page.truncated);
}
