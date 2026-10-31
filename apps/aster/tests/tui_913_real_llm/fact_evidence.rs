#![cfg(test)]

use super::*;
use serde_json::Value;

pub(super) fn facts(workspace: &Path, home: &Path) -> Vec<Value> {
    parse_facts(&session_tool_trace(workspace, home))
}

pub(super) fn facts_for_session(workspace: &Path, home: &Path, session_id: &str) -> Vec<Value> {
    let mut logs = Vec::new();
    collect_trace_files(&workspace.join(".aster"), &mut logs);
    collect_trace_files(&home.join(".aster/logs"), &mut logs);
    logs.into_iter()
        .map(|text| parse_facts(&text))
        .filter(|records| {
            records.iter().any(|fact| {
                fact["type"] == "tool.prepared"
                    && fact["payload"]["prepared"]["identity"]["admission"]["thread_id"]
                        == session_id
            })
        })
        .flatten()
        .collect()
}

fn parse_facts(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("type").is_some() && value.get("seq").is_some())
        .collect()
}

#[test]
fn session_fact_sources_are_selected_by_the_exact_admission_identity() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir_all(workspace.join(".aster")).unwrap();
    for (file, session, marker) in [
        ("a.jsonl", "new-session", "new reply"),
        ("z.jsonl", "old-session", "old reply"),
    ] {
        let records = [
            serde_json::json!({"seq":1,"type":"tool.prepared","payload":{"prepared":{"identity":{"admission":{"thread_id":session}}}}}),
            serde_json::json!({"seq":2,"type":"model.turn","payload":{"kind":"text","text":marker}}),
            serde_json::json!({"seq":4,"type":"turn.complete","payload":{"terminal":"narrative","log_cut":{"seq":3}}}),
        ];
        let text = records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(workspace.join(".aster").join(file), text).unwrap();
    }
    assert_eq!(
        completed_narrative(&facts_for_session(&workspace, &home, "new-session")),
        Some("new reply")
    );
    assert!(facts_for_session(&workspace, &home, "absent-session").is_empty());
}

pub(super) fn settled_turns(facts: &[Value]) -> usize {
    facts
        .iter()
        .filter(|fact| fact["type"] == "turn.complete")
        .count()
}

pub(super) fn completed_narrative(facts: &[Value]) -> Option<&str> {
    let terminal = facts
        .iter()
        .rev()
        .find(|fact| fact["type"] == "turn.complete")?;
    let cut = terminal["payload"]["log_cut"]["seq"].as_u64()?;
    if terminal["payload"]["terminal"] != "narrative"
        || !terminal["payload"]["forced_stop"].is_null()
        || cut >= terminal["seq"].as_u64()?
    {
        return None;
    }
    let reply = facts.iter().rev().find(|fact| {
        fact["type"] == "model.turn" && fact["seq"].as_u64().is_some_and(|seq| seq <= cut)
    })?;
    let text = reply["payload"]["text"].as_str()?;
    (reply["payload"]["kind"] == "text" && !text.trim().is_empty()).then_some(text)
}

#[test]
fn thinking_text_or_an_unsettled_reply_cannot_pass_the_prose_gate() {
    let reply = serde_json::json!({"seq":2,"type":"model.turn","payload":{"kind":"text","text":"Final answer."}});
    let terminal = serde_json::json!({"seq":4,"type":"turn.complete","payload":{"terminal":"narrative","log_cut":{"seq":3}}});
    assert!(completed_narrative(&[]).is_none());
    assert!(completed_narrative(&[reply.clone()]).is_none());
    assert_eq!(
        completed_narrative(&[reply.clone(), terminal.clone()]),
        Some("Final answer.")
    );
    for (field, value) in [
        ("terminal", serde_json::json!("verified")),
        ("forced_stop", serde_json::json!("cancelled")),
        ("log_cut", serde_json::json!({"seq":4})),
    ] {
        let mut invalid = terminal.clone();
        invalid["payload"][field] = value;
        assert!(completed_narrative(&[reply.clone(), invalid]).is_none());
    }
    let tool = serde_json::json!({"seq":3,"type":"model.turn","payload":{"kind":"tool","text":"Reasoning text."}});
    assert!(completed_narrative(&[reply.clone(), tool, terminal.clone()]).is_none());
    let mut late = reply;
    late["seq"] = 5.into();
    assert!(completed_narrative(&[terminal, late]).is_none());
}

pub(super) fn receipt_for<'a>(facts: &'a [Value], tool: &str) -> Vec<&'a Value> {
    facts
        .iter()
        .filter_map(|fact| {
            if fact["type"] != "tool.prepared" || fact["payload"]["prepared"]["tool_name"] != tool {
                return None;
            }
            let prepared = &fact["payload"]["prepared"];
            facts.iter().find_map(|candidate| {
                let receipt = &candidate["payload"]["receipt"];
                (candidate["type"] == "tool.result" && receipt["identity"] == prepared["identity"])
                    .then_some(receipt)
            })
        })
        .collect()
}
