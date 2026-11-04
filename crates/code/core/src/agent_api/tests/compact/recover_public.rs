use super::*;
use crate::store::SessionStore;

struct PageSourceClient {
    inner: RecordingCompactClient,
    next_read: Mutex<Option<String>>,
}

#[tokio::test]
async fn latest_manual_source_recovers_read_results_before_prior_compactions() {
    let root = tempfile::tempdir().unwrap();
    let old = "Earlier opaque observation 値🙂-28ea19";
    let new = "Later opaque observation Ω-91c73b";
    std::fs::write(
        root.path().join("earlier 値.txt"),
        format!("{old}\n{}", "padding\n".repeat(200)),
    )
    .unwrap();
    std::fs::write(
        root.path().join("later 値.txt"),
        format!("{new}\n{}", "padding\n".repeat(200)),
    )
    .unwrap();
    let client = Arc::new(PageSourceClient {
        inner: RecordingCompactClient {
            reply: "Observed.".into(),
            summary: "Only a lossy account remains.".into(),
            prompts: Mutex::new(vec![]),
            streams: Mutex::new(vec![]),
        },
        next_read: Mutex::new(Some("earlier 値.txt".into())),
    });
    let mut config = test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let agent = Agent::from_config(config).await.unwrap();
    let session = agent
        .build_session(
            root.path().display().to_string(),
            client.clone(),
            &SessionOptions::new()
                .with_session_id("flat-public-source")
                .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("read"))
                .with_planning_mode(crate::PlanningMode::Disabled),
        )
        .unwrap();
    finish_stream(&session, "Inspect the earlier observation.").await;
    session.compact_conversation(None).await.unwrap();
    let first = session
        .tool_executor
        .artifact_store()
        .artifacts()
        .into_iter()
        .find(|a| a.tool_name == "compaction")
        .unwrap();
    assert!(!session.history()[0].text().contains(old));
    *client.next_read.lock().unwrap() = Some("later 値.txt".into());
    finish_stream(&session, "Inspect the later observation.").await;
    session.compact_conversation(None).await.unwrap();
    let summary = session.history()[0].text();
    let latest = session
        .tool_executor
        .artifact_store()
        .artifacts()
        .into_iter()
        .find(|a| a.tool_name == "compaction" && summary.contains(&a.artifact_uri))
        .unwrap();
    assert!(
        latest.content.contains(old),
        "latest manual source must contain the earlier original read result"
    );
    assert!(latest.content.contains(new));
    assert!(!latest.content.contains(&first.artifact_uri));
    let read = session
        .tool_executor
        .registry()
        .execute(
            "read",
            &serde_json::json!({"file_path":latest.artifact_uri}),
        )
        .await
        .unwrap();
    assert_eq!(read.exit_code, 0);
    assert!(read.output.contains(old));
    session.close().await;
}

#[async_trait::async_trait]
impl crate::llm::LlmClient for PageSourceClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[crate::llm::ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        self.inner.complete(messages, system, tools).await
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[crate::llm::ToolDefinition],
        cancel: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        let uri = self.next_read.lock().unwrap().take();
        let Some(uri) = uri else {
            return self
                .inner
                .complete_streaming(messages, system, tools, cancel)
                .await;
        };
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.send(StreamEvent::Done(LlmResponse {
            message: Message {
                role: "assistant".into(),
                content: vec![crate::llm::ContentBlock::ToolUse {
                    id: "read-archive".into(),
                    name: "read".into(),
                    input: serde_json::json!({"file_path":uri}),
                }],
                reasoning_content: None,
                transcript_text: None,
                transcript_visibility: Default::default(),
            },
            usage: TokenUsage::default(),
            stop_reason: Some("tool_use".into()),
            token_logprobs: vec![],
            meta: None,
        }))
        .await
        .unwrap();
        Ok(rx)
    }
}

#[tokio::test]
async fn lossy_manual_compaction_retains_exact_public_source_for_existing_read() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(crate::store::MemorySessionStore::new());
    let token = "Observed opaque value: 値🙂-4bd79593";
    let client = Arc::new(PageSourceClient {
        inner: RecordingCompactClient {
            reply: token.into(),
            summary: "Only a general account remains.".into(),
            prompts: Mutex::new(vec![]),
            streams: Mutex::new(vec![]),
        },
        next_read: Mutex::new(None),
    });
    let mut config = test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let agent = Agent::from_config(config).await.unwrap();
    let session = agent
        .build_session(
            root.path().display().to_string(),
            client.clone(),
            &SessionOptions::new()
                .with_session_id("public-compact")
                .with_session_store(store.clone())
                .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("read"))
                .with_planning_mode(crate::PlanningMode::Disabled),
        )
        .unwrap();
    finish_stream(&session, "Inspect the observations.").await;
    finish_stream(&session, "Keep working.").await;
    session.compact_conversation(None).await.unwrap();
    let first_summary = session.history()[0].text();
    assert!(
        !first_summary.contains(token),
        "fixture must exercise actual lossy summary"
    );
    let artifacts = session.tool_executor.artifact_store().artifacts();
    let first = artifacts
        .iter()
        .find(|artifact| artifact.tool_name == "compaction")
        .expect("discarded public source must remain in the existing artifact store");
    assert!(first_summary.contains(&first.artifact_uri));
    assert!(first.content.contains(token));
    let read = session
        .tool_executor
        .registry()
        .execute("read", &serde_json::json!({"file_path":first.artifact_uri}))
        .await
        .unwrap();
    assert_eq!(read.exit_code, 0);
    assert!(read.output.contains(token));
    *client.next_read.lock().unwrap() = Some(first.artifact_uri.clone());
    finish_stream(
        &session,
        "Recover the earlier observation from the retained source.",
    )
    .await;
    let facts = crate::fact_control::read_workspace_facts(root.path(), "public-compact").unwrap();
    assert!(facts.iter().any(|fact| fact.kind == "tool.result"
        && fact.payload["toolCallId"] == "read-archive"
        && fact.payload["ok"] == true
        && fact.payload["output"]
            .as_str()
            .is_some_and(|output| output.contains(token))));
    finish_stream(&session, "Continue once more.").await;
    session.compact_conversation(None).await.unwrap();
    let latest = session.history()[0].text();
    let all = session.tool_executor.artifact_store().artifacts();
    assert_eq!(
        all.iter()
            .filter(|artifact| artifact.tool_name == "compaction")
            .count(),
        2
    );
    let next = all
        .iter()
        .find(|artifact| {
            artifact.tool_name == "compaction" && latest.contains(&artifact.artifact_uri)
        })
        .unwrap();
    assert!(next.content.contains(&first.artifact_uri));
    assert_eq!(
        session
            .tool_executor
            .get_artifact(&first.artifact_uri)
            .unwrap()
            .content,
        first.content
    );
    session.save().await.unwrap();
    let snapshot = store
        .load_snapshot("public-compact")
        .await
        .unwrap()
        .unwrap();
    assert!(snapshot
        .artifacts
        .iter()
        .any(|artifact| artifact.artifact_uri == first.artifact_uri));
    session.close().await;
    let resumed = agent
        .resume_session_async(
            "public-compact",
            SessionOptions::new()
                .with_session_store(store)
                .with_llm_client(client)
                .with_planning_mode(crate::PlanningMode::Disabled),
        )
        .await
        .unwrap();
    let restored = resumed
        .tool_executor
        .registry()
        .execute("read", &serde_json::json!({"file_path":first.artifact_uri}))
        .await
        .unwrap();
    assert_eq!(restored.exit_code, 0);
    assert!(restored.output.contains(token));
    resumed.close().await;
}
