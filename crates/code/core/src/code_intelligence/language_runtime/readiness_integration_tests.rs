use std::{path::PathBuf, sync::Arc, time::Duration};

use tokio_util::sync::CancellationToken;

use super::{LanguageRuntime, LanguageRuntimeError};
use crate::{
    code_intelligence::{
        diagnostics::DiagnosticsStore,
        document_store::DocumentStore,
        integration_test_support::{compile_fake_server, fixture_started_pids},
        language_profile::LanguageServerProfile,
        project_layout::ProjectLayoutResolver,
        CodePosition, NavigationKind,
    },
    workspace::{
        LocalWorkspaceFile, LocalWorkspaceFileStatus, LocalWorkspaceManifestSnapshot, WorkspacePath,
    },
};

struct Fixture {
    workspace: tempfile::TempDir,
    _server_directory: tempfile::TempDir,
    server: PathBuf,
    source: &'static str,
}

impl Fixture {
    fn new(mode: &str) -> Self {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir(workspace.path().join("src")).unwrap();
        std::fs::write(workspace.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let source = "pub fn answer() -> u32 { 42 }\npub fn use_answer() -> u32 { answer() }\n";
        std::fs::write(workspace.path().join("src/lib.rs"), source).unwrap();
        let server_directory = tempfile::tempdir().unwrap();
        let server = server_directory.path().join(format!(
            "readiness-{mode}-fake-lsp{}",
            if cfg!(windows) { ".exe" } else { "" }
        ));
        compile_fake_server(&server);
        Self {
            workspace,
            _server_directory: server_directory,
            server,
            source,
        }
    }

    async fn start(
        &self,
        timeout: Duration,
        cancellation: CancellationToken,
    ) -> Result<LanguageRuntime, LanguageRuntimeError> {
        let snapshot = self.snapshot();
        LanguageRuntime::start(
            LanguageServerProfile::rust_analyzer(&self.server),
            snapshot.root.clone(),
            ProjectLayoutResolver::resolve(&snapshot),
            Arc::new(DocumentStore::new(8)),
            Arc::new(DiagnosticsStore::new(8)),
            cancellation,
            timeout,
        )
        .await
    }

    fn snapshot(&self) -> LocalWorkspaceManifestSnapshot {
        LocalWorkspaceManifestSnapshot {
            version: 1,
            root: std::fs::canonicalize(self.workspace.path()).unwrap(),
            files: ["Cargo.toml", "src/lib.rs"]
                .into_iter()
                .map(|path| LocalWorkspaceFile {
                    path: path.to_owned(),
                    size: 1,
                    modified_ms: Some(1),
                    language: None,
                    status: LocalWorkspaceFileStatus::Tracked,
                    binary: false,
                    generated: false,
                })
                .collect(),
            scanned_at_ms: 1,
        }
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.server.with_extension("log")).unwrap()
    }

    fn assert_reaped(&self) {
        let pids = fixture_started_pids(&self.log());
        assert_eq!(pids.len(), 1);
        #[cfg(unix)]
        for pid in pids {
            assert!(!crate::code_intelligence::integration_test_support::process_exists(pid));
        }
    }

    async fn wait_for_initialized(&self) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if std::fs::read_to_string(self.server.with_extension("log"))
                    .is_ok_and(|log| log.contains("\"method\":\"initialized\""))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("fake server must complete initialization");
    }
}

#[tokio::test]
async fn rust_analyzer_cold_start_does_not_return_empty_success() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let fixture = Fixture::new("cold");
    let runtime = fixture
        .start(Duration::from_secs(5), CancellationToken::new())
        .await
        .unwrap();
    let result = runtime
        .navigate(
            NavigationKind::Definition,
            &WorkspacePath::from_normalized("src/lib.rs"),
            CodePosition::new(1, 31),
            fixture.source,
            CancellationToken::new(),
        )
        .await;
    runtime.shutdown().await.unwrap();
    fixture.assert_reaped();
    let initialize = fixture
        .log()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|message| message["method"] == "initialize")
        .unwrap();
    assert_eq!(
        initialize["params"]["capabilities"]["experimental"]["serverStatusNotification"],
        true
    );
    assert!(!result.unwrap().items.is_empty());
}

#[tokio::test]
async fn rust_analyzer_early_ready_signal_is_retained_without_warmup_queries() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let fixture = Fixture::new("early");
    let runtime = fixture
        .start(Duration::from_secs(2), CancellationToken::new())
        .await
        .unwrap();
    let result = runtime
        .navigate(
            NavigationKind::Definition,
            &WorkspacePath::from_normalized("src/lib.rs"),
            CodePosition::new(1, 31),
            fixture.source,
            CancellationToken::new(),
        )
        .await;
    runtime.shutdown().await.unwrap();
    fixture.assert_reaped();
    assert!(!result.unwrap().items.is_empty());
    assert_eq!(
        fixture
            .log()
            .lines()
            .filter(|line| line.contains("\"method\":\"textDocument/definition\""))
            .count(),
        1
    );
}

#[tokio::test]
async fn rust_analyzer_unhealthy_or_invalid_status_fails_closed_and_reaps() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    for (mode, expected) in [
        ("warning", "rust-analyzer reported warning health"),
        ("error", "rust-analyzer reported error health"),
        ("invalid", "invalid rust-analyzer server status"),
    ] {
        let fixture = Fixture::new(mode);
        let error = fixture
            .start(Duration::from_secs(2), CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(error, LanguageRuntimeError::Readiness { reason } if reason == expected));
        fixture.assert_reaped();
        assert!(!fixture
            .log()
            .contains("\"method\":\"textDocument/definition\""));
    }
}

#[tokio::test]
async fn rust_analyzer_missing_status_uses_existing_start_deadline_and_reaps() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let fixture = Fixture::new("missing");
    let timeout = Duration::from_secs(2);
    let started = tokio::time::Instant::now();
    let error = fixture
        .start(timeout, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, LanguageRuntimeError::Timeout { operation: "semantic_readiness", duration } if *duration == timeout),
        "expected the unchanged startup deadline, got {error:?}"
    );
    assert!(fixture.log().contains("\"method\":\"initialized\""));
    assert!(started.elapsed() < timeout + Duration::from_secs(1));
    fixture.assert_reaped();
}

#[tokio::test]
async fn rust_analyzer_cancelled_start_wait_reaps_the_owned_process() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let fixture = Fixture::new("missing");
    let cancellation = CancellationToken::new();
    let cancel = async {
        fixture.wait_for_initialized().await;
        cancellation.cancel();
    };
    let (result, ()) = tokio::join!(
        fixture.start(Duration::from_secs(5), cancellation.clone()),
        cancel
    );
    assert!(matches!(result, Err(LanguageRuntimeError::Cancelled)));
    fixture.assert_reaped();
}

#[tokio::test]
async fn rust_analyzer_workspace_shutdown_cancels_starting_query_and_reaps() {
    use crate::{
        code_intelligence::{workspace_runtime::WorkspaceRuntime, CodeIntelligenceState},
        workspace::{LocalWorkspaceBackend, WorkspaceFileSystem},
    };
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let fixture = Fixture::new("missing");
    let snapshot = fixture.snapshot();
    let file_system: Arc<dyn WorkspaceFileSystem> =
        Arc::new(LocalWorkspaceBackend::new(snapshot.root.clone()));
    let runtime = Arc::new(WorkspaceRuntime::new_with_profiles(
        snapshot.root.clone(),
        ProjectLayoutResolver::resolve(&snapshot),
        &snapshot,
        file_system,
        Duration::from_secs(5),
        vec![LanguageServerProfile::rust_analyzer(&fixture.server)],
    ));
    let query = tokio::spawn({
        let runtime = Arc::clone(&runtime);
        async move {
            runtime
                .document_symbols(
                    &WorkspacePath::from_normalized("src/lib.rs"),
                    CancellationToken::new(),
                )
                .await
        }
    });
    fixture.wait_for_initialized().await;
    assert!(!query.is_finished());
    assert_eq!(
        runtime.subscribe_status().borrow().state,
        CodeIntelligenceState::Starting
    );
    tokio::time::timeout(Duration::from_secs(2), runtime.shutdown())
        .await
        .unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(1), query)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    fixture.assert_reaped();
}

#[tokio::test]
async fn rust_analyzer_ready_signal_cannot_outlive_process_or_notification_stream() {
    use super::readiness::{wait_for_state, ReadinessState};
    use crate::code_intelligence::lsp::process::LspProcessState;
    use tokio::sync::watch;

    let timeout = Duration::from_secs(1);
    let (status_tx, status) = watch::channel(ReadinessState::Loading);
    let (process_tx, process) = watch::channel(LspProcessState::Running);
    status_tx.send_replace(ReadinessState::Ready);
    process_tx.send_replace(LspProcessState::Exited {
        code: Some(0),
        forced: false,
    });
    assert!(matches!(
        wait_for_state(
            status,
            process,
            &CancellationToken::new(),
            tokio::time::Instant::now() + timeout,
            timeout
        )
        .await,
        Err(LanguageRuntimeError::Readiness { .. })
    ));

    let (status_tx, status) = watch::channel(ReadinessState::Ready);
    let (_process_tx, process) = watch::channel(LspProcessState::Running);
    drop(status_tx);
    assert!(matches!(
        wait_for_state(
            status,
            process,
            &CancellationToken::new(),
            tokio::time::Instant::now() + timeout,
            timeout
        )
        .await,
        Err(LanguageRuntimeError::Readiness { .. })
    ));
}

#[tokio::test]
async fn rust_analyzer_readiness_wait_and_rpc_share_one_query_budget() {
    use super::readiness::ReadinessState;
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let fixture = Fixture::new("query-budget");
    let timeout = Duration::from_millis(750);
    let runtime = fixture
        .start(timeout, CancellationToken::new())
        .await
        .unwrap();
    let path = WorkspacePath::from_normalized("src/lib.rs");
    runtime
        .prepare_saved_document(&path, fixture.source, &CancellationToken::new())
        .await
        .unwrap();
    let mut status = runtime.readiness.as_ref().unwrap().clone();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if matches!(*status.borrow(), ReadinessState::Loading) {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .expect("fake server must signal its explicit reload");
    let result = runtime
        .navigate(
            NavigationKind::Definition,
            &path,
            CodePosition::new(1, 31),
            fixture.source,
            CancellationToken::new(),
        )
        .await;
    runtime.shutdown().await.unwrap();
    fixture.assert_reaped();
    assert!(matches!(
        result,
        Err(LanguageRuntimeError::Client {
            source: crate::code_intelligence::lsp::client::LspClientError::Timeout { .. },
            ..
        })
    ));
}
