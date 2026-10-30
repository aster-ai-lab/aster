use std::{ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

use a3s_code_core::hitl::{ConfirmationManager, ConfirmationPolicy, TimeoutAction};
use a3s_code_core::permissions::{PermissionDecision, PermissionPolicy};
use a3s_code_core::store::{FileSessionStore, SessionStore};
use a3s_code_core::{Agent, AgentEvent, AgentSession, SessionOptions};
use anyhow::{Context, Result, ensure};
use tokio::sync::broadcast;

pub struct Arguments {
    pub config: PathBuf,
    pub workspace: PathBuf,
    pub state: Option<PathBuf>,
    pub session_id: Option<String>,
    pub resume: bool,
    pub prompt: Option<String>,
}

impl Arguments {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut args = args.into_iter();
        let config = PathBuf::from(args.next().context("缺少可信 ACL 路径")?);
        let workspace = PathBuf::from(args.next().context("缺少工作区路径")?).canonicalize()?;
        ensure!(workspace.is_dir(), "工作区必须是目录");
        let mut result = Self {
            config,
            workspace,
            state: None,
            session_id: None,
            resume: false,
            prompt: None,
        };
        while let Some(arg) = args.next() {
            match arg.to_str().context("参数必须是 UTF-8")? {
                "--state" if result.state.is_none() => {
                    result.state = Some(PathBuf::from(args.next().context("--state 缺少目录")?))
                }
                "--session" if result.session_id.is_none() => {
                    result.session_id = Some(
                        args.next()
                            .context("--session 缺少 ID")?
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("会话 ID 必须是 UTF-8"))?,
                    )
                }
                "--resume" if !result.resume => result.resume = true,
                "--headless" if result.prompt.is_none() => {
                    result.prompt = Some(
                        args.next()
                            .context("--headless 缺少任务")?
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("任务必须是 UTF-8"))?,
                    )
                }
                _ => anyhow::bail!("未知或重复参数：{}", arg.to_string_lossy()),
            }
        }
        ensure!(
            result.state.is_some() == result.session_id.is_some(),
            "--state 与 --session 必须一起使用"
        );
        ensure!(
            !result.resume || result.state.is_some(),
            "--resume 需要存储目录与会话 ID"
        );
        if let Some(id) = &result.session_id {
            ensure!(
                !id.trim().is_empty() && id.len() <= 128,
                "会话 ID 必须为 1–128 字节"
            );
        }
        if let Some(prompt) = &result.prompt {
            ensure!(
                !prompt.trim().is_empty() && prompt.len() <= 8192,
                "任务必须为 1–8192 字节"
            );
        }
        Ok(result)
    }
}

pub struct Runtime {
    pub agent: Agent,
    pub session: AgentSession,
    pub confirmations: Arc<ConfirmationManager>,
    pub confirmation_events: broadcast::Receiver<AgentEvent>,
}

pub fn confirmation_channel() -> (Arc<ConfirmationManager>, broadcast::Receiver<AgentEvent>) {
    let (tx, rx) = broadcast::channel(64);
    let manager = ConfirmationManager::new(
        ConfirmationPolicy::enabled().with_timeout(60_000, TimeoutAction::Reject),
        tx,
    );
    (Arc::new(manager), rx)
}

pub fn options(confirmations: Arc<ConfirmationManager>) -> Result<SessionOptions> {
    let mut policy = PermissionPolicy::new()
        .allow_all(&["read", "ls", "project_label", "ask_user"])
        .ask("write")
        .ask("edit")
        .ask("patch")
        .ask("bash");
    policy.default_decision = PermissionDecision::Deny;
    Ok(crate::chapters::add_project_notes(
        SessionOptions::new()
            .with_permission_policy(policy)
            .with_confirmation_manager(confirmations)
            .with_harness(crate::chapters::coding_harness()?)
            .with_planning(false)
            .with_max_tool_rounds(12),
    ))
}

impl Runtime {
    pub async fn open(args: &Arguments) -> Result<Self> {
        let agent = Agent::new(args.config.to_str().context("ACL 路径必须是 UTF-8")?).await?;
        let (confirmations, confirmation_events) = confirmation_channel();
        let mut options = options(confirmations.clone())?;
        if args.prompt.is_some() {
            let mut policy = PermissionPolicy::new().allow_all(&["read", "ls", "project_label"]);
            policy.default_decision = PermissionDecision::Deny;
            options = options.with_permission_policy(policy);
        }
        if let (Some(state), Some(id)) = (&args.state, &args.session_id) {
            tokio::fs::create_dir_all(state).await?;
            let state = state.canonicalize()?;
            ensure!(
                !state.starts_with(&args.workspace),
                "存储目录必须放在分析工作区之外"
            );
            let store = Arc::new(FileSessionStore::new(state).await?);
            let exists = store.exists(id).await?;
            ensure!(
                exists == args.resume,
                "会话是否已存在与 --resume 不符；不会覆盖已有会话"
            );
            if args.resume {
                let saved = store.load(id).await?.context("保存的会话缺失")?;
                ensure!(
                    std::path::Path::new(&saved.config.workspace).canonicalize()? == args.workspace,
                    "保存的会话属于另一个工作区"
                );
            }
            options = options
                .with_session_id(id)
                .with_session_store(store)
                .with_auto_save(true);
        }
        let session = if args.resume {
            let session = agent
                .resume_session_async(args.session_id.as_deref().context("缺少会话 ID")?, options)
                .await?;
            if session.workspace() != args.workspace {
                session.close().await;
                agent.close().await;
                anyhow::bail!("保存的会话属于另一个工作区");
            }
            session
        } else {
            agent
                .session_async(
                    args.workspace.to_str().context("工作区路径必须是 UTF-8")?,
                    Some(options),
                )
                .await?
        };
        Self::assemble(agent, session, confirmations, confirmation_events)
    }

    pub fn assemble(
        agent: Agent,
        session: AgentSession,
        confirmations: Arc<ConfirmationManager>,
        confirmation_events: broadcast::Receiver<AgentEvent>,
    ) -> Result<Self> {
        session.register_dynamic_tool(Arc::new(crate::ProjectLabel))?;
        Ok(Self {
            agent,
            session,
            confirmations,
            confirmation_events,
        })
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.confirmations.cancel_all().await;
        let settled = self
            .session
            .cancel_and_settle(Duration::from_secs(5), Duration::from_secs(1))
            .await;
        let saved: Result<()> = if self.session.session_store().is_some() {
            match tokio::time::timeout(Duration::from_secs(5), self.session.save()).await {
                Ok(result) => result.map_err(Into::into),
                Err(error) => Err(error).context("保存会话超时"),
            }
        } else {
            Ok(())
        };
        self.session.close().await;
        self.agent.close().await;
        ensure!(settled, "原运行尚未确认退出，不能报告安全收尾");
        saved?;
        Ok(())
    }
}

pub async fn inspect_stream(session: &AgentSession, prompt: &str) -> Result<()> {
    let mut stream = session.stream_with_run(prompt, None).await?;
    while let Some(event) = stream.events.recv().await {
        match event {
            AgentEvent::TextDelta { text } => print!("{text}"),
            AgentEvent::Error { message } => eprintln!("运行错误：{message}"),
            AgentEvent::ToolEnd {
                name, exit_code, ..
            } => eprintln!("{name}：{exit_code}"),
            _ => {}
        }
    }
    stream.worker.await?;
    if let Some(run) = stream.run {
        let status = run.status().await;
        eprintln!("\n运行 {}：{status:?}", run.id());
        ensure!(
            status == Some(a3s_code_core::run::RunStatus::Completed),
            "原运行未完成：{status:?}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_refuse_ambiguous_persistence_and_duplicate_flags() {
        let workspace = tempfile::tempdir().unwrap();
        let parse = |extra: &[&str]| {
            let mut args = vec![
                OsString::from("host.acl"),
                workspace.path().as_os_str().to_owned(),
            ];
            args.extend(extra.iter().map(OsString::from));
            Arguments::parse(args)
        };
        assert!(parse(&["--resume"]).is_err());
        assert!(parse(&["--state", "/unused"]).is_err());
        assert!(parse(&["--session", "id"]).is_err());
        assert!(parse(&["--state", "/unused", "--session", ""]).is_err());
        assert!(parse(&["--headless", ""]).is_err());
        assert!(parse(&["--unknown"]).is_err());
        assert!(
            parse(&[
                "--state",
                "/unused",
                "--session",
                "id",
                "--resume",
                "--resume"
            ])
            .is_err()
        );
        assert!(
            parse(&["--state", "/unused", "--session", "id", "--resume"])
                .unwrap()
                .resume
        );
    }
}
