//! JSON-file persistence for agents, sessions, and session messages.
//! `root == None` keeps everything in memory (tests).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::domain::{
    Agent, Message, Pin, ScheduledTask, Session, TaskRun, is_resident_session, now_iso,
};
use super::session_query::{ExecutionStatus, SessionExecution};

#[derive(Default)]
struct State {
    agents: Vec<Agent>,
    sessions: Vec<Session>,
    session_executions: HashMap<String, SessionExecution>,
    unreliable_executions: HashSet<String>,
    messages: HashMap<String, Vec<Message>>,
    pins: Vec<Pin>,
    tasks: Vec<ScheduledTask>,
    runs: Vec<TaskRun>,
    session_modes: HashMap<String, String>,
}

pub struct PlaneStore {
    root: Option<PathBuf>,
    state: Mutex<State>,
}

impl PlaneStore {
    pub fn open(root: Option<PathBuf>) -> Self {
        let mut state = match &root {
            Some(root) => State {
                agents: read_json(&root.join("agents.json")).unwrap_or_default(),
                sessions: read_json(&root.join("sessions.json")).unwrap_or_default(),
                session_executions: read_json(&root.join("session-executions.json"))
                    .unwrap_or_else(|| {
                        if root.join("session-executions.json").exists() {
                            eprintln!("studio: session execution summaries are unreadable; task states are unknown");
                        }
                        HashMap::new()
                    }),
                unreliable_executions: HashSet::new(),
                messages: HashMap::new(),
                pins: read_json(&root.join("pins.json")).unwrap_or_default(),
                tasks: read_json(&root.join("tasks.json")).unwrap_or_default(),
                runs: read_json(&root.join("task-runs.json")).unwrap_or_default(),
                session_modes: read_json(&root.join("session-modes.json")).unwrap_or_default(),
            },
            None => State::default(),
        };
        for execution in state.session_executions.values_mut() {
            if execution.status.is_live() {
                execution.status = ExecutionStatus::Unknown;
            }
        }
        Self {
            root,
            state: Mutex::new(state),
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn agents(&self) -> Vec<Agent> {
        self.lock().agents.clone()
    }

    pub fn agent(&self, id: &str) -> Option<Agent> {
        self.lock()
            .agents
            .iter()
            .find(|agent| agent.id == id)
            .cloned()
    }

    pub fn save_agent(&self, agent: Agent) -> io::Result<()> {
        let mut state = self.lock();
        match state.agents.iter_mut().find(|item| item.id == agent.id) {
            Some(slot) => *slot = agent,
            None => state.agents.push(agent),
        }
        self.write("agents.json", &state.agents)
    }

    pub fn sessions(&self) -> Vec<Session> {
        self.lock().sessions.clone()
    }

    pub fn session_permission_mode(&self, id: &str) -> Option<String> {
        self.lock().session_modes.get(id).cloned()
    }

    /// Stores one session's workbench mode. Unknown values are ignored.
    /// `plan` replaces a previously stored full-access mode.
    pub fn set_session_permission_mode(&self, id: &str, mode: &str) -> io::Result<()> {
        if !matches!(mode, "default" | "plan" | "auto" | "bypassPermissions") {
            return Ok(());
        }
        let mut state = self.lock();
        state.session_modes.insert(id.to_owned(), mode.to_owned());
        let modes = state.session_modes.clone();
        self.write("session-modes.json", &modes)
    }

    pub fn session(&self, id: &str) -> Option<Session> {
        self.lock()
            .sessions
            .iter()
            .find(|session| session.id == id)
            .cloned()
    }

    pub fn save_session(&self, mut session: Session) -> io::Result<()> {
        let mut state = self.lock();
        let mut sessions = state.sessions.clone();
        match sessions.iter_mut().find(|item| item.id == session.id) {
            Some(slot) => {
                if !is_resident_session(&session.id) {
                    session.deleted_at = slot.deleted_at.clone();
                }
                *slot = session;
            }
            None => sessions.push(session),
        }
        self.write("sessions.json", &sessions)?;
        state.sessions = sessions;
        Ok(())
    }

    pub fn set_session_archived(
        &self,
        id: &str,
        archived: bool,
    ) -> io::Result<Option<(Session, bool)>> {
        let mut state = self.lock();
        let Some(index) = state.sessions.iter().position(|row| row.id == id) else {
            return Ok(None);
        };
        if state.sessions[index].deleted_at.is_some() == archived {
            return Ok(Some((state.sessions[index].clone(), false)));
        }
        let mut sessions = state.sessions.clone();
        let now = now_iso();
        sessions[index].deleted_at = archived.then(|| now.clone());
        sessions[index].updated_at = now;
        self.write("sessions.json", &sessions)?;
        let session = sessions[index].clone();
        state.sessions = sessions;
        Ok(Some((session, true)))
    }

    pub fn session_executions(&self) -> HashMap<String, SessionExecution> {
        let state = self.lock();
        let mut executions = state.session_executions.clone();
        for id in &state.unreliable_executions {
            executions.remove(id);
        }
        executions
    }

    pub fn save_session_execution(
        &self,
        session_id: &str,
        execution: SessionExecution,
        begin: bool,
    ) -> io::Result<bool> {
        let mut state = self.lock();
        if !state
            .sessions
            .iter()
            .any(|session| session.id == session_id)
        {
            return if begin {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "session no longer exists",
                ))
            } else {
                Ok(false)
            };
        }
        let previous = state.session_executions.get(session_id);
        if !begin
            && previous.is_none_or(|previous| {
                previous.anchor_message_id != execution.anchor_message_id
                    || previous.execution_id != execution.execution_id
            })
        {
            return Ok(false);
        }
        if let Some(previous) = previous {
            if previous.anchor_message_id == execution.anchor_message_id
                && previous.execution_id == execution.execution_id
                && (previous.status == execution.status || !previous.status.is_live())
            {
                return Ok(false);
            }
        }
        let mut updated = state.session_executions.clone();
        updated.insert(session_id.to_owned(), execution);
        if let Err(error) = self.write("session-executions.json", &updated) {
            state.unreliable_executions.insert(session_id.to_owned());
            return Err(error);
        }
        state.session_executions = updated;
        state.unreliable_executions.remove(session_id);
        Ok(true)
    }

    /// The host confirmed that no live turn owns this execution. A restart
    /// downgrades live summaries to Unknown until this recovery boundary runs.
    pub fn fail_orphan_execution(&self, session_id: &str) -> io::Result<bool> {
        let mut state = self.lock();
        let Some(mut execution) = state.session_executions.get(session_id).cloned() else {
            return Ok(false);
        };
        if !execution.status.is_live() && execution.status != ExecutionStatus::Unknown {
            return Ok(false);
        }
        execution.status = ExecutionStatus::Error;
        execution.observed_at = now_iso();
        let mut updated = state.session_executions.clone();
        updated.insert(session_id.to_owned(), execution);
        if let Err(error) = self.write("session-executions.json", &updated) {
            state.unreliable_executions.insert(session_id.to_owned());
            return Err(error);
        }
        state.session_executions = updated;
        state.unreliable_executions.remove(session_id);
        Ok(true)
    }

    /// Drop the row. A missing id still rewrites the file so the disk matches memory.
    pub fn remove_session(&self, id: &str) -> io::Result<()> {
        let mut state = self.lock();
        if state.session_executions.contains_key(id) {
            let mut executions = state.session_executions.clone();
            executions.remove(id);
            self.write("session-executions.json", &executions)?;
            state.session_executions = executions;
        }
        let mut sessions = state.sessions.clone();
        sessions.retain(|session| session.id != id);
        self.write("sessions.json", &sessions)?;
        state.sessions = sessions;
        state.unreliable_executions.remove(id);
        Ok(())
    }

    pub fn pins(&self) -> Vec<Pin> {
        self.lock().pins.clone()
    }

    pub fn save_pin(&self, pin: Pin) -> io::Result<()> {
        let mut state = self.lock();
        match state.pins.iter_mut().find(|item| item.id == pin.id) {
            Some(slot) => *slot = pin,
            None => state.pins.push(pin),
        }
        self.write("pins.json", &state.pins)
    }

    pub fn remove_pin(&self, id: &str) -> io::Result<bool> {
        let mut state = self.lock();
        let before = state.pins.len();
        state.pins.retain(|pin| pin.id != id);
        if state.pins.len() == before {
            return Ok(false);
        }
        self.write("pins.json", &state.pins)?;
        Ok(true)
    }

    pub fn tasks(&self) -> Vec<ScheduledTask> {
        self.lock().tasks.clone()
    }

    pub fn save_tasks(&self, tasks: &[ScheduledTask]) -> io::Result<()> {
        let mut state = self.lock();
        state.tasks = tasks.to_vec();
        self.write("tasks.json", &state.tasks)
    }

    pub fn runs(&self) -> Vec<TaskRun> {
        self.lock().runs.clone()
    }

    pub fn save_runs(&self, runs: &[TaskRun]) -> io::Result<()> {
        let mut state = self.lock();
        state.runs = runs.to_vec();
        self.write("task-runs.json", &state.runs)
    }

    pub fn remove_messages(&self, session_id: &str) -> io::Result<()> {
        let mut state = self.lock();
        state.messages.remove(session_id);
        let Some(root) = &self.root else {
            return Ok(());
        };
        let path = root.join(format!("messages/{session_id}.json"));
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Oldest first.
    pub fn messages(&self, session_id: &str) -> Vec<Message> {
        let mut state = self.lock();
        self.load_messages(&mut state, session_id).clone()
    }

    pub fn save_message(&self, message: Message) -> io::Result<()> {
        let mut state = self.lock();
        let session_id = message.session_id.clone();
        let rows = self.load_messages(&mut state, &session_id);
        match rows.iter_mut().find(|item| item.id == message.id) {
            Some(slot) => *slot = message,
            None => rows.push(message),
        }
        let rows = rows.clone();
        self.write(&format!("messages/{session_id}.json"), &rows)
    }

    pub fn delete_message(&self, session_id: &str, message_id: &str) -> io::Result<bool> {
        let mut state = self.lock();
        let mut rows = self.load_messages(&mut state, session_id).clone();
        let Some(index) = rows
            .iter()
            .position(|row| row.id == message_id && row.session_id == session_id)
        else {
            return Ok(false);
        };
        rows.remove(index);
        // Resume tokens contain the old transcript, including the removed message.
        for row in &mut rows {
            row.runtime_resume_token = None;
        }
        self.write(&format!("messages/{session_id}.json"), &rows)?;
        state.messages.insert(session_id.to_owned(), rows);
        Ok(true)
    }

    fn load_messages<'a>(&self, state: &'a mut State, session_id: &str) -> &'a mut Vec<Message> {
        state
            .messages
            .entry(session_id.to_owned())
            .or_insert_with(|| {
                self.root
                    .as_ref()
                    .and_then(|root| read_json(&root.join(format!("messages/{session_id}.json"))))
                    .unwrap_or_default()
            })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write<T: Serialize>(&self, relative: &str, value: &T) -> io::Result<()> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
        let staged = path.with_extension("json.tmp");
        fs::write(&staged, bytes)?;
        fs::rename(staged, path)
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
