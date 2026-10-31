use super::*;
use crate::error::CodeError;
use crate::store::{
    PrefixForkRequest, SessionPlanProjection, TranscriptCursor, TranscriptEntry,
    TranscriptEntryKind, TranscriptEntryPayload, TranscriptLegacyHistoryDisplay, TranscriptPage,
    TranscriptPageRequest, TranscriptSourceCut, TranscriptSourceSpan, TranscriptToolCall,
    TranscriptToolResult,
};
use aster_effect::{Fact, LogStore, ModelDecision};
use std::collections::HashSet;

const MAX_PAGE_LIMIT: usize = 256;

fn transcript_error(error: impl std::fmt::Display) -> CodeError {
    CodeError::Session(format!("session transcript: {error}"))
}

pub(crate) fn read_facts(session: &AgentSession) -> Result<Vec<Fact>> {
    let directory = crate::session_folder::effect_log_dir(&session.workspace, &session.session_id);
    if !directory.try_exists().map_err(transcript_error)? {
        if session.saved_durable_source().is_some() || !session.history().is_empty() {
            return Err(transcript_error("durable source is missing"));
        }
        return Ok(Vec::new());
    }
    let log = {
        let slot = session.run_admission.fact_log_slot();
        let mut cached = slot
            .lock()
            .map_err(|_| transcript_error("fact log slot is poisoned"))?;
        match cached.as_ref() {
            Some(log) => Arc::clone(log),
            None => {
                let log = Arc::new(aster_effect::FileLog::open(directory).map_err(transcript_error)?);
                *cached = Some(Arc::clone(&log));
                log
            }
        }
    };
    log.read(&crate::fact_control::thread_for_session(
        &session.session_id,
    ))
    .map_err(transcript_error)
}

fn required_string<'a>(fact: &'a Fact, field: &str) -> Result<&'a str> {
    fact.payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            transcript_error(format!(
                "{} seq {} lacks string {field}",
                fact.kind, fact.seq
            ))
        })
}

fn entry(facts: &[Fact], payload: TranscriptEntryPayload) -> Result<TranscriptEntry> {
    let first = facts
        .first()
        .ok_or_else(|| transcript_error("cannot project an empty source span"))?;
    let last = facts
        .last()
        .ok_or_else(|| transcript_error("cannot project an empty source span"))?;
    let kind = match &payload {
        TranscriptEntryPayload::User { .. } => TranscriptEntryKind::User,
        TranscriptEntryPayload::Assistant { .. } => TranscriptEntryKind::Assistant,
        TranscriptEntryPayload::Tool { .. } => TranscriptEntryKind::Tool,
        TranscriptEntryPayload::Compaction { .. } => TranscriptEntryKind::Compaction,
        TranscriptEntryPayload::TurnComplete { .. } => TranscriptEntryKind::TurnComplete,
        TranscriptEntryPayload::LegacyHistory { .. } => TranscriptEntryKind::LegacyHistory,
    };
    let digest = crate::content_digest::digest_json("aster.code.session-transcript-entry.v1", &facts)
        .map_err(transcript_error)?;
    Ok(TranscriptEntry {
        entry_id: format!("fact:{}:{digest}", first.seq),
        kind,
        fact_seq: first.seq,
        source_span: TranscriptSourceSpan {
            first_seq: first.seq,
            last_seq: last.seq,
        },
        digest,
        payload,
    })
}

fn legacy(fact: &Fact, reason: impl Into<String>) -> TranscriptEntryPayload {
    TranscriptEntryPayload::LegacyHistory {
        source_kind: fact.kind.clone(),
        fact_key: fact.key.clone(),
        reason: reason.into(),
        display: None,
    }
}

#[derive(serde::Deserialize)]
struct ToolEnvelope {
    kind: String,
    text: String,
    #[serde(default)]
    images: Vec<serde_json::Value>,
}

fn tool_output(fact: &Fact) -> Result<(String, bool)> {
    let output = required_string(fact, "output")?;
    let Ok(value) = serde_json::from_str::<serde_json::Value>(output) else {
        return Ok((output.to_string(), false));
    };
    if value.get("kind").and_then(serde_json::Value::as_str) != Some("aster.fact_tool_result") {
        return Ok((output.to_string(), false));
    }
    let envelope: ToolEnvelope = serde_json::from_value(value).map_err(|error| {
        transcript_error(format!("corrupt tool result at seq {}: {error}", fact.seq))
    })?;
    if envelope.kind != "aster.fact_tool_result" {
        return Err(transcript_error(format!(
            "unknown tool result envelope at seq {}",
            fact.seq
        )));
    }
    Ok((envelope.text, !envelope.images.is_empty()))
}

fn tool_call(call: aster_effect::ToolCall) -> TranscriptToolCall {
    TranscriptToolCall {
        id: call.id,
        name: call.name,
        args: call.args,
        text: call.text,
        reasoning: call.reasoning,
    }
}

pub(crate) fn project_entries(facts: &[Fact]) -> Result<Vec<TranscriptEntry>> {
    for fact in facts {
        let kind = fact.kind.as_str();
        if (kind.starts_with("user.")
            || kind.starts_with("model.")
            || kind.starts_with("tool.")
            || kind.starts_with("turn.")
            || kind.starts_with("compaction.")
            || kind.starts_with("question.")
            || kind.starts_with("confirmation.")
            || kind.starts_with("legacy."))
            && !matches!(
                kind,
                "user.message"
                    | "model.turn"
                    | "tool.result"
                    | "tool.prepared"
                    | "turn.complete"
                    | "compaction.done"
                    | "question.answered"
                    | "confirmation.answered"
                    | "legacy.studio.entry"
                    | "legacy.studio.migration_complete"
            )
        {
            return Err(transcript_error(format!(
                "unknown required fact kind {kind} at seq {}",
                fact.seq
            )));
        }
    }
    let mut entries = Vec::new();
    let mut index = 0;
    while index < facts.len() {
        let fact = &facts[index];
        match fact.kind.as_str() {
            super::migration::LEGACY_STUDIO_ENTRY_FACT_KIND => {
                let stored: super::migration::LegacyStudioEntryFact =
                    serde_json::from_value(fact.payload.clone()).map_err(|error| {
                        transcript_error(format!(
                            "corrupt imported legacy entry at seq {}: {error}",
                            fact.seq
                        ))
                    })?;
                entries.push(entry(
                    &facts[index..=index],
                    TranscriptEntryPayload::LegacyHistory {
                        source_kind: fact.kind.clone(),
                        fact_key: fact.key.clone(),
                        reason: "imported Studio history is display-only and non-executable"
                            .to_string(),
                        display: Some(TranscriptLegacyHistoryDisplay {
                            source_entry_id: stored.entry.id,
                            role: stored.entry.role,
                            text: stored.entry.text,
                            attachments: stored.entry.attachments,
                        }),
                    },
                )?);
            }
            super::migration::LEGACY_STUDIO_COMPLETION_FACT_KIND => {
                serde_json::from_value::<crate::store::LegacyStudioMigrationReceipt>(
                    fact.payload.clone(),
                )
                .map_err(|error| {
                    transcript_error(format!(
                        "corrupt imported migration receipt at seq {}: {error}",
                        fact.seq
                    ))
                })?;
            }
            "user.message" => {
                let text = required_string(fact, "text")?.to_string();
                let rich = fact
                    .payload
                    .get("images")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|images| !images.is_empty());
                entries.push(entry(
                    &facts[index..=index],
                    if rich {
                        legacy(fact, "rich user parts require legacy_history")
                    } else {
                        TranscriptEntryPayload::User { text }
                    },
                )?);
            }
            "question.answered" => {
                required_string(fact, "text")?;
                entries.push(entry(
                    &facts[index..=index],
                    legacy(fact, "question history requires legacy_history"),
                )?);
            }
            "model.turn" => {
                let decision: ModelDecision = serde_json::from_value(fact.payload.clone())
                    .map_err(|error| {
                        transcript_error(format!("corrupt model.turn at seq {}: {error}", fact.seq))
                    })?;
                match decision {
                    ModelDecision::Text { text } => entries.push(entry(
                        &facts[index..=index],
                        TranscriptEntryPayload::Assistant { text },
                    )?),
                    ModelDecision::Question { .. } => entries.push(entry(
                        &facts[index..=index],
                        legacy(fact, "question/form history requires legacy_history"),
                    )?),
                    ModelDecision::Tool { call, also } => {
                        let calls = std::iter::once(call)
                            .chain(also)
                            .map(tool_call)
                            .collect::<Vec<_>>();
                        let mut results = Vec::new();
                        let mut last = index;
                        let mut rich = false;
                        while results.len() < calls.len() && last + 1 < facts.len() {
                            let next = &facts[last + 1];
                            if next.kind != "tool.result" {
                                if matches!(
                                    next.kind.as_str(),
                                    "user.message"
                                        | "question.answered"
                                        | "model.turn"
                                        | "compaction.done"
                                        | "turn.complete"
                                ) {
                                    break;
                                }
                                last += 1;
                                continue;
                            }
                            let result = next;
                            let expected = &calls[results.len()];
                            let id = result
                                .payload
                                .get("toolCallId")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(expected.id.as_str());
                            if id != expected.id {
                                return Err(transcript_error(format!(
                                    "tool result at seq {} does not match call {}",
                                    result.seq, expected.id
                                )));
                            }
                            let ok = result
                                .payload
                                .get("ok")
                                .and_then(serde_json::Value::as_bool)
                                .ok_or_else(|| {
                                    transcript_error(format!(
                                        "tool.result seq {} lacks boolean ok",
                                        result.seq
                                    ))
                                })?;
                            let (output, has_rich_parts) = tool_output(result)?;
                            rich |= has_rich_parts;
                            results.push(TranscriptToolResult {
                                tool_call_id: id.to_string(),
                                ok,
                                output,
                            });
                            last += 1;
                        }
                        let complete = results.len() == calls.len();
                        entries.push(entry(
                            &facts[index..=last],
                            if rich {
                                legacy(fact, "rich tool parts require legacy_history")
                            } else {
                                TranscriptEntryPayload::Tool {
                                    calls,
                                    results,
                                    complete,
                                }
                            },
                        )?);
                        index = last;
                    }
                }
            }
            "tool.result" => {
                return Err(transcript_error(format!(
                    "orphan tool.result at seq {}",
                    fact.seq
                )));
            }
            "compaction.done" => {
                let summary = required_string(fact, "summary")?.to_string();
                let kept_lines = match fact.payload.get("kept") {
                    None => 0,
                    Some(serde_json::Value::Array(lines)) => {
                        if lines.iter().any(|line| !line.is_string()) {
                            return Err(transcript_error(format!(
                                "corrupt compaction.done kept lines at seq {}",
                                fact.seq
                            )));
                        }
                        lines.len()
                    }
                    Some(_) => {
                        return Err(transcript_error(format!(
                            "corrupt compaction.done kept lines at seq {}",
                            fact.seq
                        )));
                    }
                };
                entries.push(entry(
                    &facts[index..=index],
                    TranscriptEntryPayload::Compaction {
                        summary,
                        kept_lines,
                    },
                )?);
            }
            "turn.complete" => {
                let terminal = required_string(fact, "terminal")?.to_string();
                let log_cut: TranscriptSourceCut = serde_json::from_value(
                    fact.payload.get("log_cut").cloned().ok_or_else(|| {
                        transcript_error(format!("turn.complete seq {} lacks log_cut", fact.seq))
                    })?,
                )
                .map_err(|error| {
                    transcript_error(format!(
                        "corrupt turn.complete log_cut at seq {}: {error}",
                        fact.seq
                    ))
                })?;
                let optional = |field: &str| -> Result<Option<String>> {
                    match fact.payload.get(field) {
                        None | Some(serde_json::Value::Null) => Ok(None),
                        Some(serde_json::Value::String(value)) => Ok(Some(value.clone())),
                        Some(_) => Err(transcript_error(format!(
                            "turn.complete seq {} has invalid {field}",
                            fact.seq
                        ))),
                    }
                };
                entries.push(entry(
                    &facts[index..=index],
                    TranscriptEntryPayload::TurnComplete {
                        terminal,
                        effect_digest: optional("effect_digest")?,
                        forced_stop: optional("forced_stop")?,
                        log_cut,
                    },
                )?);
            }
            other
                if other.starts_with("user.")
                    || other.starts_with("model.")
                    || other.starts_with("tool.")
                    || other.starts_with("turn.")
                    || other.starts_with("compaction.")
                    || other.starts_with("question.")
                    || other.starts_with("confirmation.")
                    || other.starts_with("legacy.") =>
            {
                return Err(transcript_error(format!(
                    "unknown required fact kind {other} at seq {}",
                    fact.seq
                )));
            }
            _ => {}
        }
        index += 1;
    }
    Ok(entries)
}

fn messages_for_prefix(facts: &[Fact]) -> Result<Vec<Message>> {
    let entries = project_entries(facts)?;
    if entries.iter().any(|entry| {
        matches!(
            &entry.payload,
            TranscriptEntryPayload::LegacyHistory { display: None, .. }
        )
    }) {
        return Err(transcript_error(
            "prefix fork refuses history that requires legacy_history",
        ));
    }
    if facts.iter().any(|fact| fact.kind == "compaction.done") {
        return Ok(crate::fact_control::compaction_transcript_from_facts(facts));
    }
    let mut messages = Vec::new();
    for item in entries {
        match item.payload {
            TranscriptEntryPayload::User { text } => messages.push(Message::user(&text)),
            TranscriptEntryPayload::Assistant { text } => {
                messages.push(Message::assistant(&text));
            }
            TranscriptEntryPayload::Tool {
                calls,
                results,
                complete,
            } => {
                if !complete {
                    return Err(transcript_error(
                        "prefix fork refuses an incomplete tool entry",
                    ));
                }
                let mut content = Vec::new();
                if let Some(text) = calls[0].text.as_ref().filter(|text| !text.is_empty()) {
                    content.push(crate::llm::ContentBlock::Text { text: text.clone() });
                }
                content.extend(calls.iter().map(|call| crate::llm::ContentBlock::ToolUse {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    input: call.args.clone(),
                }));
                messages.push(Message {
                    role: "assistant".into(),
                    content,
                    reasoning_content: calls[0].reasoning.clone(),
                    transcript_text: None,
                    transcript_visibility: crate::llm::TranscriptVisibility::Wire,
                });
                messages.extend(results.into_iter().map(|result| {
                    Message::tool_result(&result.tool_call_id, &result.output, !result.ok)
                }));
            }
            TranscriptEntryPayload::TurnComplete { .. }
            | TranscriptEntryPayload::LegacyHistory {
                display: Some(_), ..
            } => {}
            TranscriptEntryPayload::Compaction { .. }
            | TranscriptEntryPayload::LegacyHistory { display: None, .. } => unreachable!(),
        }
    }
    Ok(messages)
}

fn pending_interaction(facts: &[Fact]) -> Result<bool> {
    let mut question = false;
    let mut confirmations = HashSet::new();
    for fact in facts {
        match fact.kind.as_str() {
            "model.turn" => {
                confirmations.clear();
                let decision: ModelDecision =
                    serde_json::from_value(fact.payload.clone()).map_err(transcript_error)?;
                match decision {
                    ModelDecision::Question { .. } => question = true,
                    ModelDecision::Tool { call, also } => {
                        for call in std::iter::once(call).chain(also) {
                            if call.needs_confirmation {
                                confirmations.insert(call.id);
                            }
                        }
                    }
                    ModelDecision::Text { .. } => {}
                }
            }
            "question.answered" => question = false,
            "confirmation.answered" => {
                if let Some(id) = fact
                    .payload
                    .get("tool_call_id")
                    .and_then(serde_json::Value::as_str)
                {
                    confirmations.remove(id);
                }
            }
            "tool.result" => {
                if let Some(id) = fact
                    .payload
                    .get("toolCallId")
                    .and_then(serde_json::Value::as_str)
                {
                    confirmations.remove(id);
                }
            }
            _ => {}
        }
    }
    Ok(question || !confirmations.is_empty())
}

pub(crate) struct PrefixSnapshot {
    pub(crate) snapshot: crate::store::SessionSnapshotV1,
    pub(crate) facts: Vec<Fact>,
    pub(crate) boundary_cut: TranscriptSourceCut,
}

pub(crate) fn prepare_prefix_snapshot(
    source: &crate::store::SessionSnapshotV1,
    facts: &[Fact],
    request: &PrefixForkRequest,
) -> Result<PrefixSnapshot> {
    let actual_workspace =
        super::safe_canonicalize(std::path::Path::new(&source.session.config.workspace));
    let expected_workspace = super::safe_canonicalize(&request.expected_workspace);
    if actual_workspace != expected_workspace {
        return Err(transcript_error("prefix fork workspace mismatch"));
    }
    if pending_interaction(facts)? {
        return Err(transcript_error(
            "prefix fork refuses pending confirmation or form",
        ));
    }
    let entries = project_entries(facts)?;
    if entries.iter().any(|entry| {
        matches!(
            &entry.payload,
            TranscriptEntryPayload::Tool {
                complete: false,
                ..
            }
        )
    }) {
        return Err(transcript_error(
            "prefix fork refuses unfinished tool history",
        ));
    }
    let boundary = entries
        .iter()
        .position(|entry| entry.entry_id == request.boundary_entry_id)
        .ok_or_else(|| transcript_error("prefix fork boundary entry is missing"))?;
    let marker = match entries[boundary].kind {
        TranscriptEntryKind::TurnComplete => &entries[boundary],
        TranscriptEntryKind::Assistant => entries
            .get(boundary + 1)
            .filter(|entry| entry.kind == TranscriptEntryKind::TurnComplete)
            .ok_or_else(|| {
                transcript_error(
                    "assistant fork boundary has no immediately following turn.complete",
                )
            })?,
        _ => {
            return Err(transcript_error(
                "prefix fork boundary must be assistant or turn.complete",
            ));
        }
    };
    let TranscriptEntryPayload::TurnComplete { log_cut, .. } = &marker.payload else {
        unreachable!()
    };
    let marker_index = facts
        .iter()
        .position(|fact| fact.seq == marker.fact_seq)
        .ok_or_else(|| transcript_error("turn.complete source span is missing"))?;
    let verified_cut = TranscriptSourceCut::from(aster_effect::cut_log(&facts[..marker_index]));
    if *log_cut != verified_cut {
        return Err(transcript_error(
            "assistant boundary turn.complete digest is not verified",
        ));
    }
    let prefix = facts[..=marker_index].to_vec();
    let boundary_cut = TranscriptSourceCut::from(aster_effect::cut_log(&prefix));
    if boundary_cut.seq != marker.fact_seq {
        return Err(transcript_error("prefix fork boundary sequence mismatch"));
    }
    let messages = messages_for_prefix(&prefix)?;
    let mut snapshot = source.clone();
    snapshot.session.messages = messages;
    snapshot.session.context_usage = Default::default();
    snapshot.session.total_usage = Default::default();
    snapshot.session.total_cost = 0.0;
    snapshot.session.cost_records.clear();
    snapshot.session.tasks.clear();
    snapshot.session.parent_id = None;
    snapshot.session.config.parent_id = None;
    snapshot.artifacts.clear();
    snapshot.retained_artifact_uris.clear();
    snapshot.trace_events.clear();
    snapshot.run_records.clear();
    snapshot.verification_reports.clear();
    snapshot.subagent_tasks.clear();
    snapshot.session_review = None;
    Ok(PrefixSnapshot {
        snapshot,
        facts: prefix,
        boundary_cut,
    })
}

impl AgentSession {
    /// Count original user prompts, including those before compaction.
    pub fn user_prompt_count(&self) -> Result<u64> {
        Ok(read_facts(self)?
            .iter()
            .filter(|fact| fact.kind == "user.message")
            .count() as u64)
    }

    /// Read Core-owned product history directly from the original fact log.
    /// Unlike `durable_transcript`, this preserves entries before compaction
    /// and never converts I/O or corruption into an empty successful page.
    pub fn transcript_page(&self, request: TranscriptPageRequest) -> Result<TranscriptPage> {
        if request.limit == 0 || request.limit > MAX_PAGE_LIMIT {
            return Err(transcript_error(format!(
                "page limit must be in 1..={MAX_PAGE_LIMIT}"
            )));
        }
        let facts = read_facts(self)?;
        let source_cut = TranscriptSourceCut::from(aster_effect::cut_log(&facts));
        if let Some(source) = self.saved_durable_source() {
            source
                .verify(&self.session_id, &facts)
                .map_err(transcript_error)?;
        }
        let entries = project_entries(&facts)?;
        let start = match request.cursor {
            Some(cursor) => {
                if cursor.session_id != self.session_id {
                    return Err(transcript_error("cursor/session mismatch"));
                }
                if cursor.source_cut != source_cut {
                    return Err(transcript_error("source changed since cursor"));
                }
                entries
                    .iter()
                    .position(|entry| entry.entry_id == cursor.after_entry_id)
                    .map(|index| index + 1)
                    .ok_or_else(|| transcript_error("cursor entry is missing"))?
            }
            None => 0,
        };
        let end = start.saturating_add(request.limit).min(entries.len());
        let next_cursor = (end < entries.len()).then(|| TranscriptCursor {
            session_id: self.session_id.clone(),
            after_entry_id: entries[end - 1].entry_id.clone(),
            source_cut: source_cut.clone(),
        });
        Ok(TranscriptPage {
            session_id: self.session_id.clone(),
            entries: entries[start..end].to_vec(),
            next_cursor,
            source_cut,
        })
    }

    /// Return the current Plan with a projection digest, not a fabricated
    /// revision. The task list remains independent from transcript pagination.
    pub async fn plan_projection(&self) -> Result<SessionPlanProjection> {
        let tasks = self.plan_snapshot().await?;
        let digest =
            crate::content_digest::digest_json("aster.code.session-plan-projection.v1", &tasks)
                .map_err(|error| CodeError::Session(format!("plan projection: {error}")))?;
        Ok(SessionPlanProjection {
            tasks,
            digest,
            revision: None,
        })
    }
}

#[cfg(test)]
mod tests;
