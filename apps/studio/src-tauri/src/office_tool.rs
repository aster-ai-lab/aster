//! Run the local `a3s-office` binary for a format conversion.
//!
//! The page cannot spawn a process. The command accepts only the three
//! actions the format window needs, and only this binary.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use serde::Serialize;
use tokio::time::Instant;
use uuid::Uuid;

#[path = "office_process.rs"]
mod process;

const BYTE_LIMIT: usize = 25 * 1024 * 1024;
const TEXT_LIMIT: usize = 1_000_000;
const ROW_LIMIT: usize = 500;
const JOB_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OfficeAction {
    Capture,
    Docx,
    Xlsx,
}

impl OfficeAction {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "capture" => Ok(Self::Capture),
            "docx" => Ok(Self::Docx),
            "xlsx" => Ok(Self::Xlsx),
            _ => Err("OfficeAction: unsupported value".to_owned()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeToolResponse {
    svg: Option<String>,
    text_json: Option<String>,
    bytes_base64: Option<String>,
}

#[tauri::command]
pub async fn office_tool(
    action: String,
    name: Option<String>,
    bytes_base64: Option<String>,
    text: Option<String>,
    rows: Option<Vec<Vec<String>>>,
) -> Result<OfficeToolResponse, String> {
    let action = OfficeAction::parse(&action)?;
    let deadline = Instant::now() + JOB_TIMEOUT;
    let bin = office_bin()?;
    match action {
        OfficeAction::Capture => {
            capture(
                &bin,
                name.as_deref().ok_or_else(|| missing(action))?,
                bytes_base64.as_deref().ok_or_else(|| missing(action))?,
                deadline,
            )
            .await
        }
        OfficeAction::Docx => {
            write_docx(
                &bin,
                text.as_deref().ok_or_else(|| missing(action))?,
                deadline,
            )
            .await
        }
        OfficeAction::Xlsx => {
            write_xlsx(
                &bin,
                rows.as_deref().ok_or_else(|| missing(action))?,
                deadline,
            )
            .await
        }
    }
}

fn missing(action: OfficeAction) -> String {
    format!("OfficeAction {}: missing input", action.as_str())
}

async fn capture(
    bin: &Path,
    name: &str,
    bytes_base64: &str,
    deadline: Instant,
) -> Result<OfficeToolResponse, String> {
    let bytes = decode_bytes(bytes_base64)?;
    if bytes.is_empty() {
        return Err("unreadable".to_owned());
    }
    let work = TempWork::create()?;
    let file = work.path.join(safe_name(name));
    std::fs::write(&file, bytes).map_err(|_| "office".to_owned())?;
    let svg_path = work.path.join("view.svg");
    let file_arg = path_arg(&file);
    let svg_arg = path_arg(&svg_path);
    run(
        bin,
        &["view", &file_arg, "svg", "--output", &svg_arg, "--json"],
        deadline,
    )
    .await?;
    let text_json = run(bin, &["view", &file_arg, "text", "--json"], deadline).await?;
    let svg = String::from_utf8(read_output(&svg_path)?).map_err(|_| "unreadable".to_owned())?;
    if svg.is_empty() {
        return Err("office".to_owned());
    }
    Ok(OfficeToolResponse {
        svg: Some(svg),
        text_json: Some(text_json),
        bytes_base64: None,
    })
}

async fn write_docx(
    bin: &Path,
    text: &str,
    deadline: Instant,
) -> Result<OfficeToolResponse, String> {
    if text.len() > TEXT_LIMIT || text.contains('\0') {
        return Err("unreadable".to_owned());
    }
    let work = TempWork::create()?;
    let output = work.path.join("out.docx");
    let path = path_arg(&output);
    run(bin, &["create", &path, "--json"], deadline).await?;
    run(
        bin,
        &[
            "set",
            &path,
            "/body/p[1]",
            "--text",
            text,
            "--output",
            &path,
            "--json",
        ],
        deadline,
    )
    .await?;
    file_response(&output)
}

async fn write_xlsx(
    bin: &Path,
    rows: &[Vec<String>],
    deadline: Instant,
) -> Result<OfficeToolResponse, String> {
    if rows.len() > ROW_LIMIT {
        return Err("unreadable".to_owned());
    }
    let mut total = 0usize;
    for row in rows {
        if row.len() > 26 {
            return Err("unreadable".to_owned());
        }
        for value in row {
            total = total
                .checked_add(value.len())
                .ok_or_else(|| "unreadable".to_owned())?;
            if total > TEXT_LIMIT || value.contains('\0') {
                return Err("unreadable".to_owned());
            }
        }
    }
    let work = TempWork::create()?;
    let output = work.path.join("out.xlsx");
    let path = path_arg(&output);
    run(bin, &["create", &path, "--json"], deadline).await?;
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() > 26 {
            return Err("unreadable".to_owned());
        }
        for (column_index, value) in row.iter().enumerate() {
            if value.len() > TEXT_LIMIT || value.contains('\0') {
                return Err("unreadable".to_owned());
            }
            let cell = format!("/Sheet1/{}{}", column_name(column_index)?, row_index + 1);
            run(
                bin,
                &[
                    "set", &path, &cell, "--text", value, "--output", &path, "--json",
                ],
                deadline,
            )
            .await?;
        }
    }
    file_response(&output)
}

fn file_response(path: &Path) -> Result<OfficeToolResponse, String> {
    let bytes = read_output(path)?;
    if bytes.len() < 4 || &bytes[..2] != b"PK" {
        return Err("office".to_owned());
    }
    Ok(OfficeToolResponse {
        svg: None,
        text_json: None,
        bytes_base64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
    })
}

async fn run(bin: &Path, args: &[&str], deadline: Instant) -> Result<String, String> {
    process::run(bin, args, deadline, BYTE_LIMIT).await
}

fn read_output(path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| "office".to_owned())?;
    let mut bytes = Vec::new();
    file.take((BYTE_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "office".to_owned())?;
    if bytes.len() > BYTE_LIMIT {
        return Err("office-output-limit".to_owned());
    }
    Ok(bytes)
}

fn office_bin() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    // Windows ships the sidecar as a3s-office.exe beside the host.
    let names: &[&str] = if cfg!(windows) {
        &["a3s-office.exe", "a3s-office"]
    } else {
        &["a3s-office"]
    };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in names {
                candidates.push(dir.join(name));
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for base in [dir.join("agent"), dir.join("../Resources/agent")] {
                for name in names {
                    candidates.push(base.join(name));
                }
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for profile in ["release", "debug"] {
        for name in names {
            candidates.push(root.join("target").join(profile).join(name));
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| "office".to_owned())
}

fn decode_bytes(value: &str) -> Result<Vec<u8>, String> {
    let value = value.trim();
    if value.len() > BYTE_LIMIT / 3 * 4 + 8 {
        return Err("unreadable".to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| "unreadable".to_owned())?;
    if bytes.len() > BYTE_LIMIT {
        return Err("unreadable".to_owned());
    }
    Ok(bytes)
}

fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("file.bin");
    let cleaned: String = base
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || character == '.'
                || character == '-'
                || character == '_'
            {
                character
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "file.bin".to_owned()
    } else {
        cleaned
    }
}

fn column_name(index: usize) -> Result<String, String> {
    let Some(code) = u8::try_from(index)
        .ok()
        .and_then(|value| value.checked_add(b'A'))
    else {
        return Err("unreadable".to_owned());
    };
    if code > b'Z' {
        return Err("unreadable".to_owned());
    }
    Ok((code as char).to_string())
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

struct TempWork {
    path: PathBuf,
}

impl TempWork {
    fn create() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("a3s-office-tool-{}", Uuid::now_v7()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path).map_err(|_| "office".to_owned())?;
        Ok(Self { path })
    }
}

impl Drop for TempWork {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
#[path = "office_tool_tests.rs"]
mod tests;
