//! Published desktop installer from a3s-api.
//!
//! Check reads `GET /v1/api/admin/app-releases/current`. The file is streamed
//! to disk on the same volume as the running app, hashed, then swapped in.
//! A missing digest, a hash mismatch, or an archive that is not one `.app`
//! refuses the install.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_UNPACKED: u64 = 2 * 1024 * 1024 * 1024;
const MAX_JSON: usize = 1024 * 1024;

#[derive(Clone)]
struct ReadyUpdate {
    version: String,
    notes: String,
    sha256: String,
    path: PathBuf,
    size: u64,
    published_at: String,
}

#[derive(Clone)]
struct ReleaseMeta {
    version: String,
    notes: String,
    sha256: String,
    size: u64,
    published_at: String,
}

enum Phase {
    Idle,
    Busy,
    Ready(ReadyUpdate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpdateFail {
    InProgress,
    NotPublished,
    UpToDate,
    Unavailable,
    Failed,
    InvalidPackage,
    ProductMismatch,
    IntegrityFailed,
    InstallFailed,
    RestartFailed,
}

impl UpdateFail {
    // Output-only today; retain the closed protocol parser for any input boundary.
    #[allow(dead_code)]
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "update_in_progress" => Ok(Self::InProgress),
            "not_published" => Ok(Self::NotPublished),
            "up_to_date" => Ok(Self::UpToDate),
            crate::host_family::CAPABILITY_UNAVAILABLE => Ok(Self::Unavailable),
            "update_failed" => Ok(Self::Failed),
            "update_invalid_package" => Ok(Self::InvalidPackage),
            "update_product_mismatch" => Ok(Self::ProductMismatch),
            "update_integrity_failed" => Ok(Self::IntegrityFailed),
            "update_install_failed" => Ok(Self::InstallFailed),
            "update_restart_failed" => Ok(Self::RestartFailed),
            _ => Err(format!("UpdateFail: unsupported value {value:?}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "update_in_progress",
            Self::NotPublished => "not_published",
            Self::UpToDate => "up_to_date",
            Self::Unavailable => crate::host_family::CAPABILITY_UNAVAILABLE,
            Self::Failed => "update_failed",
            Self::InvalidPackage => "update_invalid_package",
            Self::ProductMismatch => "update_product_mismatch",
            Self::IntegrityFailed => "update_integrity_failed",
            Self::InstallFailed => "update_install_failed",
            Self::RestartFailed => "update_restart_failed",
        }
    }
}

static PHASE: Mutex<Phase> = Mutex::new(Phase::Idle);

#[path = "app_updater/delivery_manifest.rs"]
mod delivery_manifest;

#[cfg(test)]
#[path = "app_updater/delivery_tests.rs"]
mod delivery_tests;

struct BusyGuard {
    armed: bool,
    previous: Option<ReadyUpdate>,
}

impl BusyGuard {
    fn enter() -> Result<Self, UpdateFail> {
        let mut phase = PHASE.lock().map_err(|_| UpdateFail::Failed)?;
        if matches!(*phase, Phase::Busy) {
            return Err(UpdateFail::InProgress);
        }
        let previous = match std::mem::replace(&mut *phase, Phase::Busy) {
            Phase::Ready(ready) => Some(ready),
            _ => None,
        };
        Ok(Self {
            armed: true,
            previous,
        })
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Ok(mut phase) = PHASE.lock() {
            if matches!(*phase, Phase::Busy) {
                *phase = self.previous.take().map_or(Phase::Idle, Phase::Ready);
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NoteRow {
    #[serde(rename = "releaseNotes")]
    release_notes: String,
    version: String,
}

#[tauri::command]
pub async fn app_updater_check(app: AppHandle) -> Result<(), String> {
    let app_for_task = app.clone();
    let outcome = tokio::task::spawn_blocking(move || check_and_download(&app_for_task))
        .await
        .map_err(|_| UpdateFail::Failed);
    let outcome = match outcome {
        Ok(inner) => inner,
        Err(fail) => Err(fail),
    };
    report(&app, outcome);
    Ok(())
}

#[tauri::command]
pub async fn app_updater_install(app: AppHandle) -> Result<(), String> {
    let app_for_task = app.clone();
    let outcome = tokio::task::spawn_blocking(move || install_staged(&app_for_task))
        .await
        .map_err(|_| UpdateFail::Failed);
    match outcome {
        Ok(Ok(())) => std::process::exit(0),
        Ok(Err(fail)) => {
            report(&app, Err(fail));
            Err(fail.as_str().to_string())
        }
        Err(fail) => {
            report(&app, Err(fail));
            Err(fail.as_str().to_string())
        }
    }
}

#[tauri::command]
pub async fn app_updater_notes() -> Result<Option<Vec<NoteRow>>, String> {
    if require_update_family(crate::host_family::updates_supported(
        crate::host_family::current_family(),
    ))
    .is_err()
    {
        return Ok(None);
    }
    let platform = desktop_platform().ok_or_else(|| "update_failed".to_string())?;
    let origin = crate::org_login::default_origin();
    tokio::task::spawn_blocking(move || match fetch_current(&origin, platform) {
        Ok(release) if !release.notes.trim().is_empty() => Ok(Some(vec![NoteRow {
            release_notes: release.notes,
            version: release.version,
        }])),
        Ok(_) | Err(UpdateFail::NotPublished) | Err(UpdateFail::UpToDate) => Ok(None),
        Err(UpdateFail::Unavailable) => Err(crate::host_family::CAPABILITY_UNAVAILABLE.to_string()),
        Err(_) => Err("update_failed".to_string()),
    })
    .await
    .map_err(|_| "update_failed".to_string())?
}

fn report(app: &AppHandle, outcome: Result<(), UpdateFail>) {
    match outcome {
        Ok(()) | Err(UpdateFail::InProgress) => {}
        Err(UpdateFail::UpToDate) => emit(app, "app.updater.not_available", Value::Null),
        Err(fail) => emit_error(app, fail.as_str()),
    }
}

fn check_and_download(app: &AppHandle) -> Result<(), UpdateFail> {
    require_update_family(crate::host_family::updates_supported(
        crate::host_family::current_family(),
    ))?;
    let mut guard = BusyGuard::enter()?;
    let platform = desktop_platform().ok_or(UpdateFail::Failed)?;
    let origin = crate::org_login::default_origin();
    let release = fetch_current(&origin, platform)?;
    let local = env!("CARGO_PKG_VERSION");
    if !remote_is_newer(&release.version, local)? {
        if let Some(previous) = guard.previous.take() {
            let _ = fs::remove_file(previous.path);
        }
        return Err(UpdateFail::UpToDate);
    }
    if let Some(ready) = guard
        .previous
        .as_ref()
        .filter(|ready| matching_ready(ready, &release))
    {
        emit_available(app, &ready);
        store_ready(ready.clone())?;
        emit_downloaded(app, &ready);
        guard.disarm();
        return Ok(());
    }
    let bundle = running_bundle().ok_or(UpdateFail::Failed)?;
    let info = ready_from_meta(&release, PathBuf::new());
    emit_available(app, &info);
    let ready = download_release(app, &origin, platform, &release, &bundle)?;
    store_ready(ready.clone())?;
    emit_downloaded(app, &ready);
    guard.disarm();
    Ok(())
}

fn install_staged(app: &AppHandle) -> Result<(), UpdateFail> {
    if !crate::host_family::updates_supported(crate::host_family::current_family()) {
        return Err(UpdateFail::Unavailable);
    }
    let ready = take_ready()?;
    let mut guard = BusyGuard {
        armed: true,
        previous: Some(ready.clone()),
    };
    let bundle = running_bundle().ok_or(UpdateFail::Failed)?;
    let mut last = 0u8;
    let hashed = hash_file(&ready.path, &ready.sha256, ready.size, &mut |percent| {
        let percent = (percent / 2).min(40);
        if percent != last {
            last = percent;
            emit_percent(app, ready.size, ready.size, percent);
        }
    });
    if hashed.is_err() {
        let _ = fs::remove_file(&ready.path);
        guard.previous = None;
        return Err(UpdateFail::IntegrityFailed);
    }
    let result = classify(&ready.path).and_then(|()| install_zip(app, &ready, &bundle));
    match result {
        Ok(()) => guard.disarm(),
        Err(
            UpdateFail::InvalidPackage | UpdateFail::ProductMismatch | UpdateFail::IntegrityFailed,
        ) => {
            guard.previous = None;
            let _ = fs::remove_file(&ready.path);
        }
        _ => {}
    }
    result
}

fn install_zip(app: &AppHandle, ready: &ReadyUpdate, bundle: &Path) -> Result<(), UpdateFail> {
    let stage = stage_dir(bundle)?;
    let unpacked = stage.join("unpacked");
    let new_app =
        extract_app_zip(&ready.path, &unpacked).map_err(|_| UpdateFail::InvalidPackage)?;
    emit_percent(app, ready.size, ready.size, 80);
    let tenant = crate::tenant::load()
        .map_err(|_| UpdateFail::ProductMismatch)?
        .slug;
    delivery_manifest::validate(&new_app, &ready.version, &tenant)?;
    ensure_bundle_executable(&new_app)?;
    verify_signature(&new_app, &ready.version)?;
    clear_quarantine(&new_app);
    let backup = replace_bundle(&new_app, bundle).map_err(|_| UpdateFail::InstallFailed)?;
    emit_percent(app, ready.size, ready.size, 95);
    if schedule_relaunch(bundle, Some(&backup)).is_err() {
        restore_bundle(bundle, &backup)?;
        return Err(UpdateFail::RestartFailed);
    }
    Ok(())
}

fn require_update_family(supported: bool) -> Result<(), UpdateFail> {
    if supported {
        Ok(())
    } else {
        Err(UpdateFail::NotPublished)
    }
}

fn connect_fail(error: &std::io::Error) -> UpdateFail {
    match error.kind() {
        std::io::ErrorKind::ConnectionRefused
        | std::io::ErrorKind::TimedOut
        | std::io::ErrorKind::AddrNotAvailable
        | std::io::ErrorKind::NotFound => UpdateFail::NotPublished,
        _ => UpdateFail::Failed,
    }
}

fn fetch_current(origin: &str, platform: &str) -> Result<ReleaseMeta, UpdateFail> {
    let path = format!("/v1/api/admin/app-releases/current?channel=desktop&platform={platform}");
    let (status, body) = exchange(origin, &path, MAX_JSON)?;
    if status == 404 {
        return Err(UpdateFail::NotPublished);
    }
    if status != 200 {
        return Err(UpdateFail::Failed);
    }
    parse_current(&body)
}

fn download_release(
    app: &AppHandle,
    origin: &str,
    platform: &str,
    release: &ReleaseMeta,
    bundle: &Path,
) -> Result<ReadyUpdate, UpdateFail> {
    let stage = stage_dir(bundle)?;
    fs::create_dir_all(&stage).map_err(|_| UpdateFail::Failed)?;
    let dest = stage.join("payload");
    let partial = stage.join("payload.partial");
    let _ = fs::remove_file(&partial);
    let path =
        format!("/v1/api/admin/app-releases/current/file?channel=desktop&platform={platform}");
    stream_verified(
        origin,
        &path,
        &partial,
        release.size,
        &release.sha256,
        |read, total| {
            emit_percent(app, read, total, download_percent(read, total));
        },
    )?;
    fs::rename(&partial, &dest).map_err(|_| UpdateFail::Failed)?;
    Ok(ReadyUpdate {
        version: release.version.clone(),
        notes: release.notes.clone(),
        sha256: release.sha256.clone(),
        path: dest,
        size: release.size,
        published_at: release.published_at.clone(),
    })
}

fn exchange(origin: &str, path: &str, max_body: usize) -> Result<(u16, Vec<u8>), UpdateFail> {
    let address = crate::org_login::parse_loopback(origin).ok_or(UpdateFail::Failed)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .map_err(|error| connect_fail(&error))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|_| UpdateFail::Failed)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|_| UpdateFail::Failed)?;
    let head = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(head.as_bytes())
        .map_err(|_| UpdateFail::Failed)?;
    stream.flush().map_err(|_| UpdateFail::Failed)?;
    let (status, length, mut body) = read_head(&mut stream)?;
    if let Some(length) = length {
        if length > max_body as u64 {
            return Err(UpdateFail::Failed);
        }
        let length = usize::try_from(length).map_err(|_| UpdateFail::Failed)?;
        while body.len() < length {
            let mut tmp = [0u8; 8192];
            let read = stream.read(&mut tmp).map_err(|_| UpdateFail::Failed)?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..read]);
        }
        if body.len() < length {
            return Err(UpdateFail::Failed);
        }
        body.truncate(length);
    } else if body.len() > max_body {
        return Err(UpdateFail::Failed);
    }
    Ok((status, body))
}

fn stream_verified(
    origin: &str,
    path: &str,
    dest: &Path,
    expected_len: u64,
    expected_sha: &str,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), UpdateFail> {
    if expected_len == 0 || expected_len > MAX_BYTES || !digest_ok(expected_sha) {
        return Err(UpdateFail::Failed);
    }
    let address = crate::org_login::parse_loopback(origin).ok_or(UpdateFail::Failed)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .map_err(|error| connect_fail(&error))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|_| UpdateFail::Failed)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|_| UpdateFail::Failed)?;
    let head = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nAccept: application/octet-stream\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(head.as_bytes())
        .map_err(|_| UpdateFail::Failed)?;
    stream.flush().map_err(|_| UpdateFail::Failed)?;
    let (status, length, first) = read_head(&mut stream)?;
    if status == 404 {
        return Err(UpdateFail::NotPublished);
    }
    if status != 200 {
        return Err(UpdateFail::Failed);
    }
    let Some(length) = length else {
        return Err(UpdateFail::Failed);
    };
    if length != expected_len {
        return Err(UpdateFail::Failed);
    }
    let mut file = File::create(dest).map_err(|_| UpdateFail::Failed)?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut write_chunk = |file: &mut File,
                           hasher: &mut Sha256,
                           received: &mut u64,
                           chunk: &[u8]|
     -> Result<(), UpdateFail> {
        let room = (length - *received) as usize;
        let take = chunk.len().min(room);
        if take == 0 {
            return Ok(());
        }
        hasher.update(&chunk[..take]);
        file.write_all(&chunk[..take])
            .map_err(|_| UpdateFail::Failed)?;
        *received += take as u64;
        on_progress(*received, length);
        Ok(())
    };
    write_chunk(&mut file, &mut hasher, &mut received, &first)?;
    let mut buf = [0u8; 64 * 1024];
    while received < length {
        let read = stream.read(&mut buf).map_err(|_| UpdateFail::Failed)?;
        if read == 0 {
            return Err(UpdateFail::Failed);
        }
        write_chunk(&mut file, &mut hasher, &mut received, &buf[..read])?;
    }
    file.sync_all().map_err(|_| UpdateFail::Failed)?;
    let actual = hex_encode(hasher.finalize());
    if !digest_matches(&actual, expected_sha) {
        let _ = fs::remove_file(dest);
        return Err(UpdateFail::Failed);
    }
    Ok(())
}

fn read_head(stream: &mut TcpStream) -> Result<(u16, Option<u64>, Vec<u8>), UpdateFail> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let split = loop {
        if buf.len() > 64 * 1024 {
            return Err(UpdateFail::Failed);
        }
        let read = stream.read(&mut tmp).map_err(|_| UpdateFail::Failed)?;
        if read == 0 {
            return Err(UpdateFail::Failed);
        }
        buf.extend_from_slice(&tmp[..read]);
        if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos;
        }
    };
    let head = String::from_utf8_lossy(&buf[..split]).into_owned();
    let status = header_status(&head).ok_or(UpdateFail::Failed)?;
    let length = content_length_of(&head);
    Ok((status, length, buf[split + 4..].to_vec()))
}

fn parse_current(body: &[u8]) -> Result<ReleaseMeta, UpdateFail> {
    let value: Value = serde_json::from_slice(body).map_err(|_| UpdateFail::Failed)?;
    if value.get("success") != Some(&Value::Bool(true))
        || value.get("code") != Some(&Value::from(0))
    {
        return Err(UpdateFail::Failed);
    }
    let data = value.get("data").ok_or(UpdateFail::Failed)?;
    let version = data
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let notes = data
        .get("notes")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let file_name = data
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let sha256 = data
        .get("sha256")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let size = data.get("size_bytes").and_then(Value::as_i64).unwrap_or(0);
    let published_at = data
        .get("published_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if version.is_empty() || !file_name_ok(&file_name) || !digest_ok(&sha256) {
        return Err(UpdateFail::Failed);
    }
    if size <= 0 || size as u64 > MAX_BYTES {
        return Err(UpdateFail::Failed);
    }
    if parse_version(&version).is_none() {
        return Err(UpdateFail::Failed);
    }
    Ok(ReleaseMeta {
        version,
        notes,
        sha256,
        size: size as u64,
        published_at,
    })
}

fn hash_file(
    path: &Path,
    expected: &str,
    total: u64,
    on_progress: &mut dyn FnMut(u8),
) -> Result<(), UpdateFail> {
    let mut file = File::open(path).map_err(|_| UpdateFail::Failed)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut read_total = 0u64;
    let mut last = 0u8;
    loop {
        let read = file.read(&mut buf).map_err(|_| UpdateFail::Failed)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
        read_total += read as u64;
        if read_total > total {
            return Err(UpdateFail::IntegrityFailed);
        }
        if total > 0 {
            let percent = ((read_total.saturating_mul(100)) / total).min(100) as u8;
            if percent != last {
                last = percent;
                on_progress(percent);
            }
        }
    }
    if read_total != total {
        return Err(UpdateFail::Failed);
    }
    let actual = hex_encode(hasher.finalize());
    if digest_matches(&actual, expected) {
        Ok(())
    } else {
        Err(UpdateFail::Failed)
    }
}

fn classify(path: &Path) -> Result<(), UpdateFail> {
    let mut file = File::open(path).map_err(|_| UpdateFail::Failed)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|_| UpdateFail::Failed)?;
    if magic.starts_with(b"PK") {
        Ok(())
    } else {
        Err(UpdateFail::InvalidPackage)
    }
}

fn extract_app_zip(archive_path: &Path, dest: &Path) -> Result<PathBuf, UpdateFail> {
    let _ = fs::remove_dir_all(dest);
    fs::create_dir_all(dest).map_err(|_| UpdateFail::Failed)?;
    if let Err(error) = extract_zip_entries(archive_path, dest) {
        let _ = fs::remove_dir_all(dest);
        return Err(error);
    }
    let app = single_app(dest)?;
    if !app.join("Contents/Info.plist").is_file() || !app.join("Contents/MacOS").is_dir() {
        let _ = fs::remove_dir_all(dest);
        return Err(UpdateFail::Failed);
    }
    Ok(app)
}

fn extract_zip_entries(archive_path: &Path, dest: &Path) -> Result<(), UpdateFail> {
    let file = File::open(archive_path).map_err(|_| UpdateFail::Failed)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| UpdateFail::Failed)?;
    if archive.len() == 0 || archive.len() > MAX_ENTRIES {
        return Err(UpdateFail::Failed);
    }
    let mut unpacked = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| UpdateFail::Failed)?;
        if let Some(mode) = entry.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                return Err(UpdateFail::Failed);
            }
        }
        let name = entry.name().to_string();
        if name.ends_with('/') {
            let Some(rel) = safe_rel(&name) else {
                return Err(UpdateFail::Failed);
            };
            fs::create_dir_all(dest.join(rel)).map_err(|_| UpdateFail::Failed)?;
            continue;
        }
        let Some(rel) = safe_rel(&name) else {
            return Err(UpdateFail::Failed);
        };
        let claimed = entry.size();
        if claimed > MAX_UNPACKED || unpacked.saturating_add(claimed) > MAX_UNPACKED {
            return Err(UpdateFail::Failed);
        }
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| UpdateFail::Failed)?;
        }
        let mut out = File::create(&target).map_err(|_| UpdateFail::Failed)?;
        let mut buf = [0u8; 64 * 1024];
        let mut wrote = 0u64;
        loop {
            let read = entry.read(&mut buf).map_err(|_| UpdateFail::Failed)?;
            if read == 0 {
                break;
            }
            wrote += read as u64;
            if wrote > MAX_UNPACKED || unpacked.saturating_add(wrote) > MAX_UNPACKED {
                return Err(UpdateFail::Failed);
            }
            out.write_all(&buf[..read])
                .map_err(|_| UpdateFail::Failed)?;
        }
        unpacked = unpacked.saturating_add(wrote);
    }
    Ok(())
}

fn single_app(dest: &Path) -> Result<PathBuf, UpdateFail> {
    let mut apps = Vec::new();
    let entries = fs::read_dir(dest).map_err(|_| UpdateFail::Failed)?;
    for entry in entries {
        let entry = entry.map_err(|_| UpdateFail::Failed)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "__MACOSX" || name == ".DS_Store" {
            continue;
        }
        if entry.path().is_dir() && name.ends_with(".app") {
            apps.push(entry.path());
            continue;
        }
        return Err(UpdateFail::Failed);
    }
    if apps.len() == 1 {
        Ok(apps.remove(0))
    } else {
        Err(UpdateFail::Failed)
    }
}

fn replace_bundle(new_app: &Path, current: &Path) -> Result<PathBuf, UpdateFail> {
    let name = current.file_name().ok_or(UpdateFail::Failed)?;
    let mut backup_name = name.to_os_string();
    backup_name.push(".previous");
    let backup = current.with_file_name(backup_name);
    if backup.exists() {
        return Err(UpdateFail::InstallFailed);
    }
    fs::rename(current, &backup).map_err(|_| UpdateFail::Failed)?;
    if fs::rename(new_app, current).is_err() {
        let _ = fs::rename(&backup, current);
        return Err(UpdateFail::Failed);
    }
    Ok(backup)
}

fn restore_bundle(current: &Path, backup: &Path) -> Result<(), UpdateFail> {
    let mut failed_name = current.file_name().unwrap_or_default().to_os_string();
    failed_name.push(format!(".failed-{}", std::process::id()));
    let failed = current.with_file_name(failed_name);
    if failed.exists() {
        return Err(UpdateFail::InstallFailed);
    }
    fs::rename(current, &failed).map_err(|_| UpdateFail::InstallFailed)?;
    if fs::rename(backup, current).is_err() {
        let _ = fs::rename(&failed, current);
        return Err(UpdateFail::InstallFailed);
    }
    let _ = fs::remove_dir_all(&failed);
    Ok(())
}

fn set_executable(path: &Path) -> Result<(), UpdateFail> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|_| UpdateFail::Failed)?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|_| UpdateFail::Failed)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn ensure_bundle_executable(app: &Path) -> Result<(), UpdateFail> {
    for name in [
        "Contents/MacOS/Studio",
        "Contents/Resources/agent/a3s-code-acp",
        "Contents/Resources/agent/a3s-office",
    ] {
        let path = app.join(name);
        if !path.is_file() {
            return Err(UpdateFail::InvalidPackage);
        }
        set_executable(&path)?;
    }
    Ok(())
}

fn verify_signature(bundle: &Path, version: &str) -> Result<(), UpdateFail> {
    let output = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(bundle)
        .output()
        .map_err(|_| UpdateFail::IntegrityFailed)?;
    if !output.status.success() {
        return Err(UpdateFail::IntegrityFailed);
    }
    for (key, expected) in [
        ("CFBundleExecutable", "Studio"),
        ("CFBundleIdentifier", "dev.a3s.studio"),
        ("CFBundleShortVersionString", version),
    ] {
        let output = Command::new("/usr/bin/plutil")
            .args(["-extract", key, "raw", "-o", "-"])
            .arg(bundle.join("Contents/Info.plist"))
            .output()
            .map_err(|_| UpdateFail::ProductMismatch)?;
        if !output.status.success()
            || std::str::from_utf8(&output.stdout).map(str::trim) != Ok(expected)
        {
            return Err(UpdateFail::ProductMismatch);
        }
    }
    Ok(())
}

pub(crate) fn installed_tenant(exe: &Path) -> std::io::Result<Option<String>> {
    static IDENTITY: OnceLock<(PathBuf, String)> = OnceLock::new();
    cached_installed_tenant(&IDENTITY, exe, delivery_manifest::installed_tenant)
        .map_err(|fail| std::io::Error::new(std::io::ErrorKind::InvalidData, fail.as_str()))
}

fn cached_installed_tenant(
    cache: &OnceLock<(PathBuf, String)>,
    exe: &Path,
    validate: impl FnOnce(&Path) -> Result<Option<String>, UpdateFail>,
) -> Result<Option<String>, UpdateFail> {
    if let Some((_, tenant)) = cache.get().filter(|(path, _)| path == exe) {
        return Ok(Some(tenant.clone()));
    }
    let tenant = validate(exe)?;
    if let Some(tenant) = &tenant {
        // An installed process keeps one generation until the updater exits it.
        let _ = cache.set((exe.to_path_buf(), tenant.clone()));
    }
    Ok(tenant)
}

fn clear_quarantine(path: &Path) {
    let _ = Command::new("/usr/bin/xattr")
        .args(["-cr"])
        .arg(path)
        .status();
}

const RELAUNCH_SCRIPT: &str = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
sleep 0.3
if "$4" "$2"; then
  if [ -n "$3" ]; then rm -rf "$3"; fi
else
  if [ -n "$3" ] && [ -d "$3" ]; then
    failed="$2.failed-$$"
    if mv "$2" "$failed"; then
      if mv "$3" "$2"; then "$4" "$2" || true
      else mv "$failed" "$2" || true; fi
    fi
  fi
  exit 1
fi"#;

fn relaunch_command(
    bundle: &Path,
    backup: Option<&Path>,
    parent_pid: u32,
    open_program: &Path,
) -> Command {
    let backup = backup.unwrap_or(Path::new(""));
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(RELAUNCH_SCRIPT)
        .arg("a3s-update")
        .arg(parent_pid.to_string())
        .arg(bundle)
        .arg(backup)
        .arg(open_program)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

fn schedule_relaunch(bundle: &Path, backup: Option<&Path>) -> Result<(), UpdateFail> {
    relaunch_command(
        bundle,
        backup,
        std::process::id(),
        Path::new("/usr/bin/open"),
    )
    .spawn()
    .map_err(|_| UpdateFail::RestartFailed)?;
    Ok(())
}

fn running_bundle() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let exe = std::env::current_exe().ok()?;
        if !crate::macos_app::is_app_executable(&exe) {
            return None;
        }
        return Some(exe.parent()?.parent()?.parent()?.to_path_buf());
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn stage_dir(bundle: &Path) -> Result<PathBuf, UpdateFail> {
    bundle
        .parent()
        .map(|parent| parent.join(".a3s-update"))
        .ok_or(UpdateFail::Failed)
}

fn matching_ready(ready: &ReadyUpdate, release: &ReleaseMeta) -> bool {
    ready.version == release.version
        && digest_matches(&ready.sha256, &release.sha256)
        && fs::metadata(&ready.path).is_ok_and(|meta| meta.is_file() && meta.len() == ready.size)
}

fn store_ready(ready: ReadyUpdate) -> Result<(), UpdateFail> {
    let mut phase = PHASE.lock().map_err(|_| UpdateFail::Failed)?;
    *phase = Phase::Ready(ready);
    Ok(())
}

fn take_ready() -> Result<ReadyUpdate, UpdateFail> {
    let mut phase = PHASE.lock().map_err(|_| UpdateFail::Failed)?;
    match std::mem::replace(&mut *phase, Phase::Busy) {
        Phase::Ready(ready) => Ok(ready),
        other => {
            let fail = if matches!(other, Phase::Busy) {
                UpdateFail::InProgress
            } else {
                UpdateFail::InvalidPackage
            };
            *phase = other;
            Err(fail)
        }
    }
}

fn ready_from_meta(release: &ReleaseMeta, path: PathBuf) -> ReadyUpdate {
    ReadyUpdate {
        version: release.version.clone(),
        notes: release.notes.clone(),
        sha256: release.sha256.clone(),
        path,
        size: release.size,
        published_at: release.published_at.clone(),
    }
}

fn emit_available(app: &AppHandle, ready: &ReadyUpdate) {
    emit(app, "app.updater.available", update_info(ready));
}

fn emit_downloaded(app: &AppHandle, ready: &ReadyUpdate) {
    emit(app, "app.updater.downloaded", update_info(ready));
}

fn emit_error(app: &AppHandle, message: &str) {
    emit(app, "app.updater.error", json!({ "message": message }));
}

fn emit_percent(app: &AppHandle, transferred: u64, total: u64, percent: u8) {
    emit(
        app,
        "app.updater.download_progress",
        json!({
            "total": total,
            "delta": 0,
            "transferred": transferred,
            "percent": percent,
            "bytesPerSecond": 0
        }),
    );
}

fn update_info(ready: &ReadyUpdate) -> Value {
    json!({
        "version": ready.version,
        "files": [],
        "releaseDate": ready.published_at,
        "releaseNotes": ready.notes
    })
}

fn emit(app: &AppHandle, event: &str, payload: Value) {
    let Ok(event) = serde_json::to_string(event) else {
        return;
    };
    let Ok(payload) = serde_json::to_string(&payload) else {
        return;
    };
    let script = format!(
        "globalThis.__A3S_UPDATER_EMIT__&&globalThis.__A3S_UPDATER_EMIT__({event},{payload})"
    );
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(script);
    }
}

fn desktop_platform() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("macos")
    } else if cfg!(target_os = "windows") {
        Some("windows")
    } else if cfg!(target_os = "linux") {
        Some("linux")
    } else {
        None
    }
}

fn download_percent(read: u64, total: u64) -> u8 {
    if total == 0 {
        0
    } else {
        ((read.saturating_mul(99)) / total).min(99) as u8
    }
}

fn remote_is_newer(remote: &str, local: &str) -> Result<bool, UpdateFail> {
    let remote = parse_version(remote).ok_or(UpdateFail::Failed)?;
    let local = parse_version(local).ok_or(UpdateFail::Failed)?;
    Ok(cmp_version(&remote, &local) == std::cmp::Ordering::Greater)
}

struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Option<String>,
}

fn parse_version(raw: &str) -> Option<Version> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 40 {
        return None;
    }
    let (core, pre) = match raw.split_once(['-', '+']) {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_string())),
        Some(_) => return None,
        None => (raw, None),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(Version {
        major,
        minor,
        patch,
        pre,
    })
}

fn cmp_version(left: &Version, right: &Version) -> std::cmp::Ordering {
    (left.major, left.minor, left.patch)
        .cmp(&(right.major, right.minor, right.patch))
        .then_with(|| match (&left.pre, &right.pre) {
            (None, None) => std::cmp::Ordering::Equal,
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(left_pre), Some(right_pre)) => cmp_pre(left_pre, right_pre),
        })
}

fn cmp_pre(left: &str, right: &str) -> std::cmp::Ordering {
    let left: Vec<&str> = left.split('.').collect();
    let right: Vec<&str> = right.split('.').collect();
    let count = left.len().max(right.len());
    for index in 0..count {
        match (left.get(index), right.get(index)) {
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(left_id), Some(right_id)) => {
                let order = match (left_id.parse::<u64>(), right_id.parse::<u64>()) {
                    (Ok(left_num), Ok(right_num)) => left_num.cmp(&right_num),
                    (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                    (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                    (Err(_), Err(_)) => left_id.cmp(right_id),
                };
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
            }
            (None, None) => {}
        }
    }
    std::cmp::Ordering::Equal
}

fn digest_ok(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn digest_matches(actual: &str, expected: &str) -> bool {
    if actual.len() != 64 || expected.len() != 64 {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in actual.bytes().zip(expected.bytes()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn file_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 180
        && !name.contains(['/', '\\', '\0'])
        && name != "."
        && name != ".."
}

fn safe_rel(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('\0') {
        return None;
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
fn is_macho(magic: &[u8; 4]) -> bool {
    let le = u32::from_le_bytes(*magic);
    let be = u32::from_be_bytes(*magic);
    matches!(le, 0xFEEDFACE | 0xFEEDFACF | 0xCAFEBABE | 0xBEBAFECA)
        || matches!(be, 0xFEEDFACE | 0xFEEDFACF | 0xCAFEBABE | 0xBEBAFECA)
}

fn header_status(head: &str) -> Option<u16> {
    head.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
}

fn content_length_of(head: &str) -> Option<u64> {
    head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse().ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::{
        UpdateFail, connect_fail, content_length_of, digest_matches, download_percent,
        extract_app_zip, header_status, is_macho, parse_current, remote_is_newer,
        require_update_family, safe_rel,
    };
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    #[test]
    fn newer_remote_versions_install_and_older_ones_do_not() {
        assert!(remote_is_newer("2.1.4", "2.1.3").unwrap());
        assert!(!remote_is_newer("2.1.3", "2.1.3").unwrap());
        assert!(!remote_is_newer("2.1.2", "2.1.3").unwrap());
        assert!(remote_is_newer("2.1.3", "2.1.3-beta.1").unwrap());
        assert!(!remote_is_newer("2.1.3-beta.2", "2.1.3").unwrap());
        assert!(remote_is_newer("2.1.4-beta.1", "2.1.3").unwrap());
        assert!(remote_is_newer("2.0.0", "1.9.9").unwrap());
        assert!(remote_is_newer("1.2.3-alpha.2", "1.2.3-alpha.1").unwrap());
        assert!(remote_is_newer("1.2.3-alpha.beta", "1.2.3-alpha.1").unwrap());
        assert!(matches!(
            remote_is_newer("nope", "2.1.3"),
            Err(UpdateFail::Failed)
        ));
    }

    #[test]
    fn progress_stops_before_one_hundred_until_the_file_is_complete() {
        assert_eq!(download_percent(0, 10), 0);
        assert_eq!(download_percent(10, 10), 99);
        assert_eq!(download_percent(5, 10), 49);
    }

    #[test]
    fn digest_compare_rejects_a_different_hash() {
        let hash = "ab".repeat(32);
        assert!(digest_matches(&hash, &hash));
        let mut other = hash.clone();
        other.replace_range(0..1, "c");
        assert!(!digest_matches(&hash, &other));
        assert!(!digest_matches("abc", &hash));
    }

    #[test]
    fn a_missing_local_release_is_unpublished() {
        assert!(matches!(
            require_update_family(false),
            Err(UpdateFail::NotPublished)
        ));
        assert!(require_update_family(true).is_ok());
        for kind in [
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::TimedOut,
            std::io::ErrorKind::AddrNotAvailable,
            std::io::ErrorKind::NotFound,
        ] {
            assert!(matches!(
                connect_fail(&std::io::Error::new(kind, "down")),
                UpdateFail::NotPublished
            ));
        }
        assert!(matches!(
            connect_fail(&std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "reset"
            )),
            UpdateFail::Failed
        ));
    }

    #[test]
    fn headers_expose_status_and_length() {
        let head =
            "HTTP/1.1 200 OK\r\nContent-Length: 12\r\nContent-Type: application/octet-stream";
        assert_eq!(header_status(head), Some(200));
        assert_eq!(content_length_of(head), Some(12));
        assert_eq!(header_status("nope"), None);
    }

    #[test]
    fn current_feed_rejects_a_missing_digest() {
        let body = br#"{"success":true,"code":0,"data":{"version":"2.1.4","notes":"n","file_name":"ToWork.zip","size_bytes":4,"sha256":"","published_at":"2026-10-06T00:00:00Z"}}"#;
        assert!(parse_current(body).is_err());
        let sha = "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";
        let ok = format!(
            r#"{{"success":true,"code":0,"data":{{"version":"2.1.4","notes":"n","file_name":"ToWork.zip","size_bytes":4,"sha256":"{sha}"}}}}"#
        );
        let release = parse_current(ok.as_bytes()).unwrap();
        assert_eq!(release.version, "2.1.4");
        assert_eq!(release.size, 4);
    }

    #[test]
    fn zip_slip_and_extra_roots_are_refused() {
        let root = std::env::temp_dir().join(format!("a3s-update-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let zip_path = root.join("bad.zip");
        write_zip(
            &zip_path,
            &[
                ("ToWork.app/Contents/Info.plist", b"plist".as_slice()),
                ("../escape.txt", b"no".as_slice()),
            ],
        );
        let dest = root.join("out");
        assert!(extract_app_zip(&zip_path, &dest).is_err());
        assert!(!root.join("escape.txt").exists());

        let two = root.join("two.zip");
        write_zip(
            &two,
            &[
                ("ToWork.app/Contents/Info.plist", b"plist"),
                ("ToWork.app/Contents/MacOS/ToWork", b"bin"),
                ("Other.app/Contents/Info.plist", b"plist"),
            ],
        );
        assert!(extract_app_zip(&two, &root.join("two-out")).is_err());

        let good = root.join("good.zip");
        write_zip(
            &good,
            &[
                ("ToWork.app/Contents/Info.plist", b"plist"),
                ("ToWork.app/Contents/MacOS/ToWork", b"bin"),
                ("__MACOSX/._ToWork", b"meta"),
            ],
        );
        let app = extract_app_zip(&good, &root.join("good-out")).unwrap();
        assert_eq!(
            app.file_name().and_then(|name| name.to_str()),
            Some("ToWork.app")
        );
        assert!(app.join("Contents/MacOS/ToWork").is_file());
        assert!(safe_rel("../x").is_none());
        assert!(safe_rel("ToWork.app/Contents/Info.plist").is_some());
        assert!(is_macho(&[0xcf, 0xfa, 0xed, 0xfe]));
        assert!(!is_macho(b"PK\x03\x04"));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn write_zip(path: &std::path::Path, files: &[(&str, &[u8])]) {
        let mut cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(&mut cursor);
        for (name, bytes) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        std::fs::write(path, cursor.into_inner()).unwrap();
    }
}
