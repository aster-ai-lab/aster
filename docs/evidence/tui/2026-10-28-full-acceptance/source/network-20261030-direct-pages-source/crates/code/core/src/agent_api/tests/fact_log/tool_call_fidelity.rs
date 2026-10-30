#![cfg(test)]

use super::*;
use serde_json::json;

#[tokio::test]
async fn diagnostic_calls_keep_literal_arguments_and_runtime_refusals_in_model_history() {
    for (name, args, policy, reason) in [
        (
            "search",
            json!({"mode":"grep", "query":"needle", "path":".", "context":0, "limit":1}),
            crate::permissions::PermissionPolicy::new().allow("*"),
            "supported only with output_mode",
        ),
        (
            "write",
            json!({"file_path":"protected.txt", "content":"replacement"}),
            crate::permissions::PermissionPolicy::new()
                .allow("*")
                .deny("write"),
            "tool_effect_effective_arguments_unapproved",
        ),
    ] {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("protected.txt"), b"original").unwrap();
        let agent = crate::Agent::from_config(test_config()).await.unwrap();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = session_for(
            &agent,
            workspace.path(),
            &format!("diagnostic-{name}"),
            Arc::new(ScriptedClient {
                seen: Arc::clone(&seen),
                responses: std::sync::Mutex::new(vec![
                    tool_response("diagnostic-call", name, args.clone()),
                    text_response("observed refusal"),
                ]),
            }),
            &crate::SessionOptions::new()
                .with_assistant_final_response(true)
                .with_permission_policy(policy)
                .with_continuation(false),
        );
        let prompt = "Submit the specified diagnostic once and report its actual result.";
        session.send(prompt, None).await.unwrap();
        {
            let requests = seen.lock().unwrap();
            // Memory extraction is a separate no-tools request after completion.
            let coding = requests
                .iter()
                .filter(|request| !request.tools.is_empty())
                .collect::<Vec<_>>();
            assert_eq!(coding.len(), 2);
            for request in &coding {
                assert!(request
                    .system
                    .as_deref()
                    .unwrap()
                    .contains(crate::prompts::RUNTIME_CONTRACT));
                assert_eq!(
                    request
                        .messages
                        .iter()
                        .filter(|message| message.role == "user"
                            && message.content.iter().any(|block| matches!(
                                block,
                                crate::llm::ContentBlock::Text { .. }
                            )))
                        .map(Message::text)
                        .collect::<Vec<_>>(),
                    vec![prompt]
                );
            }
            let calls = coding[1]
                .messages
                .iter()
                .flat_map(Message::tool_calls)
                .collect::<Vec<_>>();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, name);
            assert_eq!(calls[0].args, args);
            let results = coding[1]
                .messages
                .iter()
                .flat_map(|message| &message.content)
                .filter_map(|block| match block {
                    crate::llm::ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                        ..
                    } => Some((tool_use_id, content.as_text(), is_error)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].0, "diagnostic-call");
            assert_eq!(*results[0].2, Some(true));
            assert!(results[0].1.contains(reason), "{}", results[0].1);
            assert_eq!(
                std::fs::read(workspace.path().join("protected.txt")).unwrap(),
                b"original"
            );
        }
        session.close().await;
    }
}
