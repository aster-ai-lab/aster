//! Compile a local workspace into `wiki/` and search that published bundle.
//! Cloud drives stay on the server.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Channel;

struct StudioOcr;

impl a3s_okf::ImageRecognizer for StudioOcr {
    fn recognize(&self, bytes: &[u8]) -> Result<Vec<a3s_okf::RecognizedSpan>, String> {
        let blocks = a3s_use_ocr::recognize_image_bytes(bytes, &std::env::temp_dir())?;
        Ok(blocks
            .into_iter()
            .map(|block| a3s_okf::RecognizedSpan {
                text: block.text,
                confidence: block.confidence,
            })
            .collect())
    }
}

#[derive(Serialize)]
pub struct OkfSearchHit {
    pub name: String,
    pub passage: String,
}

#[derive(Serialize)]
pub struct OkfSearchResponse {
    pub phase: &'static str,
    pub hits: Vec<OkfSearchHit>,
}

#[derive(Serialize)]
pub struct PersonalCompileFile {
    pub path: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct PersonalCompileReport {
    pub compiled: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub failed: usize,
    pub files: Vec<PersonalCompileFile>,
}

fn personal_compile_file(path: &str, outcome: &a3s_okf::CompileOutcome) -> PersonalCompileFile {
    let (status, reason) = match outcome {
        a3s_okf::CompileOutcome::Compiled => ("compiled", None),
        a3s_okf::CompileOutcome::Unchanged => ("unchanged", None),
        a3s_okf::CompileOutcome::Skipped(reason) => ("skipped", Some(reason.as_str().to_string())),
        a3s_okf::CompileOutcome::Failed => ("failed", None),
    };
    PersonalCompileFile {
        path: path.to_string(),
        status,
        reason,
    }
}

fn personal_compile_view(report: &a3s_okf::WorkspaceReport) -> PersonalCompileReport {
    PersonalCompileReport {
        compiled: report.compiled,
        unchanged: report.unchanged,
        skipped: report.skipped,
        failed: report.failed,
        files: report
            .outcomes
            .iter()
            .map(|(path, outcome)| personal_compile_file(path, outcome))
            .collect(),
    }
}

#[tauri::command]
pub async fn compile_personal_okf(
    root: String,
    on_file: Channel<PersonalCompileFile>,
) -> Result<PersonalCompileReport, String> {
    if root.starts_with("a3s-drive://") {
        return Err("cloud encyclopedia is compiled on the server".into());
    }
    let path = PathBuf::from(&root);
    if !a3s_okf::is_personal_vault(&path) {
        return Err("personal knowledge compile only accepts wiki/local".into());
    }
    // The blocking compile reports each note on `on_file` before this command returns.
    let report = tauri::async_runtime::spawn_blocking(move || {
        a3s_okf::publish_note_bundle_with(&path, &mut |note, outcome| {
            let _ = on_file.send(personal_compile_file(note, outcome));
        })
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    Ok(personal_compile_view(&report))
}

#[derive(Serialize)]
pub struct PersonalAcceptReport {
    pub concepts: usize,
    pub skipped: usize,
    pub failed: usize,
}

#[tauri::command]
pub async fn accept_personal_okf(root: String) -> Result<PersonalAcceptReport, String> {
    if root.starts_with("a3s-drive://") {
        return Err("cloud encyclopedia is compiled on the server".into());
    }
    let path = PathBuf::from(&root);
    if !a3s_okf::is_personal_vault(&path) {
        return Err("personal knowledge compile only accepts wiki/local".into());
    }
    let report =
        tauri::async_runtime::spawn_blocking(move || a3s_okf::accept_personal_bundle(&path))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
    Ok(PersonalAcceptReport {
        concepts: report.unchanged,
        skipped: report.skipped,
        failed: report.failed,
    })
}

#[derive(Serialize)]
pub struct PersonalCompilePlan {
    pub pending: Vec<String>,
}

#[tauri::command]
pub async fn plan_personal_okf(root: String) -> Result<PersonalCompilePlan, String> {
    if root.starts_with("a3s-drive://") {
        return Err("cloud encyclopedia is compiled on the server".into());
    }
    let path = PathBuf::from(&root);
    if !a3s_okf::is_personal_vault(&path) {
        return Err("personal knowledge compile only accepts wiki/local".into());
    }
    let pending =
        tauri::async_runtime::spawn_blocking(move || a3s_okf::plan_personal_compile(&path))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
    Ok(PersonalCompilePlan { pending })
}

#[tauri::command]
pub async fn remember_personal_okf(root: String, pending: Vec<String>) -> Result<(), String> {
    if root.starts_with("a3s-drive://") {
        return Err("cloud encyclopedia is compiled on the server".into());
    }
    let path = PathBuf::from(&root);
    if !a3s_okf::is_personal_vault(&path) {
        return Err("personal knowledge compile only accepts wiki/local".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        a3s_okf::remember_personal_generations(&path, &pending)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn personal_vault(root: &str) -> Result<std::path::PathBuf, String> {
    if root.starts_with("a3s-drive://") {
        return Err("cloud encyclopedia is compiled on the server".into());
    }
    let path = PathBuf::from(root);
    if !a3s_okf::is_personal_vault(&path) {
        return Err("personal knowledge compile only accepts wiki/local".into());
    }
    Ok(path)
}

#[tauri::command]
pub async fn pin_personal_okf(root: String, pending: Vec<String>) -> Result<(), String> {
    let path = personal_vault(&root)?;
    tauri::async_runtime::spawn_blocking(move || a3s_okf::pin_personal_compile(&path, &pending))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn release_personal_okf(root: String) -> Result<(), String> {
    let path = personal_vault(&root)?;
    tauri::async_runtime::spawn_blocking(move || a3s_okf::release_personal_compile(&path))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn compile_workspace_okf(root: String) -> Result<String, String> {
    if root.starts_with("a3s-drive://") {
        return Err("cloud encyclopedia is compiled on the server".into());
    }
    let report = a3s_okf::compile_workspace_with(Path::new(&root), Some(&StudioOcr))
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "compiled={} unchanged={} skipped={} failed={}",
        report.compiled, report.unchanged, report.skipped, report.failed
    ))
}

#[tauri::command]
pub async fn search_workspace_okf(
    root: String,
    query: String,
) -> Result<OkfSearchResponse, String> {
    if root.starts_with("a3s-drive://") {
        return Err("cloud encyclopedia is searched on the server".into());
    }
    let value =
        crate::agent_plane::search_admitted_wiki(&Path::new(&root).join("wiki"), &query).await?;
    let phase = value
        .get("phase")
        .and_then(|item| item.as_str())
        .unwrap_or("not-ready");
    let hits = if phase == "published" {
        value
            .get("hits")
            .and_then(|item| item.as_array())
            .map(|hits| {
                hits.iter()
                    .filter_map(|hit| {
                        Some(OkfSearchHit {
                            name: hit.get("name")?.as_str()?.to_owned(),
                            passage: hit.get("passage")?.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(OkfSearchResponse {
        phase: if phase == "published" {
            "published"
        } else {
            "not-ready"
        },
        hits,
    })
}

#[cfg(test)]
mod tests {
    use a3s_okf::{CompileOutcome, SkipReason, WorkspaceReport};

    use super::personal_compile_view;

    #[test]
    fn personal_report_keeps_each_file_result() {
        let report = WorkspaceReport {
            compiled: 1,
            unchanged: 1,
            skipped: 1,
            failed: 1,
            outcomes: vec![
                ("甲.md".into(), CompileOutcome::Compiled),
                ("乙.md".into(), CompileOutcome::Unchanged),
                ("空.md".into(), CompileOutcome::Skipped(SkipReason::Empty)),
                ("坏.md".into(), CompileOutcome::Failed),
            ],
        };
        let view = personal_compile_view(&report);
        assert_eq!(view.compiled, 1);
        assert_eq!(view.unchanged, 1);
        assert_eq!(view.files.len(), 4);
        assert_eq!(view.files[0].status, "compiled");
        assert!(view.files[0].reason.is_none());
        assert_eq!(view.files[1].status, "unchanged");
        assert_eq!(view.files[2].path, "空.md");
        assert_eq!(view.files[2].status, "skipped");
        assert_eq!(view.files[2].reason.as_deref(), Some("empty"));
        assert_eq!(view.files[3].status, "failed");
    }
}
