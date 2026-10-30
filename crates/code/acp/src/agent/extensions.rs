//! Host extension methods beyond the ACP protocol (compact, interject, review,
//! session info, permission posture, session delete).

use super::*;

const TRANSCRIPT_UPDATE_METHOD: &str = "a3s/session/transcript/update";
const TRANSCRIPT_UPDATE_SCHEMA: &str = "a3s.acp.session-transcript-update.v1";

#[derive(Debug)]
enum ReplayNotification {
    Acp(acp::SessionNotification),
    Typed(acp::ExtNotification),
}

fn replay_meta(entry: &a3s_code_core::store::TranscriptEntry) -> acp::Meta {
    let mut meta = acp::Meta::new();
    meta.insert("isReplay".into(), serde_json::json!(true));
    meta.insert("entryId".into(), serde_json::json!(entry.entry_id));
    meta.insert("factSeq".into(), serde_json::json!(entry.fact_seq));
    meta.insert("origin".into(), serde_json::json!("core.transcript_page"));
    meta.insert("digest".into(), serde_json::json!(entry.digest));
    meta
}

fn replay_session(
    session_id: &acp::SessionId,
    update: acp::SessionUpdate,
    meta: acp::Meta,
) -> ReplayNotification {
    let mut notification = acp::SessionNotification::new(session_id.clone(), update);
    notification.meta = Some(meta);
    ReplayNotification::Acp(notification)
}

fn replay_typed(
    session_id: &acp::SessionId,
    update: serde_json::Value,
    meta: acp::Meta,
) -> acp::Result<ReplayNotification> {
    let raw = serde_json::value::to_raw_value(&serde_json::json!({
        "schema": TRANSCRIPT_UPDATE_SCHEMA,
        "sessionId": session_id.0.as_ref(),
        "update": update,
        "_meta": meta,
    }))
    .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
    Ok(ReplayNotification::Typed(acp::ExtNotification::new(
        TRANSCRIPT_UPDATE_METHOD,
        raw.into(),
    )))
}

fn transcript_replay_notifications(
    session_id: &acp::SessionId,
    entry: &a3s_code_core::store::TranscriptEntry,
) -> acp::Result<Vec<ReplayNotification>> {
    use a3s_code_core::store::TranscriptEntryPayload as Payload;

    let meta = replay_meta(entry);
    let mut notifications = Vec::new();
    match &entry.payload {
        Payload::User { text } => notifications.push(replay_session(
            session_id,
            acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(text.clone()),
            ))),
            meta,
        )),
        Payload::Assistant { text } => notifications.push(replay_session(
            session_id,
            acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(acp::ContentBlock::Text(
                acp::TextContent::new(text.clone()),
            ))),
            meta,
        )),
        Payload::Tool {
            calls,
            results,
            complete: _,
        } => {
            for call in calls {
                if let Some(text) = &call.text {
                    notifications.push(replay_session(
                        session_id,
                        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
                        )),
                        meta.clone(),
                    ));
                }
                if let Some(reasoning) = &call.reasoning {
                    notifications.push(replay_session(
                        session_id,
                        acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(
                            acp::ContentBlock::Text(acp::TextContent::new(reasoning.clone())),
                        )),
                        meta.clone(),
                    ));
                }
                let result = results.iter().find(|result| result.tool_call_id == call.id);
                let status = match result {
                    Some(result) if result.ok => acp::ToolCallStatus::Completed,
                    Some(_) => acp::ToolCallStatus::Failed,
                    None => acp::ToolCallStatus::InProgress,
                };
                let tool_call = acp::ToolCall::new(
                    acp::ToolCallId::new(call.id.clone()),
                    tool_title(&call.name),
                )
                .kind(tool_kind_for(&call.name))
                .status(status)
                .raw_input(call.args.clone());
                notifications.push(replay_session(
                    session_id,
                    acp::SessionUpdate::ToolCall(tool_call),
                    meta.clone(),
                ));
                if let Some(result) = result {
                    let fields = acp::ToolCallUpdateFields::new()
                        .title(tool_title(&call.name))
                        .kind(tool_kind_for(&call.name))
                        .status(status)
                        .content(tool_call_content(
                            &call.name,
                            Some(&call.args),
                            &result.output,
                        ))
                        .raw_output(serde_json::json!(result.output));
                    notifications.push(replay_session(
                        session_id,
                        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                            acp::ToolCallId::new(call.id.clone()),
                            fields,
                        )),
                        meta.clone(),
                    ));
                }
            }
        }
        Payload::Compaction {
            summary,
            kept_lines,
        } => {
            notifications.push(replay_typed(
                session_id,
                serde_json::json!({
                    "sessionUpdate": "compaction",
                    "summary": summary,
                    "keptLines": kept_lines,
                }),
                meta,
            )?);
        }
        Payload::TurnComplete {
            terminal,
            effect_digest,
            forced_stop,
            log_cut,
        } => {
            notifications.push(replay_typed(
                session_id,
                serde_json::json!({
                    "sessionUpdate": "turn_complete",
                    "terminal": terminal,
                    "effectDigest": effect_digest,
                    "forcedStop": forced_stop,
                    "logCut": log_cut,
                }),
                meta,
            )?);
        }
        Payload::LegacyHistory {
            source_kind,
            fact_key,
            reason,
            display,
        } => {
            let mut update = serde_json::json!({
                "sessionUpdate": "legacy_history",
                "sourceKind": source_kind,
                "factKey": fact_key,
                "reason": reason,
            });
            if let Some(display) = display {
                update["display"] = serde_json::json!(display);
            }
            notifications.push(replay_typed(session_id, update, meta)?);
        }
    }
    Ok(notifications)
}

impl A3sCodeAgent {
    pub(super) async fn handle_compact_extension(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: serde_json::Value =
            serde_json::from_str(args.params.get()).map_err(|error| {
                acp::Error::invalid_params().data(format!("compact request: {error}"))
            })?;
        let session_id = request
            .get("sessionId")
            .or_else(|| request.get("session_id"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| acp::Error::invalid_params().data("compact request: sessionId"))?;
        let user_context = request
            .get("userContext")
            .or_else(|| request.get("user_context"))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let session = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(session_id)
                .map(|live| Arc::clone(&live.session))
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?
        };
        session
            .compact_conversation(user_context.as_deref())
            .await
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let body = serde_json::value::to_raw_value(&serde_json::json!({}))
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(body.into()))
    }

    pub(super) async fn handle_interject(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request = parse_interject_params(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        let session = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&request.session_id)
                .map(|live| Arc::clone(&live.session))
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?
        };
        session
            .steer(a3s_code_core::SteerRequest::new(request.text))
            .await
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let body = serde_json::value::to_raw_value(&serde_json::json!({ "ok": true }))
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(body.into()))
    }

    /// `a3s/review/target` pins current target bytes; `a3s/review/admit` admits
    /// bound drafts; `a3s/review/settle` moves an admitted Finding under its
    /// admitted scope. All wait behind the session's turn queue like goal import.
    pub(super) async fn handle_review(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let invalid = |error: String| acp::Error::invalid_params().data(error);
        let request =
            crate::engineering::ReviewRequest::parse(args.method.as_ref(), args.params.get())
                .map_err(invalid)?;
        let (session, turn_queue) = self
            .sessions
            .lock()
            .await
            .get(request.session_id())
            .map(|live| (Arc::clone(&live.session), Arc::clone(&live.turn_queue)))
            .ok_or_else(|| acp::Error::resource_not_found(None))?;
        if session.is_closed() {
            return Err(acp::Error::resource_not_found(None));
        }
        let _turn_permit = turn_queue
            .acquire_owned()
            .await
            .ok_or_else(|| invalid("review queue is full".into()))?;
        let body = request.answer(&session).await.map_err(invalid)?;
        let raw = serde_json::value::to_raw_value(&body)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }

    /// `a3s/session/info` is the pager's `/session-info` and `/context` source.
    /// The pager decodes `ExtMethodResult<SessionInfoResponse>` and treats any
    /// other body, including JSON null, as an invalid response.
    pub(super) async fn handle_session_info(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: serde_json::Value =
            serde_json::from_str(args.params.get()).map_err(|error| {
                acp::Error::invalid_params().data(format!("session info request: {error}"))
            })?;
        let session_id = request
            .get("sessionId")
            .or_else(|| request.get("session_id"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| acp::Error::invalid_params().data("session info request: sessionId"))?;
        let live = match self.snapshot_session_info(session_id).await? {
            Some(live) => Some(live),
            None => {
                // The pager paints "Loading session" and accepts /session-info
                // before this load inserts the id.
                self.session_loads.wait(session_id).await;
                self.snapshot_session_info(session_id).await?
            }
        };
        let body = match live {
            Some((cwd, model_id, posture, turns, used, total)) => {
                let usage_pct = usage_percent(used, total);
                serde_json::json!({
                    "result": {
                        "sessionId": session_id,
                        "cwd": cwd,
                        "permissionPosture": posture,
                        "model": &model_id,
                        "resolvedModelId": &model_id,
                        "modelFingerprint": null,
                        "turns": turns,
                        "turnIndex": turns,
                        "context": {
                            "used": used,
                            "total": total,
                            "usagePct": usage_pct,
                            "turnCount": turns,
                            "freeTokens": total.saturating_sub(used)
                        }
                    }
                })
            }
            None => serde_json::json!({
                "result": null,
                "error": "unknown session id"
            }),
        };
        let raw = serde_json::value::to_raw_value(&body)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }

    /// Push the fact-log transcript as historical session updates.
    ///
    /// The pager does not read the fact log itself. `session/load` is the
    /// path `--resume` already calls, and untagged updates are dropped once
    /// the load window closes.
    pub(super) async fn replay_loaded_transcript(
        &self,
        session_id: &acp::SessionId,
        session: &AgentSession,
    ) -> acp::Result<()> {
        let mut cursor = None;
        let mut entries = Vec::new();
        loop {
            let page = session
                .transcript_page(a3s_code_core::store::TranscriptPageRequest { cursor, limit: 256 })
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
            entries.extend(page.entries);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        let plan = session
            .plan_projection()
            .await
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;

        for entry in entries {
            for notification in transcript_replay_notifications(session_id, &entry)? {
                match notification {
                    ReplayNotification::Acp(notification) => self.emit(notification).await,
                    ReplayNotification::Typed(notification) => {
                        let client = self.client.borrow().clone();
                        if let Some(client) = client {
                            let _ = client.ext_notification(notification).await;
                        }
                    }
                }
            }
        }
        if !plan.tasks.is_empty() {
            let entries = plan
                .tasks
                .iter()
                .map(task_to_plan_entry)
                .collect::<Vec<_>>();
            let mut notification = acp::SessionNotification::new(
                session_id.clone(),
                acp::SessionUpdate::Plan(acp::Plan::new(entries)),
            );
            let mut meta = acp::Meta::new();
            meta.insert("isReplay".into(), serde_json::json!(true));
            meta.insert(
                "entryId".into(),
                serde_json::json!(format!("plan:{}", plan.digest)),
            );
            meta.insert("origin".into(), serde_json::json!("core.plan_projection"));
            meta.insert("digest".into(), serde_json::json!(plan.digest));
            meta.insert("planRevision".into(), serde_json::json!(plan.revision));
            notification.meta = Some(meta);
            self.emit(notification).await;
        }
        Ok(())
    }

    /// `a3s/permission_posture` admits a named next-run posture.
    ///
    /// Optional `deny` rules are stored on that policy. This method does not
    /// execute a tool or write a caller-supplied path. A later prompt is what
    /// the real tool executor admits or denies.
    pub(super) async fn handle_permission_posture(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: serde_json::Value =
            serde_json::from_str(args.params.get()).map_err(|error| {
                acp::Error::invalid_params().data(format!("permission posture request: {error}"))
            })?;
        let session_id = request
            .get("sessionId")
            .or_else(|| request.get("session_id"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                acp::Error::invalid_params().data("permission posture request: sessionId")
            })?;
        let requested = request
            .get("posture")
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| acp::Error::invalid_params().data("posture must be a string"))
            })
            .transpose()?
            .unwrap_or("default");
        let posture = PermissionPosture::parse(requested)?;
        let mut extra = Vec::new();
        match request.get("deny") {
            Some(value) if value.is_string() => {
                let rule = value
                    .as_str()
                    .filter(|rule| !rule.trim().is_empty())
                    .ok_or_else(|| {
                        acp::Error::invalid_params().data("deny must contain nonempty strings")
                    })?;
                extra.push(rule.to_string());
            }
            Some(value) => {
                let rules = value.as_array().ok_or_else(|| {
                    acp::Error::invalid_params()
                        .data("deny must be a string or an array of nonempty strings")
                })?;
                for rule in rules {
                    extra.push(
                        rule.as_str()
                            .filter(|rule| !rule.trim().is_empty())
                            .ok_or_else(|| {
                                acp::Error::invalid_params()
                                    .data("deny must contain nonempty strings")
                            })?
                            .to_string(),
                    );
                }
            }
            None => {}
        }
        self.sessions
            .lock()
            .await
            .get_mut(session_id)
            .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?
            .set_posture(posture, extra)?;
        let posture_name = posture.as_str();
        self.emit_permission_mode(acp::SessionId::new(session_id), posture)
            .await;

        let body = serde_json::json!({
            "result": {
                "sessionId": session_id,
                "permissionPosture": posture_name
            }
        });
        let raw = serde_json::value::to_raw_value(&body)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }

    /// `a3s/session/delete` is the pager's `/delete` confirmation.
    ///
    /// The pager removes its live view only after this returns without an
    /// error. The session directory under the a3s home is what `--continue`
    /// reads, so a null body would report success and leave that directory.
    pub(super) async fn handle_session_delete(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: serde_json::Value =
            serde_json::from_str(args.params.get()).map_err(|error| {
                acp::Error::invalid_params().data(format!("session delete request: {error}"))
            })?;
        let session_id = request
            .get("sessionId")
            .or_else(|| request.get("session_id"))
            .and_then(|value| value.as_str())
            .filter(|value| uuid::Uuid::parse_str(value).is_ok())
            .ok_or_else(|| {
                acp::Error::invalid_params().data("session delete request: sessionId")
            })?;
        let cwd = request
            .get("cwd")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let live = self.sessions.lock().await.remove(session_id);
        let workspace = live.as_ref().map(|live| live.workspace.clone());
        if let Some(live) = live {
            live.session.close().await;
        }
        // A deleted session's servers leave the global union: resync so the
        // remaining sessions' sets are exactly served (no stale superset).
        if workspace.is_some() {
            if let Err(error) = self.sync_session_mcp_servers(None, &[]).await {
                tracing::warn!(
                    session_id,
                    %error,
                    "failed to resync the MCP union after session delete"
                );
            }
        }
        self.goals.lock().await.remove(session_id);
        self.turn_generation.lock().await.remove(session_id);
        self.cancelled_generation.lock().await.remove(session_id);
        let mut failures = Vec::new();
        if let Some(workspace) = workspace.as_deref() {
            if let Err(error) = remove_fact_log(workspace, session_id) {
                failures.push(error);
            }
        }
        if let Some(cwd) = cwd.as_deref() {
            if let Err(error) = remove_fact_log(std::path::Path::new(cwd), session_id) {
                failures.push(error);
            }
        }
        use a3s_code_core::store::{FileSessionStore, SessionStore};
        let directory =
            Self::interactive_session_store_dir(self.launch.config.sessions_dir.as_deref());
        if directory
            .try_exists()
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?
        {
            match FileSessionStore::new(directory).await {
                Ok(store) => {
                    if let Err(error) = store.delete(session_id).await {
                        failures.push(format!("delete session snapshot: {error}"));
                    }
                }
                Err(error) => failures.push(format!("open session snapshot store: {error}")),
            }
        }
        if let Err(error) = remove_recorded_session(session_id) {
            failures.push(error);
        }
        if !failures.is_empty() {
            return Err(acp::Error::internal_error().data(failures.join("; ")));
        }
        let body = serde_json::json!({ "success": true });
        let raw = serde_json::value::to_raw_value(&body)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }
}

pub(super) struct InterjectParams {
    pub(super) session_id: String,
    pub(super) text: String,
}

pub(super) fn parse_interject_params(raw: &str) -> Result<InterjectParams, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|error| format!("interject request: {error}"))?;
    let session_id = value
        .get("sessionId")
        .or_else(|| value.get("session_id"))
        .and_then(|item| item.as_str())
        .filter(|item| !item.is_empty())
        .ok_or_else(|| "interject request: sessionId".to_string())?;
    let text = value
        .get("text")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .ok_or_else(|| "interject request: text".to_string())?;
    Ok(InterjectParams {
        session_id: session_id.to_string(),
        text: text.to_string(),
    })
}

pub(super) async fn answer_wiki_search(args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        wiki: String,
        query: String,
        #[serde(default)]
        admit: bool,
    }
    let request: Request = serde_json::from_str(args.params.get()).map_err(|error| {
        acp::Error::invalid_params().data(format!("wiki search request: {error}"))
    })?;
    let wiki = request.wiki;
    let query = request.query;
    if wiki.is_empty()
        || wiki.len() > 8192
        || query.trim().is_empty()
        || query.chars().count() > 2000
        || query.contains('\0')
    {
        return Err(acp::Error::invalid_params().data("invalid Wiki search"));
    }
    let body = if request.admit {
        crate::wiki_search::search_admitted_body(std::path::Path::new(&wiki), &query).await
    } else {
        tokio::task::spawn_blocking(move || {
            crate::wiki_search::search_published_body(std::path::Path::new(&wiki), &query)
        })
        .await
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?
    };
    let raw = serde_json::value::to_raw_value(&body)
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

pub(super) fn remove_fact_log(workspace: &std::path::Path, session_id: &str) -> Result<(), String> {
    let thread = a3s_code_core::fact_control::thread_for_session(session_id);
    let path = a3s_code_core::session_folder::legacy_effect_log_dir(workspace)
        .join(format!("{thread}.jsonl"));
    if path.is_file() {
        std::fs::remove_file(&path)
            .map_err(|error| format!("remove fact log {}: {error}", path.display()))?;
    }
    match a3s_code_core::session_folder::find_session_dir(workspace, session_id) {
        Ok(Some(dir)) => {
            let date_dir = dir.parent().map(std::path::Path::to_path_buf);
            std::fs::remove_dir_all(&dir)
                .map_err(|error| format!("remove session folder {}: {error}", dir.display()))?;
            if let Some(date_dir) = date_dir {
                match std::fs::remove_dir(&date_dir) {
                    Ok(()) => {}
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound
                            || error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                    Err(error) => {
                        return Err(format!(
                            "remove session date folder {}: {error}",
                            date_dir.display()
                        ));
                    }
                }
            }
        }
        Ok(None) => {}
        Err(error) => return Err(error.to_string()),
    }
    Ok(())
}

/// Remove `{a3s-home}/sessions/{cwd}/{session_id}` for every recorded cwd.
///
/// The directory name is the url-encoded cwd. Matching the session id keeps
/// the delete on that session's own record.
pub(super) fn recorded_session_homes(
    a3s_home: Option<&std::ffi::OsStr>,
    grok_home: Option<&std::ffi::OsStr>,
    os_home: Option<&std::ffi::OsStr>,
) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    if let Some(env) = a3s_home.filter(|value| !value.is_empty()) {
        homes.push(PathBuf::from(env));
    }
    if let Some(env) = grok_home.filter(|value| !value.is_empty()) {
        homes.push(PathBuf::from(env));
    }
    if let Some(home) = os_home.filter(|value| !value.is_empty()) {
        let home = PathBuf::from(home);
        homes.push(home.join(".a3s"));
        homes.push(home.join(".grok"));
    }
    homes.sort();
    homes.dedup();
    homes
}

pub(super) fn remove_recorded_session_under(
    session_id: &str,
    homes: &[std::path::PathBuf],
) -> Result<(), String> {
    let mut failures = Vec::new();
    for home in homes {
        let sessions = home.join("sessions");
        if !sessions.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(&sessions) {
            Ok(entries) => entries,
            Err(error) => {
                failures.push(format!("read {}: {error}", sessions.display()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failures.push(format!("read {}: {error}", sessions.display()));
                    continue;
                }
            };
            let dir = entry.path().join(session_id);
            if dir.is_dir() {
                if let Err(error) = std::fs::remove_dir_all(&dir) {
                    failures.push(format!("remove {}: {error}", dir.display()));
                }
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

pub(super) fn remove_recorded_session(session_id: &str) -> Result<(), String> {
    let homes = recorded_session_homes(
        std::env::var_os("A3S_HOME").as_deref(),
        std::env::var_os("GROK_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    );
    remove_recorded_session_under(session_id, &homes)
}

#[cfg(test)]
#[path = "plan_replay_tests.rs"]
mod plan_replay_tests;
