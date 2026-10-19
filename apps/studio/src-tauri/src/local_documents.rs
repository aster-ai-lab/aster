//! Workspace directory listings for the documents page and the task file manager.
//! The documents listing is one level of folders plus the files directly inside them.
//! Trash and open for that page only accept a file whose parent is the workspace or one direct child.
//! The file manager lists one directory at a time and can create, rename, move, and remove
//! any entry strictly inside the canonical workspace.
//! Paths outside that workspace, filesystem roots, hidden names, and `..` are rejected.

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};

const FILE_CAP: usize = 500;
const RETRIEVAL_LEDGER: &str = ".a3s-agent-retrievals";

/// Passages from UTF-8 notes in this workspace that match the question.
/// At most 12 passages and 24000 characters. An empty question returns nothing.
pub fn retrieve_passages(raw_root: &str, query: &str) -> Vec<String> {
    let tokens = query_tokens(query);
    if tokens.is_empty() {
        return Vec::new();
    }
    let Ok(root) = canonical_workspace(raw_root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    collect_note_passages(&root, &root, 0, &tokens, &mut found);
    found.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    let mut chosen = Vec::new();
    let mut chars = 0usize;
    let mut counted = HashSet::new();
    for (score, path, body) in found {
        if score == 0 || chosen.len() == 12 {
            break;
        }
        let next = body.chars().count();
        if chars + next > 24_000 {
            break;
        }
        chars += next;
        if counted.insert(path.clone()) {
            note_agent_retrieval(&root, &path);
        }
        chosen.push(body);
    }
    chosen
}

fn query_tokens(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for token in query.split(|ch: char| !ch.is_alphanumeric()) {
        let token = token.trim().to_lowercase();
        if token.chars().count() < 2 || tokens.contains(&token) {
            continue;
        }
        tokens.push(token);
    }
    tokens
}

fn collect_note_passages(
    root: &Path,
    directory: &Path,
    depth: usize,
    tokens: &[String],
    found: &mut Vec<(i32, String, String)>,
) {
    if depth > 4 || found.len() > 200 {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(name) = entry_name(&entry) else {
            continue;
        };
        if name == "node_modules" || name == "target" || name == "dist" {
            continue;
        }
        let Some(canon) = contained_child(directory, &entry) else {
            continue;
        };
        if !canon.starts_with(root) {
            continue;
        }
        if canon.is_dir() {
            collect_note_passages(root, &canon, depth + 1, tokens, found);
            continue;
        }
        let lower = name.to_lowercase();
        if !(lower.ends_with(".md") || lower.ends_with(".txt") || lower.ends_with(".markdown")) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&canon) else {
            continue;
        };
        if text.len() > 64 * 1024 || text.contains('\0') {
            continue;
        }
        let path = path_key(&canon);
        let mut heading = String::new();
        let mut body = String::new();
        let mut flush = |heading: &str, body: &str| {
            let payload = body.trim();
            if payload.is_empty() {
                return;
            }
            let mut score = 0;
            let heading_l = heading.to_lowercase();
            let body_l = payload.to_lowercase();
            for token in tokens {
                if !heading_l.is_empty() && heading_l.contains(token) {
                    score += 3;
                }
                if body_l.contains(token) {
                    score += 1;
                }
            }
            if score > 0 {
                found.push((score, path.clone(), payload.to_string()));
            }
        };
        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                flush(&heading, &body);
                body.clear();
                heading = trimmed.trim_end().to_string();
            } else {
                body.push_str(line);
            }
        }
        flush(&heading, &body);
    }
}

/// Move a note into the one sibling folder whose name equals its first heading.
pub fn place_by_heading(root: &Path, file: &Path) -> Option<PathBuf> {
    if !file.is_file() || !file.starts_with(root) {
        return None;
    }
    let text = fs::read_to_string(file).ok()?;
    let heading = text
        .lines()
        .find(|line| line.trim_start().starts_with('#'))?;
    let title = heading.trim().trim_start_matches('#').trim();
    if title.is_empty() || title.contains('/') || title.contains('\\') {
        return None;
    }
    let parent = file.parent()?;
    let mut matches = Vec::new();
    for entry in fs::read_dir(parent).ok()?.flatten() {
        let Some(name) = entry_name(&entry) else {
            continue;
        };
        if name != title {
            continue;
        }
        let Some(canon) = contained_child(parent, &entry) else {
            continue;
        };
        if canon.is_dir() && canon.starts_with(root) {
            matches.push(canon);
        }
    }
    let [dest_dir] = matches.as_slice() else {
        return None;
    };
    let name = file.file_name()?;
    let dest = dest_dir.join(name);
    if dest.exists() {
        return None;
    }
    fs::rename(file, &dest).ok()?;
    let moved = fs::canonicalize(&dest).ok()?;
    retarget_retrievals(root, file, Some(&moved));
    Some(moved)
}

/// One successful agent retrieval of a file inside this workspace.
/// The documents list reads the same ledger. A write failure leaves the count unchanged.
pub fn note_agent_retrieval(root: &Path, raw_path: &str) {
    let Ok(canon) = fs::canonicalize(raw_path) else {
        return;
    };
    if !canon.starts_with(root) || !canon.is_file() {
        return;
    }
    let Some(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join(RETRIEVAL_LEDGER))
        .ok()
    else {
        return;
    };
    let _ = writeln!(file, "{}", path_key(&canon));
}

fn retrieval_counts(root: &Path) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    let Ok(text) = fs::read_to_string(root.join(RETRIEVAL_LEDGER)) else {
        return counts;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        *counts.entry(line.to_owned()).or_insert(0) += 1;
    }
    counts
}

fn retarget_retrievals(root: &Path, from: &Path, to: Option<&Path>) {
    let ledger = root.join(RETRIEVAL_LEDGER);
    let Ok(text) = fs::read_to_string(&ledger) else {
        return;
    };
    let from_text = path_key(from);
    let prefix = format!("{from_text}/");
    let mut out = String::new();
    for line in text.lines() {
        if line == from_text || line.starts_with(&prefix) {
            if let Some(dest) = to {
                let rest = &line[from_text.len()..];
                out.push_str(&path_key(dest));
                out.push_str(rest);
                out.push('\n');
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    let _ = fs::write(ledger, out);
}

#[derive(Deserialize)]
struct RootPathBody {
    root: String,
    path: String,
}

#[derive(Deserialize)]
struct CreateBody {
    root: String,
    #[serde(default)]
    parent: String,
    name: String,
    kind: String,
}

#[derive(Deserialize)]
struct RenameBody {
    root: String,
    path: String,
    name: String,
}

#[derive(Deserialize)]
struct MoveBody {
    root: String,
    path: String,
    #[serde(default)]
    destination: String,
}

pub fn list_json(root: &str) -> Result<String, u16> {
    let listing = list_in(root)?;
    Ok(listing.to_string())
}

pub fn trash_json(body: &[u8]) -> Result<String, u16> {
    let parsed: RootPathBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    trash_in(&parsed.root, &parsed.path)?;
    Ok(json!({ "ok": true }).to_string())
}

pub fn open_json(body: &[u8]) -> Result<String, u16> {
    let parsed: RootPathBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    open_in(&parsed.root, &parsed.path)?;
    Ok(json!({ "ok": true }).to_string())
}

pub fn entries_json(raw_root: &str, raw_path: &str) -> Result<String, u16> {
    Ok(entries_in(raw_root, raw_path)?.to_string())
}

pub fn create_json(body: &[u8]) -> Result<String, u16> {
    let parsed: CreateBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    Ok(create_in(&parsed.root, &parsed.parent, &parsed.name, &parsed.kind)?.to_string())
}

pub fn rename_json(body: &[u8]) -> Result<String, u16> {
    let parsed: RenameBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    Ok(rename_in(&parsed.root, &parsed.path, &parsed.name)?.to_string())
}

pub fn move_json(body: &[u8]) -> Result<String, u16> {
    let parsed: MoveBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    Ok(move_in(&parsed.root, &parsed.path, &parsed.destination)?.to_string())
}

pub fn remove_json(body: &[u8]) -> Result<String, u16> {
    let parsed: RootPathBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    remove_in(&parsed.root, &parsed.path)?;
    Ok(json!({ "ok": true }).to_string())
}

/// Decodes one form-encoded query component; a literal plus must be `%2B`.
pub fn decode_query_component(value: &str) -> Result<String, u16> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(400);
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).map_err(|_| 400u16)?;
            let byte = u8::from_str_radix(hex, 16).map_err(|_| 400u16)?;
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8(out).map_err(|_| 400u16)
}

pub fn list_in(raw_root: &str) -> Result<Value, u16> {
    let root = canonical_workspace(raw_root)?;
    let counts = retrieval_counts(&root);
    let mut folders: Vec<(String, PathBuf)> = Vec::new();
    let mut files = Vec::new();
    let mut seen_dirs = HashSet::new();
    let mut seen_files = HashSet::new();
    let entries = fs::read_dir(&root).map_err(|_| 500u16)?;
    for entry in entries.flatten() {
        let Some(name) = entry_name(&entry) else {
            continue;
        };
        let Some(canon) = contained_child(&root, &entry) else {
            continue;
        };
        if canon.is_dir() {
            if seen_dirs.insert(canon.clone()) {
                folders.push((name, canon));
            }
        } else if canon.is_file() && seen_files.insert(canon.clone()) {
            files.push(file_json(&counts, &name, &canon, ""));
        }
    }
    folders.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, dir) in &folders {
        let Ok(children) = fs::read_dir(dir) else {
            continue;
        };
        for entry in children.flatten() {
            let Some(file_name) = entry_name(&entry) else {
                continue;
            };
            let Some(canon) = contained_child(dir, &entry) else {
                continue;
            };
            if canon.is_file() && seen_files.insert(canon.clone()) {
                files.push(file_json(&counts, &file_name, &canon, name));
            }
        }
    }
    files.sort_by(|left, right| {
        let left_ms = left["modifiedMs"].as_u64().unwrap_or(0);
        let right_ms = right["modifiedMs"].as_u64().unwrap_or(0);
        right_ms.cmp(&left_ms)
    });
    files.truncate(FILE_CAP);
    let folder_json: Vec<Value> = folders
        .iter()
        .map(|(name, _)| json!({ "name": name }))
        .collect();
    Ok(json!({ "folders": folder_json, "files": files }))
}

pub fn trash_in(raw_root: &str, raw_path: &str) -> Result<(), u16> {
    let root = canonical_workspace(raw_root)?;
    let canon = require_workspace_file(&root, raw_path)?;
    trash_path(&canon)?;
    retarget_retrievals(&root, &canon, None);
    Ok(())
}

/// One directory inside the workspace. An empty path lists the workspace root.
pub fn entries_in(raw_root: &str, raw_path: &str) -> Result<Value, u16> {
    let root = canonical_workspace(raw_root)?;
    let counts = retrieval_counts(&root);
    let directory = directory_inside(&root, raw_path)?;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut capped = false;
    let read = fs::read_dir(&directory).map_err(|_| 500u16)?;
    for item in read.flatten() {
        if entries.len() >= FILE_CAP {
            capped = true;
            break;
        }
        let Some(name) = entry_name(&item) else {
            continue;
        };
        let Some(canon) = contained_child(&directory, &item) else {
            continue;
        };
        if !seen.insert(canon.clone()) {
            continue;
        }
        let kind = if canon.is_dir() {
            "directory"
        } else if canon.is_file() {
            "file"
        } else {
            continue;
        };
        entries.push(entry_value(&counts, &name, &canon, kind));
    }
    entries.sort_by(|left, right| {
        let left_dir = left["kind"].as_str() == Some("directory");
        let right_dir = right["kind"].as_str() == Some("directory");
        right_dir.cmp(&left_dir).then_with(|| {
            left["name"]
                .as_str()
                .unwrap_or("")
                .cmp(right["name"].as_str().unwrap_or(""))
        })
    });
    Ok(json!({ "entries": entries, "capped": capped }))
}

pub fn create_in(raw_root: &str, raw_parent: &str, name: &str, kind: &str) -> Result<Value, u16> {
    source_segment(name)?;
    let root = canonical_workspace(raw_root)?;
    let parent = directory_inside(&root, raw_parent)?;
    let dest = parent.join(name);
    // The personal vault already lives in `wiki`. Creating that name is still refused.
    if dest.exists() {
        return Err(409);
    }
    if reserved_bundle_name(name) {
        return Err(400);
    }
    match kind {
        "file" => {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&dest)
                .map_err(|_| 500u16)?;
        }
        "folder" | "directory" => {
            fs::create_dir(&dest).map_err(|_| 500u16)?;
        }
        _ => return Err(400),
    }
    let canon = fs::canonicalize(&dest).map_err(|_| 500u16)?;
    if canon.parent() != Some(parent.as_path()) {
        if canon.is_dir() {
            let _ = fs::remove_dir(&canon);
        } else {
            let _ = fs::remove_file(&canon);
        }
        return Err(400);
    }
    let stored = if canon.is_dir() { "directory" } else { "file" };
    Ok(entry_value(&retrieval_counts(&root), name, &canon, stored))
}

pub fn rename_in(raw_root: &str, raw_path: &str, name: &str) -> Result<Value, u16> {
    plain_name(name)?;
    let root = canonical_workspace(raw_root)?;
    let canon = strict_child(&root, raw_path)?;
    let parent = canon.parent().ok_or(400u16)?.to_path_buf();
    if parent.as_path() != root.as_path() && !parent.starts_with(&root) {
        return Err(400);
    }
    let current = canon
        .file_name()
        .and_then(|item| item.to_str())
        .ok_or(400u16)?;
    if current == name {
        let stored = if canon.is_dir() { "directory" } else { "file" };
        return Ok(entry_value(&retrieval_counts(&root), name, &canon, stored));
    }
    let dest = parent.join(name);
    if dest.exists() {
        return Err(409);
    }
    fs::rename(&canon, &dest).map_err(|_| 500u16)?;
    let renamed = fs::canonicalize(&dest).map_err(|_| 500u16)?;
    if renamed.parent() != Some(parent.as_path()) {
        return Err(500);
    }
    retarget_retrievals(&root, &canon, Some(&renamed));
    let stored = if renamed.is_dir() {
        "directory"
    } else {
        "file"
    };
    Ok(entry_value(
        &retrieval_counts(&root),
        name,
        &renamed,
        stored,
    ))
}

pub fn move_in(raw_root: &str, raw_path: &str, raw_destination: &str) -> Result<Value, u16> {
    let root = canonical_workspace(raw_root)?;
    let canon = strict_child(&root, raw_path)?;
    let destination = directory_inside(&root, raw_destination)?;
    if destination.starts_with(&canon) {
        return Err(400);
    }
    let name = canon
        .file_name()
        .and_then(|item| item.to_str())
        .ok_or(400u16)?;
    let parent = canon.parent().ok_or(400u16)?;
    if parent == destination.as_path() {
        let stored = if canon.is_dir() { "directory" } else { "file" };
        return Ok(entry_value(&retrieval_counts(&root), name, &canon, stored));
    }
    let dest = destination.join(name);
    if dest.exists() {
        return Err(409);
    }
    fs::rename(&canon, &dest).map_err(|_| 500u16)?;
    let moved = fs::canonicalize(&dest).map_err(|_| 500u16)?;
    if moved.parent() != Some(destination.as_path()) {
        return Err(500);
    }
    let stored = if moved.is_dir() { "directory" } else { "file" };
    retarget_retrievals(&root, &canon, Some(&moved));
    Ok(entry_value(&retrieval_counts(&root), name, &moved, stored))
}

pub fn remove_in(raw_root: &str, raw_path: &str) -> Result<(), u16> {
    let root = canonical_workspace(raw_root)?;
    let canon = strict_child(&root, raw_path)?;
    if !canon.is_file() && !canon.is_dir() {
        return Err(400);
    }
    trash_path(&canon)?;
    retarget_retrievals(&root, &canon, None);
    Ok(())
}

fn trash_path(canon: &Path) -> Result<(), u16> {
    let trash_dir = home_dir()?.join(".Trash");
    fs::create_dir_all(&trash_dir).map_err(|_| 500u16)?;
    let name = canon.file_name().ok_or(400u16)?;
    let mut dest = trash_dir.join(name);
    if dest.exists() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        dest = trash_dir.join(format!("{stamp}-{}", name.to_string_lossy()));
    }
    fs::rename(canon, &dest).map_err(|_| 500u16)?;
    Ok(())
}

/// Opens a workspace file in the system handler. Paths outside the workspace never spawn.
pub fn open_in(raw_root: &str, raw_path: &str) -> Result<(), u16> {
    let root = canonical_workspace(raw_root)?;
    let canon = require_workspace_file(&root, raw_path)?;
    let launch =
        crate::host_family::open_path_launch(crate::host_family::current_family(), &canon, false);
    let status = crate::host_family::spawn_launch(&launch).map_err(|_| 500u16)?;
    if status.success() { Ok(()) } else { Err(500) }
}

fn require_workspace_file(root: &Path, raw_path: &str) -> Result<PathBuf, u16> {
    let path = bounded_path(raw_path)?;
    let canon = fs::canonicalize(&path).map_err(|_| 404u16)?;
    if !canon.is_file() {
        return Err(400);
    }
    let parent = canon.parent().ok_or(400u16)?;
    if parent == root {
        return Ok(canon);
    }
    let grand = parent.parent().ok_or(400u16)?;
    if grand != root {
        return Err(400);
    }
    let name = parent
        .file_name()
        .and_then(|item| item.to_str())
        .ok_or(400u16)?;
    if name.starts_with('.') {
        return Err(400);
    }
    Ok(canon)
}

pub(crate) fn canonical_workspace(raw: &str) -> Result<PathBuf, u16> {
    let path = bounded_path(raw)?;
    if is_filesystem_root(&path) {
        return Err(400);
    }
    let canon = fs::canonicalize(&path).map_err(|_| 404u16)?;
    if !canon.is_dir() || is_filesystem_root(&canon) {
        return Err(400);
    }
    Ok(canon)
}

fn bounded_path(raw: &str) -> Result<PathBuf, u16> {
    if raw.is_empty() || raw.len() > 4096 || raw.contains('\0') {
        return Err(400);
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() || has_parent_component(&path) {
        return Err(400);
    }
    Ok(path)
}

fn has_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
}

fn is_filesystem_root(path: &Path) -> bool {
    if path.parent().is_none() {
        return true;
    }
    let text = path.to_string_lossy();
    text == "/"
        || text == "\\"
        || (text.len() == 2
            && text.as_bytes()[0].is_ascii_alphabetic()
            && text.as_bytes()[1] == b':')
        || (text.len() == 3
            && text.as_bytes()[0].is_ascii_alphabetic()
            && text.as_bytes()[1] == b':'
            && (text.as_bytes()[2] == b'\\' || text.as_bytes()[2] == b'/'))
}

fn entry_name(entry: &fs::DirEntry) -> Option<String> {
    let name = entry.file_name().to_str()?.to_owned();
    if name.starts_with('.') || name.contains('/') || name.contains('\\') {
        return None;
    }
    Some(name)
}

/// A direct child whose canonical path stays inside `parent`. Symlinks that leave are dropped.
fn contained_child(parent: &Path, entry: &fs::DirEntry) -> Option<PathBuf> {
    let canon = fs::canonicalize(entry.path()).ok()?;
    (canon.parent() == Some(parent)).then_some(canon)
}

fn file_json(counts: &HashMap<String, u64>, name: &str, path: &Path, folder: &str) -> Value {
    let meta = fs::metadata(path).ok();
    let size = meta.as_ref().map(|item| item.len()).unwrap_or(0);
    let modified = meta
        .and_then(|item| item.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    json!({
        "name": name,
        "path": path.to_string_lossy(),
        "sizeBytes": size,
        "modifiedMs": modified,
        "folder": folder,
        "agentRetrievals": counts.get(&path_key(path)).copied().unwrap_or(0)
    })
}

fn contained_path(root: &Path, raw_path: &str) -> Result<PathBuf, u16> {
    let path = bounded_path(raw_path)?;
    let canon = fs::canonicalize(&path).map_err(|_| 404u16)?;
    if canon.as_path() == root || canon.starts_with(root) {
        return Ok(canon);
    }
    Err(400)
}

fn strict_child(root: &Path, raw_path: &str) -> Result<PathBuf, u16> {
    let canon = contained_path(root, raw_path)?;
    if canon.as_path() == root {
        return Err(400);
    }
    Ok(canon)
}

fn directory_inside(root: &Path, raw_path: &str) -> Result<PathBuf, u16> {
    if raw_path.is_empty() {
        return Ok(root.to_path_buf());
    }
    let canon = contained_path(root, raw_path)?;
    if canon.is_dir() { Ok(canon) } else { Err(400) }
}

fn source_segment(name: &str) -> Result<(), u16> {
    if name.is_empty()
        || name.len() > 255
        || name.contains('\0')
        || name.contains('/')
        || name.contains('\\')
        || name.starts_with('.')
    {
        return Err(400);
    }
    Ok(())
}

fn plain_name(name: &str) -> Result<(), u16> {
    source_segment(name)?;
    if reserved_bundle_name(name) {
        return Err(400);
    }
    Ok(())
}

/// The compiled encyclopedia directory is not a source name.
pub fn reserved_bundle_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("wiki")
}

fn entry_value(counts: &HashMap<String, u64>, name: &str, path: &Path, kind: &str) -> Value {
    let meta = fs::metadata(path).ok();
    let size = meta.as_ref().map(|item| item.len()).unwrap_or(0);
    let modified = meta
        .and_then(|item| item.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    json!({
        "name": name,
        "path": path.to_string_lossy(),
        "kind": kind,
        "sizeBytes": size,
        "modifiedMs": modified,
        "agentRetrievals": counts.get(&path_key(path)).copied().unwrap_or(0)
    })
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn home_dir() -> Result<PathBuf, u16> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .ok_or(500)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "a3s-workspace-docs-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn names(listing: &Value, field: &str) -> Vec<(String, String)> {
        listing[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["name"].as_str().unwrap().to_owned(),
                    item["folder"].as_str().unwrap_or("").to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn retrieves_a_matching_note_and_files_it_under_the_heading_folder() {
        let root = fs::canonicalize(workspace()).unwrap();
        fs::create_dir(root.join("报销")).unwrap();
        let note = root.join("claim.md");
        fs::write(&note, "# 报销\n差旅标准写在这里\n").unwrap();
        let found = retrieve_passages(root.to_str().unwrap(), "差旅标准");
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("差旅标准"));
        let moved = place_by_heading(&root, &fs::canonicalize(&note).unwrap()).unwrap();
        assert_eq!(moved.parent().unwrap().file_name().unwrap(), "报销");
        let listing = entries_in(root.to_str().unwrap(), "").unwrap();
        let row = listing["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == "claim.md");
        assert!(row.is_none());
        let filed = entries_in(
            root.to_str().unwrap(),
            moved.parent().unwrap().to_str().unwrap(),
        )
        .unwrap();
        let filed_row = filed["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == "claim.md")
            .unwrap();
        assert_eq!(filed_row["agentRetrievals"], 1);
        assert!(retrieve_passages(root.to_str().unwrap(), "").is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn counts_agent_retrievals_on_the_file_row() {
        let root = fs::canonicalize(workspace()).unwrap();
        let note = root.join("note.txt");
        fs::write(&note, b"n").unwrap();
        let canon = fs::canonicalize(&note).unwrap();
        note_agent_retrieval(&root, canon.to_str().unwrap());
        note_agent_retrieval(&root, canon.to_str().unwrap());
        let listing = entries_in(root.to_str().unwrap(), "").unwrap();
        let row = listing["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == "note.txt")
            .unwrap();
        assert_eq!(row["agentRetrievals"], 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_only_workspace_folders_and_their_top_level_files() {
        let root = workspace();
        fs::create_dir(root.join("Notes")).unwrap();
        fs::create_dir(root.join(".hidden")).unwrap();
        fs::write(root.join("root.txt"), b"r").unwrap();
        fs::write(root.join(".secret"), b"s").unwrap();
        fs::write(root.join("Notes/note.txt"), b"n").unwrap();
        fs::create_dir(root.join("Notes/nested")).unwrap();
        fs::write(root.join("Notes/nested/deep.txt"), b"d").unwrap();
        fs::write(root.join(".hidden/nope.txt"), b"n").unwrap();
        let listing = list_in(root.to_str().unwrap()).unwrap();
        let folders: Vec<_> = listing["folders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(folders, vec!["Notes".to_owned()]);
        let mut files = names(&listing, "files");
        files.sort();
        assert_eq!(
            files,
            vec![
                ("note.txt".to_owned(), "Notes".to_owned()),
                ("root.txt".to_owned(), String::new()),
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn trash_moves_a_workspace_file_and_rejects_other_paths() {
        let root = workspace();
        fs::create_dir(root.join("Notes")).unwrap();
        let stamp = std::process::id();
        let note_name = format!("a3s-docs-test-note-{stamp}.txt");
        let top_name = format!("a3s-docs-test-root-{stamp}.txt");
        let note = root.join("Notes").join(&note_name);
        fs::write(&note, b"n").unwrap();
        let top = root.join(&top_name);
        fs::write(&top, b"r").unwrap();
        let root_text = root.to_str().unwrap();
        trash_in(root_text, note.to_str().unwrap()).unwrap();
        assert!(!note.exists());
        trash_in(root_text, top.to_str().unwrap()).unwrap();
        assert!(!top.exists());
        let trash = home_dir().unwrap().join(".Trash");
        let _ = fs::remove_file(trash.join(&note_name));
        let _ = fs::remove_file(trash.join(&top_name));
        let outside = std::env::temp_dir().join(format!("a3s-outside-{}.txt", std::process::id()));
        fs::write(&outside, b"no").unwrap();
        assert_eq!(trash_in(root_text, outside.to_str().unwrap()), Err(400));
        assert!(outside.is_file());
        let escaped = format!(
            "{root_text}/../{}",
            outside.file_name().unwrap().to_string_lossy()
        );
        assert_eq!(trash_in(root_text, &escaped), Err(400));
        assert!(outside.is_file());
        assert_eq!(open_in(root_text, outside.to_str().unwrap()), Err(400));
        assert!(outside.is_file());
        let missing = root.join("missing.txt");
        assert_eq!(open_in(root_text, missing.to_str().unwrap()), Err(404));
        assert_eq!(list_in("/"), Err(400));
        assert_eq!(list_in("relative"), Err(400));
        let nested = root.join("Notes/nested/deep.txt");
        fs::create_dir(root.join("Notes/nested")).unwrap();
        fs::write(&nested, b"d").unwrap();
        assert_eq!(trash_in(root_text, nested.to_str().unwrap()), Err(400));
        assert!(nested.is_file());
        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_wiki_name_is_not_a_source() {
        let root = workspace();
        let text = root.to_str().unwrap();
        assert_eq!(create_in(text, text, "wiki", "folder"), Err(400));
        assert_eq!(create_in(text, text, "Wiki", "file"), Err(400));
        assert!(!root.join("wiki").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_existing_wiki_directory_is_a_conflict() {
        let root = workspace();
        fs::create_dir(root.join("wiki")).unwrap();
        let text = root.to_str().unwrap();
        assert_eq!(create_in(text, text, "wiki", "folder"), Err(409));
        assert!(root.join("wiki").is_dir());
        assert!(fs::read_dir(root.join("wiki")).unwrap().next().is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn decode_query_component_unescapes_percent_bytes() {
        assert_eq!(
            decode_query_component("%2FUsers%2Fada%2FMy%20docs").unwrap(),
            "/Users/ada/My docs"
        );
        assert_eq!(decode_query_component("%zz"), Err(400));
    }

    #[test]
    fn decode_query_component_preserves_form_encoded_file_names() {
        let name = "报名 + 汇总 (2).html";
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("name", name)
            .finish();
        assert_eq!(
            decode_query_component(query.strip_prefix("name=").unwrap()).unwrap(),
            name
        );
        assert_eq!(
            decode_query_component("F05+%282%29.html").unwrap(),
            "F05 (2).html"
        );
        assert_eq!(decode_query_component("%FF"), Err(400));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_that_leaves_the_workspace_is_hidden() {
        let root = workspace();
        let outside =
            std::env::temp_dir().join(format!("a3s-workspace-outside-{}", std::process::id()));
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"s").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("secret.txt")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
        let listing = list_in(root.to_str().unwrap()).unwrap();
        assert!(listing["folders"].as_array().unwrap().is_empty());
        assert!(listing["files"].as_array().unwrap().is_empty());
        assert_eq!(
            trash_in(
                root.to_str().unwrap(),
                root.join("secret.txt").to_str().unwrap()
            ),
            Err(400)
        );
        assert!(outside.join("secret.txt").is_file());
        let entries = entries_in(root.to_str().unwrap(), "").unwrap();
        assert!(entries["entries"].as_array().unwrap().is_empty());
        assert_eq!(
            remove_in(
                root.to_str().unwrap(),
                root.join("linked").to_str().unwrap()
            ),
            Err(400)
        );
        assert!(outside.join("secret.txt").is_file());
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_manager_edits_stay_inside_one_directory() {
        let root = workspace();
        let root_text = root.to_str().unwrap();
        fs::create_dir(root.join("Notes")).unwrap();
        fs::create_dir(root.join("Notes/nested")).unwrap();
        fs::write(root.join("Notes/nested/deep.txt"), b"d").unwrap();
        fs::write(root.join(".secret"), b"s").unwrap();
        let top = entries_in(root_text, "").unwrap();
        let top_names: Vec<_> = top["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(top_names, vec!["Notes".to_owned()]);
        let notes = entries_in(root_text, root.join("Notes").to_str().unwrap()).unwrap();
        let note_names: Vec<_> = notes["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(note_names, vec!["nested".to_owned()]);
        let nested = entries_in(root_text, root.join("Notes/nested").to_str().unwrap()).unwrap();
        let nested_names: Vec<_> = nested["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["name"].as_str().unwrap().to_owned(),
                    item["kind"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            nested_names,
            vec![("deep.txt".to_owned(), "file".to_owned())]
        );
        assert_eq!(
            entries_in(
                root_text,
                root.join("Notes/nested/deep.txt").to_str().unwrap()
            ),
            Err(400)
        );
        assert_eq!(create_in(root_text, "", "../escape.txt", "file"), Err(400));
        assert_eq!(create_in(root_text, "", ".hidden.txt", "file"), Err(400));
        let created = create_in(
            root_text,
            root.join("Notes").to_str().unwrap(),
            "fresh.txt",
            "file",
        )
        .unwrap();
        assert_eq!(created["kind"], "file");
        assert!(root.join("Notes/fresh.txt").is_file());
        assert_eq!(
            create_in(
                root_text,
                root.join("Notes").to_str().unwrap(),
                "fresh.txt",
                "file"
            ),
            Err(409)
        );
        let folder = create_in(root_text, "", "Box", "folder").unwrap();
        assert_eq!(folder["kind"], "directory");
        let renamed = rename_in(
            root_text,
            root.join("Notes/fresh.txt").to_str().unwrap(),
            "kept.txt",
        )
        .unwrap();
        assert_eq!(renamed["name"], "kept.txt");
        assert!(root.join("Notes/kept.txt").is_file());
        assert_eq!(
            move_in(
                root_text,
                root.join("Notes").to_str().unwrap(),
                root.join("Notes/nested").to_str().unwrap()
            ),
            Err(400)
        );
        let moved = move_in(
            root_text,
            root.join("Notes/kept.txt").to_str().unwrap(),
            root.join("Box").to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(moved["name"], "kept.txt");
        assert!(root.join("Box/kept.txt").is_file());
        let outside =
            std::env::temp_dir().join(format!("a3s-manager-outside-{}.txt", std::process::id()));
        fs::write(&outside, b"no").unwrap();
        assert_eq!(remove_in(root_text, outside.to_str().unwrap()), Err(400));
        assert!(outside.is_file());
        remove_in(
            root_text,
            root.join("Notes/nested/deep.txt").to_str().unwrap(),
        )
        .unwrap();
        assert!(!root.join("Notes/nested/deep.txt").exists());
        remove_in(root_text, root.join("Notes/nested").to_str().unwrap()).unwrap();
        assert!(!root.join("Notes/nested").exists());
        let _ = fs::remove_file(&outside);
        let trash = home_dir().unwrap().join(".Trash");
        let _ = fs::remove_file(trash.join("deep.txt"));
        let _ = fs::remove_dir_all(trash.join("nested"));
        let _ = fs::remove_dir_all(root);
    }
}
