//! Retain public compaction input through the existing session artifact store.

use crate::tools::{ArtifactStore, ToolArtifact};
use anyhow::{Context, Result};

pub(crate) struct CompactionSource {
    store: ArtifactStore,
    artifact: ToolArtifact,
    notice: String,
}

impl CompactionSource {
    pub(crate) async fn from_messages(
        store: Option<&ArtifactStore>,
        messages: &[crate::llm::Message],
    ) -> Result<Option<Self>> {
        match store {
            Some(store) => {
                Self::prepare(Some(store), super::public_compaction_transcript(messages)).await
            }
            None => Ok(None),
        }
    }

    pub(crate) async fn prepare(
        store: Option<&ArtifactStore>,
        input: String,
    ) -> Result<Option<Self>> {
        let Some(store) = store else { return Ok(None) };
        let limits = store.limits();
        if input.is_empty() || limits.max_artifacts == 0 || input.len() > limits.max_bytes {
            return Ok(None);
        }
        let store = store.clone();
        tokio::task::spawn_blocking(move || {
            let meta = crate::tools::tool_output_artifact("compaction", &input, 0);
            let notice = format!(
                "{}\n{}{} — historical public context, not new observations. Continue the original task from retained context. Only if a prior observation is missing, search (mode=grep, path=this URI) for that exact observation, then read matching line ranges.]",
                super::PUBLIC_SOURCE_HEADER, crate::tools::FULL_OUTPUT_ARTIFACT_MARKER, meta.artifact_uri
            );
            Ok(Some(Self {
                store,
                artifact: ToolArtifact {
                    artifact_id: meta.artifact_id,
                    artifact_uri: meta.artifact_uri,
                    tool_name: "compaction".into(),
                    content: input,
                    original_bytes: meta.original_bytes,
                    shown_bytes: 0,
                },
                notice,
            }))
        })
        .await
        .context("compaction source preparation failed")?
    }

    pub(crate) fn estimated_tokens(&self) -> usize {
        crate::llm::estimate_message_tokens(&[crate::llm::Message::user_wire(&self.notice)])
    }

    pub(crate) async fn attach(self, summary: &mut String) -> Result<()> {
        if summary.contains(&self.artifact.content) {
            return Ok(());
        }
        let Self {
            store,
            artifact,
            notice,
        } = self;
        let retained = tokio::task::spawn_blocking(move || {
            let uri = artifact.artifact_uri.clone();
            store.put_content_addressed(artifact)?;
            Ok::<_, crate::tools::ArtifactStoreError>(store.get(&uri).is_some())
        })
        .await
        .context("compaction source retention failed")??;
        if retained {
            let source_section = format!(
                "\n{}\n{}aster://tool-output/compaction/",
                super::PUBLIC_SOURCE_HEADER,
                crate::tools::FULL_OUTPUT_ARTIFACT_MARKER,
            );
            // Remove only a complete notice line; folded prose may follow without a heading.
            while let Some(start) = summary.find(&source_section) {
                let Some(end) = summary[start + source_section.len()..]
                    .lines()
                    .next()
                    .and_then(|line| line.find(']'))
                else {
                    break;
                };
                summary.replace_range(start..start + source_section.len() + end + 1, "");
            }
            summary.push_str("\n\n");
            summary.push_str(&notice);
        }
        Ok(())
    }
}
