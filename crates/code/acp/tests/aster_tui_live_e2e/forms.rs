#![cfg(test)]

use super::driver::LiveTui;
use serde_json::Value;

const FORM: &str = r#"{"root":"root","elements":{"root":{"type":"Card","props":{"title":"Aster form"},"children":["name","amount","ready","submit"]},"name":{"type":"Input","props":{"label":"Name","type":"text","required":true,"value":{"$bindState":"/form/name"}}},"amount":{"type":"Input","props":{"label":"Amount","type":"number","min":1,"max":5,"value":{"$bindState":"/form/amount"}}},"ready":{"type":"Checkbox","props":{"label":"Ready","required":true,"checked":{"$bindState":"/form/ready"}}},"submit":{"type":"Button","props":{"action":"submit","label":"Continue"}}},"state":{"form":{"name":"","amount":3,"ready":false}}}"#;

fn keys(tui: &mut LiveTui, sequence: &[&[u8]]) {
    for key in sequence {
        tui.bytes(key);
        tui.pump(100);
    }
}

pub(super) fn run() {
    let mut tui = LiveTui::new("form-submit");
    std::fs::write(tui.workspace.join("form.json"), FORM).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let before = tui.facts().len();
    tui.command("Read form.json. Call ask_user_form exactly once with prompt 'Fill in details' and the exact JSON Render document from that file. After the user answers, report the returned values without calling other tools.");
    tui.wait_for("Aster form", 120);
    keys(&mut tui, &[b"\t", b"\t", b"\t", b"\r"]);
    tui.wait_for("required", 10);
    tui.record(
        "form-required",
        "native required-field validation blocks empty submission",
    );
    keys(
        &mut tui,
        &[
            b"\x1b[A", b"\x1b[A", b"\x1b[A", b"\r", b"Ada", b"\r", b"\t", b"\r", b"\x7f", b"\x7f",
            b"\x7f", b"6", b"\r", b"\t", b"\x1b[C", b"\t", b"\r",
        ],
    );
    tui.wait_for("max 5", 10);
    tui.record("form-range", "native numeric validation blocks amount 6");
    keys(
        &mut tui,
        &[
            b"\x1b[A", b"\x1b[A", b"\r", b"\x7f", b"4", b"\r", b"\t", b"\t", b"\r",
        ],
    );
    tui.wait_fact("turn.complete", before);
    let values: Vec<Value> = tui
        .facts()
        .iter()
        .skip(before)
        .filter(|f| f["type"] == "tool.result")
        .filter_map(|f| {
            serde_json::from_str::<Value>(f["payload"]["receipt"]["output"].as_str()?).ok()
        })
        .filter_map(|wire| serde_json::from_str::<Value>(wire["text"].as_str()?).ok())
        .filter_map(|result| result.get("values").cloned())
        .collect();
    assert!(
        values
            .iter()
            .any(|v| v.as_object().is_some_and(|o| o.len() == 3)
                && v["name"] == "Ada"
                && v["amount"].as_f64() == Some(4.0)
                && v["ready"] == true),
        "actual tool receipt did not contain submitted typed values: {values:?}"
    );
    tui.record(
        "form-submit",
        "actual ACP/Core tool receipt contains Ada, 4 and true",
    );
    tui.quit();
    tui.finish();

    let mut tui = LiveTui::new("form-cancel");
    std::fs::write(tui.workspace.join("form.json"), FORM).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let before = tui.facts().len();
    tui.command("Read form.json. Call ask_user_form exactly once with its exact JSON Render document. If the user cancels, do not retry; report the cancellation without calling other tools.");
    tui.wait_for("Aster form", 120);
    tui.bytes(b"\x03");
    tui.wait_fact("turn.complete", before);
    assert!(tui
        .facts()
        .iter()
        .skip(before)
        .any(|f| f["type"] == "tool.result"
            && f["payload"]["receipt"]["output"]
                .as_str()
                .is_some_and(|text| text.contains("cancel"))));
    tui.record(
        "form-cancel",
        "actual tool result carries the cancelled outcome",
    );
    tui.quit();
    tui.finish();
}
