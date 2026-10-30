#[path = "../examples/support/mod.rs"]
mod support;

use a3s_code_core::permissions::{PermissionDecision, PermissionPolicy};
use a3s_code_core::{
    AgentEvent, SessionOptions,
    run::RunStatus,
    store::{FileSessionStore, SessionStore},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tutorial_agent::{
    chapters,
    runtime::{Runtime, confirmation_channel, options},
};

async fn open(
    client: Arc<support::Script>,
    workspace: &std::path::Path,
    extra: impl FnOnce(SessionOptions) -> SessionOptions,
) -> anyhow::Result<Runtime> {
    let agent = support::agent().await?;
    let (confirmations, rx) = confirmation_channel();
    let settings = extra(
        options(confirmations.clone())?
            .with_isolated_context()
            .with_llm_client(client),
    );
    let session = agent
        .session_async(workspace.to_str().unwrap(), Some(settings))
        .await?;
    Runtime::assemble(agent, session, confirmations, rx)
}

#[tokio::test]
async fn stream_reads_real_workspace_and_keeps_exact_terminal_run() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    std::fs::write(workspace.path().join("README.md"), "fixture-evidence-42")?;
    let client = support::Script::new(vec![
        support::tool("read-1", "read", json!({"file_path":"README.md"})),
        support::text("读取完成"),
    ]);
    let runtime = open(client.clone(), workspace.path(), |o| o).await?;
    let mut stream = runtime.session.stream_with_run("读取 README", None).await?;
    let run = stream.run.clone().expect("admitted Run");
    let mut reply = String::new();
    let mut errors = Vec::new();
    while let Some(event) = stream.events.recv().await {
        match event {
            AgentEvent::TextDelta { text } => reply.push_str(&text),
            AgentEvent::Error { message } => errors.push(message),
            _ => {}
        }
    }
    stream.worker.await?;
    assert_eq!(run.status().await, Some(RunStatus::Completed), "{errors:?}");
    assert!(reply.contains("读取完成"));
    assert!(
        serde_json::to_string(&*client.inputs.lock().unwrap())?.contains("fixture-evidence-42")
    );
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn confirmation_approves_only_selected_write_and_rejects_the_other() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let runtime = open(support::Script::new(vec![]), workspace.path(), |o| o).await?;
    let first = runtime.session.governed_tool(
        "write",
        json!({"file_path":"approved.txt","content":"approved"}),
    );
    let second = runtime.session.governed_tool(
        "write",
        json!({"file_path":"rejected.txt","content":"rejected"}),
    );
    let answers = async {
        let pending = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let pending = runtime.confirmations.pending_confirmation_details().await;
                if pending.len() == 2 {
                    break pending;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert!(!workspace.path().join("approved.txt").exists());
        for item in pending {
            let approved = item.args["file_path"] == "approved.txt";
            assert!(
                runtime
                    .confirmations
                    .confirm(&item.tool_id, approved, None)
                    .await
                    .map_err(anyhow::Error::msg)?
            );
            assert!(
                !runtime
                    .confirmations
                    .confirm(&item.tool_id, true, None)
                    .await
                    .map_err(anyhow::Error::msg)?
            );
        }
        Ok::<_, anyhow::Error>(())
    };
    let (first, second, answered) = tokio::join!(first, second, answers);
    answered?;
    assert_eq!(first?.exit_code, 0);
    assert!(second.is_err() || second?.exit_code != 0);
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("approved.txt"))?,
        "approved"
    );
    assert!(!workspace.path().join("rejected.txt").exists());
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn explicit_deny_and_invalid_tool_arguments_never_write() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let runtime = open(support::Script::new(vec![]), workspace.path(), |o| {
        let mut policy = PermissionPolicy::new().allow("project_label").deny("write");
        policy.default_decision = PermissionDecision::Deny;
        o.with_permission_policy(policy)
    })
    .await?;
    let result = runtime
        .session
        .governed_tool("write", json!({"file_path":"forbidden.txt","content":"no"}))
        .await;
    assert!(result.is_err() || result?.exit_code != 0);
    assert!(!workspace.path().join("forbidden.txt").exists());
    let result = runtime
        .session
        .governed_tool("project_label", json!({"extra":true}))
        .await;
    assert!(result.is_err() || result?.exit_code != 0);
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn cancellation_settles_original_run_before_next_request() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let client = support::Script::blocking();
    let runtime = open(client.clone(), workspace.path(), |o| o).await?;
    let mut stream = runtime.session.stream_with_run("等待取消", None).await?;
    let run = stream.run.clone().expect("Run");
    tokio::time::timeout(Duration::from_secs(5), client.started.notified()).await?;
    assert!(run.cancel().await);
    tokio::time::timeout(Duration::from_secs(5), async {
        while stream.events.recv().await.is_some() {}
        stream.worker.await
    })
    .await??;
    assert_eq!(run.status().await, Some(RunStatus::Cancelled));
    runtime.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn saved_session_reopens_with_original_workspace_and_history() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let state = tempfile::tempdir()?;
    let store = Arc::new(FileSessionStore::new(state.path()).await?);
    let runtime = open(
        support::Script::new(vec![support::text("保存前的回复")]),
        workspace.path(),
        |o| {
            o.with_session_id("persistent-demo")
                .with_session_store(store.clone())
        },
    )
    .await?;
    runtime.session.send("记住这段会话", None).await?;
    runtime.session.save().await?;
    assert!(store.exists("persistent-demo").await?);
    runtime.session.close().await;
    let (confirmations, _) = confirmation_channel();
    let resumed_client = support::Script::new(vec![support::text("恢复后的回复")]);
    let settings = options(confirmations)?
        .with_isolated_context()
        .with_llm_client(resumed_client.clone())
        .with_session_store(store);
    let restored = runtime
        .agent
        .resume_session_async("persistent-demo", settings)
        .await?;
    assert_eq!(restored.workspace(), workspace.path().canonicalize()?);
    let result = restored.send("继续", None).await?;
    assert_eq!(result.text, "恢复后的回复");
    assert!(
        serde_json::to_string(&*resumed_client.inputs.lock().unwrap())?.contains("保存前的回复")
    );
    restored.close().await;
    runtime.agent.close().await;
    Ok(())
}

#[tokio::test]
async fn goal_acceptance_rejects_a_stale_proposal() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let mut workspace_ref = None;
    let runtime = open(support::Script::new(vec![]), workspace.path(), |o| {
        let (options, binding) = chapters::goal_options(workspace.path(), o).unwrap();
        workspace_ref = Some(binding);
        options
    })
    .await?;
    chapters::create_goal(&runtime.session, workspace_ref.unwrap())?;
    let shown = chapters::propose_plan(&runtime.session, "proposal-a")?;
    let _newer = chapters::propose_plan(&runtime.session, "proposal-b")?;
    assert!(chapters::accept_plan(&runtime.session, &shown, "accept-stale").is_err());
    runtime.shutdown().await?;
    Ok(())
}
