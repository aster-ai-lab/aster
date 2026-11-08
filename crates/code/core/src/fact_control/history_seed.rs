//! Seeding and reading back conversation history from facts.

use super::*;

#[derive(Clone)]
pub(super) struct RecordedCall {
    pub(super) response_seq: Option<u64>,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) args: serde_json::Value,
    pub(super) text: String,
    pub(super) reasoning: Option<String>,
}

impl RecordedCall {
    fn shares_response(&self, other: &Self) -> bool {
        self.response_seq.is_some()
            && self.response_seq == other.response_seq
            && self.text == other.text
            && self.reasoning == other.reasoning
    }
}

pub(super) struct SeededTool {
    pub(super) id: String,
    pub(super) text: String,
    pub(super) is_error: bool,
    pub(super) images: Vec<crate::llm::Attachment>,
}

pub(super) fn seeded_tool(message: &Message) -> Option<SeededTool> {
    for block in &message.content {
        let crate::llm::ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } = block
        else {
            continue;
        };
        let is_error = is_error.unwrap_or(false);
        let (text, images) = match content {
            crate::llm::ToolResultContentField::Text(text) => (text.clone(), Vec::new()),
            crate::llm::ToolResultContentField::Blocks(blocks) => {
                let mut text = String::new();
                let mut images = Vec::new();
                for block in blocks {
                    match block {
                        crate::llm::ToolResultContent::Text { text: part } => {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(part);
                        }
                        crate::llm::ToolResultContent::Image { source } => {
                            let Ok(bytes) = base64::Engine::decode(
                                &base64::engine::general_purpose::STANDARD,
                                source.data.as_bytes(),
                            ) else {
                                continue;
                            };
                            if bytes.is_empty() {
                                continue;
                            }
                            images.push(crate::llm::Attachment::new(
                                bytes,
                                source.media_type.clone(),
                            ));
                        }
                    }
                }
                (text, images)
            }
        };
        if images.is_empty() && !is_error {
            continue;
        }
        return Some(SeededTool {
            id: tool_use_id.clone(),
            text,
            is_error,
            images,
        });
    }
    None
}

/// Seed the original transcript without admitting or running an infer.
pub(crate) fn seed_history(
    log: &FileLog,
    thread: &str,
    history: &[Message],
    effect_dir: Option<&Path>,
) -> Result<()> {
    let existing = log.read(thread).map_err(|error| anyhow::anyhow!(error))?;
    if existing.iter().any(|fact| {
        fact.kind != crate::standing_context::STANDING_CONTEXT_FACT_KIND
            && fact.kind != crate::work_policy::POLICY_KIND
    }) {
        return Ok(());
    }
    let mut turn = 0u64;
    let mut cycle = 0u64;
    for message in history {
        match message.role.as_str() {
            "user" => {
                turn += 1;
                cycle = 0;
                if let Some(seeded) = seeded_tool(message) {
                    // The tool turn consumes `infer:{turn}:0`. The new
                    // prompt is the next user message and gets its own infer.
                    log.append(
                        thread,
                        &[message_fact(
                            format!("m-hist-{turn}"),
                            seeded_user_text(message),
                        )],
                        None,
                    )
                    .map_err(|error| anyhow::anyhow!(error))?;
                    append_seeded_tool(log, thread, turn, &seeded, effect_dir)?;
                } else {
                    log.append(
                        thread,
                        &[message_fact(
                            format!("m-hist-{turn}"),
                            seeded_user_text(message),
                        )],
                        None,
                    )
                    .map_err(|error| anyhow::anyhow!(error))?;
                }
            }
            "assistant" => {
                let payload = serde_json::to_value(ModelDecision::Text {
                    text: message.text(),
                })
                .map_err(|error| anyhow::anyhow!(error))?;
                log.append(
                    thread,
                    &[NewFact {
                        kind: "model.turn".into(),
                        key: format!("model:{turn}:{cycle}"),
                        payload,
                    }],
                    Some(&format!("infer:{turn}:{cycle}")),
                )
                .map_err(|error| anyhow::anyhow!(error))?;
                cycle += 1;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn append_seeded_tool(
    log: &FileLog,
    thread: &str,
    turn: u64,
    seeded: &SeededTool,
    effect_dir: Option<&Path>,
) -> Result<()> {
    let output =
        match fact_tool_runner_value(&seeded.text, seeded.is_error, &seeded.images, effect_dir) {
            serde_json::Value::String(text) => text,
            other => other.to_string(),
        };
    let decision = ModelDecision::Tool {
        call: ToolCall {
            id: seeded.id.clone(),
            name: "tool".into(),
            args: serde_json::json!({}),
            needs_confirmation: false,
            text: None,
            reasoning: None,
        },
        also: Vec::new(),
    };
    let payload = serde_json::to_value(decision).map_err(|error| anyhow::anyhow!(error))?;
    log.append(
        thread,
        &[NewFact {
            kind: "model.turn".into(),
            key: format!("model:{turn}:0"),
            payload,
        }],
        Some(&format!("infer:{turn}:0")),
    )
    .map_err(|error| anyhow::anyhow!(error))?;
    log.append(
        thread,
        &[NewFact {
            kind: "tool.result".into(),
            key: format!("tool-result:{turn}:0:{}", seeded.id),
            payload: serde_json::json!({
                "toolCallId": seeded.id,
                "ok": !seeded.is_error,
                "output": output,
            }),
        }],
        None,
    )
    .map_err(|error| anyhow::anyhow!(error))?;
    Ok(())
}

pub(super) fn seeded_user_text(message: &Message) -> String {
    let tool_text = message
        .content
        .iter()
        .filter_map(|block| {
            if let crate::llm::ContentBlock::ToolResult { content, .. } = block {
                Some(content.as_text())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");
    if !tool_text.is_empty() {
        return tool_text;
    }
    message.text()
}

pub(super) fn assistant_tool_call(call: &RecordedCall) -> Message {
    let mut content = Vec::new();
    if !call.text.is_empty() {
        content.push(crate::llm::ContentBlock::Text {
            text: call.text.clone(),
        });
    }
    content.push(crate::llm::ContentBlock::ToolUse {
        id: call.id.clone(),
        name: call.name.clone(),
        input: call.args.clone(),
    });
    Message {
        role: "assistant".into(),
        content,
        reasoning_content: call.reasoning.clone(),
        transcript_text: None,
        transcript_visibility: crate::llm::TranscriptVisibility::Wire,
    }
}

/// Persist attachments under `effect_dir/images/` and record only by-ref
/// metadata in the fact. Legacy inline `data` is still readable on reload.
pub(super) fn user_message_fact(
    key: String,
    text: &str,
    attachments: &[Attachment],
    effect_dir: &Path,
) -> Result<NewFact> {
    if attachments.is_empty() {
        return Ok(message_fact(key, text));
    }
    let mut images = Vec::with_capacity(attachments.len());
    for image in attachments {
        let stored = crate::session_folder::persist_session_image(
            effect_dir,
            image.data.as_ref(),
            &image.media_type,
        )?;
        images.push(serde_json::json!({
            "media_type": stored.media_type,
            "path": stored.path,
            "sha256": stored.sha256,
            "byte_len": stored.byte_len,
        }));
    }
    Ok(NewFact {
        kind: "user.message".into(),
        key,
        payload: serde_json::json!({
            "text": text,
            "images": images,
        }),
    })
}

#[derive(Debug, Clone)]
pub(super) struct ImageRefMeta {
    pub(super) media_type: String,
    pub(super) path: Option<String>,
    pub(super) sha256: Option<String>,
    pub(super) byte_len: Option<usize>,
}

/// How a post-compaction user line should present its images to the model.
#[derive(Debug, Clone)]
pub(super) enum PlannedUserImages {
    /// First model look after the user message: send pixels.
    Pixels(Vec<Attachment>),
    /// Later cycles after tool/model activity: keep identity, drop pixels.
    Stub(Vec<ImageRefMeta>),
}

impl PlannedUserImages {
    pub(super) fn into_message_parts(self, body: &str) -> (String, Vec<Attachment>) {
        match self {
            Self::Pixels(images) => (body.to_owned(), images),
            Self::Stub(refs) if refs.is_empty() => (body.to_owned(), Vec::new()),
            Self::Stub(refs) => {
                let notes = refs
                    .iter()
                    .map(|item| {
                        let digest = item
                            .sha256
                            .as_deref()
                            .or(item.path.as_deref())
                            .unwrap_or("unknown");
                        let len = item
                            .byte_len
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "?".into());
                        format!(
                            "[image retained: sha256={digest} media={} bytes={len}; pixels omitted after tool activity]",
                            item.media_type
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (format!("{body}\n{notes}"), Vec::new())
            }
        }
    }
}

fn image_ref_meta(image: &serde_json::Value) -> Option<ImageRefMeta> {
    let media_type = image
        .get("media_type")
        .and_then(|value| value.as_str())
        .filter(|value| value.starts_with("image/"))?
        .to_owned();
    Some(ImageRefMeta {
        media_type,
        path: image
            .get("path")
            .and_then(|value| value.as_str())
            .map(str::to_owned),
        sha256: image
            .get("sha256")
            .and_then(|value| value.as_str())
            .map(str::to_owned),
        byte_len: image
            .get("byte_len")
            .and_then(|value| value.as_u64())
            .and_then(|value| usize::try_from(value).ok()),
    })
}

pub(super) fn attachments_from_user_payload(
    payload: &serde_json::Value,
    effect_dir: Option<&Path>,
) -> Vec<Attachment> {
    let Some(images) = payload.get("images").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    images
        .iter()
        .filter_map(|image| {
            let media_type = image.get("media_type").and_then(|value| value.as_str())?;
            if !media_type.starts_with("image/") {
                return None;
            }
            if let Some(path) = image.get("path").and_then(|value| value.as_str()) {
                let bytes = crate::session_folder::load_session_image(effect_dir?, path).ok()?;
                return Some(Attachment::new(bytes, media_type));
            }
            // Legacy facts inlined base64 under `data`. Persist beside the
            // session when possible so later reads can use path/sha256.
            let data = image.get("data").and_then(|value| value.as_str())?;
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
                .ok()
                .filter(|bytes| !bytes.is_empty())?;
            if let Some(dir) = effect_dir {
                let _ = crate::session_folder::persist_session_image(dir, &bytes, media_type);
            }
            Some(Attachment::new(bytes, media_type))
        })
        .collect()
}

fn user_message_followed_by_tool_activity(facts: &[aster_effect::Fact], user_index: usize) -> bool {
    facts
        .iter()
        .skip(user_index + 1)
        .take_while(|fact| {
            !matches!(
                fact.kind.as_str(),
                "user.message" | "question.answered" | "compaction.done"
            )
        })
        .any(|fact| {
            matches!(
                fact.kind.as_str(),
                "tool.result"
                    | "tool.prepared"
                    | "model.turn"
                    | "confirmation.answered"
                    | "aster.fact_tool_result"
            )
        })
}

/// Image plan for user lines that the effect fold still has after the latest
/// compaction. `kept` lines are already in the scheduler transcript and are
/// not paired with these facts. After tool/model activity on a user image
/// turn, pixels are stubbed so later complete() calls do not re-inflate the
/// multimodal payload.
pub(super) fn user_attachment_plan(
    facts: &[aster_effect::Fact],
    effect_dir: Option<&Path>,
) -> (usize, std::collections::VecDeque<PlannedUserImages>) {
    let start = facts
        .iter()
        .rposition(|fact| fact.kind == "compaction.done")
        .map(|index| index + 1)
        .unwrap_or(0);
    let kept_len = if start > 0 {
        facts[start - 1]
            .payload
            .get("kept")
            .and_then(|value| value.as_array())
            .map(|kept| kept.len())
            .unwrap_or(0)
    } else {
        0
    };
    let mut images = std::collections::VecDeque::new();
    for (offset, fact) in facts.iter().enumerate().skip(start) {
        match fact.kind.as_str() {
            "user.message" => {
                let refs = fact
                    .payload
                    .get("images")
                    .and_then(|value| value.as_array())
                    .map(|items| items.iter().filter_map(image_ref_meta).collect::<Vec<_>>())
                    .unwrap_or_default();
                if refs.is_empty() {
                    images.push_back(PlannedUserImages::Pixels(Vec::new()));
                } else if user_message_followed_by_tool_activity(facts, offset) {
                    images.push_back(PlannedUserImages::Stub(refs));
                } else {
                    images.push_back(PlannedUserImages::Pixels(attachments_from_user_payload(
                        &fact.payload,
                        effect_dir,
                    )));
                }
            }
            "question.answered" => images.push_back(PlannedUserImages::Pixels(Vec::new())),
            _ => {}
        }
    }
    (kept_len, images)
}

pub(super) fn images_for_user_line(
    index: usize,
    summary_offset: usize,
    kept_len: usize,
    queue: &mut std::collections::VecDeque<PlannedUserImages>,
) -> PlannedUserImages {
    if index < summary_offset {
        return PlannedUserImages::Pixels(Vec::new());
    }
    let request_index = index - summary_offset;
    if request_index < kept_len {
        return PlannedUserImages::Pixels(Vec::new());
    }
    queue
        .pop_front()
        .unwrap_or_else(|| PlannedUserImages::Pixels(Vec::new()))
}

/// Indexes of `assistant\ntool <name>\n<json>` lines that belong to a closed
/// scheduler batch: one or more of those records, then the same number of
/// `tool` or `tool-error` results. The writer in the effect fold is the
/// source of that shape. An unmatched record stays in the transcript.
pub(super) fn scheduler_tool_record_indexes(lines: &[String]) -> std::collections::HashSet<usize> {
    const RECORD_PREFIX: &str = "assistant\ntool ";
    let mut indexes = std::collections::HashSet::new();
    let mut cursor = 0;
    while cursor < lines.len() {
        if !lines[cursor].starts_with(RECORD_PREFIX) {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < lines.len() && lines[cursor].starts_with(RECORD_PREFIX) {
            cursor += 1;
        }
        let records = cursor - start;
        let mut results = 0;
        while cursor + results < lines.len()
            && (lines[cursor + results].starts_with("tool\n")
                || lines[cursor + results].starts_with("tool-error\n"))
        {
            results += 1;
        }
        if results == records {
            for index in start..cursor {
                indexes.insert(index);
            }
            cursor += results;
        } else {
            cursor = start + 1;
        }
    }
    indexes
}

/// Effect's public prose is reattached from call metadata below. Match only
/// the exact prose immediately ahead of its original closed tool batch.
pub(super) fn scheduler_tool_prose_indexes(
    lines: &[String],
    records: &std::collections::HashSet<usize>,
    calls: &[RecordedCall],
) -> std::collections::HashSet<usize> {
    let mut indexes = std::collections::HashSet::new();
    let mut completed = 0;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("tool\n") || line.starts_with("tool-error\n") {
            completed += 1;
        }
        if !records.contains(&index)
            || index
                .checked_sub(1)
                .is_some_and(|prior| records.contains(&prior))
        {
            continue;
        }
        let count = (index..lines.len())
            .take_while(|next| records.contains(next))
            .count();
        let Some(batch) = calls.get(completed..completed + count) else {
            continue;
        };
        let mut spoken = std::collections::BTreeSet::new();
        let public = batch
            .iter()
            .filter(|call| !call.text.is_empty() && spoken.insert(call.text.as_str()))
            .map(|call| format!("assistant\n{}", call.text))
            .collect::<Vec<_>>();
        let Some(start) = index.checked_sub(public.len()) else {
            continue;
        };
        if lines[start..index] == public {
            indexes.extend(start..index);
        }
    }
    indexes
}

pub(super) fn folded_message_with_images(
    role: &str,
    body: &str,
    images: Vec<Attachment>,
) -> Message {
    if role == "assistant" {
        Message::assistant(body)
    } else {
        user_message_with_images(body, images)
    }
}

pub(super) fn user_message_with_images(body: &str, images: Vec<Attachment>) -> Message {
    if images.is_empty() {
        Message::user(body)
    } else {
        Message::user_with_attachments(body, &images)
    }
}

/// Reattach original call metadata to completed results in the folded view.
/// `split_compact_tail` keeps an ordered suffix, including tool results. Keep
/// the corresponding suffix of original calls and verify its result bytes;
/// never fabricate an identity when the original facts cannot be read/paired.
#[cfg(test)]
pub(super) fn recorded_tool_calls(
    workspace: &Path,
    thread: &str,
) -> Result<Vec<RecordedCall>, ActorError> {
    let facts = read_workspace_facts(workspace, thread).map_err(|error| ActorError::Handler {
        key: "model-input-tool-history".into(),
        message: format!("cannot read original tool-call facts: {error}"),
    })?;
    recorded_tool_calls_in(facts)
}

pub(super) fn recorded_tool_calls_in(
    facts: Vec<aster_effect::Fact>,
) -> Result<Vec<RecordedCall>, ActorError> {
    let mut pending = std::collections::VecDeque::new();
    let mut completed: Vec<(RecordedCall, String)> = Vec::new();
    for fact in facts {
        if fact.kind == "model.turn"
            && fact.payload.get("kind").and_then(|kind| kind.as_str()) == Some("tool")
        {
            let ModelDecision::Tool { call, also } =
                serde_json::from_value::<ModelDecision>(fact.payload).map_err(|error| {
                    ActorError::Schema(format!("invalid original tool-call metadata: {error}"))
                })?
            else {
                return Err(ActorError::Schema("invalid original tool decision".into()));
            };
            pending.clear();
            let mut ids = std::collections::HashSet::new();
            for call in std::iter::once(call).chain(also) {
                if !ids.insert(call.id.clone()) {
                    return Err(ActorError::Schema(
                        "duplicate original tool-call identity in batch".into(),
                    ));
                }
                pending.push_back(RecordedCall {
                    response_seq: Some(fact.seq),
                    id: call.id,
                    name: call.name,
                    args: call.args,
                    text: call.text.unwrap_or_default(),
                    reasoning: call.reasoning.filter(|text| !text.is_empty()),
                });
            }
        } else if fact.kind == "tool.result" {
            let receipt = fact
                .payload
                .get("receipt")
                .map(|receipt| {
                    serde_json::from_value::<aster_effect::ToolExecutionResultV1>(receipt.clone())
                        .map_err(|error| {
                            ActorError::Schema(format!("invalid original tool receipt: {error}"))
                        })
                })
                .transpose()?;
            if let Some(receipt) = &receipt {
                receipt.validate()?;
            }
            #[derive(serde::Deserialize)]
            struct ResultPayload {
                #[serde(default, rename = "toolCallId")]
                tool_call_id: Option<String>,
                ok: bool,
                #[serde(default)]
                output: String,
            }
            let output = super::fact_reads::tool_result_fact_output(&fact.payload).to_owned();
            let payload: ResultPayload = serde_json::from_value(fact.payload).map_err(|error| {
                ActorError::Schema(format!("invalid original tool-result metadata: {error}"))
            })?;
            let call = pending.pop_front().ok_or_else(|| {
                ActorError::Schema("tool result has no original pending call".into())
            })?;
            // Legacy results may omit the id. Explicit ids must agree with
            // the scheduler's serial batch order, just as the Moore fold checks.
            if payload.tool_call_id.is_some_and(|id| id != call.id)
                || receipt.is_some_and(|receipt| receipt.identity.tool_call_id != call.id)
            {
                return Err(ActorError::Schema(
                    "tool-result identity disagrees with the original pending call".into(),
                ));
            }
            let role = if payload.ok { "tool" } else { "tool-error" };
            completed.push((call, format!("{role}\n{output}")));
        } else if fact.kind == "compaction.done" {
            #[derive(serde::Deserialize)]
            struct CompactionPayload {
                #[serde(default)]
                kept: Vec<String>,
            }
            let payload: CompactionPayload =
                serde_json::from_value(fact.payload).map_err(|error| {
                    ActorError::Schema(format!("invalid retained tool history: {error}"))
                })?;
            let retained: Vec<_> = payload
                .kept
                .iter()
                .filter(|line| line.starts_with("tool\n") || line.starts_with("tool-error\n"))
                .collect();
            let start = completed.len().checked_sub(retained.len()).ok_or_else(|| {
                ActorError::Schema("retained tool results have no original calls".into())
            })?;
            if completed[start..]
                .iter()
                .zip(retained)
                .any(|((_, original), kept)| original != kept)
            {
                return Err(ActorError::Schema(
                    "retained tool results disagree with the original suffix".into(),
                ));
            }
            completed.drain(..start);
            pending.clear();
        }
    }
    Ok(completed.into_iter().map(|(call, _)| call).collect())
}

/// Restore public prose and account for retained private continuation length.
/// Retained public checkpoints also reach the summary before older results disappear.
/// Reasoning text is never projected; scheduler records and result bytes stay intact.
pub(super) fn with_original_tool_text(
    lines: &[String],
    kept: &[String],
    facts: Vec<aster_effect::Fact>,
) -> Result<([Vec<String>; 2], usize), ActorError> {
    let calls = recorded_tool_calls_in(facts)?;
    let combined = lines.iter().chain(kept).cloned().collect::<Vec<_>>();
    let records = scheduler_tool_record_indexes(&combined);
    let prose = scheduler_tool_prose_indexes(&combined, &records, &calls);
    let kept_results = kept
        .iter()
        .filter(|line| line.starts_with("tool\n") || line.starts_with("tool-error\n"))
        .count();
    let start = calls.len().saturating_sub(kept_results);
    let kept_records = scheduler_tool_record_indexes(kept);
    let mut retained = calls[start..].iter();
    let mut previous: Option<&RecordedCall> = None;
    let mut continuation_tokens = 0;
    for (index, line) in kept.iter().enumerate() {
        if line.starts_with("tool\n") || line.starts_with("tool-error\n") {
            // Standalone compaction also accepts text without a mounted call log.
            let Some(call) = retained.next() else {
                previous = None;
                continue;
            };
            if !previous.is_some_and(|previous| call.shares_response(previous)) {
                continuation_tokens += call
                    .reasoning
                    .as_ref()
                    .map_or(0, |text| text.len().div_ceil(4));
            }
            previous = Some(call);
        } else if !kept_records.contains(&index) {
            previous = None;
        }
    }
    let prefix_records = scheduler_tool_record_indexes(lines);
    let mut calls = calls.into_iter();
    let mut project = |lines: &[String],
                       records: &std::collections::HashSet<usize>,
                       offset: usize|
     -> Result<(Vec<String>, Vec<String>), ActorError> {
        let mut projected = Vec::with_capacity(lines.len() + records.len());
        let mut public = Vec::new();
        let mut previous_call: Option<RecordedCall> = None;
        for (index, line) in lines.iter().enumerate() {
            if prose.contains(&(offset + index)) {
                continue;
            }
            if records.contains(&index) {
                let call = calls.next().ok_or_else(|| {
                    ActorError::Schema("compacted tool record has no original call metadata".into())
                })?;
                if !call.text.is_empty()
                    && !previous_call
                        .as_ref()
                        .is_some_and(|previous| call.shares_response(previous))
                {
                    let text = format!("assistant\n{}", call.text);
                    projected.push(text.clone());
                    public.push(text);
                }
                previous_call = Some(call);
            } else {
                previous_call = None;
            }
            projected.push(line.clone());
        }
        Ok((projected, public))
    };
    let (mut prefix, _) = project(lines, &prefix_records, 0)?;
    let (kept, public) = project(kept, &kept_records, lines.len())?;
    prefix.extend(public);
    Ok(([prefix, kept], continuation_tokens))
}

pub(super) fn recorded_tool_messages(
    recorded: &mut std::vec::IntoIter<RecordedCall>,
    messages: &mut Vec<Message>,
    previous: &mut Option<(RecordedCall, usize)>,
    body: &str,
    is_error: bool,
    effect_dir: Option<&Path>,
    stub_tool_image_ids: &std::collections::HashSet<String>,
) -> Result<(), ActorError> {
    let call = recorded.next().ok_or_else(|| {
        ActorError::Schema("folded tool result has no original call metadata".into())
    })?;
    let assistant_index = match previous.as_ref() {
        Some((prior, index)) if call.shares_response(prior) => {
            messages[*index]
                .content
                .push(crate::llm::ContentBlock::ToolUse {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    input: call.args.clone(),
                });
            *index
        }
        _ => {
            let index = messages.len();
            messages.push(assistant_tool_call(&call));
            index
        }
    };
    let load_pixels = !stub_tool_image_ids.contains(&call.id);
    messages.push(message_from_folded_tool(
        &call.id,
        body,
        is_error,
        effect_dir,
        load_pixels,
    ));
    *previous = Some((call, assistant_index));
    Ok(())
}

/// Tool-result call ids whose images should be stubbed because a later
/// `model.turn` already consumed that observation.
pub(super) fn tool_image_ids_to_stub(
    facts: &[aster_effect::Fact],
) -> std::collections::HashSet<String> {
    let mut stub = std::collections::HashSet::new();
    for (index, fact) in facts.iter().enumerate() {
        if fact.kind != "tool.result" {
            continue;
        }
        let Some(id) = fact
            .payload
            .get("toolCallId")
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        if facts[index + 1..]
            .iter()
            .any(|next| next.kind == "model.turn")
        {
            stub.insert(id.to_owned());
        }
    }
    stub
}

pub(super) fn recorded_call_from(call: &serde_json::Value) -> RecordedCall {
    let id = call
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or("tool");
    let name = call
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("tool");
    let args = call
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let text = call
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let reasoning = call
        .get("reasoning")
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
        .map(str::to_string);
    RecordedCall {
        response_seq: None,
        id: id.to_string(),
        name: name.to_string(),
        args,
        text,
        reasoning,
    }
}

pub(super) fn workspace_boundary_error(message: &str) -> bool {
    message.contains("escapes workspace") || message.contains("Workspace boundary")
}

pub(super) fn repeat_note_prefix(name: &str) -> String {
    format!("\n\n[aster host: `{name}` returned byte-identical output for identical arguments")
}

/// Mechanical feedback when a call repeats the name, arguments and output of the
/// previous call with the same name and arguments in this run. It never blocks.
/// A JSON result stays parseable for its consumer, so it carries no note.
pub(super) fn repeated_result_note(
    repeats: &Mutex<std::collections::HashMap<String, (String, u32)>>,
    name: &str,
    args: &serde_json::Value,
    output: &str,
) -> Option<String> {
    let call = crate::content_digest::digest_bytes(
        "aster.code.tool-repeat.call.v1",
        format!("{name}\n{args}").as_bytes(),
    );
    let result =
        crate::content_digest::digest_bytes("aster.code.tool-repeat.result.v1", output.as_bytes());
    let mut repeats = repeats
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = repeats.entry(call).or_default();
    if entry.0 == result {
        entry.1 += 1;
    } else {
        *entry = (result, 1);
    }
    let structured = serde_json::from_str::<serde_json::Value>(output).is_ok();
    (entry.1 > 1 && !structured).then(|| {
        format!(
            "{} {} times in a row this run; it observed nothing new.]",
            repeat_note_prefix(name),
            entry.1
        )
    })
}

pub(super) fn visible_tool_output(
    agent: Option<&crate::agent::AgentLoop>,
    output: String,
) -> String {
    match agent {
        Some(agent) => agent.sanitize_tool_output(&output),
        None => output,
    }
}

pub(super) async fn emit_tool_request_bound(tools: &ExecutorTools, call: &aster_effect::ToolCall) {
    let Ok(snapshot) = crate::harness_evidence::ToolRequestSnapshotV1::capture(
        &call.id,
        &call.name,
        &call.args,
        crate::harness_evidence::ToolRequestOriginV1::Agent,
    ) else {
        return;
    };
    tools
        .emit(crate::agent::AgentEvent::ToolRequestBound {
            tool_id: call.id.clone(),
            tool_name: call.name.clone(),
            snapshot,
        })
        .await;
}
