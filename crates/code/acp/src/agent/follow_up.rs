use super::*;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FollowUpParams {
    session_id: String,
    prompt: String,
    response: String,
}

impl AsterCodeAgent {
    /// Independent post-turn generation. This never calls ask_user and never
    /// opens another user turn; an unavailable suggestion is returned as null.
    pub(super) async fn handle_follow_up(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let params: FollowUpParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        if params.session_id.trim().is_empty()
            || params.prompt.trim().is_empty()
            || params.response.trim().is_empty()
            || params.prompt.chars().count() > 16_000
            || params.response.chars().count() > 24_000
        {
            return Err(acp::Error::invalid_params().data("invalid follow-up request"));
        }
        let session = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&params.session_id)
                .map(|live| Arc::clone(&live.session))
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?
        };
        let suggestions = session
            .generate_follow_up_suggestions(&params.prompt, &params.response)
            .await;
        let raw =
            serde_json::value::to_raw_value(&serde_json::json!({ "suggestions": suggestions }))
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }
}

#[cfg(test)]
#[path = "follow_up_tests.rs"]
mod tests;
