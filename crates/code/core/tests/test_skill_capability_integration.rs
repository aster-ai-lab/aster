use a3s_code_core::{Agent, CodeConfig, SessionOptions};
use serde_json::json;

fn test_config() -> CodeConfig {
    CodeConfig::from_acl(
        r#"
            default_model = "openai/gpt-4o"

            providers "openai" {
              api_key = "sk-test"

              models "gpt-4o" {
                name = "GPT-4o"
              }
            }
        "#,
    )
    .expect("valid test config")
}

#[tokio::test]
async fn session_registers_skill_tool_and_loads_session_skill_dirs() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let skill_dir = tempfile::tempdir().expect("skill tempdir");
    std::fs::write(
        skill_dir.path().join("db-review.md"),
        r#"---
name: db-review
description: Review database schema changes
kind: instruction
tags:
  - database
  - review
---
# DB Review

Check migrations, schema drift, and tests.
"#,
    )
    .expect("write test skill");

    let agent = Agent::from_config(test_config()).await.expect("agent");
    let session = agent
        .session_async(
            workspace.path().display().to_string(),
            Some(SessionOptions::new().with_skill_dirs([skill_dir.path()])),
        )
        .await
        .expect("session");

    let tool_names = session.tool_names();
    assert!(tool_names.contains(&"Skill".to_string()));
    assert!(tool_names.contains(&"web_search".to_string()));
    assert!(tool_names.contains(&"web_fetch".to_string()));
    assert!(!tool_names.iter().any(|name| name == "search_skills"));

    let result = session
        .tool("Skill", json!({"skill_name": "db-review"}))
        .await
        .expect("Skill result");

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(result.output.contains("db-review"));
    assert!(result.output.contains("Check migrations"));
}
