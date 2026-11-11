//! Session-owned local input recovery. Recovered queue rows require an explicit send.
mod fields;
mod storage;
use super::{
    agent::QueuedPrompt,
    agent_view::{AgentView, PromptInputMode, PromptMode, PromptStashEntry},
};
use crate::views::prompt_widget::StashedPrompt;
use agent_client_protocol as acp;
pub(crate) use fields::{Chip, images};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Default)]
pub(crate) struct Recovery {
    pub(crate) session: Option<acp::SessionId>,
    pub(crate) loading: bool,
    preserve_disk: bool,
    pending: VecDeque<Context>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Context {
    queue: VecDeque<QueuedPrompt>,
    prompt: StashedPrompt,
    mode: PromptInputMode,
    edit: PromptMode,
    before_edit: Option<StashedPrompt>,
    stash: Option<PromptStashEntry>,
}

/// Saved local input contexts, kept separate from admitted conversation history.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    schema: u8,
    contexts: VecDeque<Context>,
}

impl std::fmt::Debug for Saved {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Saved")
            .field("schema", &self.schema)
            .field("contexts", &self.contexts.len())
            .finish()
    }
}
impl std::fmt::Debug for Context {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Context")
            .field("queue", &self.queue.len())
            .field("has_draft", &self.has_draft())
            .finish()
    }
}

pub(crate) struct Snapshot {
    info: aster_shell::session::info::Info,
    saved: Saved,
    merge_disk: bool,
}

impl Snapshot {
    pub(crate) fn take(agent: &mut AgentView) -> Option<Self> {
        let info = agent.session.local_session_info()?;
        let session = info.id.clone();
        let same_owner = agent.input_recovery.session.as_ref() == Some(&session);
        let mut contexts = VecDeque::from([Context {
            queue: std::mem::take(&mut agent.session.pending_prompts),
            prompt: agent.prompt.stash(),
            mode: agent.prompt_input_mode,
            edit: std::mem::replace(&mut agent.prompt_mode, PromptMode::Normal),
            before_edit: agent.stashed_prompt.take(),
            stash: agent.prompt_stash.take(),
        }]);
        if same_owner {
            contexts.append(&mut agent.input_recovery.pending);
        }
        Some(Self {
            info,
            saved: Saved {
                schema: 1,
                contexts,
            },
            merge_disk: same_owner && agent.input_recovery.preserve_disk,
        })
    }

    pub(crate) async fn save_all(snapshots: Vec<Self>) -> anyhow::Result<()> {
        tokio::task::spawn_blocking(move || {
            let mut errors = Vec::new();
            for snapshot in snapshots {
                let session = snapshot.info.id.clone();
                if let Err(error) = storage::save(snapshot) {
                    errors.push(format!("Session {session}: {error:#}"));
                }
            }
            anyhow::ensure!(
                errors.is_empty(),
                "Could not save unsent input: {}",
                errors.join("; ")
            );
            Ok(())
        })
        .await?
    }
}

pub(crate) async fn load(info: aster_shell::session::info::Info) -> anyhow::Result<Option<Saved>> {
    tokio::task::spawn_blocking(move || storage::load(&info)).await?
}

impl AgentView {
    pub(crate) fn begin_input_recovery(&mut self, session: &acp::SessionId) -> bool {
        if self.input_recovery.session.as_ref() == Some(session) {
            return false;
        }
        self.input_recovery = Recovery {
            session: Some(session.clone()),
            loading: true,
            preserve_disk: true,
            pending: VecDeque::new(),
        };
        true
    }

    pub(crate) fn receive_saved_input(
        &mut self,
        session: &acp::SessionId,
        result: Result<Option<Saved>, String>,
    ) -> bool {
        if self.session.session_id.as_ref() != Some(session)
            || self.input_recovery.session.as_ref() != Some(session)
            || !self.input_recovery.loading
        {
            return false;
        }
        self.input_recovery.loading = false;
        let saved = match result {
            Ok(saved) => {
                self.input_recovery.preserve_disk = false;
                saved
            }
            Err(error) => {
                tracing::warn!(%error, "Could not recover unsent input");
                self.input_notice("Could not recover unsent input");
                return true;
            }
        };
        let Some(saved) = saved else {
            return true;
        };
        let mut restored = VecDeque::new();
        for mut context in saved.contexts {
            let mut ids = HashMap::new();
            for mut row in std::mem::take(&mut context.queue) {
                let id = self.session.next_queue_id;
                self.session.next_queue_id += 1;
                ids.insert(row.id, id);
                row.id = id;
                row.recovered = true;
                restored.push_back(row);
            }
            let edit_exists = match &mut context.edit {
                PromptMode::Normal => true,
                PromptMode::EditingQueued {
                    id,
                    server_id: None,
                    ..
                } => {
                    if let Some(rebound) = ids.get(id) {
                        *id = *rebound;
                        true
                    } else {
                        false
                    }
                }
                PromptMode::EditingQueued {
                    server_id: Some(id),
                    ..
                } => self.shared_queue.iter().any(|row| &row.id == id),
            };
            let before_edit = if edit_exists {
                None
            } else {
                context.edit = PromptMode::Normal;
                context.before_edit.take()
            };
            if context.has_draft() {
                self.input_recovery.pending.push_back(context);
            }
            if let Some(prompt) = before_edit {
                let context = Context {
                    queue: VecDeque::new(),
                    prompt,
                    mode: PromptInputMode::Normal,
                    edit: PromptMode::Normal,
                    before_edit: None,
                    stash: None,
                };
                if context.has_draft() {
                    self.input_recovery.pending.push_back(context);
                }
            }
        }
        let count = restored.len();
        restored.append(&mut self.session.pending_prompts);
        self.session.pending_prompts = restored;
        self.sync_queue_pane();
        let waiting = !self.input_recovery.pending.is_empty() && !self.restore_saved_draft();
        if count > 0 {
            self.input_notice("Queued messages restored. Review before sending");
        } else if waiting {
            self.input_notice("Saved drafts will appear when the composer is empty");
        }
        true
    }

    fn input_notice(&mut self, text: &str) {
        if self.is_minimal_mode() {
            self.scrollback
                .push_block(crate::scrollback::block::RenderBlock::system(text));
        } else {
            self.show_toast(text);
        }
    }

    pub(crate) fn restore_saved_draft(&mut self) -> bool {
        if self.input_recovery.session.as_ref() != self.session.session_id.as_ref()
            || !self.prompt.text().is_empty()
            || !self.prompt.images.is_empty()
            || !matches!(self.prompt_mode, PromptMode::Normal)
            || self.stashed_prompt.is_some()
            || self.prompt_stash.is_some()
            || self.permission_stashed_prompt.is_some()
            || self.casual_stashed_prompt.is_some()
            || self.active_modal.is_some()
        {
            return false;
        }
        let Some(context) = self.input_recovery.pending.pop_front() else {
            return false;
        };
        self.prompt.restore(context.prompt);
        self.prompt_input_mode = context.mode;
        self.prompt_mode = context.edit;
        self.stashed_prompt = context.before_edit;
        self.prompt_stash = context.stash;
        self.input_notice("Draft restored");
        true
    }
}

impl Context {
    fn has_draft(&self) -> bool {
        !self.prompt.text.is_empty()
            || !self.prompt.images.is_empty()
            || self.before_edit.is_some()
            || self.stash.is_some()
    }

    fn for_each_prompt(
        &mut self,
        mut visit: impl FnMut(&mut StashedPrompt) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        visit(&mut self.prompt)?;
        if let Some(prompt) = self.before_edit.as_mut() {
            visit(prompt)?;
        }
        if let Some(stash) = self.stash.as_mut() {
            visit(&mut stash.prompt)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
