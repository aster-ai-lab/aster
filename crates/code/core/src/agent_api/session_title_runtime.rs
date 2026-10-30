use super::*;
use crate::llm::structured::StructuredDirective;

impl AgentSession {
    /// Generate the first session title without adding a user turn or executing tools.
    pub async fn generate_session_title(&self, prompt: &str) -> Option<String> {
        if self.closed.load(std::sync::atomic::Ordering::Relaxed)
            || self
                .history()
                .iter()
                .filter(|message| message.role == "user")
                .count()
                != 1
        {
            return None;
        }
        let client = self
            .llm_client
            .fork_for_session(&format!("{}:session-title", self.session_id))
            .unwrap_or_else(|| Arc::clone(&self.llm_client))
            .with_output_cap(100)?;
        let schema = serde_json::json!({
            "type": "object", "additionalProperties": false,
            "required": ["session_title"],
            "properties": {"session_title": {"type": "string", "minLength": 1, "maxLength": 128}},
        });
        let tools = [crate::llm::ToolDefinition {
            name: "session_title".into(),
            description: "Generate the session title for the user request".into(),
            parameters: schema.clone(),
        }];
        let directive = StructuredDirective {
            force_tool: Some("session_title".into()),
            validation_schema: Some(schema),
            ..Default::default()
        };
        let messages = [crate::llm::Message::user(&format!(
            "<user_query>\n{}\n</user_query>",
            prompt.chars().take(4_000).collect::<String>(),
        ))];
        let system = "You are tasked with generating the session title. Produce a short and distinctive 5-10 word descriptive title for the user's request. Use the user's language. Generate the session_title and nothing else.";
        let cancellation = self.session_cancel.child_token();
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return None,
            result = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                client.complete_structured(&messages, Some(system), &tools, &directive),
            ) => result.ok()?.ok()?,
        };
        let mut calls = response
            .message
            .content
            .into_iter()
            .filter_map(|block| match block {
                crate::llm::ContentBlock::ToolUse { name, input, .. } => Some((name, input)),
                _ => None,
            });
        let (name, input) = calls.next()?;
        if name != "session_title" || calls.next().is_some() {
            return None;
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Title {
            session_title: String,
        }
        let title: Title = serde_json::from_value(input).ok()?;
        if title.session_title.trim().is_empty() || title.session_title.chars().count() > 128 {
            return None;
        }
        Some(title.session_title)
    }
}
