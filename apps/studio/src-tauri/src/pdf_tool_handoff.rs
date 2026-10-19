//! One format-conversion job waiting for the PDF applet window.
//!
//! The homepage file picker runs in the main webview. The applet window has
//! its own data store, so the bytes cross through this command instead of
//! localStorage.

use std::sync::Mutex;

use base64::Engine;
use serde::{Deserialize, Serialize};

const FILE_LIMIT: usize = 20;
const BYTE_LIMIT: usize = 25 * 1024 * 1024;
const NAME_LIMIT: usize = 180;
const MEDIA_TYPE_LIMIT: usize = 120;
const PDF_WINDOW: &str = "applet-pdf";

struct StoredFile {
    name: String,
    media_type: String,
    bytes: Vec<u8>,
}

struct StoredJob {
    kind: String,
    files: Vec<StoredFile>,
}

static JOB: Mutex<Option<StoredJob>> = Mutex::new(None);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfHandoffFileInput {
    name: String,
    media_type: String,
    bytes_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfHandoffFileOutput {
    name: String,
    media_type: String,
    bytes_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfHandoffOutput {
    kind: String,
    files: Vec<PdfHandoffFileOutput>,
}

#[tauri::command]
pub fn stage_pdf_tool_handoff(kind: String, files: Vec<PdfHandoffFileInput>) -> Result<(), String> {
    let job = decode_job(&kind, &files)?;
    let mut slot = JOB
        .lock()
        .map_err(|_| "The format job could not be stored.".to_owned())?;
    *slot = Some(job);
    Ok(())
}

#[tauri::command]
pub fn take_pdf_tool_handoff(
    window: tauri::WebviewWindow,
) -> Result<Option<PdfHandoffOutput>, String> {
    if window.label() != PDF_WINDOW {
        return Err("This window cannot take the format job.".to_owned());
    }
    take_stored()
}

#[tauri::command]
pub fn drop_pdf_tool_handoff() -> Result<(), String> {
    let mut slot = JOB
        .lock()
        .map_err(|_| "The format job could not be stored.".to_owned())?;
    *slot = None;
    Ok(())
}

fn take_stored() -> Result<Option<PdfHandoffOutput>, String> {
    let mut slot = JOB
        .lock()
        .map_err(|_| "The format job could not be stored.".to_owned())?;
    let Some(job) = slot.take() else {
        return Ok(None);
    };
    Ok(Some(PdfHandoffOutput {
        kind: job.kind,
        files: job
            .files
            .into_iter()
            .map(|file| PdfHandoffFileOutput {
                name: file.name,
                media_type: file.media_type,
                bytes_base64: base64::engine::general_purpose::STANDARD.encode(file.bytes),
            })
            .collect(),
    }))
}

fn decode_job(kind: &str, files: &[PdfHandoffFileInput]) -> Result<StoredJob, String> {
    let kind = kind.trim();
    if !crate::applet_window::is_pdf_tool(kind) {
        return Err("Unknown format tool.".to_owned());
    }
    if files.is_empty() || files.len() > FILE_LIMIT {
        return Err("The format job could not be read.".to_owned());
    }
    let mut stored = Vec::with_capacity(files.len());
    for file in files {
        stored.push(decode_file(file)?);
    }
    Ok(StoredJob {
        kind: kind.to_owned(),
        files: stored,
    })
}

fn decode_file(file: &PdfHandoffFileInput) -> Result<StoredFile, String> {
    let name = file.name.trim();
    if !valid_name(name) {
        return Err("The format job could not be read.".to_owned());
    }
    let media_type = file.media_type.trim();
    if !valid_media_type(media_type) {
        return Err("The format job could not be read.".to_owned());
    }
    let encoded = file.bytes_base64.trim();
    if encoded.len() > BYTE_LIMIT / 3 * 4 + 8 {
        return Err("The format job could not be read.".to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "The format job could not be read.".to_owned())?;
    if bytes.is_empty() || bytes.len() > BYTE_LIMIT {
        return Err("The format job could not be read.".to_owned());
    }
    Ok(StoredFile {
        name: name.to_owned(),
        media_type: media_type.to_owned(),
        bytes,
    })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.chars().count() <= NAME_LIMIT
        && !name.contains(['/', '\\', '\0'])
        && !name.chars().any(char::is_control)
}

fn valid_media_type(value: &str) -> bool {
    value.chars().count() <= MEDIA_TYPE_LIMIT && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use base64::Engine;

    use super::{PdfHandoffFileInput, drop_pdf_tool_handoff, stage_pdf_tool_handoff, take_stored};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn file(name: &str, bytes: &str) -> PdfHandoffFileInput {
        PdfHandoffFileInput {
            name: name.to_owned(),
            media_type: "application/pdf".to_owned(),
            bytes_base64: base64::engine::general_purpose::STANDARD.encode(bytes.as_bytes()),
        }
    }

    #[test]
    fn stores_one_job_and_returns_it_once() {
        let _guard = TEST_LOCK.lock().expect("test lock");
        drop_pdf_tool_handoff().unwrap();
        stage_pdf_tool_handoff("pdf-to-word".to_owned(), vec![file("纪要.pdf", "%PDF")]).unwrap();
        let first = take_stored().unwrap().expect("job");
        assert_eq!(first.kind, "pdf-to-word");
        assert_eq!(first.files.len(), 1);
        assert_eq!(first.files[0].name, "纪要.pdf");
        assert_eq!(
            first.files[0].bytes_base64,
            base64::engine::general_purpose::STANDARD.encode("%PDF")
        );
        assert!(take_stored().unwrap().is_none());
    }

    #[test]
    fn rejects_a_bad_job_without_replacing_the_stored_one() {
        let _guard = TEST_LOCK.lock().expect("test lock");
        drop_pdf_tool_handoff().unwrap();
        stage_pdf_tool_handoff("word-to-pdf".to_owned(), vec![file("notes.docx", "PK")]).unwrap();
        assert!(
            stage_pdf_tool_handoff("translate".to_owned(), vec![file("a.pdf", "%PDF")]).is_err()
        );
        assert!(
            stage_pdf_tool_handoff("pdf-to-word".to_owned(), vec![file("../a.pdf", "%PDF")])
                .is_err()
        );
        assert!(stage_pdf_tool_handoff("pdf-to-word".to_owned(), vec![file("a.pdf", "")]).is_err());
        assert!(stage_pdf_tool_handoff("pdf-to-word".to_owned(), vec![]).is_err());
        let kept = take_stored().unwrap().expect("previous job");
        assert_eq!(kept.kind, "word-to-pdf");
        assert_eq!(kept.files[0].name, "notes.docx");
    }
}
