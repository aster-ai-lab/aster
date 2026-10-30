#![cfg(test)]

use super::{driver::LiveTui, questions_and_forms::named_tool_output};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires the paired native Aster/ACP build and original kimi/k3.1-pro ACL"]
fn kimi_catalog_forms_render_all_components_and_preserve_typed_edits() {
    let form = catalog_form();
    let catalog = a3s_form_core::json_render_component_catalog();
    let expected = catalog
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let represented = form["elements"]
        .as_object()
        .unwrap()
        .values()
        .map(|node| node["type"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        represented, expected,
        "cover the actual shared catalog, including containers and text"
    );
    assert_eq!(represented.len(), 13);
    let mut tui = LiveTui::new("form-all-components");
    let source = serde_json::to_vec(&form).unwrap();
    let path = tui.workspace.join("catalog-form.json");
    std::fs::write(&path, &source).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let prompt = "Read catalog-form.json once. Call ask_user_form exactly once with the exact JSON Render document from the file and prompt 'Catalog acceptance'. After the user submits, report the returned values without calling other tools or modifying files.";
    let before = tui.facts().len();
    tui.command(prompt);
    let screen = tui.wait_for("MCP “Aster” requests your input", 120);
    for label in [
        "Catalog 13",
        "All controls",
        "Latency: 42ms",
        "West: 2",
        "East: 5",
        "Name",
        "Count",
        "Email",
        "Phone",
        "Notes",
        "Choice",
        "Level",
        "Enabled",
    ] {
        assert!(
            screen.contains(label),
            "missing native catalog content {label}: {screen}"
        );
    }
    tui.record("catalog-native-render", "the real form accepts all 13 shared catalog node types and renders all eight bound fields and the source text/metric/chart values");
    move_fields(&mut tui, 8);
    press(&mut tui, b"\r");
    wait_error(&mut tui, "required");
    move_fields(&mut tui, -8);
    edit(&mut tui, "用户甲");
    move_fields(&mut tui, 1);
    for (input, error) in [("oops", "invalid number"), ("6", "max 5"), ("0", "min 1")] {
        edit(&mut tui, input);
        move_fields(&mut tui, 7);
        press(&mut tui, b"\r");
        wait_error(&mut tui, error);
        assert!(tui.screen().contains(input));
        tui.record(error, "the actual native field error blocks submission and keeps the invalid numeric draft editable");
        move_fields(&mut tui, -7);
    }
    edit(&mut tui, "4");
    for value in ["host@example.invalid", "020-42", "中文备注"] {
        move_fields(&mut tui, 1);
        edit(&mut tui, value);
    }
    move_fields(&mut tui, 1);
    for _ in 0..3 {
        press(&mut tui, b"\x1b[C");
    }
    press(&mut tui, b"z");
    move_fields(&mut tui, 1);
    edit(&mut tui, "6");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\x1b[C");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\r");
    wait_error(&mut tui, "required");
    move_fields(&mut tui, -1);
    press(&mut tui, b"\x1b[C");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\r");
    let facts = tui.settle_turn(before, prompt, None);
    let result = named_tool_output(&facts, "ask_user_form");
    assert_eq!(result["is_error"], false);
    let payload: Value = serde_json::from_str(result["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["status"], "answered");
    assert_eq!(
        payload["values"],
        json!({"name":"用户甲","count":4.0,"email":"host@example.invalid",
        "phone":"020-42","notes":"中文备注","choice":"beta","level":6.0,"enabled":true})
    );
    assert_eq!(std::fs::read(&path).unwrap(), source);
    tui.record("catalog-typed-values", "the actual tool receipt has exactly eight named values, including Unicode strings, numeric inputs/slider, constrained enum and required boolean; source bytes remain unchanged");
    tui.checkpoint_session("form-all-components");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired native Aster/ACP build and original kimi/k3.1-pro ACL"]
fn kimi_long_form_small_window_reaches_fields_submit_and_cancel() {
    let mut elements = serde_json::Map::new();
    let mut state = serde_json::Map::new();
    let mut children = Vec::new();
    for n in 1..=24 {
        let id = format!("field{n:02}");
        elements.insert(id.clone(), json!({"type":"Input","props":{"label":format!("Required {n:02}"),"required":true,"value":{"$bindState":format!("/form/{id}")}}}));
        state.insert(
            id.clone(),
            json!(if n == 24 {
                String::new()
            } else {
                format!("seed{n:02}")
            }),
        );
        children.push(id);
    }
    children.push("submit".into());
    elements.insert(
        "root".into(),
        json!({"type":"Card","props":{"title":"Long form 24"},"children":children}),
    );
    elements.insert(
        "submit".into(),
        json!({"type":"Button","props":{"action":"submit","label":"Continue"}}),
    );
    let form = json!({"root":"root","elements":elements,"state":{"form":state}});
    let mut tui = LiveTui::new("form-long-small-window");
    let path = tui.workspace.join("long-form.json");
    let source = serde_json::to_vec(&form).unwrap();
    std::fs::write(&path, &source).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let prompt = "Read long-form.json once. Call ask_user_form exactly once with its exact JSON Render document and prompt 'Long form acceptance'. After the user submits, report the returned values without calling other tools or modifying files.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("MCP “Aster” requests your input", 120);
    tui.resize(14, 64);
    for n in 1..=24 {
        let label = format!("Required {n:02}");
        let screen = tui.wait_for(&label, 8);
        assert!(screen.contains("Accept") && screen.contains("Decline"));
        tui.record(
            &format!("small-window-field-{n:02}"),
            &format!(
                "{label} is reachable while native actions remain visible in a 14x64 terminal"
            ),
        );
        move_fields(&mut tui, 1);
    }
    press(&mut tui, b"\r");
    wait_error(&mut tui, "required");
    move_fields(&mut tui, -2);
    tui.wait_for("Required 23", 8);
    move_fields(&mut tui, 1);
    tui.wait_for("Required 24", 8);
    edit(&mut tui, "LAST-ANSWER-中文");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\r");
    let facts = tui.settle_turn(before, prompt, None);
    let result = named_tool_output(&facts, "ask_user_form");
    assert_eq!(result["is_error"], false);
    let payload: Value = serde_json::from_str(result["text"].as_str().unwrap()).unwrap();
    let mut expected = form["state"]["form"].clone();
    expected["field24"] = json!("LAST-ANSWER-中文");
    assert_eq!(payload["values"], expected);
    tui.record("small-window-submit", "all 24 named required values reach the actual tool receipt; reverse Tab reaches the preceding field before editing and submitting the last field");
    tui.resize(40, 120);
    let prompt = "Read long-form.json once. Call ask_user_form exactly once with its exact JSON Render document and prompt 'Long form cancellation'. If the user cancels, report cancellation without retrying, modifying files or calling other tools.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("MCP “Aster” requests your input", 120);
    tui.resize(14, 64);
    assert!(tui.screen().contains("Ctrl+c:cancel"));
    press(&mut tui, b"\x03");
    let facts = tui.settle_turn(before, prompt, None);
    let result = named_tool_output(&facts, "ask_user_form");
    assert_eq!(result["is_error"], true);
    assert_eq!(std::fs::read(&path).unwrap(), source);
    tui.record("small-window-cancel", "the real form cancel is reachable at 14x64 and returns an error without submitting default values or changing source bytes");
    tui.resize(40, 120);
    let facts = tui.turn(
        "Do not call tools. Reply exactly LONG-FORM-RECOVERED.",
        None,
    );
    assert!(tui.assistant_text(&facts).contains("LONG-FORM-RECOVERED"));
    tui.record(
        "small-window-cancel-recovery",
        "the original session completes a fresh real model turn after cancelling the long form",
    );
    tui.checkpoint_session("form-long-small-window");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired native Aster/ACP build and original kimi/k3.1-pro ACL"]
fn kimi_catalog_native_viewport_and_typed_values_match_source() {
    let form = catalog_form();
    let catalog = a3s_form_core::json_render_component_catalog();
    let represented = form["elements"]
        .as_object()
        .unwrap()
        .values()
        .map(|node| node["type"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        represented,
        catalog
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect()
    );
    assert_eq!(represented.len(), 13);
    let mut tui = LiveTui::new("form-catalog-native-profile");
    let source = serde_json::to_vec(&form).unwrap();
    let path = tui.workspace.join("catalog-form.json");
    std::fs::write(&path, &source).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let prompt = "Read catalog-form.json once. Call ask_user_form exactly once with the exact JSON Render document from the file and prompt 'Catalog acceptance'. After the user submits, report the returned values without calling other tools or modifying files.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("MCP “Aster” requests your input", 120);
    let screen = tui.wait_for("East: 5.0", 8);
    for content in [
        "Catalog 13",
        "All controls",
        "Latency: 42.0ms",
        "West: 2.0",
        "East: 5.0",
    ] {
        assert!(
            screen.contains(content),
            "missing actual native text {content}"
        );
    }
    tui.record("catalog-text-values", "all 13 shared component types are represented; native metric/chart text retains the numeric values and units using the shared parser's floating-point display");
    for (label, value) in [
        ("Name", "用户甲"),
        ("Count", "4"),
        ("Email", "host@example.invalid"),
        ("Phone", "020-42"),
        ("Notes", "中文备注"),
    ] {
        tui.wait_for(label, 8);
        edit(&mut tui, value);
        tui.record(
            &format!("catalog-field-{label}"),
            "the actual focused native field accepts its host-only edit",
        );
        move_fields(&mut tui, 1);
    }
    tui.wait_for("Choice", 8);
    press(&mut tui, b"\x1b[C");
    move_fields(&mut tui, 1);
    tui.wait_for("Level", 8);
    edit(&mut tui, "6");
    move_fields(&mut tui, 1);
    tui.wait_for("Enabled", 8);
    press(&mut tui, b"\x1b[C");
    press(&mut tui, b"\x1b[C");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\r");
    let facts = tui.settle_turn(before, prompt, None);
    let result = named_tool_output(&facts, "ask_user_form");
    assert_eq!(result["is_error"], false);
    let payload: Value = serde_json::from_str(result["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        payload["values"],
        json!({"name":"用户甲","count":4.0,"email":"host@example.invalid","phone":"020-42","notes":"中文备注","choice":"beta","level":6.0,"enabled":true})
    );
    assert_eq!(std::fs::read(&path).unwrap(), source);
    tui.record("catalog-eight-typed-values", "the actual tool receipt contains exactly the eight edited fields with correct Unicode strings, enum, numbers and boolean; the source document is unchanged");
    tui.checkpoint_session("form-catalog-native-profile");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired native Aster/ACP build and original kimi/k3.1-pro ACL"]
fn kimi_form_type_bounds_choice_and_required_errors_keep_edits() {
    let form = json!({"root":"root","elements":{
        "root":{"type":"Card","props":{"title":"Constraint form"},"children":["count","choice","enabled","submit"]},
        "count":{"type":"Input","props":{"label":"Count","type":"number","required":true,"min":1,"max":5,"value":{"$bindState":"/form/count"}}},
        "choice":{"type":"Select","props":{"label":"Choice","required":true,"options":["alpha","beta"],"value":{"$bindState":"/form/choice"}}},
        "enabled":{"type":"Checkbox","props":{"label":"Enabled","required":true,"checked":{"$bindState":"/form/enabled"}}},
        "submit":{"type":"Button","props":{"action":"submit","label":"Continue"}}
    },"state":{"form":{"count":2,"choice":"alpha","enabled":true}}});
    let mut tui = LiveTui::new("form-constraint-edits");
    let source = serde_json::to_vec(&form).unwrap();
    let path = tui.workspace.join("constraint-form.json");
    std::fs::write(&path, &source).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let prompt = "Read constraint-form.json once. Call ask_user_form exactly once with its exact JSON Render document and prompt 'Constraint acceptance'. After the user submits, report the returned values without calling other tools or modifying files.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("MCP “Aster” requests your input", 120);
    for (input, message) in [
        ("", "required"),
        ("oops", "invalid number"),
        ("0", "min 1"),
        ("6", "max 5"),
    ] {
        edit(&mut tui, input);
        move_fields(&mut tui, 3);
        press(&mut tui, b"\r");
        wait_error(&mut tui, message);
        tui.record(&format!("constraint-{message}"), "the actual field error rejects submission without closing the form or replacing the editable draft");
        move_fields(&mut tui, -3);
    }
    edit(&mut tui, "4");
    move_fields(&mut tui, 1);
    for _ in 0..3 {
        press(&mut tui, b"\x1b[C");
    }
    press(&mut tui, b"z");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\x1b[C");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\r");
    wait_error(&mut tui, "required");
    move_fields(&mut tui, -1);
    press(&mut tui, b"\x1b[C");
    move_fields(&mut tui, 1);
    press(&mut tui, b"\r");
    let facts = tui.settle_turn(before, prompt, None);
    let result = named_tool_output(&facts, "ask_user_form");
    assert_eq!(result["is_error"], false);
    let payload: Value = serde_json::from_str(result["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        payload["values"],
        json!({"count":4.0,"choice":"beta","enabled":true})
    );
    assert_eq!(std::fs::read(&path).unwrap(), source);
    tui.record("constraints-typed-submission", "after all rejected drafts, the exact numeric value, constrained enum and required boolean reach the actual tool receipt; forward and reverse Tab keep edits on their fields");
    tui.checkpoint_session("form-constraint-edits");
    tui.quit();
    tui.finish();
}

#[test]
#[ignore = "requires the paired native Aster/ACP build and original kimi/k3.1-pro ACL"]
fn kimi_form_cancel_returns_no_values_and_next_turn_succeeds() {
    let form = json!({"root":"root","elements":{
        "root":{"type":"Card","props":{"title":"Cancel form"},"children":["name","submit"]},
        "name":{"type":"Input","props":{"label":"Name","required":true,"value":{"$bindState":"/form/name"}}},
        "submit":{"type":"Button","props":{"action":"submit","label":"Continue"}}
    },"state":{"form":{"name":"DEFAULT-NOT-SUBMITTED"}}});
    let mut tui = LiveTui::new("form-cancel-continue");
    let source = serde_json::to_vec(&form).unwrap();
    let path = tui.workspace.join("cancel-form.json");
    std::fs::write(&path, &source).unwrap();
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    let prompt = "Read cancel-form.json once. Call ask_user_form exactly once with the exact JSON Render document and prompt 'Cancel probe'. If the user cancels, report cancellation without retrying, modifying files or calling any other tool.";
    let before = tui.facts().len();
    tui.command(prompt);
    tui.wait_for("MCP “Aster” requests your input", 120);
    press(&mut tui, b"\x03");
    let facts = tui.settle_turn(before, prompt, None);
    let result = named_tool_output(&facts, "ask_user_form");
    assert_eq!(result["is_error"], true);
    assert!(!result["text"]
        .as_str()
        .unwrap()
        .contains("DEFAULT-NOT-SUBMITTED"));
    assert_eq!(std::fs::read(&path).unwrap(), source);
    tui.record("form-cancel-no-default-values", "the actual cancelled tool returns an error, never returns the prefilled default as accepted values, and preserves the form source bytes");
    let facts = tui.turn(
        "Do not call tools. Reply exactly FORM-CANCEL-RECOVERED.",
        None,
    );
    assert!(tui.assistant_text(&facts).contains("FORM-CANCEL-RECOVERED"));
    assert!(!facts.iter().any(|fact| fact["type"] == "tool.prepared"));
    tui.record("form-cancel-next-turn", "the same session completes a fresh real model request after form cancellation without retrying the form");
    tui.checkpoint_session("form-cancel-continue");
    tui.quit();
    tui.finish();
}

fn press(tui: &mut LiveTui, bytes: &[u8]) {
    tui.bytes(bytes);
    tui.pump(100);
}

fn move_fields(tui: &mut LiveTui, count: i32) {
    for _ in 0..count.unsigned_abs() {
        press(tui, if count < 0 { b"\x1b[Z" } else { b"\t" });
    }
}

fn edit(tui: &mut LiveTui, value: &str) {
    press(tui, b"\r");
    press(tui, &[127; 128]);
    press(tui, value.as_bytes());
    press(tui, b"\r");
}

fn wait_error(tui: &mut LiveTui, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        tui.pump(80);
        let screen = tui.physical_screen();
        if screen
            .lines()
            .any(|row| row.trim().trim_matches('│').trim() == expected)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "missing actual field error {expected:?}\n{screen}"
        );
    }
}

fn catalog_form() -> Value {
    json!({"root":"root","elements":{
        "root":{"type":"Form","props":{"submitAction":"submit"},"children":["card"]},
        "card":{"type":"Card","props":{"title":"Catalog 13"},"children":["stack"]},
        "stack":{"type":"Stack","props":{"direction":"vertical","gap":1},"children":["intro","grid","metric","chart","submit"]},
        "grid":{"type":"Grid","props":{"columns":2},"children":["name","count","email","phone","notes","choice","level","enabled"]},
        "intro":{"type":"Text","props":{"content":"All controls"}},
        "name":{"type":"Input","props":{"label":"Name","type":"text","required":true,"value":{"$bindState":"/form/name"}}},
        "count":{"type":"Input","props":{"label":"Count","type":"number","min":1,"max":5,"step":1,"value":{"$bindState":"/form/count"}}},
        "email":{"type":"Input","props":{"label":"Email","type":"email","value":{"$bindState":"/form/email"}}},
        "phone":{"type":"Input","props":{"label":"Phone","type":"tel","value":{"$bindState":"/form/phone"}}},
        "notes":{"type":"Textarea","props":{"label":"Notes","value":{"$bindState":"/form/notes"}}},
        "choice":{"type":"Select","props":{"label":"Choice","required":true,"options":["alpha","beta"],"value":{"$bindState":"/form/choice"}}},
        "level":{"type":"Slider","props":{"label":"Level","min":0,"max":10,"step":2,"value":{"$bindState":"/form/level"}}},
        "enabled":{"type":"Checkbox","props":{"label":"Enabled","required":true,"checked":{"$bindState":"/form/enabled"}}},
        "metric":{"type":"Metric","props":{"label":"Latency","value":42,"unit":"ms"}},
        "chart":{"type":"BarChart","props":{"labels":["West","East"],"values":[2,5]}},
        "submit":{"type":"Button","props":{"action":"submit","label":"Continue"}}
    },"state":{"form":{"name":"","count":2,"email":"seed@example.invalid","phone":"seed-phone","notes":"seed-note","choice":"alpha","level":4,"enabled":true}}})
}
