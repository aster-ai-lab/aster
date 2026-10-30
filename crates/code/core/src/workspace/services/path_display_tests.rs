#![cfg(test)]

use super::*;

#[test]
fn local_display_paths_are_actual_native_file_paths() {
    let directory = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(directory.path()).unwrap();
    std::fs::create_dir(root.join("nested")).unwrap();
    let expected = root.join("nested").join("中文 file.txt");
    std::fs::write(&expected, b"native path bytes").unwrap();
    let services = WorkspaceServices::local(root.clone());
    let path = WorkspacePath::from_normalized("nested/中文 file.txt");
    let display = services.display_path(&path);
    assert_eq!(PathBuf::from(&display), expected);
    assert_eq!(std::fs::read(&display).unwrap(), b"native path bytes");
    assert_eq!(
        services.display_path(&WorkspacePath::root()),
        root.display().to_string()
    );
}

#[test]
fn virtual_display_roots_keep_their_declared_label() {
    let directory = tempfile::tempdir().unwrap();
    let local = WorkspaceServices::local(directory.path().to_path_buf());
    let virtual_services = WorkspaceServices::builder(
        WorkspaceRef::new("virtual-owner", "workspace://display-root/"),
        local.fs(),
    )
    .build();
    assert!(virtual_services.local_root().is_none());
    assert_eq!(
        virtual_services.display_path(&WorkspacePath::from_normalized("nested/file.txt")),
        "workspace://display-root/nested/file.txt"
    );
}
