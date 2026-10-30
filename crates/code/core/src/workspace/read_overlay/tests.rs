#![cfg(test)]

use super::super::{WorkspaceRef, WorkspaceServices, WorkspaceServicesBuilder};
use super::*;

struct Buffers;

#[tokio::test]
async fn sdk_snapshot_normalizes_paths_and_preserves_backend_fallback() {
    let snapshot = WorkspaceReadOverlaySnapshot::new(HashMap::from([(
        "./dir/open.txt".into(),
        "unsaved\n".into(),
    )]))
    .unwrap();
    assert_eq!(
        snapshot
            .read_text(&WorkspacePath::from_normalized("dir/open.txt"))
            .await
            .as_deref(),
        Some("unsaved\n")
    );
    assert!(snapshot
        .read_text(&WorkspacePath::from_normalized("other.txt"))
        .await
        .is_none());
}

#[test]
fn sdk_snapshot_refuses_escaping_duplicate_root_and_oversized_files() {
    for path in ["../outside.txt", "/outside.txt", "C:\\outside.txt", "."] {
        assert!(
            WorkspaceReadOverlaySnapshot::new(HashMap::from([(path.into(), "x".into())])).is_err(),
            "{path}"
        );
    }
    assert!(WorkspaceReadOverlaySnapshot::new(HashMap::from([
        ("a.txt".into(), "a".into()),
        ("./a.txt".into(), "b".into()),
    ]))
    .is_err());
    assert!(WorkspaceReadOverlaySnapshot::new(HashMap::from([(
        "large.txt".into(),
        "x".repeat(super::super::local::MAX_TEXT_FILE_BYTES as usize + 1)
    ),]))
    .is_err());
}

#[async_trait]
impl WorkspaceReadOverlay for Buffers {
    async fn read_text(&self, path: &WorkspacePath) -> Option<String> {
        (path.as_str() == "open.txt").then(|| "unsaved one\nunsaved two\nunsaved three\n".into())
    }
}

#[tokio::test]
async fn overlay_reads_win_and_other_operations_use_the_backend() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("open.txt"), "saved\n").unwrap();
    std::fs::write(temp.path().join("closed.txt"), "closed on disk\n").unwrap();
    let services = WorkspaceServices::local(temp.path()).with_read_overlay(Arc::new(Buffers));
    let open = services.normalize_path("open.txt").unwrap();
    let closed = services.normalize_path("closed.txt").unwrap();

    assert_eq!(
        services.fs().read_text(&open).await.unwrap(),
        "unsaved one\nunsaved two\nunsaved three\n"
    );
    assert_eq!(
        services.fs().read_text(&closed).await.unwrap(),
        "closed on disk\n"
    );
    let (edit_view, _) = services.read_for_edit(&open).await.unwrap();
    assert!(edit_view.starts_with("unsaved one"));

    let reader = services.text_reader().unwrap();
    let range = reader.read_text_range(&open, 1, 1).await.unwrap();
    assert_eq!(range.lines, vec!["unsaved two".to_string()]);
    assert_eq!(range.next_offset, Some(2));
    assert_eq!(range.total_lines, Some(3));
    let range = reader.read_text_range(&closed, 0, 10).await.unwrap();
    assert_eq!(range.lines, vec!["closed on disk".to_string()]);

    services.fs().write_text(&open, "written\n").await.unwrap();
    assert_eq!(
        std::fs::read_to_string(temp.path().join("open.txt")).unwrap(),
        "written\n"
    );
    let names = services
        .fs()
        .list_dir(&WorkspacePath::root())
        .await
        .unwrap()
        .into_iter()
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert!(names.contains(&"closed.txt".to_string()));
}

struct CountingFs {
    reads: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl WorkspaceFileSystem for CountingFs {
    async fn read_text(&self, _path: &WorkspacePath) -> WorkspaceResult<String> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok("stale disk bytes".into())
    }

    async fn write_text(
        &self,
        _path: &WorkspacePath,
        _content: &str,
    ) -> WorkspaceResult<WorkspaceWriteOutcome> {
        Err(WorkspaceError::Unsupported("test write".into()))
    }

    async fn list_dir(&self, _path: &WorkspacePath) -> WorkspaceResult<Vec<WorkspaceDirEntry>> {
        Ok(Vec::new())
    }
}

struct FailingOverlay(WorkspaceReadOverlayErrorKind);

#[async_trait]
impl WorkspaceReadOverlay for FailingOverlay {
    async fn read_text(&self, _path: &WorkspacePath) -> Option<String> {
        None
    }

    async fn read_text_result(
        &self,
        _path: &WorkspacePath,
    ) -> Result<Option<String>, WorkspaceReadOverlayError> {
        Err(WorkspaceReadOverlayError::new(self.0, "host read failed"))
    }
}

fn failing_services(
    kind: WorkspaceReadOverlayErrorKind,
) -> (Arc<WorkspaceServices>, Arc<std::sync::atomic::AtomicUsize>) {
    let reads = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fs: Arc<dyn WorkspaceFileSystem> = Arc::new(CountingFs {
        reads: Arc::clone(&reads),
    });
    let services = WorkspaceServicesBuilder::new(WorkspaceRef::new("test", "test"), fs)
        .build()
        .with_read_overlay(Arc::new(FailingOverlay(kind)));
    (services, reads)
}

fn assert_overlay_kind(error: WorkspaceError, expected: WorkspaceReadOverlayErrorKind) {
    let WorkspaceError::Backend(error) = error else {
        panic!("expected overlay backend error")
    };
    let error = error
        .downcast_ref::<WorkspaceReadOverlayError>()
        .expect("typed overlay error must survive the workspace boundary");
    assert_eq!(error.kind(), expected);
}

#[tokio::test]
async fn overlay_failures_never_fall_back_to_stale_disk_bytes() {
    for kind in [
        WorkspaceReadOverlayErrorKind::Conflict,
        WorkspaceReadOverlayErrorKind::Timeout,
        WorkspaceReadOverlayErrorKind::Foreign,
        WorkspaceReadOverlayErrorKind::Transport,
        WorkspaceReadOverlayErrorKind::Invalid,
    ] {
        let (services, reads) = failing_services(kind);
        let path = WorkspacePath::from_normalized("note.txt");
        assert_overlay_kind(services.fs().read_text(&path).await.unwrap_err(), kind);
        assert_overlay_kind(
            services
                .text_reader()
                .unwrap()
                .read_text_range(&path, 0, 10)
                .await
                .unwrap_err(),
            kind,
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0, "{kind}");
    }
}

#[tokio::test]
async fn only_not_found_and_unsupported_overlay_errors_use_the_backend() {
    for kind in [
        WorkspaceReadOverlayErrorKind::NotFound,
        WorkspaceReadOverlayErrorKind::Unsupported,
    ] {
        let (services, reads) = failing_services(kind);
        let path = WorkspacePath::from_normalized("note.txt");
        assert_eq!(
            services.fs().read_text(&path).await.unwrap(),
            "stale disk bytes"
        );
        let range = services
            .text_reader()
            .unwrap()
            .read_text_range(&path, 0, 10)
            .await
            .unwrap();
        assert_eq!(range.lines, vec!["stale disk bytes"]);
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2, "{kind}");
    }
}

#[test]
fn overlay_error_kind_is_a_closed_stable_wire_contract() {
    for kind in [
        WorkspaceReadOverlayErrorKind::NotFound,
        WorkspaceReadOverlayErrorKind::Unsupported,
        WorkspaceReadOverlayErrorKind::Conflict,
        WorkspaceReadOverlayErrorKind::Timeout,
        WorkspaceReadOverlayErrorKind::Foreign,
        WorkspaceReadOverlayErrorKind::Transport,
        WorkspaceReadOverlayErrorKind::Invalid,
    ] {
        assert_eq!(kind.as_str().parse(), Ok(kind));
        assert_eq!(serde_json::to_value(kind).unwrap(), kind.as_str());
    }
    assert!("conflicted"
        .parse::<WorkspaceReadOverlayErrorKind>()
        .is_err());
    assert!("".parse::<WorkspaceReadOverlayErrorKind>().is_err());
}

#[test]
fn text_range_from_text_matches_streaming_semantics() {
    let range = WorkspaceTextRange::from_text("a\nb\nc", 3, 5);
    assert!(range.lines.is_empty());
    assert!(range.eof);
    assert_eq!(range.next_offset, None);
    let range = WorkspaceTextRange::from_text("a\nb\nc", 0, 2);
    assert_eq!(range.lines, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(range.next_offset, Some(2));
    assert!(!range.eof);
}
