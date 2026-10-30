use super::*;
use a3s_code_shell::extensions::notification::PromptUsage;
use a3s_code_shell::session::info::Info;
use a3s_code_shell::session::storage::{JsonlStorageAdapter, SessionUpdate, StorageAdapter};
use a3s_code_shell::session::usage_file::{SessionUsageFile, UsageSummary};

pub(super) struct HeadlessPersistence {
    storage: JsonlStorageAdapter,
    info: Info,
    prompt_id: Option<String>,
    accept_replay: bool,
}

impl HeadlessPersistence {
    pub(super) async fn open(root: PathBuf, info: Info, model: acp::ModelId) -> Result<Self> {
        let storage = JsonlStorageAdapter::with_root(root);
        let summary = storage.init_session(&info, model.clone()).await?;
        storage
            .set_session_kind_if_absent(&info, "headless".into())
            .await?;
        storage
            .set_generated_title_if_absent(&info, model.0.to_string())
            .await?;
        Ok(Self {
            storage,
            info,
            prompt_id: None,
            accept_replay: summary.num_messages == 0,
        })
    }

    pub(super) async fn begin_prompt(
        &mut self,
        prompt_id: &str,
        blocks: &[acp::ContentBlock],
    ) -> Result<()> {
        self.accept_replay = false;
        self.prompt_id = Some(prompt_id.into());
        for block in blocks {
            self.storage
                .append_update(
                    &self.info,
                    &SessionUpdate::Acp(Box::new(acp::SessionNotification::new(
                        self.info.id.clone(),
                        acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(block.clone())),
                    ))),
                )
                .await?;
        }
        Ok(())
    }

    pub(super) async fn record(&self, notification: &acp::SessionNotification) -> Result<()> {
        let prompt = notification
            .meta
            .as_ref()
            .and_then(|meta| meta.get("promptId"))
            .and_then(serde_json::Value::as_str);
        let replay = notification
            .meta
            .as_ref()
            .and_then(|meta| meta.get("isReplay"))
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        if notification.session_id == self.info.id
            && ((self.accept_replay && replay)
                || self
                    .prompt_id
                    .as_deref()
                    .is_some_and(|id| prompt == Some(id)))
        {
            self.storage
                .append_update(
                    &self.info,
                    &SessionUpdate::Acp(Box::new(notification.clone())),
                )
                .await?;
        }
        Ok(())
    }

    pub(super) async fn finish(&self, meta: Option<&acp::Meta>) -> Result<()> {
        if let Some(value) = meta.and_then(|meta| meta.get("sessionTitle")) {
            let title = value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("sessionTitle must be a string"))?;
            self.storage
                .regenerate_generated_title(&self.info, title.to_owned())
                .await?;
        }
        let Some(value) = meta.and_then(|meta| meta.get("usage")) else {
            return Ok(());
        };
        let usage: PromptUsage = serde_json::from_value(value.clone())?;
        let row =
            |value: &a3s_code_shell::extensions::notification::PromptUsageModel| UsageSummary {
                input_tokens: value.input_tokens,
                output_tokens: value.output_tokens,
                total_tokens: value.total_tokens,
                cached_read_tokens: value.cached_read_tokens,
                cache_creation_tokens: value.cache_creation_tokens,
                reasoning_tokens: value.reasoning_tokens,
                model_calls: value.model_calls,
                cost_usd_ticks: value.cost_usd_ticks,
                cost_is_partial: value.cost_is_partial,
                usage_is_incomplete: usage.usage_is_incomplete,
                ..Default::default()
            };
        let mut totals = row(&usage.totals);
        totals.model_usage = usage
            .model_usage
            .iter()
            .map(|(name, value)| (name.clone(), row(value)))
            .collect();
        let mut file = self
            .storage
            .read_usage(&self.info)
            .await?
            .unwrap_or_else(|| SessionUsageFile::new(self.info.id.0.to_string()));
        file.apply_turn(1, chrono::Utc::now().to_rfc3339(), &totals, None);
        self.storage.write_usage(&self.info, &file).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn headless_projection_reopens_without_replaying_history_and_accumulates_usage() {
        let temporary = tempfile::tempdir().unwrap();
        let info = Info {
            id: acp::SessionId::new("headless-session"),
            cwd: temporary
                .path()
                .join("workspace")
                .to_string_lossy()
                .into_owned(),
        };
        let root = temporary.path().join("home");
        let model = acp::ModelId::new("fixture/model");
        let mut projection = HeadlessPersistence::open(root.clone(), info.clone(), model.clone())
            .await
            .unwrap();
        let text = |value: &str| acp::ContentBlock::Text(acp::TextContent::new(value));
        let update = |session: acp::SessionId, prompt: &str, value: &str| {
            let mut meta = acp::Meta::new();
            meta.insert("promptId".into(), prompt.into());
            acp::SessionNotification::new(
                session,
                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(text(value))),
            )
            .meta(Some(meta))
        };
        projection
            .begin_prompt("first", &[text("question")])
            .await
            .unwrap();
        let first = update(info.id.clone(), "first", "answer");
        projection.record(&first).await.unwrap();
        projection
            .record(&update(
                acp::SessionId::new("other-session"),
                "first",
                "foreign",
            ))
            .await
            .unwrap();
        let mut meta = acp::Meta::new();
        meta.insert("sessionTitle".into(), "Generated title".into());
        meta.insert(
            "usage".into(),
            serde_json::json!({
                "inputTokens": 17, "outputTokens": 5, "totalTokens": 22,
                "modelCalls": 2, "costIsPartial": true,
                "numTurns": 2, "usageIsIncomplete": true,
            }),
        );
        projection.finish(Some(&meta)).await.unwrap();
        assert_eq!(
            projection
                .storage
                .load_summary(&info)
                .await
                .unwrap()
                .display_title(),
            "Generated title"
        );
        projection
            .storage
            .update_session_title(&info, "User title".into())
            .await
            .unwrap();
        assert_eq!(
            projection.storage.list_sessions(None).await.unwrap().len(),
            1
        );
        drop(projection);

        let mut projection = HeadlessPersistence::open(root, info.clone(), model)
            .await
            .unwrap();
        projection
            .begin_prompt("second", &[text("next question")])
            .await
            .unwrap();
        projection.record(&first).await.unwrap();
        projection
            .record(&update(info.id.clone(), "second", "next answer"))
            .await
            .unwrap();
        projection.finish(Some(&meta)).await.unwrap();
        assert_eq!(
            projection
                .storage
                .load_summary(&info)
                .await
                .unwrap()
                .display_title(),
            "User title"
        );
        let usage = projection.storage.read_usage(&info).await.unwrap().unwrap();
        assert_eq!(usage.session.total_tokens, 44);
        assert_eq!(usage.session.model_calls, 4);
        assert!(usage.session.usage_is_incomplete && usage.session.cost_is_partial);
        assert_eq!(usage.session.cost_usd_ticks, None);
        assert_eq!(
            usage
                .turns
                .iter()
                .map(|turn| turn.turn_number)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        let history = projection.storage.load_session(&info).await.unwrap();
        let updates: Vec<_> = history
            .updates
            .iter()
            .map(|entry| serde_json::to_value(entry).unwrap())
            .collect();
        assert_eq!(updates.len(), 4);
        let serialized = serde_json::to_string(&updates).unwrap();
        assert_eq!(serialized.matches("next answer").count(), 1);
        assert!(!serialized.contains("foreign"));
    }

    #[tokio::test]
    async fn headless_projection_refuses_an_unwritable_store() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("file");
        std::fs::write(&root, "occupied").unwrap();
        assert!(
            HeadlessPersistence::open(
                root,
                Info {
                    id: acp::SessionId::new("unwritten"),
                    cwd: temporary.path().to_string_lossy().into_owned(),
                },
                acp::ModelId::new("fixture/model")
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn fresh_projection_imports_replay_before_the_next_prompt_only_once() {
        let temporary = tempfile::tempdir().unwrap();
        let info = Info {
            id: acp::SessionId::new("fork-child"),
            cwd: temporary.path().to_string_lossy().into_owned(),
        };
        let mut projection = HeadlessPersistence::open(
            temporary.path().join("home"),
            info.clone(),
            acp::ModelId::new("fixture/model"),
        )
        .await
        .unwrap();
        let text = |value: &str| acp::ContentBlock::Text(acp::TextContent::new(value));
        let mut replay = acp::SessionNotification::new(
            info.id.clone(),
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(text("parent answer"))),
        );
        let mut meta = acp::Meta::new();
        meta.insert("isReplay".into(), true.into());
        replay.meta = Some(meta);
        projection.record(&replay).await.unwrap();
        projection
            .begin_prompt("child-turn", &[text("child question")])
            .await
            .unwrap();
        projection.record(&replay).await.unwrap();
        let history = projection.storage.load_session(&info).await.unwrap();
        assert_eq!(history.updates.len(), 2);
        let serialized = serde_json::to_string(&history.updates).unwrap();
        assert_eq!(serialized.matches("parent answer").count(), 1);
        assert!(
            serialized.find("parent answer").unwrap() < serialized.find("child question").unwrap()
        );
    }
}
