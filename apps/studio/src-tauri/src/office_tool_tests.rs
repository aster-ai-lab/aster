#![cfg(test)]

use super::*;
use std::io::Read;

#[tokio::test]
async fn closed_actions_and_required_inputs_refuse_without_conversion() {
    assert!(
        office_tool("translate".to_owned(), None, None, None, None)
            .await
            .err()
            .unwrap()
            .contains("OfficeAction")
    );
    for value in [
        OfficeAction::Capture,
        OfficeAction::Docx,
        OfficeAction::Xlsx,
    ] {
        assert_eq!(OfficeAction::parse(value.as_str()).unwrap(), value);
        assert!(
            office_tool(value.as_str().to_owned(), None, None, None, None)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn real_docx_reopens_and_captures_through_the_native_command() {
    let result = office_tool(
        "docx".to_owned(),
        None,
        None,
        Some("Hello office".to_owned()),
        None,
    )
    .await
    .unwrap();
    let encoded = result.bytes_base64.unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert!(xml.contains("Hello office"));
    let capture = office_tool(
        "capture".to_owned(),
        Some("notes.docx".to_owned()),
        Some(encoded),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(capture.svg.unwrap().contains("<svg"));
    assert!(capture.text_json.unwrap().contains("Hello office"));
}

#[tokio::test]
async fn real_xlsx_reopens_and_captures_through_the_native_command() {
    let result = office_tool(
        "xlsx".to_owned(),
        None,
        None,
        None,
        Some(vec![vec!["Budget".into(), "42".into()]]),
    )
    .await
    .unwrap();
    let encoded = result.bytes_base64.unwrap();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(zip.by_name("xl/workbook.xml").is_ok());
    let capture = office_tool(
        "capture".to_owned(),
        Some("budget.xlsx".to_owned()),
        Some(encoded),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(capture.svg.unwrap().contains("spreadsheet"));
    let text = capture.text_json.unwrap();
    assert!(text.contains("Budget") && text.contains("42"));
}

#[tokio::test]
async fn rejects_oversized_aggregate_rows_before_creating_an_output() {
    let rows = vec![vec![
        "a".repeat(TEXT_LIMIT / 2 + 1),
        "b".repeat(TEXT_LIMIT / 2),
    ]];
    assert_eq!(
        write_xlsx(
            Path::new("/missing-engine"),
            &rows,
            Instant::now() + JOB_TIMEOUT
        )
        .await
        .err()
        .unwrap(),
        "unreadable"
    );
    assert_eq!(
        write_xlsx(
            Path::new("/missing-engine"),
            &[vec!["x".into(); 27]],
            Instant::now() + JOB_TIMEOUT
        )
        .await
        .err()
        .unwrap(),
        "unreadable"
    );
}

#[test]
fn temporary_inputs_are_private_and_output_reads_have_an_exact_bound() {
    let work = TempWork::create().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&work.path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    let path = work.path.join("large");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len((BYTE_LIMIT + 1) as u64).unwrap();
    assert_eq!(read_output(&path).unwrap_err(), "office-output-limit");
    for name in [".", "..", "../../..", ""] {
        assert_eq!(safe_name(name), "file.bin");
    }
}
