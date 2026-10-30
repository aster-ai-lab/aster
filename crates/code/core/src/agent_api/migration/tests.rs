use super::*;
use crate::agent_api::tests::test_config;
use crate::store::{
    LegacyStudioAttachmentMetadata, LegacyStudioEntryKind, LegacyStudioRole, MemorySessionStore,
    PrefixForkRequest, SessionSnapshotV1, TranscriptEntryKind, TranscriptPageRequest,
    TranscriptSourceCut,
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct CountingClient {
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl LlmClient for CountingClient {
    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[crate::llm::ToolDefinition],
    ) -> anyhow::Result<crate::llm::LlmResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(crate::llm::LlmResponse {
            message: Message::assistant("unexpected model call"),
            usage: Default::default(),
            stop_reason: None,
            token_logprobs: Vec::new(),
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[crate::llm::ToolDefinition],
        _cancel_token: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<crate::llm::StreamEvent>> {
        anyhow::bail!("unused")
    }
}

struct Fixture {
    root: tempfile::TempDir,
    agent: Agent,
    store: Arc<MemorySessionStore>,
    client: Arc<CountingClient>,
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let agent = Agent::from_config(test_config()).await.unwrap();
        let store = Arc::new(MemorySessionStore::new());
        let client = Arc::new(CountingClient::default());
        Self {
            root,
            agent,
            store,
            client,
        }
    }

    fn options(&self, id: &str) -> SessionOptions {
        options(id, self.store.clone(), self.client.clone())
    }

    fn request(
        &self,
        target: &str,
        entries: Vec<LegacyStudioEntry>,
    ) -> LegacyStudioMigrationRequest {
        request(self.root.path(), target, entries)
    }

    fn facts(&self, id: &str) -> Vec<Fact> {
        crate::fact_control::read_workspace_facts(
            self.root.path(),
            &crate::fact_control::thread_for_session(id),
        )
        .unwrap()
    }
}

fn options(id: &str, store: Arc<dyn SessionStore>, client: Arc<CountingClient>) -> SessionOptions {
    SessionOptions::new()
        .with_session_id(id)
        .with_session_store(store)
        .with_llm_client(client)
        .with_planning_mode(crate::PlanningMode::Disabled)
        .with_auto_delegation_enabled(false)
        .with_manual_delegation_enabled(false)
        .with_continuation(false)
}

fn legacy_entry(id: &str, role: LegacyStudioRole, text: &str) -> LegacyStudioEntry {
    LegacyStudioEntry {
        id: id.into(),
        kind: LegacyStudioEntryKind::Text,
        role,
        text: text.into(),
        attachments: Vec::new(),
    }
}

fn ordinary_entries() -> Vec<LegacyStudioEntry> {
    vec![
        legacy_entry("studio-1", LegacyStudioRole::User, "old question"),
        legacy_entry("studio-2", LegacyStudioRole::Assistant, "old answer"),
    ]
}

fn request(
    workspace: &Path,
    target: &str,
    entries: Vec<LegacyStudioEntry>,
) -> LegacyStudioMigrationRequest {
    let source_digest =
        LegacyStudioMigrationRequest::compute_source_digest("studio-source", workspace, &entries)
            .unwrap();
    LegacyStudioMigrationRequest {
        source_session_id: "studio-source".into(),
        target_session_id: target.into(),
        expected_workspace: workspace.to_path_buf(),
        source_digest,
        backup_reference: "backup-opaque-001".into(),
        entries,
    }
}

async fn close(fixture: Fixture) {
    fixture.agent.close().await;
}

#[tokio::test]
async fn imports_display_only_history_with_strict_receipt_and_reopen_readback() {
    let fixture = Fixture::new().await;
    let mut entries = ordinary_entries();
    entries[1].attachments.push(LegacyStudioAttachmentMetadata {
        name: "image-1".into(),
        media_type: "image/png".into(),
        size_bytes: 42,
    });
    let (session, receipt) = fixture
        .agent
        .import_legacy_studio_history_async(
            fixture.root.path().display().to_string(),
            fixture.request("core-target", entries),
            fixture.options("core-target"),
        )
        .await
        .unwrap();
    assert_eq!(receipt.schema, LEGACY_STUDIO_MIGRATION_SCHEMA_V1);
    assert_eq!(receipt.imported_count, 2);
    assert_eq!(receipt.non_executable_count, 2);
    assert_eq!(receipt.unmapped_count, 0);
    assert_eq!(receipt.conflicting_count, 0);
    assert_eq!(receipt.backup_reference, "backup-opaque-001");
    assert_eq!(
        receipt.completion,
        LegacyStudioMigrationCompletion::Completed
    );

    let page = session
        .transcript_page(TranscriptPageRequest::default())
        .unwrap();
    assert_eq!(page.entries.len(), 2);
    assert!(page
        .entries
        .iter()
        .all(|entry| entry.kind == TranscriptEntryKind::LegacyHistory));
    let displays = page
        .entries
        .iter()
        .map(|entry| match &entry.payload {
            TranscriptEntryPayload::LegacyHistory {
                display: Some(display),
                ..
            } => display,
            _ => panic!("missing legacy display"),
        })
        .collect::<Vec<_>>();
    assert_eq!(displays[0].source_entry_id, "studio-1");
    assert_eq!(displays[0].role, LegacyStudioRole::User);
    assert_eq!(displays[0].text, "old question");
    assert_eq!(displays[1].source_entry_id, "studio-2");
    assert_eq!(displays[1].role, LegacyStudioRole::Assistant);
    assert_eq!(displays[1].text, "old answer");
    assert_eq!(displays[1].attachments[0].size_bytes, 42);
    assert!(session.history().is_empty());
    assert!(session.runs().await.is_empty());
    assert!(session.plan_snapshot().await.unwrap().is_empty());
    assert!(session.pending_confirmations().await.is_empty());
    assert!(session.goal_definition().unwrap().is_none());
    assert_eq!(
        session.legacy_studio_migration_receipt().unwrap(),
        Some(receipt.clone())
    );
    let facts = fixture.facts("core-target");
    assert_eq!(facts.len(), 3);
    assert!(facts.iter().all(|fact| matches!(
        fact.kind.as_str(),
        LEGACY_STUDIO_ENTRY_FACT_KIND | LEGACY_STUDIO_COMPLETION_FACT_KIND
    )));
    let saved = fixture
        .store
        .load_snapshot("core-target")
        .await
        .unwrap()
        .unwrap();
    assert!(saved.session.messages.is_empty());
    assert!(saved.run_records.is_empty());
    assert_eq!(saved.session.total_usage.total_tokens, 0);

    let original_ids = page
        .entries
        .iter()
        .map(|entry| entry.entry_id.clone())
        .collect::<Vec<_>>();
    session.close().await;
    drop(session);
    let reopened = fixture
        .agent
        .resume_session_async("core-target", fixture.options("core-target"))
        .await
        .unwrap();
    assert_eq!(
        reopened.legacy_studio_migration_receipt().unwrap(),
        Some(receipt)
    );
    let reread = reopened
        .transcript_page(TranscriptPageRequest::default())
        .unwrap();
    assert_eq!(
        reread
            .entries
            .iter()
            .map(|entry| entry.entry_id.clone())
            .collect::<Vec<_>>(),
        original_ids
    );
    assert!(reopened.runs().await.is_empty());
    assert_eq!(fixture.client.calls.load(Ordering::SeqCst), 0);
    reopened.close().await;
    close(fixture).await;
}

#[tokio::test]
async fn closed_input_refusals_do_not_create_targets() {
    let fixture = Fixture::new().await;
    let mut cases = Vec::new();

    let mut duplicate = ordinary_entries();
    duplicate[1].id = duplicate[0].id.clone();
    cases.push(("duplicate", fixture.request("duplicate-target", duplicate)));
    let mut digest = fixture.request("digest-target", ordinary_entries());
    digest.source_digest = "wrong-digest".into();
    cases.push(("digest", digest));
    let mut same = fixture.request("same-target", ordinary_entries());
    same.source_session_id = same.target_session_id.clone();
    same.source_digest = LegacyStudioMigrationRequest::compute_source_digest(
        &same.source_session_id,
        &same.expected_workspace,
        &same.entries,
    )
    .unwrap();
    cases.push(("same", same));
    cases.push(("empty", fixture.request("empty-target", Vec::new())));
    cases.push((
        "oversize",
        fixture.request(
            "oversize-target",
            vec![legacy_entry(
                "large",
                LegacyStudioRole::User,
                &"x".repeat(MAX_TEXT_BYTES + 1),
            )],
        ),
    ));
    let mut backup = fixture.request("backup-target", ordinary_entries());
    backup.backup_reference = "../delete/me".into();
    cases.push(("backup", backup));

    for (name, request) in cases {
        let target = request.target_session_id.clone();
        let error = fixture
            .agent
            .import_legacy_studio_history_async(
                fixture.root.path().display().to_string(),
                request,
                fixture.options(&target),
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("legacy Studio migration"),
            "{name}: {error}"
        );
        assert!(!fixture.store.exists(&target).await.unwrap(), "{name}");
        assert!(
            !target_already_exists(fixture.root.path(), &target).unwrap(),
            "{name}"
        );
    }

    let wire = fixture.request("wire-target", ordinary_entries());
    let mut unknown_role = serde_json::to_value(&wire).unwrap();
    unknown_role["entries"][0]["role"] = serde_json::json!("system");
    assert!(serde_json::from_value::<LegacyStudioMigrationRequest>(unknown_role).is_err());
    let mut unknown_kind = serde_json::to_value(&wire).unwrap();
    unknown_kind["entries"][0]["kind"] = serde_json::json!("rich_parts");
    assert!(serde_json::from_value::<LegacyStudioMigrationRequest>(unknown_kind).is_err());
    let mut unknown_field = serde_json::to_value(&wire).unwrap();
    unknown_field["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<LegacyStudioMigrationRequest>(unknown_field).is_err());
    assert!(!fixture.store.exists("wire-target").await.unwrap());
    assert_eq!(fixture.client.calls.load(Ordering::SeqCst), 0);
    close(fixture).await;
}

#[tokio::test]
async fn rejects_active_snapshot_and_fact_targets_and_foreign_workspace() {
    for scenario in ["active", "snapshot", "facts"] {
        let fixture = Fixture::new().await;
        let target = format!("existing-{scenario}");
        match scenario {
            "active" => {
                let existing = fixture
                    .agent
                    .session_async(
                        fixture.root.path().display().to_string(),
                        Some(fixture.options(&target)),
                    )
                    .await
                    .unwrap();
                let error = fixture
                    .agent
                    .import_legacy_studio_history_async(
                        fixture.root.path().display().to_string(),
                        fixture.request(&target, ordinary_entries()),
                        fixture.options(&target),
                    )
                    .await
                    .unwrap_err();
                assert!(error.to_string().contains("target is active"));
                existing.close().await;
            }
            "snapshot" => {
                let existing = fixture
                    .agent
                    .session_async(
                        fixture.root.path().display().to_string(),
                        Some(fixture.options(&target)),
                    )
                    .await
                    .unwrap();
                existing.save().await.unwrap();
                existing.close().await;
                drop(existing);
                let error = fixture
                    .agent
                    .import_legacy_studio_history_async(
                        fixture.root.path().display().to_string(),
                        fixture.request(&target, ordinary_entries()),
                        fixture.options(&target),
                    )
                    .await
                    .unwrap_err();
                assert!(error.to_string().contains("target already"));
            }
            _ => {
                let log = a3s_effect::FileLog::open(crate::session_folder::effect_log_dir(
                    fixture.root.path(),
                    &target,
                ))
                .unwrap();
                log.append(
                    &crate::fact_control::thread_for_session(&target),
                    &[NewFact {
                        kind: "user.message".into(),
                        key: "existing-user".into(),
                        payload: serde_json::json!({"text":"do not overwrite"}),
                    }],
                    None,
                )
                .unwrap();
                assert!(fixture
                    .agent
                    .import_legacy_studio_history_async(
                        fixture.root.path().display().to_string(),
                        fixture.request(&target, ordinary_entries()),
                        fixture.options(&target),
                    )
                    .await
                    .is_err());
                assert_eq!(fixture.facts(&target)[0].key, "existing-user");
            }
        }
        close(fixture).await;
    }

    let fixture = Fixture::new().await;
    let foreign = tempfile::tempdir().unwrap();
    let request = request(foreign.path(), "foreign-target", ordinary_entries());
    assert!(fixture
        .agent
        .import_legacy_studio_history_async(
            fixture.root.path().display().to_string(),
            request,
            fixture.options("foreign-target"),
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("workspace mismatch"));
    assert!(!fixture.store.exists("foreign-target").await.unwrap());
    assert!(!target_already_exists(fixture.root.path(), "foreign-target").unwrap());
    close(fixture).await;
}

struct FailingSaveStore {
    inner: MemorySessionStore,
}

#[async_trait::async_trait]
impl SessionStore for FailingSaveStore {
    async fn save(&self, session: &crate::store::SessionData) -> anyhow::Result<()> {
        self.inner.save(session).await
    }
    async fn load(&self, id: &str) -> anyhow::Result<Option<crate::store::SessionData>> {
        self.inner.load(id).await
    }
    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.inner.delete(id).await
    }
    async fn list(&self) -> anyhow::Result<Vec<String>> {
        self.inner.list().await
    }
    async fn exists(&self, id: &str) -> anyhow::Result<bool> {
        self.inner.exists(id).await
    }
    async fn save_snapshot(&self, _snapshot: &SessionSnapshotV1) -> anyhow::Result<()> {
        anyhow::bail!("injected snapshot failure")
    }
    async fn load_snapshot(&self, id: &str) -> anyhow::Result<Option<SessionSnapshotV1>> {
        self.inner.load_snapshot(id).await
    }
    fn capabilities(&self) -> crate::store::SessionStoreCapabilities {
        self.inner.capabilities()
    }
}

#[tokio::test]
async fn publication_failure_leaves_no_snapshot_facts_or_active_target() {
    let root = tempfile::tempdir().unwrap();
    let agent = Agent::from_config(test_config()).await.unwrap();
    let store = Arc::new(FailingSaveStore {
        inner: MemorySessionStore::new(),
    });
    let client = Arc::new(CountingClient::default());
    let request = request(root.path(), "failed-target", ordinary_entries());
    let error = agent
        .import_legacy_studio_history_async(
            root.path().display().to_string(),
            request,
            options("failed-target", store.clone(), client),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("injected snapshot failure"),
        "{error}"
    );
    assert!(!store.exists("failed-target").await.unwrap());
    assert!(!agent
        .list_sessions()
        .await
        .contains(&"failed-target".to_string()));
    assert!(!target_already_exists(root.path(), "failed-target").unwrap());
    agent.close().await;
}

#[tokio::test]
async fn legacy_assistant_is_not_a_fork_boundary_and_pagination_is_stable() {
    let fixture = Fixture::new().await;
    let entries = vec![
        legacy_entry("entry-1", LegacyStudioRole::User, "one"),
        legacy_entry("entry-2", LegacyStudioRole::Assistant, "two"),
        legacy_entry("entry-3", LegacyStudioRole::User, "three"),
    ];
    let (session, _) = fixture
        .agent
        .import_legacy_studio_history_async(
            fixture.root.path().display().to_string(),
            fixture.request("legacy-pages", entries),
            fixture.options("legacy-pages"),
        )
        .await
        .unwrap();
    let collect = |session: &AgentSession| {
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let page = session
                .transcript_page(TranscriptPageRequest { cursor, limit: 1 })
                .unwrap();
            cursor = page.next_cursor;
            ids.extend(page.entries.into_iter().map(|entry| entry.entry_id));
            if cursor.is_none() {
                return ids;
            }
        }
    };
    let ids = collect(&session);
    assert_eq!(ids.len(), 3);
    assert_eq!(collect(&session), ids);

    let full = session
        .transcript_page(TranscriptPageRequest::default())
        .unwrap();
    let legacy_assistant = full.entries[1].entry_id.clone();
    let error = fixture
        .agent
        .fork_session_prefix_async(
            &session,
            PrefixForkRequest {
                boundary_entry_id: legacy_assistant,
                expected_cut: full.source_cut,
                target_session_id: "illegal-fork".into(),
                expected_workspace: fixture.root.path().to_path_buf(),
            },
            fixture.options("illegal-fork"),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("prefix fork boundary"),
        "{error}"
    );
    assert!(!fixture.store.exists("illegal-fork").await.unwrap());

    let log = a3s_effect::FileLog::open(crate::session_folder::effect_log_dir(
        fixture.root.path(),
        session.id(),
    ))
    .unwrap();
    let thread = crate::fact_control::thread_for_session(session.id());
    log.append(
        &thread,
        &[
            NewFact {
                kind: "user.message".into(),
                key: "new-user".into(),
                payload: serde_json::json!({"text":"new question"}),
            },
            NewFact {
                kind: "model.turn".into(),
                key: "new-model".into(),
                payload: serde_json::json!({"kind":"text", "text":"new answer"}),
            },
        ],
        None,
    )
    .unwrap();
    let cut = a3s_effect::cut_log(&log.read(&thread).unwrap());
    log.append(
        &thread,
        &[NewFact {
            kind: "turn.complete".into(),
            key: "new-complete".into(),
            payload: serde_json::json!({
                "terminal":"narrative", "effect_digest":null, "forced_stop":null,
                "receipt_keys":[], "background_receipt_keys":[],
                "log_cut":{"seq":cut.seq,"digest":cut.digest}
            }),
        }],
        None,
    )
    .unwrap();
    let full = session
        .transcript_page(TranscriptPageRequest::default())
        .unwrap();
    let new_assistant = full
        .entries
        .iter()
        .find(|entry| entry.kind == TranscriptEntryKind::Assistant)
        .unwrap()
        .entry_id
        .clone();
    let child = fixture
        .agent
        .fork_session_prefix_async(
            &session,
            PrefixForkRequest {
                boundary_entry_id: new_assistant,
                expected_cut: TranscriptSourceCut::from(a3s_effect::cut_log(
                    &log.read(&thread).unwrap(),
                )),
                target_session_id: "new-real-fork".into(),
                expected_workspace: fixture.root.path().to_path_buf(),
            },
            fixture.options("new-real-fork"),
        )
        .await
        .unwrap();
    assert_eq!(child.history().len(), 2);
    assert!(child
        .transcript_page(TranscriptPageRequest::default())
        .unwrap()
        .entries
        .iter()
        .all(|entry| entry.kind != TranscriptEntryKind::LegacyHistory));
    child.close().await;
    session.close().await;
    close(fixture).await;
}

#[tokio::test]
async fn legacy_facts_without_a_completed_receipt_cannot_activate() {
    let fixture = Fixture::new().await;
    let session = fixture
        .agent
        .session_async(
            fixture.root.path().display().to_string(),
            Some(fixture.options("incomplete-import")),
        )
        .await
        .unwrap();
    let request = fixture.request("incomplete-import", ordinary_entries());
    let log = a3s_effect::FileLog::open(crate::session_folder::effect_log_dir(
        fixture.root.path(),
        session.id(),
    ))
    .unwrap();
    log.append(
        &crate::fact_control::thread_for_session(session.id()),
        &[NewFact {
            kind: LEGACY_STUDIO_ENTRY_FACT_KIND.into(),
            key: "legacy.studio.entry:0:studio-1".into(),
            payload: serde_json::to_value(LegacyStudioEntryFact {
                source_session_id: request.source_session_id,
                target_session_id: request.target_session_id,
                source_digest: request.source_digest,
                entry: request.entries[0].clone(),
            })
            .unwrap(),
        }],
        None,
    )
    .unwrap();
    assert!(session
        .legacy_studio_migration_receipt()
        .unwrap_err()
        .to_string()
        .contains("incomplete import"));
    assert!(!fixture.store.exists("incomplete-import").await.unwrap());
    session.close().await;
    close(fixture).await;
}

#[test]
fn legacy_history_wire_compatibility_is_optional_and_still_closed() {
    let old = serde_json::json!({
        "kind":"legacy_history", "source_kind":"user.message",
        "fact_key":"old", "reason":"old rich parts"
    });
    let TranscriptEntryPayload::LegacyHistory { display, .. } =
        serde_json::from_value(old).unwrap()
    else {
        panic!("wrong payload")
    };
    assert!(display.is_none());
    let unknown = serde_json::json!({
        "kind":"legacy_history", "source_kind":"user.message",
        "fact_key":"old", "reason":"old rich parts", "unexpected":true
    });
    assert!(serde_json::from_value::<TranscriptEntryPayload>(unknown).is_err());
}
