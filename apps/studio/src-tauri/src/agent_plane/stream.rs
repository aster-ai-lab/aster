//! ACP `session/update` → AI SDK `UIMessageChunk`, plus the `parts` the renderer's
//! `readUIMessageStream` would build from the same chunks (persisted on the assistant row).

use std::collections::HashMap;

use super::forms::{FormBinding, FormValues};
use serde_json::{Value, json};

const TRANSPORT: &str = "a3s-code-acp";

#[derive(Clone, Copy, PartialEq)]
enum Block {
    Text,
    Reasoning,
}

pub struct TurnStream {
    message_id: String,
    started: bool,
    open: Option<(Block, String, usize)>,
    counter: usize,
    tools: HashMap<String, usize>,
    pub parts: Vec<Value>,
    capture_epoch: Option<u64>,
}

impl TurnStream {
    pub fn new(message_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            started: false,
            open: None,
            counter: 0,
            tools: HashMap::new(),
            parts: Vec::new(),
            capture_epoch: None,
        }
    }

    pub(super) fn capturing(message_id: &str, epoch: u64) -> Self {
        let mut stream = Self::new(message_id);
        stream.capture_epoch = Some(epoch);
        stream
    }

    pub(super) fn capture(&self) -> Option<(&str, u64)> {
        self.capture_epoch
            .map(|epoch| (self.message_id.as_str(), epoch))
    }

    pub fn update(&mut self, update: &Value) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        match update.get("sessionUpdate").and_then(Value::as_str) {
            Some("agent_message_chunk") => {
                if let Some(text) = update.get("content").and_then(text_of) {
                    self.delta(Block::Text, &text, &mut chunks);
                }
            }
            Some("agent_thought_chunk") => {
                if let Some(text) = update.get("content").and_then(text_of) {
                    self.delta(Block::Reasoning, &text, &mut chunks);
                }
            }
            Some("tool_call") | Some("tool_call_update") => self.tool(update, &mut chunks),
            _ => {}
        }
        chunks
    }

    pub(super) fn review_document(&mut self, path: &str, revision: &str) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        self.close_block(&mut chunks);
        let part = json!({"type":"data-review-source","data":{"kind":"document","path":path,"sha256":revision}});
        self.parts.push(part.clone());
        chunks.push(part);
        chunks
    }

    pub(super) fn review_progress(
        &mut self,
        index: usize,
        total: usize,
        segment: &str,
    ) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        self.close_block(&mut chunks);
        let quote: String = segment
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .trim()
            .chars()
            .take(72)
            .collect();
        let part = json!({"type":"data-review-progress","id":"review-progress","data":{"index":index + 1,"total":total,"quote":quote}});
        self.parts
            .retain(|item| item["type"] != "data-review-progress");
        self.parts.push(part.clone());
        chunks.push(part);
        chunks
    }

    /// Live retry status stays out of the persisted review result.
    pub(super) fn review_retry(
        &mut self,
        segment: usize,
        model: &str,
        reason: Option<&str>,
    ) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        self.close_block(&mut chunks);
        let data = match reason {
            Some(reason) => json!({"state":"retrying","modelId":model,"attempt":2,"reason":reason}),
            None => json!({"state":"settled"}),
        };
        chunks
            .push(json!({"type":"data-retry","id":format!("review-retry-{segment}"),"data":data}));
        chunks
    }

    pub(super) fn proof(&mut self, proof: &super::workbench_proof::Proof) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        self.close_block(&mut chunks);
        let data = json!(proof);
        let part = json!({"type":"data-workbench-proof","id":"workbench-proof","data":data});
        if let Some(slot) = self
            .parts
            .iter_mut()
            .find(|part| part["type"] == "data-workbench-proof")
        {
            *slot = part.clone();
        } else {
            self.parts.push(part.clone());
        }
        chunks.push(part);
        chunks
    }

    pub fn approval_request(&mut self, approval_id: &str, tool_call_id: &str) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        self.close_block(&mut chunks);
        if let Some(part) = self.tool_part(tool_call_id) {
            part["state"] = json!("approval-requested");
            part["approval"] = json!({ "id": approval_id });
        }
        chunks.push(json!({
            "type": "tool-approval-request",
            "approvalId": approval_id,
            "toolCallId": tool_call_id,
        }));
        chunks
    }

    pub fn approval_responded(&mut self, approval_id: &str, tool_call_id: &str, approved: bool) {
        if let Some(part) = self.tool_part(tool_call_id) {
            part["state"] = json!("approval-responded");
            part["approval"] = json!({ "id": approval_id, "approved": approved });
        }
    }

    /// A held ask records the decision itself. `approval-responded` still reads as running.
    pub fn decide_tool(&mut self, tool_call_id: &str, approved: bool) -> Vec<Value> {
        let mut chunks = Vec::new();
        if self.tool_part(tool_call_id).is_none() {
            return chunks;
        }
        if approved {
            let output = json!("已完成");
            if let Some(part) = self.tool_part(tool_call_id) {
                part["state"] = json!("output-available");
                part["output"] = output.clone();
            }
            chunks.push(json!({
                "type": "tool-output-available",
                "toolCallId": tool_call_id,
                "output": output,
                "providerExecuted": true,
                "dynamic": true,
            }));
        } else if let Some(part) = self.tool_part(tool_call_id) {
            part["state"] = json!("output-denied");
            chunks.push(json!({
                "type": "tool-output-error",
                "toolCallId": tool_call_id,
                "errorText": "denied",
                "providerExecuted": true,
                "dynamic": true,
            }));
        }
        chunks
    }

    /// One OpenUI Lang form the turn is parked on (`data-form` part).
    pub(super) fn form_request(&mut self, binding: &FormBinding, spec: &str) -> Vec<Value> {
        self.form_request_format(binding, spec, super::forms::FormFormat::OpenUi)
    }

    pub(super) fn question_request(&mut self, binding: &FormBinding, spec: &str) -> Vec<Value> {
        self.form_request_format(binding, spec, super::forms::FormFormat::Question)
    }

    fn form_request_format(
        &mut self,
        binding: &FormBinding,
        spec: &str,
        format: super::forms::FormFormat,
    ) -> Vec<Value> {
        let form_id = &binding.request_id;
        let mut chunks = self.ensure_started();
        self.close_block(&mut chunks);
        let data = json!({ "id": form_id, "format": format.as_str(), "spec": spec, "state": "form-requested", "binding": binding });
        match self
            .parts
            .iter_mut()
            .find(|part| part["type"] == "data-form" && part["data"]["id"] == form_id.as_str())
        {
            Some(part) => part["data"] = data.clone(),
            None => self
                .parts
                .push(json!({ "type": "data-form", "id": form_id, "data": data })),
        }
        chunks.push(json!({ "type": "data-form", "id": form_id, "data": data }));
        chunks
    }

    /// The member submitted; the part locks into its answered summary.
    pub(super) fn form_answered(
        &mut self,
        binding: &FormBinding,
        values: &FormValues,
    ) -> Vec<Value> {
        let form_id = &binding.request_id;
        let mut chunks = Vec::new();
        let Some(part) = self.parts.iter_mut().find(|part| {
            part["type"] == "data-form"
                && part["data"]["id"] == form_id.as_str()
                && part["data"]["binding"] == json!(binding)
                && part["data"]["state"] == "form-requested"
        }) else {
            return chunks;
        };
        part["data"]["state"] = json!("answered");
        part["data"]["values"] = json!(values);
        chunks.push(json!({ "type": "data-form", "id": form_id, "data": part["data"] }));
        chunks
    }

    pub(super) fn close_forms(&mut self) -> Vec<Value> {
        let mut chunks = Vec::new();
        for part in &mut self.parts {
            if part["type"] == "data-form" && part["data"]["state"] == "form-requested" {
                part["data"]["state"] = json!("closed");
                chunks.push(
                    json!({ "type": "data-form", "id": part["data"]["id"], "data": part["data"] }),
                );
            }
        }
        chunks
    }

    /// Tool permission requests can precede the `tool_call` update; register the call first.
    pub fn ensure_tool(&mut self, tool_call: &Value) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        let Some(id) = tool_call.get("toolCallId").and_then(Value::as_str) else {
            return chunks;
        };
        if !self.tools.contains_key(id) {
            self.close_block(&mut chunks);
            self.start_tool(id, tool_call, &mut chunks);
        } else {
            self.adopt_input(id, tool_call);
        }
        chunks
    }

    pub fn finish(&mut self, finish_reason: &str) -> Vec<Value> {
        let mut chunks = self.ensure_started();
        self.close_block(&mut chunks);
        chunks.extend(self.close_forms());
        chunks.push(json!({ "type": "finish-step" }));
        chunks.push(json!({ "type": "finish", "finishReason": finish_reason }));
        chunks
    }

    pub fn abort(&mut self) -> Vec<Value> {
        let mut chunks = Vec::new();
        self.close_block(&mut chunks);
        chunks.extend(self.close_forms());
        chunks.push(json!({ "type": "abort", "reason": "cancelled" }));
        self.settle("interrupted");
        chunks
    }

    pub fn error(&mut self, message: &str) -> Vec<Value> {
        let mut chunks = Vec::new();
        self.close_block(&mut chunks);
        chunks.extend(self.close_forms());
        chunks.push(json!({ "type": "error", "errorText": message }));
        self.settle(message);
        self.parts.push(json!({
            "type": "data-error",
            "data": { "name": "Error", "message": message },
        }));
        chunks
    }

    fn ensure_started(&mut self) -> Vec<Value> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        self.parts.push(json!({ "type": "step-start" }));
        vec![
            json!({ "type": "start", "messageId": self.message_id }),
            json!({ "type": "start-step" }),
        ]
    }

    fn delta(&mut self, block: Block, text: &str, chunks: &mut Vec<Value>) {
        if text.is_empty() {
            return;
        }
        if self.open.as_ref().map(|(open, _, _)| *open) != Some(block) {
            self.close_block(chunks);
            self.counter += 1;
            let (kind, prefix) = match block {
                Block::Text => ("text", "a3s-text"),
                Block::Reasoning => ("reasoning", "a3s-reasoning"),
            };
            let id = format!("{prefix}-{}", self.counter);
            chunks.push(json!({ "type": format!("{kind}-start"), "id": id }));
            self.parts
                .push(json!({ "type": kind, "text": "", "state": "streaming" }));
            self.open = Some((block, id, self.parts.len() - 1));
        }
        let Some((block, id, index)) = &self.open else {
            return;
        };
        let kind = if *block == Block::Text {
            "text"
        } else {
            "reasoning"
        };
        chunks.push(json!({ "type": format!("{kind}-delta"), "id": id, "delta": text }));
        if let Some(existing) = self.parts[*index]["text"].as_str() {
            self.parts[*index]["text"] = json!(format!("{existing}{text}"));
        }
    }

    fn close_block(&mut self, chunks: &mut Vec<Value>) {
        if let Some((block, id, index)) = self.open.take() {
            let kind = if block == Block::Text {
                "text"
            } else {
                "reasoning"
            };
            chunks.push(json!({ "type": format!("{kind}-end"), "id": id }));
            self.parts[index]["state"] = json!("done");
        }
    }

    fn tool(&mut self, update: &Value, chunks: &mut Vec<Value>) {
        let Some(id) = update.get("toolCallId").and_then(Value::as_str) else {
            return;
        };
        if !self.tools.contains_key(id) {
            self.close_block(chunks);
            self.start_tool(id, update, chunks);
        } else {
            self.adopt_input(id, update);
        }
        if let Some(kind) = update.get("kind").and_then(Value::as_str) {
            if let Some(part) = self.tool_part(id) {
                part["callProviderMetadata"]["cherry"]["toolKind"] = json!(kind);
            }
        }
        let status = update.get("status").and_then(Value::as_str);
        match status {
            Some("completed") => {
                let output = update
                    .get("rawOutput")
                    .filter(|value| !value.is_null())
                    .cloned()
                    .unwrap_or_else(|| json!(content_text(update.get("content"))));
                chunks.push(json!({
                    "type": "tool-output-available",
                    "toolCallId": id,
                    "output": output,
                    "providerExecuted": true,
                    "dynamic": true,
                }));
                if let Some(part) = self.tool_part(id) {
                    part["state"] = json!("output-available");
                    part["output"] = output;
                }
            }
            Some("failed") => {
                let text = content_text(update.get("content"));
                let error = if text.is_empty() {
                    "Tool failed".to_owned()
                } else {
                    text
                };
                chunks.push(json!({
                    "type": "tool-output-error",
                    "toolCallId": id,
                    "errorText": error,
                    "providerExecuted": true,
                    "dynamic": true,
                }));
                if let Some(part) = self.tool_part(id) {
                    part["state"] = json!("output-error");
                    part["errorText"] = json!(error);
                    part.as_object_mut().map(|object| object.remove("files"));
                }
            }
            _ => {}
        }
        if status != Some("failed") {
            self.remember_files(id, update);
        }
    }

    /// A v9.1.2 edit names the written file on `content` as `{ type: "diff", path }`.
    fn remember_files(&mut self, id: &str, update: &Value) {
        let paths = written_paths(update);
        if paths.is_empty() {
            return;
        }
        let Some(part) = self.tool_part(id) else {
            return;
        };
        let mut files: Vec<String> = part
            .get("files")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        for path in paths {
            if !files.iter().any(|existing| existing == &path) {
                files.push(path);
            }
        }
        part["files"] = json!(files);
    }

    fn start_tool(&mut self, id: &str, update: &Value, chunks: &mut Vec<Value>) {
        let name = update
            .get("title")
            .and_then(Value::as_str)
            .filter(|title| !title.is_empty())
            .unwrap_or("tool")
            .to_owned();
        let input = update.get("rawInput").cloned().unwrap_or_else(|| json!({}));
        let metadata = json!({ "cherry": { "transport": TRANSPORT, "toolName": name, "toolKind": update.get("kind").and_then(Value::as_str) } });
        chunks.push(json!({
            "type": "tool-input-start",
            "toolCallId": id,
            "toolName": name,
            "providerExecuted": true,
            "dynamic": true,
            "providerMetadata": metadata,
        }));
        chunks.push(json!({
            "type": "tool-input-available",
            "toolCallId": id,
            "toolName": name,
            "input": input,
            "providerExecuted": true,
            "dynamic": true,
            "providerMetadata": metadata,
        }));
        self.parts.push(json!({
            "type": "dynamic-tool",
            "toolName": name,
            "toolCallId": id,
            "state": "input-available",
            "input": input,
            "providerExecuted": true,
            "callProviderMetadata": metadata,
        }));
        self.tools.insert(id.to_owned(), self.parts.len() - 1);
    }

    fn tool_part(&mut self, id: &str) -> Option<&mut Value> {
        let index = *self.tools.get(id)?;
        self.parts.get_mut(index)
    }

    /// A permission request can follow a tool row that was opened before
    /// arguments existed. Keep the first non-empty `rawInput` as `input`.
    fn adopt_input(&mut self, id: &str, source: &Value) {
        let Some(raw) = source.get("rawInput").filter(|value| !value.is_null()) else {
            return;
        };
        if raw.as_object().is_some_and(|object| object.is_empty()) {
            return;
        }
        let Some(part) = self.tool_part(id) else {
            return;
        };
        let empty = part.get("input").is_none_or(|input| {
            input.is_null() || input.as_object().is_some_and(|object| object.is_empty())
        });
        if empty {
            part["input"] = raw.clone();
        }
    }

    /// Paused or failed turns: nothing may be left streaming or waiting on a tool.
    fn settle(&mut self, reason: &str) {
        for part in &mut self.parts {
            if part["state"] == "streaming" {
                part["state"] = json!("done");
            }
            if part["type"] == "dynamic-tool"
                && !matches!(
                    part["state"].as_str(),
                    Some("output-available" | "output-error")
                )
            {
                part["state"] = json!("output-error");
                part["errorText"] = json!(reason);
            }
        }
    }
}

pub fn finish_reason(stop_reason: &str) -> &'static str {
    match stop_reason {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "refusal" => "content-filter",
        _ => "other",
    }
}

fn text_of(content: &Value) -> Option<String> {
    match content.get("type").and_then(Value::as_str) {
        Some("text") => content
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Some("content") => content.get("content").and_then(text_of),
        _ => None,
    }
}

/// Paths a completed edit wrote. Read-tool locations are not deliverables.
fn written_paths(update: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    let Some(items) = update.get("content").and_then(Value::as_array) else {
        return paths;
    };
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("diff") {
            continue;
        }
        let Some(path) = item.get("path").and_then(Value::as_str) else {
            continue;
        };
        let path = path.trim();
        if path.is_empty() || paths.iter().any(|existing| existing == path) {
            continue;
        }
        paths.push(path.to_owned());
    }
    paths
}

fn content_text(content: Option<&Value>) -> String {
    content
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(text_of)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(chunks: &[Value]) -> Vec<&str> {
        chunks
            .iter()
            .map(|chunk| chunk["type"].as_str().unwrap())
            .collect()
    }

    #[test]
    fn streams_text_and_reasoning_blocks() {
        let mut stream = TurnStream::new("m1");
        let first = stream.update(&json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": "think" },
        }));
        assert_eq!(
            kinds(&first),
            ["start", "start-step", "reasoning-start", "reasoning-delta"]
        );
        let second = stream.update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "hel" },
        }));
        assert_eq!(
            kinds(&second),
            ["reasoning-end", "text-start", "text-delta"]
        );
        stream.update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "lo" },
        }));
        let end = stream.finish("stop");
        assert_eq!(kinds(&end), ["text-end", "finish-step", "finish"]);
        assert_eq!(
            stream.parts,
            vec![
                json!({ "type": "step-start" }),
                json!({ "type": "reasoning", "text": "think", "state": "done" }),
                json!({ "type": "text", "text": "hello", "state": "done" }),
            ]
        );
    }

    #[test]
    fn maps_tool_calls_and_their_outputs() {
        let mut stream = TurnStream::new("m1");
        let start = stream.update(&json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "t1",
            "title": "bash",
            "status": "in_progress",
            "rawInput": { "command": "ls" },
        }));
        assert_eq!(
            kinds(&start),
            [
                "start",
                "start-step",
                "tool-input-start",
                "tool-input-available"
            ]
        );
        let done = stream.update(&json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "t1",
            "status": "completed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "out" } }],
        }));
        assert_eq!(kinds(&done), ["tool-output-available"]);
        assert_eq!(done[0]["output"], json!("out"));
        assert_eq!(stream.parts[1]["state"], "output-available");
        assert_eq!(stream.parts[1]["input"], json!({ "command": "ls" }));
    }

    #[test]
    fn a_completed_edit_keeps_the_written_file_on_the_tool() {
        let mut stream = TurnStream::new("m1");
        stream.update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "先写文件。" },
        }));
        stream.update(&json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "t-write",
            "title": "edit",
            "status": "in_progress",
        }));
        stream.update(&json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "t-write",
            "status": "completed",
            "content": [{
                "type": "diff",
                "path": "/workspace/notes.md",
                "oldText": "",
                "newText": "hello",
            }],
        }));
        stream.update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "写好了。" },
        }));
        stream.finish("stop");
        assert_eq!(stream.parts[1]["text"], "先写文件。");
        assert_eq!(stream.parts[2]["type"], "dynamic-tool");
        assert_eq!(stream.parts[2]["state"], "output-available");
        assert_eq!(stream.parts[2]["toolName"], "edit");
        assert_eq!(stream.parts[2]["files"], json!(["/workspace/notes.md"]));
        assert_eq!(stream.parts[3]["text"], "写好了。");
    }

    #[test]
    fn a_failed_tool_settles_as_an_error_without_a_product() {
        let mut stream = TurnStream::new("m1");
        stream.update(&json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "t1",
            "title": "edit",
            "status": "in_progress",
            "content": [{ "type": "diff", "path": "/workspace/notes.md" }],
        }));
        stream.update(&json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "t1",
            "status": "failed",
            "content": [{ "type": "text", "text": "disk full" }],
        }));
        assert_eq!(stream.parts[1]["state"], "output-error");
        assert_eq!(stream.parts[1]["errorText"], "disk full");
        assert!(stream.parts[1].get("files").is_none());
    }

    #[test]
    fn aborting_settles_streaming_parts_and_pending_tools() {
        let mut stream = TurnStream::new("m1");
        stream.update(&json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "t1",
            "title": "bash",
        }));
        stream.update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "partial" },
        }));
        let chunks = stream.abort();
        assert_eq!(kinds(&chunks), ["text-end", "abort"]);
        assert_eq!(stream.parts[1]["state"], "output-error");
        assert_eq!(stream.parts[2]["state"], "done");
    }

    #[test]
    fn permission_raw_input_fills_a_tool_opened_without_arguments() {
        let mut stream = TurnStream::new("m1");
        stream.update(&json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "t1",
            "title": "bash",
        }));
        assert_eq!(stream.parts[1]["input"], json!({}));
        stream.ensure_tool(&json!({
            "toolCallId": "t1",
            "title": "bash",
            "rawInput": { "command": "printf token" },
        }));
        assert_eq!(stream.parts[1]["input"]["command"], "printf token");
        stream.ensure_tool(&json!({
            "toolCallId": "t1",
            "rawInput": { "command": "printf other" },
        }));
        assert_eq!(stream.parts[1]["input"]["command"], "printf token");
    }

    #[test]
    fn approval_marks_the_tool_part() {
        let mut stream = TurnStream::new("m1");
        stream.ensure_tool(&json!({ "toolCallId": "t1", "title": "bash" }));
        let chunks = stream.approval_request("a1", "t1");
        assert_eq!(kinds(&chunks), ["tool-approval-request"]);
        assert_eq!(stream.parts[1]["state"], "approval-requested");
        stream.approval_responded("a1", "t1", true);
        assert_eq!(
            stream.parts[1]["approval"],
            json!({ "id": "a1", "approved": true })
        );
    }
}

#[cfg(test)]
#[path = "stream_form_tests.rs"]
mod form_tests;

#[cfg(test)]
#[path = "review_progress_tests.rs"]
mod review_progress_tests;
