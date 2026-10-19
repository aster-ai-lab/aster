//! Scheduled tasks: persist, list, and fire them by opening an agent turn.
//! The ticker catches up a slot that is at most a day late. Older slots stay
//! missed until the user runs or resumes them, so a week-old prompt does not
//! start on its own.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use serde_json::{Value, json};

use super::catalog::{AgentCatalog, SessionCatalog};
use super::domain::{ScheduledTask, ScheduledTaskFile, TaskRun, new_id, now_iso, split_model_id};
use super::error::PlaneError;
use super::schedule::{CATCH_UP, format_instant, next_cron, next_trigger, parse_instant};
use super::store::PlaneStore;
use super::turns::{EventHub, INTERRUPT_PREFIX, TurnService};

const PAGE_MAX: usize = 500;
const LOG_LIMIT: usize = 50;

pub struct TaskService {
    store: Arc<PlaneStore>,
    agents: Arc<AgentCatalog>,
    sessions: Arc<SessionCatalog>,
    turns: Arc<TurnService>,
    hub: Arc<EventHub>,
    gate: Mutex<()>,
    armed: AtomicBool,
}

impl TaskService {
    pub fn new(
        store: Arc<PlaneStore>,
        agents: Arc<AgentCatalog>,
        sessions: Arc<SessionCatalog>,
        turns: Arc<TurnService>,
        hub: Arc<EventHub>,
    ) -> Self {
        Self {
            store,
            agents,
            sessions,
            turns,
            hub,
            gate: Mutex::new(()),
            armed: AtomicBool::new(false),
        }
    }

    /// One 15s pass for the process. The first tick runs immediately.
    /// The loop stays on the application runtime so a fired turn can spawn processes.
    pub fn arm(self: &Arc<Self>) {
        if self.armed.swap(true, Ordering::SeqCst) {
            return;
        }
        let service = Arc::clone(self);
        let run = |service: Arc<Self>| async move {
            let mut tick = tokio::time::interval(Duration::from_secs(15));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                service.tick();
            }
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            self.armed.store(false, Ordering::SeqCst);
            eprintln!("studio: scheduled task ticker needs the application runtime");
            return;
        };
        handle.spawn(run(service));
    }

    pub(super) fn ticker_started(&self) -> bool {
        self.armed.load(Ordering::SeqCst)
    }

    pub fn list(&self, agent_id: Option<&str>, page: usize, limit: usize) -> Value {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Utc::now().fixed_offset();
        let runs = self.store.runs();
        let mut rows: Vec<ScheduledTask> = self.store.tasks();
        if let Some(agent_id) = agent_id {
            rows.retain(|task| task.agent_id == agent_id);
        }
        rows.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        let total = rows.len();
        let limit = limit.clamp(1, PAGE_MAX);
        let page = page.max(1);
        let items: Vec<Value> = rows
            .iter()
            .skip((page - 1) * limit)
            .take(limit)
            .map(|task| self.list_item(task, &runs, now))
            .collect();
        json!({ "items": items, "total": total, "page": page })
    }

    pub fn get(&self, task_id: &str) -> Result<Value, PlaneError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let task = self.task(task_id)?;
        Ok(self.entity(&task, Utc::now().fixed_offset()))
    }

    pub fn logs(&self, agent_id: &str, task_id: &str, limit: usize) -> Result<Value, PlaneError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let task = self.task(task_id)?;
        if task.agent_id != agent_id {
            return Err(PlaneError::NotFound(format!("task {task_id}")));
        }
        let mut rows: Vec<TaskRun> = self
            .store
            .runs()
            .into_iter()
            .filter(|run| run.task_id == task_id)
            .collect();
        rows.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        let total = rows.len();
        let items: Vec<Value> = rows
            .iter()
            .take(limit.clamp(1, LOG_LIMIT))
            .map(log_json)
            .collect();
        Ok(json!({ "items": items, "total": total, "page": 1 }))
    }

    pub fn create(&self, body: &Value) -> Result<Value, PlaneError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let agent_id =
            text(body, "agentId").ok_or_else(|| PlaneError::bad("agentId is required"))?;
        self.agents.get(&agent_id)?;
        let now = Utc::now().fixed_offset();
        let task = self.draft(new_id(), &agent_id, body, None, now)?;
        let mut tasks = self.store.tasks();
        tasks.push(task.clone());
        self.store.save_tasks(&tasks)?;
        let view = self.entity(&task, now);
        self.changed(&task.id);
        Ok(view)
    }

    pub fn update(
        &self,
        agent_id: &str,
        task_id: &str,
        patch: &Value,
    ) -> Result<Value, PlaneError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = self.owned(agent_id, task_id)?;
        let now = Utc::now().fixed_offset();
        let task = self.draft(current.id.clone(), agent_id, patch, Some(&current), now)?;
        self.replace(task.clone())?;
        let view = self.entity(&task, now);
        self.changed(&task.id);
        Ok(view)
    }

    pub fn set_enabled(
        &self,
        agent_id: &str,
        task_id: &str,
        enabled: bool,
    ) -> Result<Value, PlaneError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut task = self.owned(agent_id, task_id)?;
        let now = Utc::now().fixed_offset();
        task.enabled = enabled;
        task.updated_at = now_iso();
        if enabled {
            task.next_run = upcoming(&task.trigger, now).unwrap_or(None);
        }
        self.replace(task.clone())?;
        let view = self.entity(&task, now);
        self.changed(&task.id);
        Ok(view)
    }

    pub fn delete(&self, agent_id: &str, task_id: &str) -> Result<(), PlaneError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _task = self.owned(agent_id, task_id)?;
        let mut tasks = self.store.tasks();
        tasks.retain(|task| task.id != task_id);
        self.store.save_tasks(&tasks)?;
        let mut runs = self.store.runs();
        runs.retain(|run| run.task_id != task_id);
        self.store.save_runs(&runs)?;
        self.changed(task_id);
        Ok(())
    }

    pub fn run(&self, agent_id: &str, task_id: &str) -> Result<(), PlaneError> {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut task = self.owned(agent_id, task_id)?;
        self.reconcile(Utc::now().fixed_offset());
        self.fire(&mut task)?;
        Ok(())
    }

    pub(super) fn tick(&self) {
        let _guard = self
            .gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Utc::now().fixed_offset();
        let interrupted = self.reconcile(now);
        self.requeue_interrupted(&interrupted, now);
        let due: Vec<String> = self
            .store
            .tasks()
            .into_iter()
            .filter(|task| is_due(task, now) && !self.running(&task.id))
            .map(|task| task.id)
            .collect();
        for task_id in due {
            let Ok(mut task) = self.task(&task_id) else {
                continue;
            };
            if let Err(error) = self.fire(&mut task) {
                eprintln!("studio: scheduled task {task_id} did not start: {error}");
            }
        }
    }

    fn reconcile(&self, now: DateTime<FixedOffset>) -> Vec<(String, String, String, u32)> {
        let mut runs = self.store.runs();
        let mut tasks = self.store.tasks();
        let mut changed = false;
        let mut interrupted = Vec::new();
        let planned: Vec<(usize, u32)> = runs
            .iter()
            .enumerate()
            .filter(|(_, run)| run.status == "running")
            .map(|(index, run)| (index, next_interrupt_attempt(&runs, &run.task_id, &run.id)))
            .collect();
        for (index, attempt) in planned {
            let run = &mut runs[index];
            let Some(session_id) = run.session_id.clone() else {
                continue;
            };
            // A restart has no live owner for its persisted pending reply.
            // Settle it before testing terminal status or blocking the next run.
            let elapsed = parse_instant(&run.started_at)
                .map(|started| now.signed_duration_since(started))
                .unwrap_or_default();
            let timeout = tasks
                .iter()
                .find(|task| task.id == run.task_id)
                .map(|task| task.timeout_minutes)
                .unwrap_or(0);
            if timeout > 0 && elapsed > chrono::Duration::minutes(i64::from(timeout)) {
                self.turns.abort(&session_id);
            }
            let Some(message_id) = run.message_id.clone() else {
                continue;
            };
            let Some(message) = self
                .store
                .messages(&session_id)
                .into_iter()
                .find(|message| message.id == message_id)
            else {
                continue;
            };
            let interrupt =
                message.status == "pending" || message.searchable_text.contains(INTERRUPT_PREFIX);
            if interrupt && message.status != "success" && message.status != "paused" {
                self.turns
                    .write_interrupt_note(&session_id, interrupt_reason(attempt), true);
            } else if message.status == "pending" {
                self.turns.settle_orphan_pending(&session_id);
            }
            let Some(message) = self
                .store
                .messages(&session_id)
                .into_iter()
                .find(|message| message.id == message_id)
            else {
                continue;
            };
            let status = match message.status.as_str() {
                "success" => "completed",
                "error" if message.searchable_text.contains(INTERRUPT_PREFIX) => "interrupted",
                "error" => "failed",
                "paused" => "cancelled",
                _ => continue,
            };
            if status == "interrupted" {
                interrupted.push((run.task_id.clone(), run.id.clone(), session_id, attempt));
                run.attempt = attempt;
            }
            run.status = status.to_owned();
            run.finished_at = Some(message.updated_at.clone());
            run.duration_ms = Some(elapsed.num_milliseconds().max(0));
            if status == "failed" || status == "interrupted" {
                run.error = Some(message.searchable_text);
            }
            if let Some(task) = tasks.iter_mut().find(|task| task.id == run.task_id) {
                finish_task(task, status);
            }
            changed = true;
        }
        if changed {
            let _ = self.store.save_runs(&runs);
            if let Err(error) = self.store.save_tasks(&tasks) {
                eprintln!("studio: scheduled task result was not saved: {error}");
            }
            self.hub.publish(
                "data.changed",
                json!({ "taskId": runs.last().map(|run| run.task_id.clone()).unwrap_or_default() }),
            );
        }
        interrupted
    }

    /// One replacement run after the owning process disappeared.
    /// Attempt 2 stays interrupted so a crash loop cannot refire forever.
    fn requeue_interrupted(
        &self,
        interrupted: &[(String, String, String, u32)],
        now: DateTime<FixedOffset>,
    ) {
        if interrupted.is_empty() {
            return;
        }
        let mut tasks = self.store.tasks();
        let mut changed = false;
        for (task_id, _run_id, session_id, attempt) in interrupted {
            changed |= queue_interrupt_retry(&mut tasks, task_id, session_id, *attempt, now);
        }
        if changed {
            if let Err(error) = self.store.save_tasks(&tasks) {
                eprintln!("studio: interrupted scheduled task was not requeued: {error}");
            }
        }
    }

    fn fire(&self, task: &mut ScheduledTask) -> Result<(), PlaneError> {
        if self.running(&task.id) {
            return Err(PlaneError::Conflict("task is already running".into()));
        }
        let now = Utc::now().fixed_offset();
        let started_turn = self.start_turn(task);
        let started = now_iso();
        let mut run = TaskRun {
            id: new_id(),
            task_id: task.id.clone(),
            session_id: None,
            message_id: None,
            started_at: started.clone(),
            finished_at: None,
            duration_ms: None,
            status: "running".into(),
            error: None,
            attempt: 1,
        };
        match &started_turn {
            Ok((session_id, message_id)) => {
                run.session_id = Some(session_id.clone());
                run.message_id = Some(message_id.clone());
                if task.reuse_session {
                    task.reuse_session_id = Some(session_id.clone());
                }
            }
            Err(error) => {
                run.status = "failed".into();
                run.finished_at = Some(started);
                run.duration_ms = Some(0);
                run.error = Some(error.to_string());
            }
        }
        let mut runs = self.store.runs();
        runs.push(run);
        trim_runs(&mut runs, &task.id);
        self.store.save_runs(&runs)?;
        task.last_run = Some(now_iso());
        task.updated_at = now_iso();
        task.resume_session_id = None;
        let once = task.trigger.get("kind").and_then(Value::as_str) == Some("once");
        if started_turn.is_ok() {
            // A one-shot stays due until the run succeeds. A repeating trigger moves
            // to its next slot now so a long run does not start twice.
            if !once {
                task.next_run = upcoming(&task.trigger, now).unwrap_or(task.next_run.clone());
            }
        } else if once {
            // A one-shot that failed to start stays runnable by hand and is not due on the next tick.
            task.next_run = None;
        } else {
            // A failed start must not retry on every tick.
            task.next_run = upcoming(&task.trigger, now).unwrap_or(task.next_run.clone());
        }
        self.replace(task.clone())?;
        self.changed(&task.id);
        started_turn.map(|_| ())
    }

    fn start_turn(&self, task: &ScheduledTask) -> Result<(String, String), PlaneError> {
        if task.workspace.get("type").and_then(Value::as_str) == Some("user")
            && task.workspace_path.is_none()
        {
            return Err(PlaneError::bad("workspace was not resolved"));
        }
        let session_id = task
            .resume_session_id
            .as_deref()
            .filter(|id| self.sessions.get(id).is_ok())
            .map(str::to_owned)
            .or_else(|| {
                if task.reuse_session {
                    task.reuse_session_id
                        .as_deref()
                        .filter(|id| self.sessions.get(id).is_ok())
                        .map(str::to_owned)
                } else {
                    None
                }
            });
        let session_id = match session_id {
            Some(id) => id,
            None => self.sessions.create(&session_body(task))?.id,
        };
        let turn = task_turn_body(task);
        let opened = self.turns.open(&session_id, &turn)?;
        let message_id = opened
            .get("reservedMessages")
            .and_then(Value::as_array)
            .and_then(|messages| messages.get(1))
            .and_then(|message| message.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| PlaneError::bad("turn did not start"))?
            .to_owned();
        Ok((session_id, message_id))
    }

    fn draft(
        &self,
        id: String,
        agent_id: &str,
        body: &Value,
        current: Option<&ScheduledTask>,
        now: DateTime<FixedOffset>,
    ) -> Result<ScheduledTask, PlaneError> {
        let name = text(body, "name")
            .or_else(|| current.map(|task| task.name.clone()))
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| PlaneError::bad("name is required"))?;
        let prompt = text(body, "prompt")
            .or_else(|| current.map(|task| task.prompt.clone()))
            .filter(|prompt| !prompt.trim().is_empty())
            .ok_or_else(|| PlaneError::bad("prompt is required"))?;
        let trigger = body
            .get("trigger")
            .cloned()
            .or_else(|| current.map(|task| task.trigger.clone()))
            .ok_or_else(|| PlaneError::bad("trigger is required"))?;
        if upcoming(&trigger, now).is_none() {
            return Err(PlaneError::bad("invalid trigger"));
        }
        let workspace = body
            .get("workspace")
            .cloned()
            .or_else(|| current.map(|task| task.workspace.clone()))
            .ok_or_else(|| PlaneError::bad("workspace is required"))?;
        let timeout = body
            .get("timeoutMinutes")
            .and_then(Value::as_u64)
            .map(|value| value as u32)
            .or_else(|| current.map(|task| task.timeout_minutes))
            .unwrap_or(0);
        let reuse_session = body
            .get("reuseSession")
            .and_then(Value::as_bool)
            .or_else(|| current.map(|task| task.reuse_session))
            .unwrap_or(false);
        let model = task_model(body, current)?;
        let skills = task_skills(body, current)?;
        let files = task_files(body, current)?;
        let channel_ids = body
            .get("channelIds")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .or_else(|| current.and_then(|task| task.channel_ids.clone()));
        let (workspace_path, workspace_name) = resolved_workspace(body, current);
        let trigger_changed = current.is_none_or(|task| task.trigger != trigger);
        let next_run = if trigger_changed {
            upcoming(&trigger, now).unwrap_or(None)
        } else {
            current.and_then(|task| task.next_run.clone())
        };
        let stamp = now_iso();
        Ok(ScheduledTask {
            id,
            agent_id: agent_id.to_owned(),
            name,
            prompt,
            trigger,
            timeout_minutes: timeout,
            workspace,
            workspace_path,
            workspace_name,
            reuse_session,
            model,
            skills,
            files,
            reuse_session_id: if reuse_session {
                current.and_then(|task| task.reuse_session_id.clone())
            } else {
                None
            },
            resume_session_id: current.and_then(|task| task.resume_session_id.clone()),
            channel_ids,
            next_run,
            last_run: current.and_then(|task| task.last_run.clone()),
            enabled: current.is_none_or(|task| task.enabled),
            fire_count: current.map(|task| task.fire_count).unwrap_or(0),
            created_at: current
                .map(|task| task.created_at.clone())
                .unwrap_or_else(|| stamp.clone()),
            updated_at: stamp,
        })
    }

    fn list_item(
        &self,
        task: &ScheduledTask,
        runs: &[TaskRun],
        now: DateTime<FixedOffset>,
    ) -> Value {
        let mut view = self.entity(task, now);
        let summary = runs
            .iter()
            .rev()
            .find(|run| run.task_id == task.id)
            .map(run_summary);
        if let Some(object) = view.as_object_mut() {
            object.insert("runSummary".into(), summary.unwrap_or(Value::Null));
        }
        view
    }

    fn entity(&self, task: &ScheduledTask, now: DateTime<FixedOffset>) -> Value {
        json!({
            "id": task.id,
            "agentId": task.agent_id,
            "name": task.name,
            "prompt": task.prompt,
            "trigger": task.trigger,
            "timeoutMinutes": task.timeout_minutes,
            "workspace": task.workspace,
            "reuseSession": task.reuse_session,
            "reuseSessionId": task.reuse_session_id,
            "model": task.model,
            "skills": task.skills,
            "files": task.files,
            "channelIds": task.channel_ids.clone().unwrap_or_default(),
            "nextRun": task.next_run,
            "lastRun": task.last_run,
            "enabled": task.enabled,
            "status": status_of(task, now),
            "createdAt": task.created_at,
            "updatedAt": task.updated_at,
        })
    }

    fn owned(&self, agent_id: &str, task_id: &str) -> Result<ScheduledTask, PlaneError> {
        let task = self.task(task_id)?;
        if task.agent_id != agent_id {
            return Err(PlaneError::NotFound(format!("task {task_id}")));
        }
        Ok(task)
    }

    fn task(&self, task_id: &str) -> Result<ScheduledTask, PlaneError> {
        self.store
            .tasks()
            .into_iter()
            .find(|task| task.id == task_id)
            .ok_or_else(|| PlaneError::NotFound(format!("task {task_id}")))
    }

    fn replace(&self, task: ScheduledTask) -> Result<(), PlaneError> {
        let mut tasks = self.store.tasks();
        match tasks.iter_mut().find(|item| item.id == task.id) {
            Some(slot) => *slot = task,
            None => tasks.push(task),
        }
        self.store.save_tasks(&tasks)?;
        Ok(())
    }

    fn running(&self, task_id: &str) -> bool {
        self.store
            .runs()
            .iter()
            .any(|run| run.task_id == task_id && run.status == "running")
    }

    fn changed(&self, task_id: &str) {
        self.hub
            .publish("data.changed", json!({ "taskId": task_id }));
    }
}

fn task_turn_body(task: &ScheduledTask) -> Value {
    let mut turn = json!({ "userMessageParts": task_message_parts(task) });
    if let Some(model) = task.model.as_deref() {
        turn["model"] = json!(model);
    }
    turn
}

fn task_message_parts(task: &ScheduledTask) -> Vec<Value> {
    let mut parts = vec![json!({ "type": "text", "text": task_prompt(task) })];
    for file in &task.files {
        parts.push(json!({
            "type": "file",
            "mediaType": "application/octet-stream",
            "url": file_url(&file.path),
            "filename": file.name,
        }));
    }
    parts
}

fn task_prompt(task: &ScheduledTask) -> String {
    if task.skills.is_empty() {
        return task.prompt.clone();
    }
    let mut prompt = String::new();
    for skill in &task.skills {
        prompt.push_str("使用技能 ");
        prompt.push_str(skill);
        prompt.push_str("：\n");
    }
    prompt.push_str(&task.prompt);
    prompt
}

fn file_url(path: &str) -> String {
    let mut url = String::from("file://");
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                url.push(byte as char);
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

fn task_skills(body: &Value, current: Option<&ScheduledTask>) -> Result<Vec<String>, PlaneError> {
    let Some(value) = body.get("skills") else {
        return Ok(current.map(|task| task.skills.clone()).unwrap_or_default());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err(PlaneError::bad("skills must be a list of names"));
    };
    if items.len() > 8 {
        return Err(PlaneError::bad("at most 8 skills"));
    }
    let mut skills = Vec::with_capacity(items.len());
    for item in items {
        let Some(name) = item
            .as_str()
            .map(str::trim)
            .filter(|name| skill_name_ok(name))
        else {
            return Err(PlaneError::bad("skill name is invalid"));
        };
        if !skills.iter().any(|skill| skill == name) {
            skills.push(name.to_owned());
        }
    }
    Ok(skills)
}

fn skill_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '/' || ch == '\\')
}

fn task_files(
    body: &Value,
    current: Option<&ScheduledTask>,
) -> Result<Vec<ScheduledTaskFile>, PlaneError> {
    let Some(value) = body.get("files") else {
        return Ok(current.map(|task| task.files.clone()).unwrap_or_default());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err(PlaneError::bad("files must be a list"));
    };
    if items.len() > 20 {
        return Err(PlaneError::bad("at most 20 files"));
    }
    let mut files: Vec<ScheduledTaskFile> = Vec::with_capacity(items.len());
    for item in items {
        let Some(path) = item.get("path").and_then(Value::as_str).map(str::trim) else {
            return Err(PlaneError::bad("file path is required"));
        };
        if !absolute_file_path(path) {
            return Err(PlaneError::bad("file path must be absolute"));
        }
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path).trim();
        if name.is_empty() || name.len() > 255 {
            return Err(PlaneError::bad("file name is invalid"));
        }
        if files.iter().any(|file| file.path == path) {
            continue;
        }
        files.push(ScheduledTaskFile {
            path: path.to_owned(),
            name: name.to_owned(),
        });
    }
    Ok(files)
}

fn absolute_file_path(path: &str) -> bool {
    if path.contains('\0') || path.split(['/', '\\']).any(|segment| segment == "..") {
        return false;
    }
    path.starts_with('/')
        || (path.len() > 2
            && path.as_bytes()[1] == b':'
            && matches!(path.as_bytes()[2], b'/' | b'\\'))
}

fn task_model(body: &Value, current: Option<&ScheduledTask>) -> Result<Option<String>, PlaneError> {
    match body.get("model") {
        None => Ok(current.and_then(|task| task.model.clone())),
        Some(Value::Null) => Ok(None),
        Some(Value::String(model)) if split_model_id(model).is_some() => Ok(Some(model.clone())),
        Some(_) => Err(PlaneError::bad("model must be providerId::modelId")),
    }
}

fn upcoming(trigger: &Value, after: DateTime<FixedOffset>) -> Option<Option<String>> {
    if trigger.get("kind").and_then(Value::as_str) == Some("cron") {
        let expr = trigger.get("expr")?.as_str()?;
        return Some(Some(format_instant(next_cron(expr, after)?)));
    }
    next_trigger(trigger, after)
}

pub(super) fn next_interrupt_attempt(runs: &[TaskRun], task_id: &str, current_id: &str) -> u32 {
    runs.iter()
        .rev()
        .find(|run| run.task_id == task_id && run.id != current_id && run.status != "running")
        .filter(|run| run.status == "interrupted")
        .map(|run| run.attempt.saturating_add(1))
        .unwrap_or(1)
        .max(1)
}

pub(super) fn interrupt_reason(attempt: u32) -> &'static str {
    if attempt < 2 {
        super::turns::INTERRUPTED_RETRYING
    } else {
        super::turns::INTERRUPTED_EXHAUSTED
    }
}

pub(super) fn queue_interrupt_retry(
    tasks: &mut [ScheduledTask],
    task_id: &str,
    session_id: &str,
    attempt: u32,
    now: DateTime<FixedOffset>,
) -> bool {
    if attempt >= 2 || session_id.is_empty() {
        return false;
    }
    let Some(task) = tasks.iter_mut().find(|task| task.id == task_id) else {
        return false;
    };
    let once = task.trigger.get("kind").and_then(Value::as_str) == Some("once");
    if !task.enabled && !once {
        return false;
    }
    task.enabled = true;
    task.next_run = Some(format_instant(now));
    task.resume_session_id = Some(session_id.to_owned());
    task.updated_at = now_iso();
    true
}

fn finish_task(task: &mut ScheduledTask, status: &str) {
    let once = task.trigger.get("kind").and_then(Value::as_str) == Some("once");
    match status {
        "completed" => {
            task.fire_count = task.fire_count.saturating_add(1);
            let limit = task
                .trigger
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if once || (limit > 0 && u64::from(task.fire_count) >= limit) {
                task.next_run = None;
                task.enabled = false;
            }
            task.updated_at = now_iso();
        }
        "failed" | "cancelled" if once => {
            task.next_run = None;
            task.updated_at = now_iso();
        }
        _ => {}
    }
}

fn is_due(task: &ScheduledTask, now: DateTime<FixedOffset>) -> bool {
    if !task.enabled || status_of(task, now) == "missed" {
        return false;
    }
    task.next_run
        .as_deref()
        .and_then(parse_instant)
        .is_some_and(|next| next <= now)
}

fn status_of(task: &ScheduledTask, now: DateTime<FixedOffset>) -> &'static str {
    if !task.enabled {
        if task.trigger.get("kind").and_then(Value::as_str) == Some("once")
            && task.last_run.is_some()
        {
            return "completed";
        }
        return "paused";
    }
    if let Some(next) = task.next_run.as_deref().and_then(parse_instant) {
        if now.signed_duration_since(next) > CATCH_UP {
            return "missed";
        }
    }
    "active"
}

fn resolved_workspace(
    body: &Value,
    current: Option<&ScheduledTask>,
) -> (Option<String>, Option<String>) {
    let path = text(body, "workspacePath")
        .or_else(|| current.and_then(|task| task.workspace_path.clone()));
    let name = text(body, "workspaceName")
        .or_else(|| current.and_then(|task| task.workspace_name.clone()));
    if body.get("workspace").is_some() && text(body, "workspacePath").is_none() && current.is_some()
    {
        let same = current.is_some_and(|task| body.get("workspace") == Some(&task.workspace));
        if !same {
            return (None, None);
        }
    }
    (path, name)
}

fn session_body(task: &ScheduledTask) -> Value {
    let kind = task.workspace.get("type").and_then(Value::as_str);
    if kind == Some("user") {
        let Some(path) = task.workspace_path.clone() else {
            return json!({ "agentId": task.agent_id, "name": task.name, "workspace": { "type": "system" } });
        };
        let id = task
            .workspace
            .get("workspaceId")
            .and_then(Value::as_str)
            .unwrap_or("workspace");
        return json!({
            "agentId": task.agent_id,
            "name": task.name,
            "workspace": {
                "type": "user",
                "workspace": {
                    "id": id,
                    "name": task.workspace_name.clone().unwrap_or_else(|| id.to_owned()),
                    "path": path,
                    "type": "user",
                    "orderKey": "a0",
                    "createdAt": task.created_at,
                    "updatedAt": task.updated_at,
                }
            }
        });
    }
    json!({
        "agentId": task.agent_id,
        "name": task.name,
        "workspace": { "type": "system" }
    })
}

fn run_summary(run: &TaskRun) -> Value {
    match run.status.as_str() {
        "running" => json!({ "status": "running" }),
        "failed" => json!({ "status": "failed", "finishedAt": run.finished_at }),
        "cancelled" => json!({ "status": "cancelled", "finishedAt": run.finished_at }),
        _ => json!({ "status": "completed", "finishedAt": run.finished_at }),
    }
}

fn log_json(run: &TaskRun) -> Value {
    json!({
        "id": run.id,
        "scheduleId": run.task_id,
        "sessionId": run.session_id,
        "startedAt": run.started_at,
        "durationMs": run.duration_ms,
        "status": run.status,
        "error": run.error,
    })
}

fn trim_runs(runs: &mut Vec<TaskRun>, task_id: &str) {
    let mut kept = runs.iter().filter(|run| run.task_id == task_id).count();
    if kept <= LOG_LIMIT {
        return;
    }
    runs.retain(|run| {
        if run.task_id != task_id || kept <= LOG_LIMIT {
            return true;
        }
        kept -= 1;
        false
    });
}

fn text(body: &Value, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_owned)
}

impl std::fmt::Display for PlaneError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(message)
            | Self::BadRequest(message)
            | Self::Conflict(message)
            | Self::Internal(message) => formatter.write_str(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_plane::PlaneConfig;

    fn service() -> TaskService {
        let store = Arc::new(PlaneStore::open(None));
        let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
        let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
        let hub = Arc::new(EventHub::new());
        let config = Arc::new(PlaneConfig {
            root: None,
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        });
        let turns = Arc::new(TurnService::new(
            Arc::clone(&store),
            Arc::clone(&agents),
            Arc::clone(&sessions),
            config,
            Arc::clone(&hub),
        ));
        agents
            .create(&json!({
                "type": "a3s-code",
                "name": "Planner",
                "model": "openai::gpt"
            }))
            .unwrap();
        TaskService::new(store, agents, sessions, turns, hub)
    }

    fn agent_id(service: &TaskService) -> String {
        service.agents.list(None, &[], 1, 10)["items"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn creates_lists_and_pauses_without_firing() {
        let service = service();
        let agent_id = agent_id(&service);
        let created = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Daily",
                "prompt": "Summarize the inbox",
                "trigger": { "kind": "cron", "expr": "0 9 * * *" },
                "workspace": { "type": "system" },
                "timeoutMinutes": 15,
                "reuseSession": false
            }))
            .unwrap();
        assert_eq!(created["status"], "active");
        assert!(created["nextRun"].as_str().is_some());
        let id = created["id"].as_str().unwrap();
        let listed = service.list(None, 1, 50);
        assert_eq!(listed["total"], 1);
        assert_eq!(listed["items"][0]["runSummary"], Value::Null);

        let paused = service.set_enabled(&agent_id, id, false).unwrap();
        assert_eq!(paused["status"], "paused");
        assert_eq!(paused["enabled"], false);
        service.delete(&agent_id, id).unwrap();
        assert_eq!(service.list(None, 1, 50)["total"], 0);
    }

    #[test]
    fn stores_a_chosen_model_and_rejects_a_bare_name() {
        let service = service();
        let agent_id = agent_id(&service);
        let created = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Daily",
                "prompt": "Summarize the inbox",
                "trigger": { "kind": "cron", "expr": "0 9 * * *" },
                "workspace": { "type": "system" },
                "model": "anthropic::claude-sonnet"
            }))
            .unwrap();
        assert_eq!(created["model"], "anthropic::claude-sonnet");
        let id = created["id"].as_str().unwrap();
        let cleared = service
            .update(&agent_id, id, &json!({ "model": Value::Null }))
            .unwrap();
        assert_eq!(cleared["model"], Value::Null);
        let error = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Broken model",
                "prompt": "Go",
                "trigger": { "kind": "interval", "ms": 60000 },
                "workspace": { "type": "system" },
                "model": "claude"
            }))
            .unwrap_err();
        assert!(error.to_string().contains("providerId::modelId"));
    }

    #[test]
    fn stores_skills_and_workspace_files_for_the_fired_turn() {
        let service = service();
        let agent_id = agent_id(&service);
        let created = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Review notes",
                "prompt": "Summarize the attached notes",
                "trigger": { "kind": "interval", "ms": 60000 },
                "workspace": { "type": "system" },
                "skills": ["pdf", "pdf"],
                "files": [{ "path": "/tmp/a b.md", "name": "ignored" }]
            }))
            .unwrap();
        assert_eq!(created["skills"], json!(["pdf"]));
        assert_eq!(created["files"][0]["path"], "/tmp/a b.md");
        assert_eq!(created["files"][0]["name"], "a b.md");
        let stored = service.task(created["id"].as_str().unwrap()).unwrap();
        let parts = task_message_parts(&stored);
        assert_eq!(
            parts[0]["text"],
            "使用技能 pdf：\nSummarize the attached notes"
        );
        assert_eq!(parts[1]["url"], "file:///tmp/a%20b.md");
        assert_eq!(parts[1]["filename"], "a b.md");

        let cleared = service
            .update(
                &agent_id,
                stored.id.as_str(),
                &json!({ "skills": [], "files": [] }),
            )
            .unwrap();
        assert_eq!(cleared["skills"], json!([]));
        assert_eq!(cleared["files"], json!([]));

        let relative = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Bad file",
                "prompt": "Go",
                "trigger": { "kind": "interval", "ms": 60000 },
                "workspace": { "type": "system" },
                "files": [{ "path": "notes.md", "name": "notes.md" }]
            }))
            .unwrap_err();
        assert!(relative.to_string().contains("absolute"));
    }

    #[test]
    fn rejects_an_unparsed_trigger() {
        let service = service();
        let agent_id = agent_id(&service);
        let error = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Broken",
                "prompt": "Go",
                "trigger": { "kind": "cron", "expr": "not cron" },
                "workspace": { "type": "system" }
            }))
            .unwrap_err();
        assert!(error.to_string().contains("invalid trigger"));
    }

    fn user_workspace() -> Value {
        json!({ "type": "user", "workspaceId": "ws-1" })
    }

    fn create_interval(service: &TaskService, agent_id: &str, name: &str) -> Value {
        service
            .create(&json!({
                "agentId": agent_id,
                "name": name,
                "prompt": "Go",
                "trigger": { "kind": "interval", "ms": 60_000 },
                "workspace": user_workspace(),
            }))
            .unwrap()
    }

    #[test]
    fn rejects_incomplete_and_unsafe_task_input() {
        let service = service();
        let agent_id = agent_id(&service);
        let base = json!({
            "agentId": agent_id,
            "name": "Daily",
            "prompt": "Go",
            "trigger": { "kind": "interval", "ms": 60_000 },
            "workspace": { "type": "system" }
        });

        let missing_agent = service
            .create(&json!({
                "agentId": "missing",
                "name": "Daily",
                "prompt": "Go",
                "trigger": { "kind": "interval", "ms": 60_000 },
                "workspace": { "type": "system" }
            }))
            .unwrap_err();
        assert!(
            missing_agent.to_string().contains("missing")
                || missing_agent.to_string().contains("not found")
        );

        let mut blank_name = base.clone();
        blank_name["name"] = json!("   ");
        assert!(
            service
                .create(&blank_name)
                .unwrap_err()
                .to_string()
                .contains("name")
        );

        let mut short_interval = base.clone();
        short_interval["trigger"] = json!({ "kind": "interval", "ms": 999 });
        assert!(
            service
                .create(&short_interval)
                .unwrap_err()
                .to_string()
                .contains("invalid trigger")
        );

        let mut bad_skill = base.clone();
        bad_skill["skills"] = json!(["../secret"]);
        assert!(
            service
                .create(&bad_skill)
                .unwrap_err()
                .to_string()
                .contains("skill")
        );

        let mut too_many_skills = base.clone();
        too_many_skills["skills"] = json!(["a", "b", "c", "d", "e", "f", "g", "h", "i"]);
        assert!(
            service
                .create(&too_many_skills)
                .unwrap_err()
                .to_string()
                .contains("8")
        );

        let mut escaped = base.clone();
        escaped["files"] = json!([{ "path": "/tmp/../etc/passwd", "name": "passwd" }]);
        assert!(
            service
                .create(&escaped)
                .unwrap_err()
                .to_string()
                .contains("absolute")
        );

        let mut too_many_files = base.clone();
        too_many_files["files"] = json!(
            (0..21)
                .map(|index| json!({ "path": format!("/tmp/note-{index}.md"), "name": "n" }))
                .collect::<Vec<_>>()
        );
        assert!(
            service
                .create(&too_many_files)
                .unwrap_err()
                .to_string()
                .contains("20")
        );
    }

    #[test]
    fn keeps_omitted_model_skills_and_files_when_renaming() {
        let service = service();
        let agent_id = agent_id(&service);
        let created = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Daily",
                "prompt": "Go",
                "trigger": { "kind": "interval", "ms": 60_000 },
                "workspace": { "type": "system" },
                "model": "openai::gpt-4o",
                "skills": ["pdf"],
                "files": [{ "path": "/tmp/notes.md", "name": "notes.md" }],
                "reuseSession": true,
                "timeoutMinutes": 15,
                "channelIds": ["channel-1"]
            }))
            .unwrap();
        let id = created["id"].as_str().unwrap();
        let next_run = created["nextRun"].clone();
        let renamed = service
            .update(&agent_id, id, &json!({ "name": "Renamed" }))
            .unwrap();
        assert_eq!(renamed["name"], "Renamed");
        assert_eq!(renamed["model"], "openai::gpt-4o");
        assert_eq!(renamed["skills"], json!(["pdf"]));
        assert_eq!(renamed["files"][0]["path"], "/tmp/notes.md");
        assert_eq!(renamed["reuseSession"], true);
        assert_eq!(renamed["timeoutMinutes"], 15);
        assert_eq!(renamed["channelIds"], json!(["channel-1"]));
        assert_eq!(renamed["nextRun"], next_run);

        let rescheduled = service
            .update(
                &agent_id,
                id,
                &json!({ "trigger": { "kind": "interval", "ms": 120_000 } }),
            )
            .unwrap();
        assert_ne!(rescheduled["nextRun"], next_run);
        assert_eq!(rescheduled["skills"], json!(["pdf"]));
    }

    #[test]
    fn isolates_pause_resume_and_history_by_owning_agent() {
        let service = service();
        let owner = agent_id(&service);
        let other = service
            .agents
            .create(&json!({ "type": "a3s-code", "name": "Other", "model": "openai::gpt" }))
            .unwrap()
            .id;
        let due_at = chrono::Utc::now().timestamp_millis() - 60_000;
        let created = service
            .create(&json!({
                "agentId": owner,
                "name": "Owned",
                "prompt": "Go",
                "trigger": { "kind": "once", "at": due_at },
                "workspace": user_workspace(),
            }))
            .unwrap();
        let id = created["id"].as_str().unwrap();

        let denied = service
            .update(&other, id, &json!({ "name": "Stolen" }))
            .unwrap_err();
        assert!(denied.to_string().contains(id));
        assert_eq!(service.get(id).unwrap()["name"], "Owned");
        assert!(service.delete(&other, id).is_err());

        let paused = service.set_enabled(&owner, id, false).unwrap();
        assert_eq!(paused["status"], "paused");
        assert_eq!(paused["enabled"], false);
        service.tick();
        assert_eq!(service.logs(&owner, id, 20).unwrap()["total"], 0);

        let resumed = service.set_enabled(&owner, id, true).unwrap();
        assert_eq!(resumed["status"], "active");
        assert!(resumed["nextRun"].as_str().is_some());

        service.delete(&owner, id).unwrap();
        assert!(service.get(id).is_err());
        assert!(service.logs(&owner, id, 20).is_err());
    }

    #[test]
    fn skips_a_missed_task_and_does_not_retry_a_failed_start() {
        let service = service();
        let agent_id = agent_id(&service);
        let missed_at = chrono::Utc::now().timestamp_millis() - 48 * 60 * 60 * 1000;
        let missed = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Missed once",
                "prompt": "Go",
                "trigger": { "kind": "once", "at": missed_at },
                "workspace": user_workspace(),
            }))
            .unwrap();
        assert_eq!(missed["status"], "missed");
        let missed_id = missed["id"].as_str().unwrap();
        service.tick();
        assert_eq!(service.logs(&agent_id, missed_id, 20).unwrap()["total"], 0);

        let due_at = chrono::Utc::now().timestamp_millis() - 60_000;
        let once = service
            .create(&json!({
                "agentId": agent_id,
                "name": "Due once",
                "prompt": "Go",
                "trigger": { "kind": "once", "at": due_at },
                "workspace": user_workspace(),
            }))
            .unwrap();
        let once_id = once["id"].as_str().unwrap();
        assert_eq!(once["status"], "active");
        service.tick();
        let once_logs = service.logs(&agent_id, once_id, 20).unwrap();
        assert_eq!(once_logs["total"], 1);
        assert_eq!(once_logs["items"][0]["status"], "failed");
        assert!(
            once_logs["items"][0]["error"]
                .as_str()
                .unwrap()
                .contains("workspace was not resolved")
        );
        let parked = service.get(once_id).unwrap();
        assert_eq!(parked["nextRun"], Value::Null);
        assert_eq!(parked["enabled"], true);
        assert_eq!(parked["status"], "active");
        service.tick();
        assert_eq!(service.logs(&agent_id, once_id, 20).unwrap()["total"], 1);
        service.run(&agent_id, once_id).unwrap_err();
        assert_eq!(service.logs(&agent_id, once_id, 20).unwrap()["total"], 2);

        let interval = create_interval(&service, &agent_id, "Repeating");
        let interval_id = interval["id"].as_str().unwrap();
        assert!(service.run(&agent_id, interval_id).is_err());
        service.tick();
        assert_eq!(
            service.logs(&agent_id, interval_id, 20).unwrap()["total"],
            1
        );
        let advanced = service.get(interval_id).unwrap();
        assert!(advanced["nextRun"].as_str().unwrap() > advanced["lastRun"].as_str().unwrap());
    }

    #[test]
    fn ticker_stays_off_without_the_application_runtime() {
        let service = Arc::new(service());
        service.arm();
        assert!(!service.ticker_started());
    }
}
