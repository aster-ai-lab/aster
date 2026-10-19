#![cfg(test)]

use super::*;

const ORG: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af12";
const LEAVE: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af10";
const APPROVAL: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af11";

struct Fake {
    calls: Mutex<Vec<String>>,
}

impl WorkReader for Fake {
    fn summary(&self, org_id: &str) -> std::result::Result<Value, String> {
        assert_eq!(org_id, ORG);
        self.calls.lock().expect("calls").push("summary".to_owned());
        Ok(json!({ "todo": 2, "done": 0, "cc": 0, "sent": 0 }))
    }

    fn list(
        &self,
        org_id: &str,
        box_name: &str,
        kind: &str,
        query: &str,
    ) -> std::result::Result<Value, String> {
        assert_eq!(org_id, ORG);
        self.calls
            .lock()
            .expect("calls")
            .push(format!("list {box_name} {kind} {query}"));
        Ok(json!([
            { "id": LEAVE, "kind": "leave", "title": "事假" },
            { "id": APPROVAL, "kind": "approval", "title": "采购" }
        ]))
    }

    fn read(&self, org_id: &str, id: &str) -> std::result::Result<Value, String> {
        assert_eq!(org_id, ORG);
        self.calls.lock().expect("calls").push(format!("read {id}"));
        if id == LEAVE {
            return Ok(
                json!({ "id": LEAVE, "kind": "leave", "title": "事假", "fields": { "reason": "事假" } }),
            );
        }
        Ok(json!({ "id": id, "kind": "notice", "title": "公告" }))
    }
}

fn open_policy() -> BotPolicy {
    BotPolicy {
        allow: true,
        list: true,
        read: true,
        kinds: DEFAULT_KINDS,
    }
}

fn call(reader: &Fake, name: &str, arguments: Value, policy: BotPolicy) -> Value {
    handle_rpc_with(
        reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }),
        policy,
    )
}

#[test]
fn refuses_a_missing_team_before_the_reader_and_keeps_every_kind() {
    let reader = Fake {
        calls: Mutex::new(Vec::new()),
    };
    for org in [Value::Null, json!("personal"), json!("bad?org=other")] {
        for name in ["list_work", "read_work"] {
            let response = call(
                &reader,
                name,
                json!({ "org_id": org, "id": LEAVE, "q": "采购" }),
                open_policy(),
            );
            assert_eq!(response["result"]["isError"], true);
        }
    }
    assert!(reader.calls.lock().expect("calls").is_empty());
    assert_eq!(
        team_path("/v1/api/org/members/work-items/summary", ORG).unwrap(),
        format!("/v1/api/org/members/work-items/summary?org={ORG}")
    );
    assert_eq!(
        team_path(&list_path("todo", "leave", "采购"), ORG).unwrap(),
        format!(
            "/v1/api/org/members/work-items?box=todo&kind=leave&q=%E9%87%87%E8%B4%AD&org={ORG}"
        )
    );
    let stored = store_policy(&json!({
        "allow": true,
        "list": true,
        "read": false,
        "kinds": ["nope", "seal", "seal"]
    }));
    assert!(stored.list_ok("seal"));
    assert!(!stored.list_ok("leave"));
    assert_eq!(policy_json(stored)["kinds"], json!(["seal"]));
    let every = store_policy(&json!({ "allow": true }));
    assert_eq!(policy_json(every)["kinds"].as_array().unwrap().len(), 8);
    *policy_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = BotPolicy::closed();
}

#[test]
fn lists_any_allowed_task_and_hides_the_rest() {
    let reader = Fake {
        calls: Mutex::new(Vec::new()),
    };
    let listed = call(
        &reader,
        "list_work",
        json!({ "org_id": ORG, "box": "todo", "q": "采购" }),
        open_policy(),
    );
    let text = listed["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(listed["result"]["isError"], false);
    assert!(text.contains("事假"));
    assert!(text.contains("采购"));
    assert!(text.contains("\"todo\":2"));

    let leave_only = BotPolicy {
        allow: true,
        list: true,
        read: true,
        kinds: kind_bit("leave").unwrap(),
    };
    let narrowed = call(
        &reader,
        "list_work",
        json!({ "org_id": ORG, "kind": "leave" }),
        leave_only,
    );
    let narrowed_text = narrowed["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(narrowed["result"]["isError"], false);
    assert!(narrowed_text.contains("事假"));
    assert!(!narrowed_text.contains("采购"));

    let refused = call(
        &reader,
        "list_work",
        json!({ "org_id": ORG, "kind": "notice" }),
        leave_only,
    );
    assert_eq!(refused["result"]["isError"], true);
    assert!(
        refused["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not allowed")
    );
}

#[test]
fn reads_an_allowed_task_and_refuses_a_decision() {
    let reader = Fake {
        calls: Mutex::new(Vec::new()),
    };
    let opened = call(
        &reader,
        "read_work",
        json!({ "org_id": ORG, "id": LEAVE }),
        open_policy(),
    );
    assert_eq!(opened["result"]["isError"], false);
    assert!(
        opened["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("事假")
    );

    let leave_only = BotPolicy {
        allow: true,
        list: true,
        read: true,
        kinds: kind_bit("leave").unwrap(),
    };
    let hidden = call(
        &reader,
        "read_work",
        json!({ "org_id": ORG, "id": APPROVAL }),
        leave_only,
    );
    assert_eq!(hidden["result"]["isError"], true);

    let decision = call(
        &reader,
        "decide_work",
        json!({ "org_id": ORG, "id": LEAVE, "decision": "approve" }),
        open_policy(),
    );
    assert_eq!(decision["result"]["isError"], true);
    assert!(
        decision["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown tool")
    );

    let closed = handle_rpc_with(
        &reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": { "name": "list_work", "arguments": { "org_id": ORG } }
        }),
        BotPolicy::closed(),
    );
    assert_eq!(closed["result"]["isError"], true);
    let listed = handle_rpc_with(
        &reader,
        &json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/list" }),
        BotPolicy::closed(),
    );
    assert_eq!(listed["result"]["tools"], json!([]));
    *policy_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = BotPolicy::closed();
}
