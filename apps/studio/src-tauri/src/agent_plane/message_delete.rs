use super::{PlaneError, TurnService};

impl TurnService {
    pub async fn delete_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), PlaneError> {
        let runtime = {
            let mut runtimes = self.runtimes.lock().await;
            let _gate = self.lock_settle();
            let live = self.lock_live();
            self.sessions.get(session_id)?;
            let message = self
                .store
                .messages(session_id)
                .into_iter()
                .find(|row| row.id == message_id && row.session_id == session_id)
                .ok_or_else(|| PlaneError::NotFound("message not found".into()))?;
            if live.contains_key(session_id)
                || self
                    .warming
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .contains(session_id)
                || !matches!(message.status.as_str(), "success" | "error" | "paused")
            {
                return Err(PlaneError::Conflict(
                    "Cannot delete messages while the task is running.".into(),
                ));
            }
            if !self.store.delete_message(session_id, message_id)? {
                return Err(PlaneError::NotFound("message not found".into()));
            }
            self.clear_review_memory(session_id);
            runtimes.remove(session_id)
        };
        if let Some(runtime) = runtime {
            runtime.client.kill().await;
        }
        self.changed(session_id);
        Ok(())
    }
}

#[cfg(test)]
#[path = "message_delete_tests.rs"]
mod tests;
