#![cfg(test)]

use super::*;
use crate::workspace::LocalWorkspaceFileStatus;

fn write(root: &Path, path: &str, bytes: &[u8]) -> LocalWorkspaceFile {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(&full, bytes).unwrap();
    entry(root, path)
}

fn entry(root: &Path, path: &str) -> LocalWorkspaceFile {
    let metadata = std::fs::metadata(root.join(path)).unwrap();
    LocalWorkspaceFile {
        path: path.into(),
        size: metadata.len(),
        modified_ms: Some(
            metadata
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        ),
        language: path.ends_with(".rs").then(|| "rust".into()),
        status: LocalWorkspaceFileStatus::Unknown,
        binary: false,
        generated: false,
    }
}

fn manifest(root: &Path, files: Vec<LocalWorkspaceFile>) -> LocalWorkspaceManifestSnapshot {
    LocalWorkspaceManifestSnapshot {
        root: root.to_path_buf(),
        files,
        version: 1,
        scanned_at_ms: 1,
    }
}

fn capture(manifest: &LocalWorkspaceManifestSnapshot) -> WorkspaceWikiSources {
    capture_workspace_wiki_sources(
        &manifest.root,
        manifest,
        "tenant-a:permission-a:binding-a",
        &[],
        WorkspaceWikiSourceLimits::default(),
        &CancellationToken::new(),
    )
    .unwrap()
}

#[test]
fn workspace_wiki_admits_binary_documents_and_images_without_making_them_text() {
    let root = tempfile::tempdir().unwrap();
    let files = [
        "scan.png",
        "scan.PDF",
        "report.doc",
        "book.xls",
        "deck.ppt",
        "work.xlsx",
        "archive.zip",
    ]
    .into_iter()
    .map(|path| {
        let mut entry = write(root.path(), path, b"binary\0snapshot");
        entry.binary = true;
        entry
    })
    .collect();
    let result = capture(&manifest(root.path(), files));
    assert_eq!(result.files().len(), 6);
    assert!(result.files().iter().all(
        |file| file.format() == WorkspaceWikiSourceFormat::Document && file.chunks().is_empty()
    ));
    assert_eq!(
        result.coverage().skipped[&WorkspaceWikiSourceSkipReason::Unsupported],
        1
    );
}

#[test]
fn workspace_wiki_sources_preserve_code_markdown_and_exact_retrieval_anchors() {
    let root = tempfile::tempdir().unwrap();
    let code = "pub fn 入口() {\r\n    println!(\"世界\");\r\n}\r\n";
    let files = vec![
        write(root.path(), "src/lib.rs", code.as_bytes()),
        write(
            root.path(),
            "README.md",
            "# 项目\n依据原始资料。\n".as_bytes(),
        ),
    ];
    let snapshot = capture(&manifest(root.path(), files));
    assert_eq!(snapshot.coverage().included_files, 2);
    assert!(snapshot.coverage().skipped.is_empty());
    let source = snapshot
        .files()
        .iter()
        .find(|file| file.path() == "src/lib.rs")
        .unwrap();
    assert_eq!(source.bytes(), code.as_bytes());
    let chunk = &source.chunks()[0];
    assert_eq!((chunk.start_line, chunk.end_line), (1, 3));
    assert_eq!(
        code.get(chunk.start_byte..chunk.end_byte),
        Some(chunk.text.as_ref())
    );
    assert_eq!(chunk.content_digest.as_ref(), source.generation());
    assert!(!format!("{snapshot:?}").contains("println!"));
    assert_eq!(
        std::fs::read(root.path().join("src/lib.rs")).unwrap(),
        code.as_bytes()
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}

#[test]
fn workspace_wiki_sources_exclude_secrets_state_dependencies_and_host_output() {
    let root = tempfile::tempdir().unwrap();
    let files = [
        ".env",
        ".a3s/config.acl",
        ".a3s/kb/sources/private.md",
        ".git/config",
        "node_modules/pkg/source.ts",
        "target/out.rs",
        "meta/a3s-session.json",
        "child-task/meta/a3s-session.json",
        "knowledge-output/overview.md",
        "src/main.rs",
        "meta/design.md",
    ]
    .into_iter()
    .map(|p| write(root.path(), p, b"fixture bytes\n"))
    .collect();
    let manifest = manifest(root.path(), files);
    let snapshot = capture_workspace_wiki_sources(
        root.path(),
        &manifest,
        "scope",
        &[WorkspacePath::from_normalized("knowledge-output")],
        WorkspaceWikiSourceLimits::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(
        snapshot
            .files()
            .iter()
            .map(|f| f.path())
            .collect::<Vec<_>>(),
        ["meta/design.md", "src/main.rs"]
    );
    assert_eq!(
        snapshot.coverage().skipped[&WorkspaceWikiSourceSkipReason::Excluded],
        9
    );
    assert_eq!(snapshot.coverage().included_bytes, 28);
}

#[cfg(unix)]
#[test]
fn workspace_wiki_sources_refuse_symlink_parents_leaves_and_hardlink_aliases() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "private.rs", b"must not be sent");
    std::os::unix::fs::symlink(outside.path(), root.path().join("linked-dir")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("private.rs"),
        root.path().join("linked.rs"),
    )
    .unwrap();
    std::fs::hard_link(
        outside.path().join("private.rs"),
        root.path().join("alias.rs"),
    )
    .unwrap();
    let snapshot = capture(&manifest(
        root.path(),
        ["linked-dir/private.rs", "linked.rs", "alias.rs"]
            .into_iter()
            .map(|p| entry(root.path(), p))
            .collect(),
    ));
    assert!(snapshot.files().is_empty());
    assert_eq!(
        snapshot.coverage().skipped[&WorkspaceWikiSourceSkipReason::Unavailable],
        3
    );
    assert!(!format!("{snapshot:?}").contains("must not be sent"));
}

#[test]
fn workspace_wiki_sources_enforce_actual_read_size_total_count_and_report_bounds() {
    let root = tempfile::tempdir().unwrap();
    let files = (0..24)
        .map(|n| write(root.path(), &format!("{n:02}.txt"), b"1234"))
        .collect();
    let manifest = manifest(root.path(), files);
    let snapshot = capture_workspace_wiki_sources(
        root.path(),
        &manifest,
        "scope",
        &[],
        WorkspaceWikiSourceLimits {
            max_files: 1,
            max_file_bytes: 4,
            max_total_bytes: 4,
        },
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(snapshot.files().len(), 1);
    assert_eq!(snapshot.coverage().included_bytes, 4);
    assert_eq!(
        snapshot.coverage().skipped[&WorkspaceWikiSourceSkipReason::Budget],
        23
    );
    assert_eq!(snapshot.coverage().examples.len(), MAX_SKIP_EXAMPLES);
    let mut forged = manifest;
    forged.files.truncate(1);
    std::fs::write(root.path().join("00.txt"), b"123456").unwrap();
    let snapshot = capture_workspace_wiki_sources(
        root.path(),
        &forged,
        "scope",
        &[],
        WorkspaceWikiSourceLimits {
            max_files: 1,
            max_file_bytes: 4,
            max_total_bytes: 4,
        },
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(snapshot.files().is_empty());
    assert_eq!(
        snapshot.coverage().skipped[&WorkspaceWikiSourceSkipReason::TooLarge],
        1
    );
}

#[test]
fn workspace_wiki_sources_identity_tracks_content_scope_and_exclusions_not_rescans() {
    let root = tempfile::tempdir().unwrap();
    let mut manifest = manifest(
        root.path(),
        vec![write(root.path(), "source.txt", b"first")],
    );
    let first = capture(&manifest);
    manifest.version = 9;
    manifest.scanned_at_ms = 100;
    assert_eq!(first.generation(), capture(&manifest).generation());
    assert_eq!(capture(&manifest).manifest_version(), 9);
    std::fs::write(root.path().join("source.txt"), b"other").unwrap();
    manifest.files[0] = entry(root.path(), "source.txt");
    assert_ne!(first.generation(), capture(&manifest).generation());
    let other_scope = capture_workspace_wiki_sources(
        root.path(),
        &manifest,
        "permission-b",
        &[],
        WorkspaceWikiSourceLimits::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_ne!(other_scope.generation(), capture(&manifest).generation());
    let excluded = capture_workspace_wiki_sources(
        root.path(),
        &manifest,
        "tenant-a:permission-a:binding-a",
        &[WorkspacePath::from_normalized("unused-output")],
        WorkspaceWikiSourceLimits::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_ne!(excluded.generation(), capture(&manifest).generation());
}

#[test]
fn workspace_wiki_sources_reject_wrong_roots_invalid_manifest_and_cancelled_capture() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let entry = write(root.path(), "source.md", b"evidence");
    let mut manifest = manifest(root.path(), vec![entry.clone()]);
    assert!(capture_workspace_wiki_sources(
        other.path(),
        &manifest,
        "scope",
        &[],
        WorkspaceWikiSourceLimits::default(),
        &CancellationToken::new()
    )
    .is_err());
    manifest.files.push(entry);
    assert!(capture_workspace_wiki_sources(
        root.path(),
        &manifest,
        "scope",
        &[],
        WorkspaceWikiSourceLimits::default(),
        &CancellationToken::new()
    )
    .is_err());
    manifest.files.pop();
    manifest.files[0].path = "../outside.md".into();
    assert!(capture_workspace_wiki_sources(
        root.path(),
        &manifest,
        "scope",
        &[],
        WorkspaceWikiSourceLimits::default(),
        &CancellationToken::new()
    )
    .is_err());
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        capture_workspace_wiki_sources(
            root.path(),
            &manifest,
            "scope",
            &[],
            WorkspaceWikiSourceLimits::default(),
            &token
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::Interrupted
    );
    assert!(WorkspaceWikiSourceFormat::parse("guessed").is_err());
    assert!(WorkspaceWikiSourceSkipReason::parse("ignored").is_err());
}

#[test]
fn workspace_wiki_sources_report_unavailable_changed_empty_and_unsupported_files() {
    let root = tempfile::tempdir().unwrap();
    let mut files = vec![
        write(root.path(), "gone.md", b"evidence"),
        write(root.path(), "changed.md", b"old"),
        write(root.path(), "empty.md", b""),
        write(root.path(), "unknown.dat", b"\xff\0"),
    ];
    files[3].binary = true;
    std::fs::remove_file(root.path().join("gone.md")).unwrap();
    std::fs::write(root.path().join("changed.md"), b"new length").unwrap();
    let snapshot = capture(&manifest(root.path(), files));
    assert!(snapshot.files().is_empty());
    for reason in [
        WorkspaceWikiSourceSkipReason::Unavailable,
        WorkspaceWikiSourceSkipReason::Changed,
        WorkspaceWikiSourceSkipReason::Empty,
        WorkspaceWikiSourceSkipReason::Unsupported,
    ] {
        assert_eq!(snapshot.coverage().skipped[&reason], 1);
    }
}

#[test]
fn workspace_wiki_sources_keep_document_bytes_without_inventing_source_lines() {
    let root = tempfile::tempdir().unwrap();
    let mut pdf = write(root.path(), "report.pdf", b"%PDF-1.4\nfixture\0");
    pdf.binary = true;
    let snapshot = capture(&manifest(root.path(), vec![pdf]));
    assert_eq!(
        snapshot.files()[0].format(),
        WorkspaceWikiSourceFormat::Document
    );
    assert!(snapshot.files()[0].chunks().is_empty());
    assert_eq!(snapshot.files()[0].bytes(), b"%PDF-1.4\nfixture\0");
}

#[test]
fn workspace_wiki_sources_output_changes_do_not_invalidate_the_input_generation() {
    let root = tempfile::tempdir().unwrap();
    let source = write(root.path(), "src/lib.rs", b"pub fn source() {}\n");
    let capture = |files| {
        capture_workspace_wiki_sources(
            root.path(),
            &manifest(root.path(), files),
            "tenant:permission:binding",
            &[WorkspacePath::from_normalized("wiki")],
            WorkspaceWikiSourceLimits::default(),
            &CancellationToken::new(),
        )
        .unwrap()
    };
    let before = capture(vec![source.clone()]);
    let output = write(
        root.path(),
        "wiki/workspace/concepts/overview.md",
        b"Generated page\n",
    );
    let after = capture(vec![source.clone(), output]);
    assert_eq!(after.generation(), before.generation());
    assert_eq!(after.coverage().included_files, 1);
    assert_eq!(
        after.coverage().skipped[&WorkspaceWikiSourceSkipReason::Excluded],
        1
    );
    let changed_output = write(
        root.path(),
        "wiki/workspace/concepts/overview.md",
        b"Regenerated page with a different size\n",
    );
    let repeated = capture(vec![source, changed_output]);
    assert_eq!(repeated.generation(), before.generation());
    let changed_source = write(root.path(), "src/lib.rs", b"pub fn changed_source() {}\n");
    assert_ne!(
        capture(vec![changed_source]).generation(),
        before.generation()
    );
}
