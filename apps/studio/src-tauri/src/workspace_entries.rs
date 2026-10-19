//! Directories and files under one workspace's `projects/` and `sessions/` trees.
//! A path outside that root, a filesystem root, `..`, or a symlink that leaves the root is rejected.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::json;

const WRITE_LIMIT: usize = 256 * 1024;
const SEGMENT_LIMIT: usize = 255;

#[derive(Deserialize)]
struct EntryBody {
    op: String,
    root: String,
    path: String,
    to: Option<String>,
    data: Option<String>,
}

pub fn apply_json(body: &[u8]) -> Result<String, u16> {
    let parsed: EntryBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let path = match parsed.op.as_str() {
        "mkdir" => mkdir(&parsed.root, &parsed.path)?,
        "move" => move_dir(
            &parsed.root,
            &parsed.path,
            parsed.to.as_deref().unwrap_or(""),
        )?,
        "write" => write_file(
            &parsed.root,
            &parsed.path,
            parsed.data.as_deref().unwrap_or(""),
        )?,
        _ => return Err(400),
    };
    Ok(json!({ "ok": true, "path": path }).to_string())
}

fn mkdir(raw_root: &str, raw_path: &str) -> Result<String, u16> {
    let declared = bounded_path(raw_root)?;
    let root = prepare_root(&declared)?;
    if let Ok(segments) = entry_segments(&declared, raw_path) {
        let path = materialize(&root, &segments, true)?;
        return Ok(path.to_string_lossy().into_owned());
    }
    // A project can live outside the app workspace. Creating one there is
    // refused; an existing real directory is already the parent the move needs.
    existing_directory(raw_path)
}

fn move_dir(raw_root: &str, raw_from: &str, raw_to: &str) -> Result<String, u16> {
    let declared = bounded_path(raw_root)?;
    let root = prepare_root(&declared)?;
    match (
        entry_segments(&declared, raw_from),
        entry_segments(&declared, raw_to),
    ) {
        (Ok(from), Ok(to)) => rename_entry(&root, &from, &to),
        (Ok(from), Err(_)) => place_session_outside(&root, &from, raw_to),
        (Err(_), Ok(to)) => take_session_inside(&root, raw_from, &to),
        (Err(code), Err(_)) => Err(code),
    }
}

fn rename_entry(root: &Path, from: &[String], to: &[String]) -> Result<String, u16> {
    let source = materialize(root, from, false)?;
    if !source.is_dir() {
        return Err(400);
    }
    let parent = materialize(root, &to[..to.len() - 1], false)?;
    let name = to.last().ok_or(400u16)?;
    let dest = parent.join(name);
    if symlink_metadata_exists(&dest) {
        return Err(409);
    }
    fs::rename(&source, &dest).map_err(|_| 500u16)?;
    Ok(dest.to_string_lossy().into_owned())
}

/// Move `sessions/<day>/<id>` or `projects/<name>/<id>` into an existing project
/// directory that sits outside the app workspace. The folder keeps its name.
fn place_session_outside(root: &Path, from: &[String], raw_to: &str) -> Result<String, u16> {
    if from.len() < 3 {
        return Err(400);
    }
    let source = materialize(root, from, false)?;
    if !source.is_dir() {
        return Err(400);
    }
    let dest = session_dest_outside(raw_to, from.last().ok_or(400u16)?)?;
    refuse_nested(&source, dest.parent().ok_or(400u16)?)?;
    if symlink_metadata_exists(&dest) {
        return Err(409);
    }
    fs::rename(&source, &dest).map_err(|_| 500u16)?;
    Ok(dest.to_string_lossy().into_owned())
}

/// Move `<project>/<id>` back under the app workspace. The folder keeps its name.
fn take_session_inside(root: &Path, raw_from: &str, to: &[String]) -> Result<String, u16> {
    if to.len() < 3 {
        return Err(400);
    }
    let name = to.last().ok_or(400u16)?;
    let source = session_dest_outside(raw_from, name)?;
    let parent = materialize(root, &to[..to.len() - 1], false)?;
    let dest = parent.join(name);
    refuse_nested(&source, &parent)?;
    if symlink_metadata_exists(&dest) {
        return Err(409);
    }
    fs::rename(&source, &dest).map_err(|_| 500u16)?;
    Ok(dest.to_string_lossy().into_owned())
}

fn session_dest_outside(raw: &str, name: &str) -> Result<PathBuf, u16> {
    let dest = bounded_path(raw)?;
    if dest.file_name().and_then(|item| item.to_str()) != Some(name) {
        return Err(400);
    }
    let parent = dest
        .parent()
        .filter(|parent| !is_filesystem_root(parent))
        .ok_or(400u16)?;
    require_real_dir(parent)?;
    Ok(dest)
}

fn existing_directory(raw_path: &str) -> Result<String, u16> {
    let path = bounded_path(raw_path)?;
    if is_filesystem_root(&path) {
        return Err(400);
    }
    require_real_dir(&path)?;
    Ok(path.to_string_lossy().into_owned())
}

fn require_real_dir(path: &Path) -> Result<(), u16> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => Err(400),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(404),
        Err(_) => Err(500),
    }
}

fn refuse_nested(source: &Path, parent: &Path) -> Result<(), u16> {
    let source = fs::canonicalize(source).map_err(|_| 400u16)?;
    let parent = fs::canonicalize(parent).map_err(|_| 400u16)?;
    if parent.starts_with(&source) {
        return Err(400);
    }
    Ok(())
}

fn write_file(raw_root: &str, raw_path: &str, data: &str) -> Result<String, u16> {
    if data.len() > WRITE_LIMIT {
        return Err(413);
    }
    let (root, segments) = open_entry(raw_root, raw_path)?;
    let parent = materialize(&root, &segments[..segments.len() - 1], false)?;
    let name = segments.last().ok_or(400u16)?;
    let dest = parent.join(name);
    match fs::symlink_metadata(&dest) {
        Ok(meta) if meta.is_dir() || meta.file_type().is_symlink() => return Err(400),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err(500),
    }
    let temporary = parent.join(format!(".{name}.a3s-tmp"));
    fs::write(&temporary, data).map_err(|_| 500u16)?;
    if fs::rename(&temporary, &dest).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(500);
    }
    Ok(dest.to_string_lossy().into_owned())
}

fn open_entry(raw_root: &str, raw_path: &str) -> Result<(PathBuf, Vec<String>), u16> {
    let declared = bounded_path(raw_root)?;
    let root = prepare_root(&declared)?;
    let segments = entry_segments(&declared, raw_path)?;
    Ok((root, segments))
}

fn prepare_root(declared: &Path) -> Result<PathBuf, u16> {
    if is_filesystem_root(declared) {
        return Err(400);
    }
    if declared.exists() {
        let canon = fs::canonicalize(declared).map_err(|_| 400u16)?;
        if !canon.is_dir() || is_filesystem_root(&canon) {
            return Err(400);
        }
        return Ok(canon);
    }
    if !is_default_workspace(declared) {
        return Err(400);
    }
    fs::create_dir_all(declared).map_err(|_| 500u16)?;
    fs::canonicalize(declared).map_err(|_| 500u16)
}

fn is_default_workspace(path: &Path) -> bool {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return false;
    };
    path == home.join(".a3s")
}

fn entry_segments(root: &Path, raw_path: &str) -> Result<Vec<String>, u16> {
    let path = bounded_path(raw_path)?;
    let root_text = slash(root);
    let path_text = slash(&path);
    let prefix = format!("{root_text}/");
    if !path_text.starts_with(&prefix) {
        return Err(400);
    }
    let segments: Vec<String> = path_text[prefix.len()..]
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect();
    if segments.len() < 2 || (segments[0] != "projects" && segments[0] != "sessions") {
        return Err(400);
    }
    if segments.iter().any(|segment| !segment_ok(segment)) {
        return Err(400);
    }
    Ok(segments)
}

fn materialize(root: &Path, segments: &[String], create: bool) -> Result<PathBuf, u16> {
    if segments.is_empty() {
        return Err(400);
    }
    let mut current = root.to_path_buf();
    for segment in segments {
        let next = current.join(segment);
        match fs::symlink_metadata(&next) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let canon = fs::canonicalize(&next).map_err(|_| 400u16)?;
                if !canon.starts_with(root) || !canon.is_dir() {
                    return Err(400);
                }
                current = canon;
            }
            Ok(meta) if meta.is_dir() => current = next,
            Ok(_) => return Err(400),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                if !create {
                    return Err(404);
                }
                fs::create_dir(&next).map_err(|_| 500u16)?;
                current = next;
            }
            Err(_) => return Err(500),
        }
    }
    let canon = fs::canonicalize(&current).map_err(|_| 500u16)?;
    if !canon.starts_with(root) {
        return Err(400);
    }
    Ok(canon)
}

fn segment_ok(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= SEGMENT_LIMIT
        && segment != "."
        && segment != ".."
        && !segment.starts_with('.')
        && !segment.ends_with('.')
        && !segment.ends_with(' ')
        && !segment.chars().any(|ch| {
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '/' | '\\')
        })
}

fn bounded_path(raw: &str) -> Result<PathBuf, u16> {
    let trimmed = raw.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed.len() > 4096 || trimmed.contains('\0') {
        return Err(400);
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| component == Component::ParentDir)
    {
        return Err(400);
    }
    Ok(path)
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_filesystem_root(path: &Path) -> bool {
    path.parent().is_none() || path == Path::new("/") || path == Path::new("\\")
}

fn symlink_metadata_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "a3s-workspace-entry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn mkdir_creates_a_project_and_rejects_escape() {
        let root = scratch();
        let project = root.join("projects/建设方案");
        let created = mkdir(&root.to_string_lossy(), &project.to_string_lossy()).unwrap();
        assert!(Path::new(&created).is_dir());
        assert!(
            mkdir(
                &root.to_string_lossy(),
                &root.join("projects/../.ssh").to_string_lossy()
            )
            .is_err()
        );
        assert!(
            mkdir(
                &root.to_string_lossy(),
                &root.join(".a3s").to_string_lossy()
            )
            .is_err()
        );
        assert!(
            mkdir(
                "/tmp/does-not-exist-a3s-workspace",
                "/tmp/does-not-exist-a3s-workspace/projects/a"
            )
            .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_that_leaves_the_workspace_is_rejected() {
        let root = scratch();
        let outside = scratch();
        fs::create_dir_all(root.join("projects")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("projects/link")).unwrap();
        let escaped = root.join("projects/link/secret");
        assert!(mkdir(&root.to_string_lossy(), &escaped.to_string_lossy()).is_err());
        assert!(!outside.join("secret").exists());
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_and_move_stay_inside_the_workspace() {
        let root = scratch();
        let session = root.join("sessions/2026-10-02/session-1/meta");
        mkdir(&root.to_string_lossy(), &session.to_string_lossy()).unwrap();
        let file = session.join("a3s-session.json");
        write_file(
            &root.to_string_lossy(),
            &file.to_string_lossy(),
            "{\"id\":\"session-1\"}\n",
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "{\"id\":\"session-1\"}\n"
        );
        let project = root.join("projects/建设方案");
        mkdir(&root.to_string_lossy(), &project.to_string_lossy()).unwrap();
        let from = root.join("sessions/2026-10-02/session-1");
        let to = project.join("session-1");
        move_dir(
            &root.to_string_lossy(),
            &from.to_string_lossy(),
            &to.to_string_lossy(),
        )
        .unwrap();
        assert!(to.join("meta/a3s-session.json").is_file());
        assert!(
            move_dir(
                &root.to_string_lossy(),
                &from.to_string_lossy(),
                &to.to_string_lossy()
            )
            .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn moves_a_session_into_an_outside_project_and_back() {
        let root = scratch();
        let project = scratch();
        let session = root.join("sessions/2026-10-03/session-1");
        mkdir(&root.to_string_lossy(), &session.to_string_lossy()).unwrap();
        fs::write(session.join("note.txt"), "hi").unwrap();
        let root_text = root.to_string_lossy().into_owned();
        let project_text = project.to_string_lossy().into_owned();
        assert_eq!(mkdir(&root_text, &project_text).unwrap(), project_text);
        assert!(mkdir(&root_text, &project.join("missing").to_string_lossy()).is_err());

        let dest = project.join("session-1");
        move_dir(
            &root_text,
            &session.to_string_lossy(),
            &dest.to_string_lossy(),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(dest.join("note.txt")).unwrap(), "hi");
        assert!(!session.exists());
        assert!(
            move_dir(
                &root_text,
                &session.to_string_lossy(),
                &project.join("other-name").to_string_lossy()
            )
            .is_err()
        );

        #[cfg(unix)]
        {
            let second = root.join("sessions/2026-10-03/session-2");
            mkdir(&root_text, &second.to_string_lossy()).unwrap();
            let link = scratch();
            let _ = fs::remove_dir_all(&link);
            std::os::unix::fs::symlink(&project, &link).unwrap();
            assert!(
                move_dir(
                    &root_text,
                    &second.to_string_lossy(),
                    &link.join("session-2").to_string_lossy()
                )
                .is_err()
            );
            let _ = fs::remove_dir_all(&link);
        }

        let inbox = root.join("sessions/2026-10-04/session-1");
        mkdir(
            &root_text,
            &root.join("sessions/2026-10-04").to_string_lossy(),
        )
        .unwrap();
        move_dir(
            &root_text,
            &dest.to_string_lossy(),
            &inbox.to_string_lossy(),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(inbox.join("note.txt")).unwrap(), "hi");
        assert!(!dest.exists());
        assert!(
            move_dir(
                &root_text,
                &root.join("projects/only").to_string_lossy(),
                &project.join("only").to_string_lossy()
            )
            .is_err()
        );
        let _ = fs::remove_dir_all(project);
        let _ = fs::remove_dir_all(root);
    }
}
