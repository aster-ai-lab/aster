#![cfg(test)]

#[path = "../src/host_family.rs"]
mod host_family;

#[path = "../src/local_documents.rs"]
mod local_documents;
#[path = "../src/workspace_file.rs"]
mod workspace_file;

#[test]
fn result_import_preserves_existing_documents_and_publishes_exact_binary_bytes() {
    let root = std::env::temp_dir().join(format!("a3s-result-import-{}", uuid::Uuid::new_v4()));
    let folder = root.join("项目资料").join("导出");
    std::fs::create_dir_all(&folder).unwrap();
    let source = include_bytes!("../../tests/e2e/document-tools/fixtures/formatted-layout.docx");
    let root_path = root.to_string_lossy();
    let folder_path = folder.to_string_lossy();
    workspace_file::import_new_bytes(&root_path, &folder_path, "result.docx", source).unwrap();
    assert_eq!(std::fs::read(folder.join("result.docx")).unwrap(), source);
    assert_eq!(
        workspace_file::import_new_bytes(&root_path, &folder_path, "result.docx", b"replacement"),
        Err(409)
    );
    assert_eq!(std::fs::read(folder.join("result.docx")).unwrap(), source);
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
    assert_eq!(
        workspace_file::import_new_bytes(&root_path, &folder_path, "../escape.pdf", source),
        Err(400)
    );
    std::fs::remove_dir_all(&root).unwrap();
}
