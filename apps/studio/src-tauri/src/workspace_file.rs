//! Read, and replace, one file or folder inside a workspace for the file preview page.
//! The canonical target must stay inside the canonical workspace; symlinks that leave it are rejected.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde_json::json;

use crate::local_documents::canonical_workspace;

const TEXT_LIMIT: u64 = 10 * 1024 * 1024;
pub const RAW_LIMIT: u64 = 64 * 1024 * 1024;
const SNIFF_BYTES: usize = 8192;
const UNSAFE_OPEN_EXTENSIONS: &[&str] = &[
    "app",
    "command",
    "sh",
    "zsh",
    "bash",
    "csh",
    "fish",
    "tool",
    "terminal",
    "workflow",
    "scpt",
    "applescript",
    "pkg",
    "mpkg",
    "dmg",
    "exe",
    "bat",
    "cmd",
    "com",
    "msi",
    "ps1",
    "jar",
    "vbs",
];

#[derive(Deserialize)]
struct RootPathBody {
    root: String,
    path: String,
}

pub fn metadata_json(raw_root: &str, raw_path: &str) -> Result<String, u16> {
    let target = contained(raw_root, raw_path)?;
    let meta = fs::metadata(&target).map_err(|_| 404u16)?;
    let millis = |time: std::io::Result<std::time::SystemTime>| {
        time.ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    };
    let modified = millis(meta.modified());
    let created = match millis(meta.created()) {
        0 => modified,
        value => value,
    };
    if meta.is_dir() {
        return Ok(json!({
            "kind": "directory",
            "size": 0,
            "createdAt": created,
            "modifiedAt": modified,
        })
        .to_string());
    }
    Ok(json!({
        "kind": "file",
        "size": meta.len(),
        "createdAt": created,
        "modifiedAt": modified,
        "isText": looks_like_text(&target),
    })
    .to_string())
}

pub fn text_json(raw_root: &str, raw_path: &str) -> Result<String, u16> {
    let target = contained(raw_root, raw_path)?;
    let meta = fs::metadata(&target).map_err(|_| 404u16)?;
    if !meta.is_file() {
        return Err(400);
    }
    if meta.len() > TEXT_LIMIT {
        return Err(413);
    }
    let bytes = fs::read(&target).map_err(|_| 500u16)?;
    if bytes.contains(&0) {
        return Err(415);
    }
    let text = String::from_utf8(bytes).map_err(|_| 415u16)?;
    Ok(json!({ "text": text }).to_string())
}

pub fn raw_bytes(raw_root: &str, raw_path: &str) -> Result<Vec<u8>, u16> {
    let target = contained(raw_root, raw_path)?;
    let meta = fs::metadata(&target).map_err(|_| 404u16)?;
    if !meta.is_file() {
        return Err(400);
    }
    if meta.len() > RAW_LIMIT {
        return Err(413);
    }
    fs::read(&target).map_err(|_| 500u16)
}

#[cfg(any(test, not(unix)))]
pub fn write_bytes(raw_root: &str, raw_path: &str, bytes: &[u8]) -> Result<(), u16> {
    if bytes.len() as u64 > RAW_LIMIT {
        return Err(413);
    }
    let target = contained(raw_root, raw_path)?;
    let meta = fs::symlink_metadata(&target).map_err(|_| 404u16)?;
    if !meta.is_file() {
        return Err(400);
    }
    let parent = target.parent().ok_or(400u16)?;
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(400u16)?;
    let temporary = parent.join(format!(".{name}.{}.a3s-tmp", uuid::Uuid::new_v4()));
    if fs::write(&temporary, bytes).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(500);
    }
    if fs::rename(&temporary, &target).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(500);
    }
    Ok(())
}

/// Writes a dropped file into an existing workspace directory. An existing file is replaced.
pub fn import_bytes(
    raw_root: &str,
    raw_directory: &str,
    name: &str,
    bytes: &[u8],
) -> Result<String, u16> {
    import_bytes_with_policy(raw_root, raw_directory, name, bytes, true)
}

/// Atomically publishes a new result without replacing an existing document.
pub fn import_new_bytes(
    raw_root: &str,
    raw_directory: &str,
    name: &str,
    bytes: &[u8],
) -> Result<String, u16> {
    import_bytes_with_policy(raw_root, raw_directory, name, bytes, false)
}

fn import_bytes_with_policy(
    raw_root: &str,
    raw_directory: &str,
    name: &str,
    bytes: &[u8],
    replace: bool,
) -> Result<String, u16> {
    if bytes.len() as u64 > RAW_LIMIT {
        return Err(413);
    }
    if name.is_empty()
        || name.len() > 255
        || name.contains('\0')
        || name.contains('/')
        || name.contains('\\')
        || name.starts_with('.')
        || crate::local_documents::reserved_bundle_name(name)
    {
        return Err(400);
    }
    let root = crate::local_documents::canonical_workspace(raw_root)?;
    let directory = import_directory(&root, raw_directory)?;
    let dest = directory.join(name);
    match fs::symlink_metadata(&dest) {
        Ok(meta) if meta.file_type().is_symlink() || meta.is_dir() => return Err(400),
        Ok(_) if !replace => return Err(409),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(500),
    }
    let mut temporary = tempfile::NamedTempFile::new_in(&directory).map_err(|_| 500u16)?;
    temporary.write_all(bytes).map_err(|_| 500u16)?;
    let publish = if replace {
        temporary.persist(&dest)
    } else {
        temporary.persist_noclobber(&dest)
    };
    if let Err(error) = publish {
        return Err(if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            409
        } else {
            500
        });
    }
    let canon = fs::canonicalize(&dest).map_err(|_| 500u16)?;
    if canon.parent() != Some(directory.as_path()) {
        return Err(400);
    }
    let placed = if replace {
        crate::local_documents::place_by_heading(&root, &canon).unwrap_or(canon)
    } else {
        canon
    };
    Ok(placed.to_string_lossy().into_owned())
}

fn import_directory(root: &Path, raw_directory: &str) -> Result<PathBuf, u16> {
    if raw_directory.is_empty() {
        return Ok(root.to_path_buf());
    }
    if raw_directory.len() > 4096
        || raw_directory.contains('\0')
        || PathBuf::from(raw_directory)
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(400);
    }
    let canon = fs::canonicalize(raw_directory).map_err(|_| 404u16)?;
    if !canon.is_dir() || !(canon == *root || canon.starts_with(root)) {
        return Err(400);
    }
    Ok(canon)
}

pub fn open_json(body: &[u8]) -> Result<String, u16> {
    let parsed: RootPathBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let target = contained(&parsed.root, &parsed.path)?;
    if !target.is_file() {
        return Err(400);
    }
    if is_unsafe_to_open(&target) {
        return Err(403);
    }
    spawn_system_open(&target, false)?;
    Ok(json!({ "ok": true }).to_string())
}

pub fn reveal_json(body: &[u8]) -> Result<String, u16> {
    let parsed: RootPathBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let target = contained(&parsed.root, &parsed.path)?;
    spawn_system_open(&target, true)?;
    Ok(json!({ "ok": true }).to_string())
}

fn contained(raw_root: &str, raw_path: &str) -> Result<PathBuf, u16> {
    let root = canonical_workspace(raw_root)?;
    let path = PathBuf::from(raw_path);
    if raw_path.is_empty()
        || raw_path.len() > 4096
        || raw_path.contains('\0')
        || !path.is_absolute()
        || path
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(400);
    }
    let canon = fs::canonicalize(&path).map_err(|_| 404u16)?;
    if !canon.starts_with(&root) {
        return Err(403);
    }
    Ok(canon)
}

fn looks_like_text(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut buffer = [0u8; SNIFF_BYTES];
    let Ok(read) = file.read(&mut buffer) else {
        return false;
    };
    let sample = &buffer[..read];
    if sample.contains(&0) {
        return false;
    }
    match std::str::from_utf8(sample) {
        Ok(_) => true,
        // A multi-byte character cut by the sniff window is still text.
        Err(error) => error.error_len().is_none(),
    }
}

fn is_unsafe_to_open(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| UNSAFE_OPEN_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn spawn_system_open(path: &Path, reveal: bool) -> Result<(), u16> {
    let launch =
        crate::host_family::open_path_launch(crate::host_family::current_family(), path, reveal);
    let status = crate::host_family::spawn_launch(&launch).map_err(|_| 500u16)?;
    if status.success() { Ok(()) } else { Err(500) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn workspace(tag: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("a3s-workspace-file-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        fs::canonicalize(path).unwrap()
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn reports_text_binary_and_directory_metadata() {
        let root = workspace("meta");
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"a\"\n").unwrap();
        fs::write(root.join("blob.bin"), [0u8, 159, 146, 150]).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();

        let toml: Value = serde_json::from_str(
            &metadata_json(&text(&root), &text(&root.join("Cargo.toml"))).unwrap(),
        )
        .unwrap();
        assert_eq!(toml["kind"], "file");
        assert_eq!(toml["isText"], true);
        assert_eq!(toml["size"], 21);

        let blob: Value = serde_json::from_str(
            &metadata_json(&text(&root), &text(&root.join("blob.bin"))).unwrap(),
        )
        .unwrap();
        assert_eq!(blob["isText"], false);

        let dir: Value =
            serde_json::from_str(&metadata_json(&text(&root), &text(&root.join("src"))).unwrap())
                .unwrap();
        assert_eq!(dir["kind"], "directory");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reads_utf8_text_and_rejects_binary() {
        let root = workspace("text");
        fs::write(root.join("说明.md"), "# 标题\n").unwrap();
        fs::write(root.join("blob.bin"), [1u8, 0, 2]).unwrap();

        let body: Value =
            serde_json::from_str(&text_json(&text(&root), &text(&root.join("说明.md"))).unwrap())
                .unwrap();
        assert_eq!(body["text"], "# 标题\n");
        assert_eq!(
            text_json(&text(&root), &text(&root.join("blob.bin"))),
            Err(415)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_paths_outside_the_workspace() {
        let root = workspace("escape");
        let outside = workspace("escape-outside");
        fs::write(outside.join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt")).unwrap();

        assert_eq!(
            metadata_json(&text(&root), &text(&outside.join("secret.txt"))),
            Err(403)
        );
        assert_eq!(
            text_json(&text(&root), &text(&root.join("link.txt"))),
            Err(403)
        );
        assert_eq!(
            text_json(&text(&root), &format!("{}/../secret.txt", text(&root))),
            Err(400)
        );
        assert_eq!(text_json(&text(&root), "relative.txt"), Err(400));
        assert_eq!(
            text_json(&text(&root), &text(&root.join("missing.txt"))),
            Err(404)
        );
        let _ = fs::remove_dir_all(outside);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn replaces_file_bytes_inside_the_workspace() {
        let root = workspace("write");
        let file = root.join("notes.txt");
        fs::write(&file, b"old").unwrap();
        write_bytes(&text(&root), &text(&file), b"new bytes").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"new bytes");
        assert_eq!(
            write_bytes(&text(&root), &text(&root.join("missing.txt")), b"x"),
            Err(404)
        );
        fs::create_dir_all(root.join("dir")).unwrap();
        assert_eq!(
            write_bytes(&text(&root), &text(&root.join("dir")), b"x"),
            Err(400)
        );
        let outside = workspace("write-outside");
        fs::write(outside.join("secret.txt"), b"secret").unwrap();
        assert_eq!(
            write_bytes(&text(&root), &text(&outside.join("secret.txt")), b"x"),
            Err(403)
        );
        assert_eq!(fs::read(outside.join("secret.txt")).unwrap(), b"secret");
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn imports_a_new_file_and_refuses_to_leave_the_workspace() {
        let root = workspace("import");
        let nested = root.join("box");
        fs::create_dir_all(&nested).unwrap();
        let path = import_bytes(&text(&root), &text(&nested), "note.txt", b"hello").unwrap();
        assert_eq!(fs::read(nested.join("note.txt")).unwrap(), b"hello");
        assert!(path.ends_with("note.txt"));
        import_bytes(&text(&root), &text(&nested), "note.txt", b"again").unwrap();
        assert_eq!(fs::read(nested.join("note.txt")).unwrap(), b"again");
        assert_eq!(
            import_bytes(&text(&root), &text(&nested), ".secret", b"x"),
            Err(400)
        );
        assert_eq!(
            import_bytes(&text(&root), &text(&nested), "wiki", b"x"),
            Err(400)
        );
        assert_eq!(
            import_bytes(&text(&root), &text(&nested), "../note.txt", b"x"),
            Err(400)
        );
        fs::create_dir_all(nested.join("dir")).unwrap();
        assert_eq!(
            import_bytes(&text(&root), &text(&nested), "dir", b"x"),
            Err(400)
        );
        let outside = workspace("import-outside");
        assert_eq!(
            import_bytes(&text(&root), &text(&outside), "note.txt", b"x"),
            Err(400)
        );
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn new_import_does_not_write_through_a_preexisting_temporary_symlink() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let original = outside.path().join("original.bin");
        fs::write(&original, b"original bytes").unwrap();
        let planted = root.path().join(".result.bin.a3s-tmp");
        std::os::unix::fs::symlink(&original, &planted).unwrap();
        let saved = import_new_bytes(
            &text(root.path()),
            &text(root.path()),
            "result.bin",
            b"exported bytes",
        );
        assert_eq!(fs::read(&original).unwrap(), b"original bytes");
        assert_eq!(fs::read(saved.unwrap()).unwrap(), b"exported bytes");
        assert!(
            fs::symlink_metadata(planted)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn concurrent_new_imports_publish_one_complete_winner_without_replacing_it() {
        let root = tempfile::tempdir().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let jobs: Vec<_> = [17u8, 238]
            .into_iter()
            .map(|value| {
                let path = text(root.path());
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let bytes = vec![value; 100_000];
                    barrier.wait();
                    (value, import_new_bytes(&path, &path, "result.bin", &bytes))
                })
            })
            .collect();
        let results: Vec<_> = jobs.into_iter().map(|job| job.join().unwrap()).collect();
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_ok()).count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|(_, result)| *result == Err(409))
                .count(),
            1
        );
        let winner = results.iter().find(|(_, result)| result.is_ok()).unwrap().0;
        assert!(
            fs::read(root.path().join("result.bin")).unwrap() == vec![winner; 100_000],
            "published bytes differ from the winning import"
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn new_import_keeps_the_selected_directory_even_when_a_heading_matches_a_folder() {
        let root = tempfile::tempdir().unwrap();
        let other = root.path().join("Reports");
        fs::create_dir(&other).unwrap();
        let saved = import_new_bytes(
            &text(root.path()),
            &text(root.path()),
            "notes.md",
            b"# Reports\nbody",
        )
        .unwrap();
        assert_eq!(
            PathBuf::from(saved),
            root.path().canonicalize().unwrap().join("notes.md")
        );
        assert!(!other.join("notes.md").exists());
    }

    #[test]
    fn blocks_opening_executables() {
        assert!(is_unsafe_to_open(Path::new("/tmp/install.command")));
        assert!(is_unsafe_to_open(Path::new("/tmp/Run.APP")));
        assert!(!is_unsafe_to_open(Path::new("/tmp/report.pdf")));
    }
}
