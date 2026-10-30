#![cfg(test)]

use super::*;

#[test]
fn bounded_artifacts_refuse_directories_oversize_and_traversal() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("valid"), b"current bytes").unwrap();
    assert_eq!(read(root.path(), "valid").unwrap(), b"current bytes");
    std::fs::create_dir(root.path().join("directory")).unwrap();
    std::fs::write(root.path().join("oversize"), vec![0; LIMIT + 1]).unwrap();
    for name in [
        "directory",
        "oversize",
        "../outside",
        "/absolute",
        "nested/file",
        "missing",
    ] {
        assert!(read(root.path(), name).is_err(), "{name}");
    }
}

#[cfg(unix)]
#[test]
fn fixture_links_and_nonregular_files_are_refused_before_reading() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("original"), b"protected").unwrap();
    std::os::unix::fs::symlink("original", root.path().join("symbolic")).unwrap();
    std::fs::hard_link(root.path().join("original"), root.path().join("hard")).unwrap();
    for name in ["symbolic", "hard", "original"] {
        assert!(read(root.path(), name).is_err(), "{name}");
    }
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket")).unwrap();
    assert!(read(root.path(), "socket").is_err());
}
