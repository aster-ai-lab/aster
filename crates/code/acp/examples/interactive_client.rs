//! Small ACP client with explicit permission, question and form replies.
//! Run from the workspace root; see the ACP client walkthrough in apps/web.

use agent_client_protocol::{self as acp, Agent as _};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    cell::Cell,
    collections::VecDeque,
    future::Future,
    io::{BufRead, Read},
    path::PathBuf,
    pin::Pin,
    process::Stdio,
    rc::Rc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

enum Pending {
    Permission(
        acp::RequestPermissionRequest,
        oneshot::Sender<acp::RequestPermissionResponse>,
    ),
    Form(oneshot::Sender<acp::ExtResponse>),
}

fn cancelled() -> acp::RequestPermissionResponse {
    acp::RequestPermissionResponse::new(acp::RequestPermissionOutcome::Cancelled)
}
fn ext(value: Value) -> acp::ExtResponse {
    acp::ExtResponse::new(
        serde_json::value::to_raw_value(&value)
            .expect("JSON value serializes")
            .into(),
    )
}
fn cancel_pending(pending: &mut VecDeque<Pending>) {
    for item in pending.drain(..) {
        match item {
            Pending::Permission(_, tx) => {
                let _ = tx.send(cancelled());
            }
            Pending::Form(tx) => {
                let _ = tx.send(ext(json!({"outcome":"cancel"})));
            }
        }
    }
}

struct Client {
    requests: mpsc::Sender<Pending>,
    cancelling: Rc<Cell<bool>>,
}
#[async_trait::async_trait(?Send)]
impl acp::Client for Client {
    async fn session_notification(&self, args: acp::SessionNotification) -> acp::Result<()> {
        println!(
            "{}",
            serde_json::to_string(&args).map_err(|_| acp::Error::internal_error())?
        );
        Ok(())
    }
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        if self.cancelling.get() {
            return Ok(cancelled());
        }
        let view = serde_json::to_string_pretty(&args).map_err(|_| acp::Error::internal_error())?;
        if view.len() > 64 * 1024 {
            return Err(acp::Error::invalid_params().data("交互内容超过显示上限"));
        }
        eprintln!(
            "待处理的确认或问题：\n{view}\n/choose N 选择第 N 项；问题可用 /answer TEXT 回答"
        );
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Pending::Permission(args, tx))
            .await
            .map_err(|_| acp::Error::internal_error())?;
        Ok(rx.await.unwrap_or_else(|_| cancelled()))
    }
    async fn ext_method(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        if args.method.as_ref() != "a3s/form/request" {
            return Err(acp::Error::method_not_found());
        }
        if self.cancelling.get() {
            return Ok(ext(json!({"outcome":"cancel"})));
        }
        if args.params.get().len() > 64 * 1024 {
            return Err(acp::Error::invalid_params());
        }
        eprintln!(
            "结构化表单：\n{}\n/form {{...}} 提交 values 对象，/cancel 取消",
            args.params.get()
        );
        let (tx, rx) = oneshot::channel();
        self.requests
            .send(Pending::Form(tx))
            .await
            .map_err(|_| acp::Error::internal_error())?;
        Ok(rx
            .await
            .unwrap_or_else(|_| ext(json!({"outcome":"cancel"}))))
    }
    async fn ext_notification(&self, args: acp::ExtNotification) -> acp::Result<()> {
        println!("扩展通知：{} {}", args.method, args.params.get());
        Ok(())
    }
}

fn choose(
    args: &acp::RequestPermissionRequest,
    raw: &str,
) -> Result<acp::RequestPermissionResponse> {
    let number: usize = raw.parse().context("请输入从 1 开始的选项编号")?;
    let option = number
        .checked_sub(1)
        .and_then(|n| args.options.get(n))
        .context("选项不存在")?;
    Ok(acp::RequestPermissionResponse::new(
        acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
            option.option_id.clone(),
        )),
    ))
}
fn answer(
    args: &acp::RequestPermissionRequest,
    text: &str,
) -> Result<acp::RequestPermissionResponse> {
    ensure!(!text.trim().is_empty(), "回答不能为空");
    ensure!(
        args.meta
            .as_ref()
            .and_then(|m| m.get("question"))
            .and_then(|q| q.get("allowFreeText"))
            .and_then(Value::as_bool)
            == Some(true),
        "该请求不是可自由回答的业务问题"
    );
    Ok(cancelled().meta(serde_json::Map::from_iter([(
        "followup_message".into(),
        Value::String(text.into()),
    )])))
}
fn form(raw: &str) -> Result<acp::ExtResponse> {
    let values: Value = serde_json::from_str(raw)?;
    ensure!(values.is_object(), "表单 values 必须是对象");
    Ok(ext(json!({"values":values})))
}

fn input_lines() -> mpsc::Receiver<Result<String>> {
    let (tx, rx) = mpsc::channel(8);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        loop {
            let mut line = String::new();
            let result = (&mut input).take(8193).read_line(&mut line);
            let value = match result {
                Ok(0) => break,
                Ok(_) if line.len() <= 8192 => Ok(line.trim_end_matches(['\r', '\n']).to_owned()),
                Ok(_) => Err(anyhow::anyhow!("输入超过 8192 字节")),
                Err(error) => Err(error.into()),
            };
            let failed = value.is_err();
            if tx.blocking_send(value).is_err() || failed {
                break;
            }
        }
    });
    rx
}

type Turn = Pin<Box<dyn Future<Output = acp::Result<acp::PromptResponse>>>>;
async fn next_turn(turn: &mut Option<Turn>) -> acp::Result<acp::PromptResponse> {
    match turn {
        Some(turn) => turn.await,
        None => std::future::pending().await,
    }
}

async fn interact(
    conn: Rc<acp::ClientSideConnection>,
    workspace: PathBuf,
    restore: Option<String>,
    mut requests: mpsc::Receiver<Pending>,
    cancelling: Rc<Cell<bool>>,
) -> Result<()> {
    let initialized = conn
        .initialize(acp::InitializeRequest::new(acp::ProtocolVersion::V1))
        .await?;
    println!("初始化：{}", serde_json::to_string(&initialized)?);
    let session_id = match restore {
        Some(id) => {
            let id = acp::SessionId::new(id);
            conn.load_session(acp::LoadSessionRequest::new(id.clone(), workspace))
                .await?;
            id
        }
        None => {
            conn.new_session(acp::NewSessionRequest::new(workspace))
                .await?
                .session_id
        }
    };
    println!("会话 ID：{}", session_id.0);
    println!("输入任务；/cancel 取消，/model ID 切换，/quit 退出。交互回复不会授予其他请求权限。");
    let mut lines = input_lines();
    let mut turn: Option<Turn> = None;
    let mut pending = VecDeque::new();
    loop {
        tokio::select! {
            item = requests.recv() => match item { Some(item) => pending.push_back(item), None => break },
            result = next_turn(&mut turn), if turn.is_some() => {
                println!("本轮结果：{result:?}");
                turn = None;
                cancel_pending(&mut pending);
            }
            line = lines.recv() => {
                let Some(line) = line else { break; };
                let line = line?;
                if line == "/quit" { break; }
                if line == "/cancel" {
                    cancelling.set(true);
                    while let Ok(item) = requests.try_recv() { pending.push_back(item); }
                    cancel_pending(&mut pending);
                    conn.cancel(acp::CancelNotification::new(session_id.clone())).await?;
                } else if let Some(raw) = line.strip_prefix("/choose ") {
                    if let Some(Pending::Permission(request, _)) = pending.front() {
                        match choose(request, raw) {
                            Ok(response) => if let Some(Pending::Permission(_, tx)) = pending.pop_front() { let _ = tx.send(response); },
                            Err(error) => eprintln!("{error}"),
                        }
                    } else { eprintln!("当前没有待选择的确认或问题"); }
                } else if let Some(text) = line.strip_prefix("/answer ") {
                    if let Some(Pending::Permission(request, _)) = pending.front() {
                        match answer(request, text) {
                            Ok(response) => if let Some(Pending::Permission(_, tx)) = pending.pop_front() { let _ = tx.send(response); },
                            Err(error) => eprintln!("{error}"),
                        }
                    } else { eprintln!("当前没有待回答的问题"); }
                } else if let Some(raw) = line.strip_prefix("/form ") {
                    if matches!(pending.front(), Some(Pending::Form(_))) {
                        match form(raw) {
                            Ok(response) => if let Some(Pending::Form(tx)) = pending.pop_front() { let _ = tx.send(response); },
                            Err(error) => eprintln!("{error}"),
                        }
                    } else { eprintln!("当前没有表单"); }
                } else if let Some(model) = line.strip_prefix("/model ") {
                    if turn.is_some() { eprintln!("请先等待原运行收尾"); continue; }
                    println!("模型切换：{:?}", conn.set_session_model(acp::SetSessionModelRequest::new(session_id.clone(), acp::ModelId::new(model))).await);
                } else if turn.is_some() { eprintln!("当前仍在运行；请等待或 /cancel"); }
                else if !line.trim().is_empty() {
                    cancelling.set(false);
                    let request = acp::PromptRequest::new(session_id.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new(line))]);
                    let conn = conn.clone();
                    turn = Some(Box::pin(async move { conn.prompt(request).await }));
                }
            }
        }
    }
    cancelling.set(true);
    while let Ok(item) = requests.try_recv() {
        pending.push_back(item);
    }
    cancel_pending(&mut pending);
    if let Some(mut turn) = turn {
        conn.cancel(acp::CancelNotification::new(session_id))
            .await?;
        tokio::time::timeout(Duration::from_secs(10), &mut turn)
            .await
            .context("取消后未收到原 prompt 的终态")??;
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let binary = PathBuf::from(args.next().context("缺少 ACP 二进制路径")?).canonicalize()?;
    let config = PathBuf::from(args.next().context("缺少可信 ACL 路径")?).canonicalize()?;
    let workspace = PathBuf::from(args.next().context("缺少工作区路径")?).canonicalize()?;
    let restore = args
        .next()
        .map(|s| {
            s.into_string()
                .map_err(|_| anyhow::anyhow!("会话 ID 必须是 UTF-8"))
        })
        .transpose()?;
    ensure!(
        args.next().is_none(),
        "只接受 binary、ACL、workspace 和可选 sessionId"
    );
    let mut child = tokio::process::Command::new(binary)
        .arg("--cwd")
        .arg(&workspace)
        .arg("--config")
        .arg(config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let result = tokio::task::LocalSet::new()
        .run_until(async {
            let (tx, rx) = mpsc::channel(16);
            let cancelling = Rc::new(Cell::new(false));
            let client = Client {
                requests: tx,
                cancelling: cancelling.clone(),
            };
            let (conn, io) = acp::ClientSideConnection::new(
                client,
                child.stdin.take().context("stdin 缺失")?.compat_write(),
                child.stdout.take().context("stdout 缺失")?.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let io = tokio::task::spawn_local(io);
            let result = interact(Rc::new(conn), workspace, restore, rx, cancelling).await;
            io.abort();
            result
        })
        .await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(question: bool) -> acp::RequestPermissionRequest {
        let request = acp::RequestPermissionRequest::new(
            acp::SessionId::new("test"),
            acp::ToolCallUpdate::new(
                acp::ToolCallId::new("exact-tool"),
                acp::ToolCallUpdateFields::new(),
            ),
            vec![acp::PermissionOption::new(
                acp::PermissionOptionId::new("exact-choice"),
                "选项",
                acp::PermissionOptionKind::RejectOnce,
            )],
        );
        if question {
            request.meta(serde_json::Map::from_iter([(
                "question".into(),
                json!({"allowFreeText":true}),
            )]))
        } else {
            request
        }
    }
    #[test]
    fn replies_preserve_option_identity_and_do_not_turn_answers_into_approval() {
        let response = choose(&request(false), "1").unwrap();
        assert!(
            matches!(response.outcome, acp::RequestPermissionOutcome::Selected(s) if s.option_id.0.as_ref() == "exact-choice")
        );
        assert!(choose(&request(false), "0").is_err());
        assert!(answer(&request(false), "yes").is_err());
        let response = answer(&request(true), "业务回答").unwrap();
        assert!(matches!(
            response.outcome,
            acp::RequestPermissionOutcome::Cancelled
        ));
        assert_eq!(response.meta.unwrap()["followup_message"], "业务回答");
    }
    #[test]
    fn form_replies_require_an_object_and_cancel_drains_pending_requests() {
        assert!(form("[]").is_err());
        assert!(form(r#"{"format":"csv"}"#).is_ok());
        let (tx, mut rx) = oneshot::channel();
        let mut pending = VecDeque::from([Pending::Permission(request(false), tx)]);
        cancel_pending(&mut pending);
        assert!(pending.is_empty());
        assert!(matches!(
            rx.try_recv().unwrap().outcome,
            acp::RequestPermissionOutcome::Cancelled
        ));
    }
}
