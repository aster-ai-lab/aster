use super::*;
use crate::llm::structured::{
    generate_blocking_with_cancellation, parse_validated_output, StructuredMode, StructuredRequest,
};

/// Validate a caller-supplied structured output schema before starting a turn.
pub fn validate_output_schema(schema: &serde_json::Value) -> anyhow::Result<()> {
    jsonschema::options()
        .build(schema)
        .map(|_| ())
        .map_err(|error| anyhow::anyhow!("invalid structured output schema: {error}"))
}

impl AgentSession {
    /// Project an already completed agent answer into the requested schema.
    /// A valid final answer needs no extra model call. Invalid answers use the
    /// same bounded structured-generation and repair engine as other Core flows.
    pub async fn structured_output_from_response(
        &self,
        prompt: &str,
        response: &str,
        schema: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        validate_output_schema(&schema)?;
        if let Some(value) = parse_validated_output(response, &schema) {
            return Ok(value);
        }
        let request = StructuredRequest {
            prompt: format!(
                "Original user request:\n{prompt}\n\nCompleted agent answer:\n{response}"
            ),
            system: Some(
                "Convert the completed agent answer into JSON that satisfies the supplied schema. Preserve its supported facts, do not invent new claims, and return JSON only."
                    .to_string(),
            ),
            schema,
            schema_name: "acp_prompt_output".to_string(),
            schema_description: Some(
                "Structured projection of the completed agent answer".to_string(),
            ),
            mode: StructuredMode::Auto,
            max_repair_attempts: 2,
        };
        let client = self
            .llm_client
            .fork_for_session(&format!("{}:structured-output", self.session_id))
            .unwrap_or_else(|| Arc::clone(&self.llm_client));
        generate_blocking_with_cancellation(
            client.as_ref(),
            &request,
            self.session_cancel.child_token(),
        )
        .await
        .map(|result| result.object)
    }
}
