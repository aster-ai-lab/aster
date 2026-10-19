#![cfg(test)]

use super::*;

struct Fake {
    mailbox: Value,
    page: Value,
    message: Value,
}

impl MailReader for Fake {
    fn mailbox(&self, org_id: &str) -> std::result::Result<Value, String> {
        assert_eq!(org_id, "0197e5a2-89f1-7d33-a11c-5f53d9b3af12");
        Ok(self.mailbox.clone())
    }
    fn list(&self, org_id: &str, folder: &str, query: &str) -> std::result::Result<Value, String> {
        assert_eq!(org_id, "0197e5a2-89f1-7d33-a11c-5f53d9b3af12");
        assert_eq!(folder, "inbox");
        assert_eq!(query, "crates");
        Ok(self.page.clone())
    }
    fn read(&self, org_id: &str, id: &str) -> std::result::Result<Value, String> {
        assert_eq!(org_id, "0197e5a2-89f1-7d33-a11c-5f53d9b3af12");
        assert_eq!(id, "0197e5a2-89f1-7d33-a11c-5f53d9b3af10");
        Ok(self.message.clone())
    }
}

fn fake() -> Fake {
    Fake {
        mailbox: json!({ "address": "roy-abcdef12@mail.towork", "unread": 1 }),
        page: json!({ "items": [{ "id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af10", "subject": "crates" }], "unread": 1 }),
        message: json!({ "id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af10", "subject": "crates", "body": "published" }),
    }
}

#[test]
fn refuses_missing_or_forged_team_before_accessing_the_mail_reader() {
    let reader = fake();
    for org in [Value::Null, json!("personal"), json!("bad?org_id=other")] {
        for name in ["list_mail", "read_mail"] {
            let response = handle_rpc_with(
                &reader,
                &json!({
                    "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": {"name": name, "arguments": {"org_id": org, "id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af10", "q": "crates"}}
                }),
                open_policy(),
            );
            assert_eq!(response["result"]["isError"], true);
        }
    }
    let org = "0197e5a2-89f1-7d33-a11c-5f53d9b3af12";
    assert_eq!(
        team_path("/v1/api/org/mail/mailbox", org).unwrap(),
        format!("/v1/api/org/mail/mailbox?org_id={org}")
    );
    assert_eq!(
        team_path(&list_path("inbox", "crates"), org).unwrap(),
        format!("/v1/api/org/mail/messages?folder=inbox&q=crates&limit=20&org_id={org}")
    );
}

fn open_policy() -> BotPolicy {
    BotPolicy {
        allow: true,
        list: true,
        read: true,
        folders: 0b111_1111,
    }
}

#[test]
fn lists_and_reads_mail_without_calling_http() {
    let reader = fake();
    let listed = handle_rpc_with(
        &reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "list_mail", "arguments": {"org_id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af12",  "folder": "inbox", "q": "crates" } }
        }),
        open_policy(),
    );
    let text = listed["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(listed["result"]["isError"], false);
    assert!(text.contains("roy-abcdef12@mail.towork"));
    assert!(text.contains("crates"));

    let opened = handle_rpc_with(
        &reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "read_mail", "arguments": {"org_id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af12",  "id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af10" } }
        }),
        open_policy(),
    );
    assert_eq!(opened["result"]["isError"], false);
    assert!(
        opened["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("published")
    );
}

#[test]
fn rejects_an_unknown_tool_and_a_missing_id() {
    let reader = fake();
    let unknown = handle_rpc_with(
        &reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "delete_mail", "arguments": {"org_id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af12", } }
        }),
        open_policy(),
    );
    assert_eq!(unknown["result"]["isError"], true);
    let missing = handle_rpc_with(
        &reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": { "name": "read_mail", "arguments": {"org_id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af12", } }
        }),
        open_policy(),
    );
    *policy_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = BotPolicy::closed();
    let closed = handle_rpc(
        &reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": { "name": "list_mail", "arguments": {"org_id": "0197e5a2-89f1-7d33-a11c-5f53d9b3af12",  "folder": "inbox" } }
        }),
    );
    assert_eq!(closed["result"]["isError"], true);
    let stored = store_policy(&json!({
        "allow": true,
        "list": true,
        "read": true,
        "folders": ["inbox"]
    }));
    assert!(stored.allow);
    assert!(stored.list_ok("inbox"));
    assert!(!stored.list_ok("drafts"));
    let listed = handle_rpc(
        &reader,
        &json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/list"
        }),
    );
    let names = listed["result"]["tools"].to_string();
    assert!(names.contains("list_mail"));
    assert!(names.contains("read_mail"));
    *policy_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = BotPolicy::closed();
    assert_eq!(missing["result"]["isError"], true);
    assert!(
        missing["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("id is required")
    );
}

#[test]
fn list_path_escapes_the_query() {
    assert_eq!(
        list_path("inbox", "a b"),
        "/v1/api/org/mail/messages?folder=inbox&q=a%20b&limit=20"
    );
}

#[test]
fn unwraps_the_member_envelope() {
    let payload = br#"{"success":true,"code":0,"message":"OK","data":{"address":"m@mail.towork"}}"#;
    let data = unwrap_data(200, payload).expect("data");
    assert_eq!(data["address"], "m@mail.towork");
    let denied = unwrap_data(
        200,
        br#"{"success":false,"code":40400,"message":"missing","data":null}"#,
    );
    assert_eq!(denied.expect_err("denied"), "missing");
}
