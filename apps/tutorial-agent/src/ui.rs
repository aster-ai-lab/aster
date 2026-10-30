use std::collections::VecDeque;

use a3s_code_core::{AgentEvent, AgentRunStream, hitl::PendingConfirmationInfo};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph, Wrap},
};

use crate::runtime::Runtime;

const REPLY_LIMIT: usize = 128 * 1024;
const CONFIRMATION_LIMIT: usize = 32 * 1024;

#[derive(Default)]
pub struct App {
    pub input: String,
    pub reply: String,
    pub status: String,
    pub scroll: u16,
    pub pending: Vec<PendingConfirmationInfo>,
    selected: Option<String>,
    turn: Option<usize>,
    log: VecDeque<String>,
    truncated: bool,
    questions: VecDeque<Question>,
    interaction_scroll: u16,
}

struct Question {
    id: String,
    text: String,
    options: Vec<String>,
    allow_free_text: bool,
    selected: usize,
}

fn append_bounded(target: &mut String, text: &str, limit: usize) -> bool {
    let mut end = text.len().min(limit.saturating_sub(target.len()));
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    target.push_str(&text[..end]);
    end != text.len()
}

impl App {
    fn record(&mut self, text: impl AsRef<str>) {
        if self.log.len() == 64 {
            self.log.pop_front();
        }
        let mut entry = String::new();
        if append_bounded(&mut entry, text.as_ref(), 16 * 1024) {
            entry.push_str("\n[显示已截断]");
        }
        self.log.push_back(entry);
    }

    pub fn apply(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::TurnStart { turn } => {
                if self.turn != Some(turn) && !self.reply.is_empty() {
                    let previous = std::mem::take(&mut self.reply);
                    self.record(previous);
                }
                self.reply.clear();
                self.truncated = false;
                self.turn = Some(turn);
            }
            AgentEvent::TextDelta { text } => {
                self.truncated |= append_bounded(&mut self.reply, &text, REPLY_LIMIT)
            }
            AgentEvent::ToolStart { name, .. } => self.status = format!("准备 {name}"),
            AgentEvent::ToolExecutionStart { name, .. } => self.status = format!("执行 {name}"),
            AgentEvent::ToolEnd {
                name, exit_code, ..
            } => self.record(format!("{name}：退出码 {exit_code}")),
            AgentEvent::End {
                completion_terminal,
                verification_summary,
                forced_stop,
                ..
            } => {
                self.record(format!("完成检查：{completion_terminal:?}\n验证：{verification_summary:?}\n停止原因：{forced_stop:?}"));
                self.status = "模型已结束，等待原运行收尾".into();
            }
            AgentEvent::Error { message } => self.record(format!("错误：{message}")),
            AgentEvent::UserQuestion {
                question_id,
                question,
                options,
                allow_free_text,
                form,
            } => {
                if form.is_some() {
                    self.status =
                        "当前练习界面不支持结构化表单，请取消并使用 ACP 表单客户端".into();
                } else {
                    if self.questions.len() < 3
                        && !self.questions.iter().any(|q| q.id == question_id)
                    {
                        self.questions.push_back(Question {
                            id: question_id,
                            text: question,
                            options,
                            allow_free_text,
                            selected: 0,
                        });
                    }
                    self.status = "问题待回答：F8 切换选项，F9 提交选项或输入框中的回答".into();
                }
            }
            _ => {}
        }
    }

    pub fn update_pending(&mut self, mut pending: Vec<PendingConfirmationInfo>) {
        pending.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
        if !pending
            .iter()
            .any(|p| Some(&p.tool_id) == self.selected.as_ref())
        {
            self.selected = pending.first().map(|p| p.tool_id.clone());
            self.interaction_scroll = 0;
        }
        self.pending = pending;
    }

    pub fn selected_confirmation(&self) -> Option<&PendingConfirmationInfo> {
        self.pending
            .iter()
            .find(|p| Some(&p.tool_id) == self.selected.as_ref())
    }

    fn next_confirmation(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let position = self
            .pending
            .iter()
            .position(|p| Some(&p.tool_id) == self.selected.as_ref())
            .unwrap_or(0);
        self.selected = Some(
            self.pending[(position + 1) % self.pending.len()]
                .tool_id
                .clone(),
        );
        self.interaction_scroll = 0;
    }

    fn answer_question(&mut self) -> anyhow::Result<()> {
        let question = self
            .questions
            .front()
            .ok_or_else(|| anyhow::anyhow!("当前没有待回答的问题"))?;
        let answer = if self.input.trim().is_empty() {
            question
                .options
                .get(question.selected)
                .cloned()
                .unwrap_or_default()
        } else {
            self.input.clone()
        };
        anyhow::ensure!(!answer.trim().is_empty(), "请填写回答或选择一个选项");
        anyhow::ensure!(
            question.allow_free_text || question.options.contains(&answer),
            "这道问题只接受列出的选项"
        );
        let _ = a3s_code_core::ask_user::answer(&question.id, &answer);
        self.input.clear();
        self.questions.pop_front();
        self.interaction_scroll = 0;
        self.status = "回答已提交，等待原运行处理".into();
        Ok(())
    }

    pub fn draw(&self, frame: &mut ratatui::Frame) {
        let [body, interaction, input, status] = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(8),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .areas(frame.area());
        let text = self
            .log
            .iter()
            .map(String::as_str)
            .chain([self.reply.as_str()])
            .collect::<Vec<_>>()
            .join("\n\n");
        let marker = if self.truncated {
            " · 显示已截断，原记录保留在 Core"
        } else {
            ""
        };
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((self.scroll, 0))
                .block(Block::bordered().title(format!("对话与工具{marker}"))),
            body,
        );
        let detail = if let Some(item) = self.selected_confirmation() {
            let raw = serde_json::to_string_pretty(&item.args).unwrap_or_default();
            let mut display = String::new();
            if append_bounded(&mut display, &raw, CONFIRMATION_LIMIT) {
                display.push_str("\n[参数超出显示上限，禁止批准，请拒绝并缩小请求]");
            }
            format!(
                "待确认 {} 项 · F5 切换 · F6 批准 · F7 拒绝\n{}\nID: {} · 剩余 {}ms\nF3 / F4 滚动参数\n{}",
                self.pending.len(),
                item.tool_name,
                item.tool_id,
                item.remaining_ms,
                display
            )
        } else if let Some(q) = self.questions.front() {
            format!(
                "业务提问：{}\n{}\n选项：{}\nF8 切换 · F9 提交回答",
                q.id,
                q.text,
                q.options
                    .get(q.selected)
                    .map(String::as_str)
                    .unwrap_or("在输入框填写答案")
            )
        } else {
            "当前没有待处理的交互\nF2 保存 · Esc 取消当前 Run · Ctrl+C 退出\nPageUp / PageDown 阅读回复".into()
        };
        frame.render_widget(
            Paragraph::new(detail)
                .wrap(Wrap { trim: false })
                .scroll((self.interaction_scroll, 0))
                .block(Block::bordered().title("确认与问题")),
            interaction,
        );
        frame.render_widget(
            Paragraph::new(self.input.as_str())
                .block(Block::bordered().title("输入任务 · Enter 发送")),
            input,
        );
        frame.render_widget(
            Paragraph::new(self.status.as_str()).wrap(Wrap { trim: false }),
            status,
        );
    }
}

struct Active {
    stream: AgentRunStream,
    events_open: bool,
    worker_result: Option<Result<(), tokio::task::JoinError>>,
}

enum Update {
    Event(AgentEvent),
    Progress,
    Finished,
}

async fn next_update(active: &mut Option<Active>) -> Update {
    let Some(active) = active else {
        return std::future::pending().await;
    };
    if !active.events_open && active.worker_result.is_some() {
        return Update::Finished;
    }
    tokio::select! {
        event = active.stream.events.recv(), if active.events_open => {
            match event {
                Some(event) => Update::Event(event),
                None => { active.events_open = false; Update::Progress }
            }
        }
        result = &mut active.stream.worker, if active.worker_result.is_none() => {
            active.worker_result = Some(result);
            Update::Progress
        }
    }
}

pub async fn run(
    runtime: &mut Runtime,
    terminal: &mut ratatui::DefaultTerminal,
) -> anyhow::Result<()> {
    let mut app = App::default();
    let mut keys = EventStream::new();
    let mut active: Option<Active> = None;
    let mut refresh = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        tokio::select! {
            event = keys.next() => {
                let Some(event) = event else { break; };
                let Event::Key(key) = event? else { continue; };
                if key.kind != KeyEventKind::Press { continue; }
                let code = if key.modifiers.contains(KeyModifiers::ALT) {
                    match key.code {
                        KeyCode::Char('y') => KeyCode::F(6),
                        KeyCode::Char('n') => KeyCode::F(7),
                        KeyCode::Char('j') => KeyCode::F(5),
                        KeyCode::Char('o') => KeyCode::F(8),
                        KeyCode::Char('a') => KeyCode::F(9),
                        code => code,
                    }
                } else { key.code };
                match code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::PageUp => app.scroll = app.scroll.saturating_sub(8),
                    KeyCode::PageDown => app.scroll = app.scroll.saturating_add(8),
                    KeyCode::Esc => if let Some(run) = active.as_ref().and_then(|a| a.stream.run.as_ref()) {
                        let accepted = run.cancel().await;
                        app.status = format!("取消请求：{accepted}，等待原运行收尾");
                    },
                    KeyCode::F(2) => app.status = if runtime.session.session_store().is_none() {
                        "未配置 --state 和 --session，不能持久保存".into()
                    } else if active.is_some() {
                        "请等待当前 Run 收尾后再显式保存".into()
                    } else { match runtime.session.save().await { Ok(()) => "会话已保存".into(), Err(error) => format!("保存失败：{error}") } },
                    KeyCode::F(5) => app.next_confirmation(),
                    KeyCode::F(3) => app.interaction_scroll = app.interaction_scroll.saturating_sub(3),
                    KeyCode::F(4) => app.interaction_scroll = app.interaction_scroll.saturating_add(3),
                    KeyCode::F(6) | KeyCode::F(7) => {
                        if code == KeyCode::F(6) && app.selected_confirmation().is_some_and(|p| serde_json::to_string_pretty(&p.args).map_or(true, |text| text.len() > CONFIRMATION_LIMIT)) {
                            app.status = "参数超出显示上限，禁止批准；请拒绝并缩小请求".into();
                            continue;
                        }
                        if let Some(id) = app.selected_confirmation().map(|p| p.tool_id.clone()) {
                            app.status = match runtime.confirmations.confirm(&id, code == KeyCode::F(6), None).await {
                                Ok(true) => format!("已回答确认 {id}，等待执行结果"),
                                Ok(false) => "确认已过期或已处理，正在刷新".into(),
                                Err(error) => format!("确认失败：{error}"),
                            };
                            app.update_pending(runtime.confirmations.pending_confirmation_details().await);
                        }
                    }
                    KeyCode::F(8) => if let Some(q) = app.questions.front_mut() { if !q.options.is_empty() { q.selected = (q.selected + 1) % q.options.len(); } },
                    KeyCode::F(9) => if let Err(error) = app.answer_question() { app.status = error.to_string(); },
                    KeyCode::Backspace => { app.input.pop(); }
                    KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) && app.input.len() + c.len_utf8() <= 8192 => app.input.push(c),
                    KeyCode::Enter if active.is_none() && !app.input.trim().is_empty() => {
                        let prompt = std::mem::take(&mut app.input);
                        if !app.reply.is_empty() { let previous = std::mem::take(&mut app.reply); app.record(previous); }
                        app.record(format!("你：{prompt}"));
                        app.scroll = 0; app.turn = None; app.truncated = false; app.questions.clear();
                        app.status = "正在提交".into();
                        match runtime.session.stream_with_run(&prompt, None).await {
                            Ok(stream) => active = Some(Active { stream, events_open: true, worker_result: None }),
                            Err(error) => app.status = format!("提交失败：{error}"),
                        }
                    }
                    _ => {}
                }
            }
            update = next_update(&mut active), if active.is_some() => {
                match update {
                    Update::Event(event) => app.apply(event),
                    Update::Progress => {},
                    Update::Finished => if let Some(completed) = active.take() {
                        app.questions.clear();
                        app.status = match completed.worker_result {
                            Some(Err(error)) => format!("运行线程失败：{error}"),
                            _ => match completed.stream.run {
                                Some(run) => format!("运行 {}：{:?}", run.id(), run.status().await),
                                None => "命令处理结束".into(),
                            }
                        };
                    }
                }
            }
            event = runtime.confirmation_events.recv() => {
                match event {
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => app.update_pending(runtime.confirmations.pending_confirmation_details().await),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        runtime.confirmations.cancel_all().await;
                        anyhow::bail!("确认通道已关闭，停止批准操作");
                    }
                }
            }
            _ = refresh.tick() => if !app.pending.is_empty() { app.update_pending(runtime.confirmations.pending_confirmation_details().await); }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_replaces_provisional_text_and_reports_truncation() {
        let mut app = App::default();
        app.apply(AgentEvent::TurnStart { turn: 1 });
        app.apply(AgentEvent::TextDelta {
            text: "旧的回复".into(),
        });
        app.apply(AgentEvent::TurnStart { turn: 1 });
        app.apply(AgentEvent::TextDelta {
            text: "新回复".into(),
        });
        assert_eq!(app.reply, "新回复");
        app.apply(AgentEvent::TextDelta {
            text: "中文".repeat(REPLY_LIMIT),
        });
        assert!(app.truncated);
        assert!(app.reply.len() <= REPLY_LIMIT);
        assert!(app.reply.is_char_boundary(app.reply.len()));
    }

    #[test]
    fn confirmation_selection_tracks_exact_identity_when_list_changes() {
        let item = |id: &str| PendingConfirmationInfo {
            tool_id: id.into(),
            tool_name: "write".into(),
            args: serde_json::json!({}),
            remaining_ms: 1000,
        };
        let mut app = App::default();
        app.update_pending(vec![item("b"), item("a")]);
        app.next_confirmation();
        assert_eq!(app.selected_confirmation().unwrap().tool_id, "b");
        app.update_pending(vec![item("c"), item("b")]);
        assert_eq!(app.selected_confirmation().unwrap().tool_id, "b");
        app.update_pending(vec![item("c")]);
        assert_eq!(app.selected_confirmation().unwrap().tool_id, "c");
    }

    #[test]
    fn drawing_is_bounded_and_shows_the_selected_confirmation() {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        let mut app = App::default();
        app.update_pending(vec![PendingConfirmationInfo {
            tool_id: "exact-id".into(),
            tool_name: "write".into(),
            args: serde_json::json!({"file_path":"demo.txt"}),
            remaining_ms: 1000,
        }]);
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("exact-id"));
        assert!(text.contains("demo.txt"));
    }
}
