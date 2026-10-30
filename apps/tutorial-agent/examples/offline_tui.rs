mod support;

use a3s_code_core::{
    SessionOptions,
    permissions::{PermissionDecision, PermissionPolicy},
};
use serde_json::json;
use tutorial_agent::runtime::{Runtime, confirmation_channel};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    std::fs::write(
        workspace.path().join("README.md"),
        "Offline tutorial project. Test entry: cargo test -p tutorial-agent.\n",
    )?;
    let client = support::Script::new(vec![
        support::tool("read-demo", "read", json!({"file_path":"README.md"})),
        support::tool("label-demo", "project_label", json!({})),
        support::tool(
            "question-demo",
            "ask_user",
            json!({"question":"请选择输出格式", "options":["Markdown","Text"],"allow_free_text":true}),
        ),
        support::text("离线练习完成：已读取 README、处理工具确认，并收到业务回答。"),
    ]);
    let agent = support::agent().await?;
    let (confirmations, confirmation_events) = confirmation_channel();
    let mut policy = PermissionPolicy::new()
        .allow_all(&["read", "ls", "ask_user"])
        .ask("project_label");
    policy.default_decision = PermissionDecision::Deny;
    let options = SessionOptions::new()
        .with_llm_client(client)
        .with_isolated_context()
        .with_permission_policy(policy)
        .with_confirmation_manager(confirmations.clone())
        .with_planning(false)
        .with_max_tool_rounds(8);
    let session = agent
        .session_async(
            workspace
                .path()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("工作区路径必须是 UTF-8"))?,
            Some(options),
        )
        .await?;
    let mut runtime = Runtime::assemble(agent, session, confirmations, confirmation_events)?;
    let mut terminal = ratatui::init();
    let result = tutorial_agent::ui::run(&mut runtime, &mut terminal).await;
    ratatui::restore();
    let closed = runtime.shutdown().await;
    closed?;
    result
}
