//! One ToBot turn reads the member memory bank, then writes the exchange back.
//! The member token stays in this process. A missing session or a failed call
//! leaves the turn unchanged.

use serde_json::{Value, json};

const REFLECT: &str = "/v1/api/org/tobot/memory/reflects";
const RETAIN: &str = "/v1/api/org/tobot/memory/retains";
const QUERY_CAP: usize = 500;
const NOTE_CAP: usize = 1500;
const CONTENT_CAP: usize = 8000;
const LEAD: &str = "Long-term memory about this member. Use a fact from this memory when it answers the user. If a fact is not in this memory, do not invent it.";
/// One host-turn constraint. a3s-code keeps each directive to 500 characters.
const MEMORY_RULE: &str = "Long-term memory for this member is already in the following constraints. Use a listed fact when it answers the user. If a fact is not listed, do not invent it. Do not search the workspace and do not run ls, search, glob, or bash to find a memory file.";
/// Room facts only. The personal memory rule stays off a group turn.
const ROOM_MEMORY_RULE: &str = "只用下面列出的群事实。";
const DIRECTIVE_CAP: usize = 500;
const DIRECTIVE_SLOTS: usize = 8;

pub(crate) fn recall_note(origin: &str, slug: &str, query: &str) -> Option<String> {
    recall_note_in(origin, slug, query, "personal", "")
}

/// `scope` is `personal` or `private`. `org_id` is sent when it is a uuid.
pub(crate) fn recall_note_in(
    origin: &str,
    slug: &str,
    query: &str,
    scope: &str,
    org_id: &str,
) -> Option<String> {
    if slug.is_empty() {
        return None;
    }
    let token = crate::member_session::MemberSessionStore::open(slug).token()?;
    recall_with_token(origin, &token, query, scope, org_id)
}

pub(crate) fn retain_exchange(origin: &str, slug: &str, key: &str, user: &str, assistant: &str) {
    retain_exchange_in(origin, slug, key, user, assistant, "personal", "", "ToBot")
}

pub(crate) fn retain_exchange_in(
    origin: &str,
    slug: &str,
    key: &str,
    user: &str,
    assistant: &str,
    scope: &str,
    org_id: &str,
    label: &str,
) {
    if slug.is_empty() {
        return;
    }
    let Some(token) = crate::member_session::MemberSessionStore::open(slug).token() else {
        return;
    };
    retain_with_token(origin, &token, key, user, assistant, scope, org_id, label);
}

/// Recall the room bank. The query is this turn's ask when the prompt has one.
pub(crate) fn room_recall_note(
    origin: &str,
    slug: &str,
    org_id: &str,
    conversation_id: &str,
    prompt: &str,
) -> Option<String> {
    if slug.is_empty() {
        return None;
    }
    let token = crate::member_session::MemberSessionStore::open(slug).token()?;
    let query = room_turn_ask(prompt).unwrap_or(prompt);
    let path = room_path(conversation_id, "recalls")?;
    let body = recall_body(query, "personal", org_id).to_string();
    let (status, _, _, payload) =
        crate::org_login::forward_member_api(origin, "POST", &path, body.as_bytes(), &token);
    note_from_recall(status, &payload)
}

/// Retain this turn's ask and answer. A prompt that is only a transcript is not stored.
pub fn room_retain(
    origin: &str,
    slug: &str,
    org_id: &str,
    conversation_id: &str,
    key: &str,
    prompt: &str,
    assistant: &str,
) {
    if slug.is_empty() {
        return;
    }
    let Some(ask) = room_turn_ask(prompt) else {
        return;
    };
    let Some(token) = crate::member_session::MemberSessionStore::open(slug).token() else {
        return;
    };
    let Some(path) = room_path(conversation_id, "retains") else {
        return;
    };
    let Some(body) = retain_body(key, ask, assistant, "personal", org_id, "ToBot") else {
        return;
    };
    let (status, _, _, payload) =
        crate::org_login::forward_member_api(origin, "POST", &path, &body, &token);
    if !(200..300).contains(&status) {
        log_failure("retain", status, &payload);
    }
}

/// The host prompt ends with `{asker} asked: {ask}`. That ask is the turn to retain.
pub fn room_turn_ask(prompt: &str) -> Option<&str> {
    let line = prompt.trim_end().lines().next_back()?.trim();
    let (_, ask) = line.split_once(" asked: ")?;
    let ask = ask.trim();
    (!ask.is_empty()).then_some(ask)
}

fn room_path(conversation_id: &str, action: &str) -> Option<String> {
    let conversation_id = conversation_id.trim();
    uuid::Uuid::parse_str(conversation_id).ok()?;
    Some(format!(
        "/v1/api/org/tobot/memory/rooms/{conversation_id}/{action}"
    ))
}

/// Host-turn constraints for a real memory note. An empty bank adds nothing,
/// so the user's words stay the user request.
pub fn memory_directives(note: Option<&str>) -> Vec<String> {
    directives_for(note, MEMORY_RULE)
}

pub fn room_memory_directives(note: Option<&str>) -> Vec<String> {
    directives_for(note, ROOM_MEMORY_RULE)
}

fn directives_for(note: Option<&str>, rule: &str) -> Vec<String> {
    let Some(note) = useful_note(note.unwrap_or("")) else {
        return Vec::new();
    };
    let mut directives = vec![rule.to_owned()];
    directives.extend(directive_chunks(&note));
    directives
}

/// Keep constraints already on the turn, then append memory. No note leaves `turn` as it was.
pub fn merge_memory_turn(turn: Option<&Value>, note: Option<&str>) -> Option<Value> {
    merge_directive_turn(turn, memory_directives(note))
}

/// Group turns list room facts under the room rule. The personal memory rule stays out.
pub fn merge_room_memory_turn(turn: Option<&Value>, note: Option<&str>) -> Option<Value> {
    merge_directive_turn(turn, room_memory_directives(note))
}

fn merge_directive_turn(turn: Option<&Value>, extra: Vec<String>) -> Option<Value> {
    if extra.is_empty() {
        return turn.cloned();
    }
    let mut directives = Vec::new();
    if let Some(items) = turn
        .and_then(|value| value.get("directives"))
        .and_then(Value::as_array)
    {
        for item in items {
            let Some(text) = item.as_str() else {
                continue;
            };
            if text.is_empty() || directives.len() >= DIRECTIVE_SLOTS {
                continue;
            }
            directives.push(text.to_owned());
        }
    }
    for text in extra {
        if directives.len() >= DIRECTIVE_SLOTS {
            break;
        }
        directives.push(text);
    }
    let research = turn
        .and_then(|value| value.get("research"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if directives.is_empty() && !research {
        return None;
    }
    Some(json!({ "directives": directives, "research": research }))
}

fn recall_body(query: &str, scope: &str, org_id: &str) -> Value {
    let mut body = json!({ "query": query, "budget_tokens": 400, "scope": scope });
    if let Some(org_id) = uuid_text(org_id) {
        body["org_id"] = json!(org_id);
    }
    body
}

fn recall_with_token(
    origin: &str,
    token: &str,
    query: &str,
    scope: &str,
    org_id: &str,
) -> Option<String> {
    let query = clip(query, QUERY_CAP);
    if query.is_empty() {
        return None;
    }
    let body = recall_body(&query, scope, org_id).to_string();
    let (status, _, _, payload) =
        crate::org_login::forward_member_api(origin, "POST", REFLECT, body.as_bytes(), token);
    note_from_response(status, &payload)
}

fn retain_with_token(
    origin: &str,
    token: &str,
    key: &str,
    user: &str,
    assistant: &str,
    scope: &str,
    org_id: &str,
    label: &str,
) {
    let Some(body) = retain_body(key, user, assistant, scope, org_id, label) else {
        return;
    };
    let (status, _, _, payload) =
        crate::org_login::forward_member_api(origin, "POST", RETAIN, &body, token);
    if !(200..300).contains(&status) {
        log_failure("retain", status, &payload);
    }
}

fn retain_body(
    key: &str,
    user: &str,
    assistant: &str,
    scope: &str,
    org_id: &str,
    label: &str,
) -> Option<Vec<u8>> {
    let key = turn_key(key)?;
    let content = exchange_content(user, assistant, label);
    if content.is_empty() {
        return None;
    }
    let mut body = json!({
        "content": content,
        "scope": scope,
        "idempotency_key": key,
    });
    if let Some(org_id) = uuid_text(org_id) {
        body["org_id"] = json!(org_id);
    }
    serde_json::to_vec(&body).ok()
}

fn uuid_text(raw: &str) -> Option<&str> {
    let raw = raw.trim();
    uuid::Uuid::parse_str(raw).ok()?;
    Some(raw)
}

fn exchange_content(user: &str, assistant: &str, label: &str) -> String {
    let user = clip(user, 3900);
    let assistant = clip(assistant, 3900);
    let content = match (user.is_empty(), assistant.is_empty()) {
        (true, true) => return String::new(),
        (false, true) => user,
        (true, false) => format!("{label}：{assistant}"),
        (false, false) => format!("{user}\n{label}：{assistant}"),
    };
    clip(&content, CONTENT_CAP)
}

fn turn_key(id: &str) -> Option<String> {
    let id: String = id
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '\0')
        .collect();
    if id.is_empty() || id.chars().count() > 74 {
        return None;
    }
    Some(format!("turn-{id}"))
}

fn note_from_response(status: u16, payload: &[u8]) -> Option<String> {
    if !(200..300).contains(&status) {
        log_failure("reflect", status, payload);
        return None;
    }
    let value: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
    if value.get("success") == Some(&Value::Bool(false)) {
        log_failure("reflect", status, payload);
        return None;
    }
    let data = value.get("data").unwrap_or(&value);
    useful_note(data.get("text").and_then(Value::as_str).unwrap_or(""))
}

fn note_from_recall(status: u16, payload: &[u8]) -> Option<String> {
    if !(200..300).contains(&status) {
        log_failure("recall", status, payload);
        return None;
    }
    let value: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
    if value.get("success") == Some(&Value::Bool(false)) {
        log_failure("recall", status, payload);
        return None;
    }
    let data = value.get("data").unwrap_or(&value);
    let rows = data.as_array()?;
    let mut lines = Vec::new();
    for row in rows {
        let text = row
            .get("fact")
            .and_then(|fact| fact.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let text = text.trim();
        if !text.is_empty() {
            lines.push(text.to_owned());
        }
    }
    useful_note(&lines.join("\n"))
}

fn useful_note(raw: &str) -> Option<String> {
    let note = strip_instruction(&clip(raw, NOTE_CAP));
    if note.is_empty() || is_blank_memory(&note) {
        None
    } else {
        Some(note)
    }
}

/// A bank that echoes the instruction, and nothing else, is not a fact.
fn strip_instruction(note: &str) -> String {
    let mut rest = note.trim();
    for lead in [LEAD, MEMORY_RULE] {
        if let Some(stripped) = rest.strip_prefix(lead) {
            rest = stripped.trim();
        }
    }
    rest.to_owned()
}

fn directive_chunks(note: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in note.split_inclusive('\n') {
        if current.chars().count() + line.chars().count() > DIRECTIVE_CAP && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if line.chars().count() <= DIRECTIVE_CAP {
            current.push_str(line);
            continue;
        }
        for ch in line.chars() {
            current.push(ch);
            if current.chars().count() == DIRECTIVE_CAP {
                chunks.push(std::mem::take(&mut current));
            }
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        chunks.push(tail.to_owned());
    }
    chunks
}

fn is_blank_memory(note: &str) -> bool {
    matches!(note, "没有相关记忆。" | "No related memories.")
}

fn clip(raw: &str, max_chars: usize) -> String {
    let cleaned: String = raw.chars().filter(|ch| *ch != '\0').collect();
    cleaned.trim().chars().take(max_chars).collect()
}

fn log_failure(action: &str, status: u16, payload: &[u8]) {
    let message = serde_json::from_slice::<Value>(payload)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();
    eprintln!(
        "studio: tobot memory {action} failed: {status} {}",
        clip(&message, 120)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, TcpListener};
    use std::time::Duration;

    #[test]
    fn a_fact_is_a_turn_constraint_and_an_empty_bank_adds_none() {
        let directives = memory_directives(Some("成员喜欢美式。"));
        assert_eq!(directives[0], MEMORY_RULE);
        assert!(directives[0].contains("Do not search the workspace"));
        assert_eq!(directives[1], "成员喜欢美式。");
        assert!(
            directives
                .iter()
                .all(|line| line.chars().count() <= DIRECTIVE_CAP)
        );
        assert!(memory_directives(None).is_empty());
        assert!(memory_directives(Some("没有相关记忆。")).is_empty());
        assert!(memory_directives(Some("No related memories.")).is_empty());
        assert!(memory_directives(Some(LEAD)).is_empty());
        assert_eq!(
            memory_directives(Some(&format!("{LEAD}\n\n成员喜欢美式。")))[1],
            "成员喜欢美式。"
        );

        let turn = json!({
            "directives": ["ToBot is not allowed to manage this mailbox."],
            "research": false
        });
        let merged = merge_memory_turn(Some(&turn), Some("成员喜欢美式。")).expect("turn");
        assert_eq!(
            merged["directives"][0],
            "ToBot is not allowed to manage this mailbox."
        );
        assert_eq!(merged["directives"][2], "成员喜欢美式。");
        assert_eq!(
            merge_memory_turn(Some(&turn), None).expect("kept")["directives"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert!(merge_memory_turn(None, Some("没有相关记忆。")).is_none());

        let long = "事".repeat(1200);
        let wide = memory_directives(Some(&long));
        assert!(wide.len() > 2);
        assert!(
            wide.iter()
                .all(|line| line.chars().count() <= DIRECTIVE_CAP)
        );
        assert_eq!(
            wide.iter()
                .skip(1)
                .map(|line| line.chars().count())
                .sum::<usize>(),
            1200
        );
    }

    #[test]
    fn retain_keeps_the_member_sentence_and_a_stable_key() {
        let body = retain_body(
            "11111111-1111-4111-8111-111111111111",
            "我喜欢美式。",
            "记下了。",
            "personal",
            "",
            "ToBot",
        )
        .expect("body");
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["idempotency_key"],
            "turn-11111111-1111-4111-8111-111111111111"
        );
        assert_eq!(value["content"], "我喜欢美式。\nToBot：记下了。");
        assert_eq!(value["scope"], "personal");
        assert!(value.get("context").is_none());
        assert!(value.get("org_id").is_none());
        let boss = retain_body(
            "11111111-1111-4111-8111-111111111111",
            "本周目标。",
            "记下了。",
            "private",
            "22222222-2222-4222-8222-222222222222",
            "ToBot",
        )
        .expect("boss");
        let boss: Value = serde_json::from_slice(&boss).unwrap();
        assert_eq!(boss["content"], "本周目标。\nToBot：记下了。");
        assert_eq!(boss["scope"], "private");
        assert_eq!(boss["org_id"], "22222222-2222-4222-8222-222222222222");
        let open = recall_body("咖啡", "personal", "");
        assert!(open.get("org_id").is_none());
        assert_eq!(open["scope"], "personal");
        assert_eq!(
            recall_body("咖啡", "private", "22222222-2222-4222-8222-222222222222")["scope"],
            "private"
        );
        assert!(retain_body("id", "  ", "  ", "personal", "", "ToBot").is_none());
        assert!(retain_body("   ", "我喜欢美式。", "记下了。", "personal", "", "ToBot").is_none());
        assert!(
            retain_body(
                &"x".repeat(75),
                "我喜欢美式。",
                "记下了。",
                "personal",
                "",
                "ToBot"
            )
            .is_none()
        );
    }

    #[test]
    fn a_group_turn_keeps_the_ask_and_the_room_rule() {
        let prompt = "Group: launch\nAnswer from this transcript. Do not continue an earlier private reply.\n\nAda: hello\n\nAda asked: ship Friday";
        assert_eq!(room_turn_ask(prompt), Some("ship Friday"));
        assert!(room_turn_ask("Group: launch\nonly a transcript").is_none());
        let directives = room_memory_directives(Some("群决定周五发布。"));
        assert_eq!(directives[0], ROOM_MEMORY_RULE);
        assert!(directives.iter().all(|line| line != MEMORY_RULE));
        assert_eq!(directives[1], "群决定周五发布。");
        let merged = merge_room_memory_turn(
            Some(&json!({ "directives": ["This turn is a group."], "research": false })),
            Some("群决定周五发布。"),
        )
        .expect("turn");
        let lines = merged["directives"].as_array().expect("lines");
        assert_eq!(lines[0], "This turn is a group.");
        assert_eq!(lines[1], ROOM_MEMORY_RULE);
        assert!(lines.iter().all(|line| line != MEMORY_RULE));
        let room = "11111111-1111-4111-8111-111111111111";
        assert_eq!(
            room_path(room, "recalls").as_deref(),
            Some("/v1/api/org/tobot/memory/rooms/11111111-1111-4111-8111-111111111111/recalls")
        );
        assert!(room_path("not-a-room", "retains").is_none());
    }

    #[test]
    fn reflect_and_retain_use_the_member_memory_routes() {
        let (origin, seen) = serve_two();
        let note = recall_with_token(
            &origin,
            "a3s_ms_session.token",
            &format!("咖啡{}", "啊".repeat(600)),
            "personal",
            "",
        );
        assert_eq!(note.as_deref(), Some("成员喜欢美式。"));
        retain_with_token(
            &origin,
            "a3s_ms_session.token",
            "11111111-1111-4111-8111-111111111111",
            "我喜欢美式。",
            "记下了。",
            "personal",
            "",
            "ToBot",
        );
        let first = seen.recv_timeout(Duration::from_secs(3)).expect("reflect");
        let second = seen.recv_timeout(Duration::from_secs(3)).expect("retain");
        assert!(first.contains("POST /v1/api/org/tobot/memory/reflects "));
        let reflect: Value = json_body(&first);
        assert_eq!(reflect["query"].as_str().unwrap().chars().count(), 500);
        assert_eq!(reflect["budget_tokens"], 400);
        assert_eq!(reflect["scope"], "personal");
        assert!(second.contains("POST /v1/api/org/tobot/memory/retains "));
        let retain: Value = json_body(&second);
        assert_eq!(retain["scope"], "personal");
        assert!(retain.get("context").is_none());
        assert_eq!(
            retain["idempotency_key"],
            "turn-11111111-1111-4111-8111-111111111111"
        );
        assert_eq!(retain["content"], "我喜欢美式。\nToBot：记下了。");
    }

    fn json_body(raw: &str) -> Value {
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or("");
        serde_json::from_str(body).unwrap_or_else(|error| panic!("{error}: {body}"))
    }

    fn serve_two() -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            if request_complete(&buf) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
                let body = r#"{"success":true,"code":0,"message":"OK","data":{"text":"成员喜欢美式。","based_on":[],"confidence":0.4}}"#;
                let raw = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(raw.as_bytes());
            }
        });
        (format!("http://{addr}"), rx)
    }

    fn request_complete(buf: &[u8]) -> bool {
        let Some(split) = buf.windows(4).position(|mark| mark == b"\r\n\r\n") else {
            return false;
        };
        let head = String::from_utf8_lossy(&buf[..split]);
        let length = head
            .split("\r\n")
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        buf.len() >= split + 4 + length
    }

    #[test]
    fn catalog_memory_contexts_follow_the_session() {
        use super::super::domain::{
            member_toboss_room_session_id, member_toboss_session_id, member_tobot_session_id,
            private_memory,
        };

        let member = "8089eba1-5b69-435d-ba4b-727e4af5b91a";
        let other = "225e2510-5521-4aa7-b96c-e0b0c0da6037";
        let room = "11111111-1111-4111-8111-111111111111";
        let personal_id = member_tobot_session_id(member).expect("personal");
        let boss_id = member_toboss_session_id(member).expect("boss");
        let room_id = member_toboss_room_session_id(member, room).expect("room");
        let other_id = member_tobot_session_id(other).expect("other");
        let personal = private_memory(&personal_id).expect("personal memory");
        let boss = private_memory(&boss_id).expect("boss memory");
        assert!(private_memory(&room_id).is_none());
        assert_ne!(personal_id, other_id);
        assert_eq!(personal.scope, "personal");
        assert_eq!(personal.scope, "personal");
        assert_eq!(boss.scope, "private");
        assert_eq!(boss.scope, "private");
        assert_ne!(boss.scope, personal.scope);

        let query = format!("例会{}", "啊".repeat(600));
        let clipped = clip(&query, QUERY_CAP);
        let query_chars = clipped.chars().count();
        assert_eq!(query_chars, QUERY_CAP);
        let reflect = recall_body(&clipped, personal.scope, member);
        assert_eq!(reflect["query"], clipped);
        assert_eq!(reflect["scope"], personal.scope);
        assert_eq!(reflect["budget_tokens"], 400);

        let note = "事".repeat(2_000);
        let directives = memory_directives(Some(&note));
        let note_chars: usize = directives
            .iter()
            .skip(1)
            .map(|line| line.chars().count())
            .sum();
        assert_eq!(note_chars, NOTE_CAP);
        assert!(
            directives
                .iter()
                .all(|line| line.chars().count() <= DIRECTIVE_CAP)
        );
        assert!(memory_directives(Some("没有相关记忆。")).is_empty());
        assert!(memory_directives(Some("No related memories.")).is_empty());

        let full = json!({
            "directives": (0..DIRECTIVE_SLOTS).map(|index| format!("client-{index}")).collect::<Vec<_>>(),
            "research": false
        });
        let merged = merge_memory_turn(Some(&full), Some("成员喜欢美式。")).expect("merged");
        let slots = merged["directives"].as_array().expect("slots");
        assert_eq!(slots.len(), DIRECTIVE_SLOTS);
        assert!(
            slots
                .iter()
                .all(|item| item.as_str() != Some("成员喜欢美式。"))
        );

        let retained = retain_body(
            "11111111-1111-4111-8111-111111111111",
            "例会固定周三 15:00，我只喝美式",
            "记下了。",
            personal.scope,
            member,
            personal.label,
        )
        .expect("retain");
        let retained: Value = serde_json::from_slice(&retained).expect("json");
        assert_eq!(retained["scope"], "personal");
        assert_eq!(
            retained["idempotency_key"],
            "turn-11111111-1111-4111-8111-111111111111"
        );
        let boss_retained = retain_body(
            "22222222-2222-4222-8222-222222222222",
            "例会固定周三 15:00，我只喝美式",
            "记下了。",
            boss.scope,
            member,
            boss.label,
        )
        .expect("boss retain");
        let boss_retained: Value = serde_json::from_slice(&boss_retained).expect("json");
        assert_eq!(boss_retained["scope"], "private");
        assert_ne!(
            boss_retained["idempotency_key"],
            retained["idempotency_key"]
        );

        eprintln!(
            "ORACLE personal context={} session={}",
            personal.scope, personal_id
        );
        eprintln!("ORACLE private context={} session={}", boss.scope, boss_id);
        eprintln!("ORACLE room session={room_id} reflect=0 retain=0");
        eprintln!("ORACLE query_chars={query_chars} note_chars={note_chars}");
        eprintln!("ORACLE full_slots={} memory_injected=0", slots.len());
        eprintln!(
            "ORACLE retain context={} idempotency_key={}",
            retained["scope"].as_str().unwrap_or(""),
            retained["idempotency_key"].as_str().unwrap_or("")
        );
        eprintln!(
            "ORACLE boss retain context={} idempotency_key={}",
            boss_retained["scope"].as_str().unwrap_or(""),
            boss_retained["idempotency_key"].as_str().unwrap_or("")
        );
        eprintln!("ORACLE blank_directives=0 other_session={other_id}");
    }
}
