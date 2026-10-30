#![cfg(test)]

use super::driver::LiveTui;
use super::tool_wave::{finish, one, request, start};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_program_code_search_registered_template_returns_real_match() {
    let case = "coverage-program-code-search";
    let mut tui = start(case, &[]);
    let token = format!("CATALOG-{}", uuid::Uuid::new_v4());
    std::fs::create_dir(tui.workspace.join("catalog")).unwrap();
    std::fs::write(
        tui.workspace.join("catalog/module.rs"),
        format!("// CATALOG-ANCHOR\n// {token}\n"),
    )
    .unwrap();
    let catalog = a3s_code_core::program::ProgramCatalog::with_builtin_programs();
    assert_eq!(
        catalog
            .list()
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["program_code_search", "program_repo_map"]
    );
    let facts = request(
        &mut tui,
        "template-code",
        "program",
        json!({
            "type":"template","name":"program_code_search",
            "inputs":{"query":"CATALOG-ANCHOR","path":"catalog","glob":"*.rs"},
            "allowed_tools":["search"],"limits":{"timeoutMs":10000,"maxToolCalls":2,"maxOutputBytes":16000}
        }),
        "Report the actual returned matches. Do not retry, modify files or call other tools.",
    );
    let output = one(&facts, "program");
    assert_eq!(output["exit_code"], 0);
    assert_eq!(output["metadata"]["program"]["name"], "program_code_search");
    assert_eq!(output["metadata"]["program"]["type"], "template");
    assert_eq!(program_calls(&output).len(), 1);
    let steps = output["metadata"]["script_result"]["steps"]
        .as_array()
        .unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["label"], "search_code");
    assert_eq!(steps[0]["exitCode"], 0);
    assert!(steps[0]["output"].as_str().unwrap().contains(&token));
    tui.record("registered-code-search-template", "the actual default catalog has exactly two templates; code search dispatches one real search and returns unseen fixture context with its label and complete receipt");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_program_repo_map_registered_template_runs_all_seven_steps() {
    let case = "coverage-program-repo-map";
    let mut tui = start(case, &[]);
    std::fs::create_dir(tui.workspace.join("catalog")).unwrap();
    let names = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "README.md",
        "AGENTS.md",
    ];
    for name in names {
        std::fs::write(tui.workspace.join("catalog").join(name), "fixture\n").unwrap();
    }
    let facts = request(
        &mut tui,
        "template-map",
        "program",
        json!({
            "type":"template","name":"program_repo_map","inputs":{"path":"catalog"},
            "allowed_tools":["ls","search"],"limits":{"timeoutMs":20000,"maxToolCalls":8,"maxOutputBytes":48000}
        }),
        "Report the actual map. Do not retry, modify files or call other tools.",
    );
    let output = one(&facts, "program");
    assert_eq!(output["exit_code"], 0);
    assert_eq!(output["metadata"]["program"]["name"], "program_repo_map");
    assert_eq!(program_calls(&output).len(), 7);
    let steps = output["metadata"]["script_result"]["steps"]
        .as_array()
        .unwrap();
    assert_eq!(steps.len(), 7);
    assert_eq!(steps[0]["label"], "list_root");
    for (step, name) in steps[1..].iter().zip(names) {
        assert_eq!(step["label"], format!("find_{name}"));
        assert_eq!(step["exitCode"], 0);
        assert!(step["output"].as_str().unwrap().contains(name));
    }
    tui.record("registered-repo-map-template", "the second default template runs root ls and all six real project-file searches in catalog order with complete nested receipts");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_program_template_rejects_numeric_search_query_with_owned_error() {
    let case = "coverage-program-typed-error";
    let mut tui = start(case, &[]);
    let facts = request(&mut tui, "template-typed", "program", json!({
        "type":"template","name":"program_code_search","inputs":{"query":1},
        "allowed_tools":["search"],"limits":{"timeoutMs":10000,"maxToolCalls":2,"maxOutputBytes":8192}
    }), "Report the exact parameter error without retrying, correcting arguments or calling other tools.");
    let output = one(&facts, "program");
    assert_eq!(output["is_error"], true);
    let calls = program_calls(&output);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["success"], false);
    assert!(output["text"].as_str().unwrap().contains("query"));
    tui.record("template-typed-parameter-error", "a numeric template query is preserved to the real nested search boundary and returns one failed complete receipt without retries");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_program_max_tool_calls_stops_before_second_dispatch() {
    let case = "coverage-program-call-limit";
    let mut tui = start(case, &[]);
    let token = uuid::Uuid::new_v4().to_string();
    std::fs::write(tui.workspace.join("proof.txt"), &token).unwrap();
    let facts = request(
        &mut tui,
        "max-calls",
        "program",
        json!({
            "type":"script","source":"async function run(ctx, inputs) { await ctx.tool('read', {file_path:'proof.txt'}); return await ctx.tool('read', {file_path:'proof.txt'}); }",
            "allowed_tools":["read"],"limits":{"timeoutMs":10000,"maxToolCalls":1,"maxOutputBytes":8192}
        }),
        "Report the actual limit error without retrying or calling other tools.",
    );
    let output = one(&facts, "program");
    assert_eq!(output["is_error"], true);
    assert!(output["text"]
        .as_str()
        .unwrap()
        .contains("exceeded maxToolCalls=1"));
    assert_eq!(program_calls(&output).len(), 1);
    assert_eq!(
        std::fs::read(tui.workspace.join("proof.txt")).unwrap(),
        token.as_bytes()
    );
    tui.record("program-max-calls", "a one-call script budget permits the first real read, rejects the second before dispatch and retains the complete first receipt");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_program_timeout_terminates_vm_and_next_turn_still_runs() {
    let case = "coverage-program-timeout";
    let mut tui = start(case, &[]);
    let facts = request(
        &mut tui,
        "timeout",
        "program",
        json!({
            "type":"script","source":"async function run(ctx, inputs) { while (true) {} }",
            "allowed_tools":["read"],"limits":{"timeoutMs":100,"maxToolCalls":1,"maxOutputBytes":8192}
        }),
        "Report the actual timeout without retrying or calling other tools.",
    );
    let output = one(&facts, "program");
    assert_eq!(output["is_error"], true);
    assert!(output["text"]
        .as_str()
        .unwrap()
        .contains("timed out after 100 ms"));
    assert!(program_calls(&output).is_empty());
    let facts = tui.turn("Reply only TIMEOUT-RECOVERY. Do not call tools.", None);
    assert_eq!(tui.assistant_text(&facts).trim(), "TIMEOUT-RECOVERY");
    tui.record("program-timeout-and-recovery", "the real embedded VM times out with zero nested dispatches and the same session completes a fresh model turn");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_program_output_limit_preserves_utf8_and_marks_truncation() {
    let case = "coverage-program-output-limit";
    let mut tui = start(case, &[]);
    let facts = request(
        &mut tui,
        "output-limit",
        "program",
        json!({
            "type":"script","source":"async function run(ctx, inputs) { return {summary:'UTF8-LIMIT', text:'界'.repeat(600)}; }",
            "allowed_tools":["read"],"limits":{"timeoutMs":10000,"maxToolCalls":1,"maxOutputBytes":256}
        }),
        "Report the actual truncation marker without retrying or calling other tools.",
    );
    let output = one(&facts, "program");
    assert_eq!(output["exit_code"], 0);
    let text = output["text"].as_str().unwrap();
    assert!(text.len() <= 256);
    assert!(text.contains('界') && !text.contains('\u{fffd}'));
    assert!(text.ends_with("\n[output truncated]"));
    assert_eq!(output["metadata"]["script_result"]["truncated"], true);
    assert!(program_calls(&output).is_empty());
    tui.record("program-utf8-output-budget", "the real script's multibyte output fits the 256-byte ceiling including its truncation notice, has no replacement characters and explicitly marks truncated structured output");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_batch_staged_reference_uses_real_first_result() {
    let case = "coverage-batch-staged";
    let mut tui = start(case, &[]);
    batch_fixture(&tui);
    let facts = request(&mut tui, "staged", "batch", json!({"max_concurrency":2,"invocations":[
        {"id":"first","step":1,"tool":"ls","args":{"path":"entries","limit":1}},
        {"id":"second","step":2,"tool":"ls","args":{"path":"entries","limit":{"$ref":"first.metadata.page.returned_items"}}}
    ]}), "Report the actual staged results without retrying, modifying files or calling other tools.");
    let output = one(&facts, "batch");
    let meta = &output["metadata"];
    assert_eq!(meta["status"], "complete");
    assert_eq!(meta["execution_mode"], "staged");
    assert_eq!(meta["steps"], 2);
    assert_eq!(meta["success_count"], 2);
    assert_eq!(meta["failure_count"], 0);
    let rows = meta["results"].as_array().unwrap();
    assert_eq!(rows[0]["step"], 1);
    assert_eq!(rows[1]["step"], 2);
    for row in rows {
        assert_eq!(row["metadata"]["page"]["returned_items"], 1);
        assert_eq!(row["metadata"]["page"]["applied_limit"], 1);
    }
    assert!(meta["binding_errors"].as_array().unwrap().is_empty());
    tui.record("batch-real-staged-binding", "stage two resolves a real stage-one pagination count as its typed limit; both native ls calls retain ordered successful metadata");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_batch_missing_reference_fails_only_dependent_item() {
    let case = "coverage-batch-missing-reference";
    let mut tui = start(case, &[]);
    batch_fixture(&tui);
    let facts = request(&mut tui, "missing-ref", "batch", json!({"max_concurrency":2,"invocations":[
        {"id":"first","step":1,"tool":"ls","args":{"path":"entries","limit":1}},
        {"id":"second","step":2,"tool":"read","args":{"file_path":{"$ref":"first.missing_path"}}}
    ]}), "Report the actual reference error without retrying, correcting arguments or calling other tools.");
    let output = one(&facts, "batch");
    let meta = &output["metadata"];
    assert_eq!(meta["status"], "partial_failure");
    assert_eq!(meta["success_count"], 1);
    assert_eq!(meta["failure_count"], 1);
    assert_eq!(meta["failed_indices"], json!([1]));
    assert_eq!(meta["results"][0]["success"], true);
    assert_eq!(meta["results"][1]["success"], false);
    assert!(output["text"].as_str().unwrap().contains("missing_path"));
    tui.record("batch-dependent-reference-failure", "the actual first stage succeeds; an absent bound path fails only the dependent item and is not silently guessed or repaired");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_settings_hidden_backend_rows_are_not_actionable() {
    let case = "coverage-unavailable-settings";
    let mut tui = start(case, &[]);
    let baseline = tui.facts();
    let preferences = tui.preferences();
    tui.command("/settings");
    key(&mut tui, b"/");
    for setting in [
        "coding_data_sharing",
        "dashboard_preview",
        "combine_queued_prompts",
        "follow_up_behavior",
        "plan_mode",
        "contextual_hints.plan_mode",
        "toolset.ask_user_question.timeout_enabled",
        "fork_secondary_model",
        "prompt_suggestions",
        "auto_update",
        "remember_tool_approvals",
        "default_selected_permission",
        "default_model",
        "subagent_model_inheritance",
        "hunk_tracker_mode",
        "show_tips",
        "confirm_before_rewind",
        "contextual_hints.image_input",
        "voice_keybind_enabled",
        "voice_capture_mode",
        "voice_stt_language",
    ] {
        key(&mut tui, b"\x15");
        key(&mut tui, setting.as_bytes());
        visible(&mut tui, &format!("No matches for \"{setting}\""));
        tui.record(&format!("hidden-{setting}"), "the exact key has no rendered actionable setting under the current backend and voice-disabled configuration");
    }
    for setting in ["vim_mode", "permission_mode"] {
        key(&mut tui, b"\x15");
        key(&mut tui, setting.as_bytes());
        let screen = tui.physical_screen();
        assert!(
            !screen.contains("No matches for"),
            "supported local row disappeared: {setting}\n{screen}"
        );
    }
    key(&mut tui, b"\x1bOQ");
    assert_eq!(tui.preferences(), preferences);
    assert_eq!(tui.facts(), baseline);
    tui.record("hidden-settings-no-side-effects", "all 18 backend-inapplicable and three conditional voice keys are absent while two supported local settings remain searchable; filtering changes neither preferences nor backend facts");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_theme_enum_preview_cancel_commit_and_reset_affect_real_palette() {
    let case = "coverage-theme-enum";
    let mut tui = start(case, &[]);
    tui.command("/theme groknight");
    tui.wait_preference("theme = \"groknight\"");
    tui.pump(6500);
    let baseline = tui.facts();
    let night = tui.palette();
    let preferences = tui.preferences();
    theme_picker(&mut tui);
    key(&mut tui, b"\x1b[B");
    assert_eq!(
        tui.preferences(),
        preferences,
        "preview persisted prematurely"
    );
    key(&mut tui, b"\x1b");
    key(&mut tui, b"\x1bOQ");
    tui.pump(6500);
    assert_eq!(
        tui.palette(),
        night,
        "cancel failed to restore the rendered night palette"
    );
    assert_eq!(tui.preferences(), preferences);
    tui.record("theme-preview-cancel", "moving from Aster Night to Aster Day previews without persistence; Escape restores exact idle palette and preferences");
    theme_picker(&mut tui);
    key(&mut tui, b"\x1b[B");
    key(&mut tui, b"\r");
    key(&mut tui, b"\x1bOQ");
    tui.wait_preference("theme = \"grokday\"");
    tui.pump(6500);
    let day = tui.palette();
    assert_ne!(
        day, night,
        "committing Day did not change the rendered palette"
    );
    tui.record(
        "theme-commit",
        "Enter commits the canonical grokday value and changes actual rendered terminal colors",
    );
    theme_row(&mut tui);
    key(&mut tui, b"d");
    visible(&mut tui, "Reset 'Theme' to default");
    key(&mut tui, b"n");
    key(&mut tui, b"\x1bOQ");
    tui.pump(6500);
    assert_eq!(tui.palette(), day);
    assert!(tui.preferences().contains("theme = \"grokday\""));
    theme_row(&mut tui);
    key(&mut tui, b"d");
    visible(&mut tui, "Reset 'Theme' to default");
    key(&mut tui, b"y");
    key(&mut tui, b"\x1bOQ");
    tui.pump(6500);
    assert_eq!(tui.palette(), night);
    assert!(!tui.preferences().contains("theme = \"grokday\""));
    assert_eq!(tui.facts(), baseline);
    tui.record("theme-reset-confirm", "No preserves the committed Day value and palette; Yes restores the real default Night palette without backend turns");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_markdown_viewer_raw_toggle_keeps_canonical_export_bytes() {
    let case = "coverage-markdown-raw";
    let mut tui = start(case, &[]);
    let token = format!("RAW-{}", uuid::Uuid::new_v4());
    let markdown = format!("# Viewer heading\n\n**{token}**\n\n- alpha\n- beta");
    let prompt = format!("Reply with exactly the following Markdown without a code fence or commentary. Do not call tools.\n{markdown}");
    let facts = tui.turn(&prompt, None);
    assert_eq!(tui.assistant_text(&facts).trim(), markdown);
    let expected = tui.assistant_text(&facts);
    let baseline = tui.facts();
    tui.command(&format!("/find {token}"));
    tui.wait_for("1/2", 8);
    key(&mut tui, b"\r");
    key(&mut tui, b"n");
    tui.wait_for("2/2", 8);
    key(&mut tui, b"\x1b");
    key(&mut tui, b"\x06");
    visible(&mut tui, "raw");
    visible(&mut tui, &token);
    let rendered = tui.physical_screen();
    assert!(!rendered.contains(&format!("**{token}**")));
    key(&mut tui, b"r");
    visible(&mut tui, &format!("**{token}**"));
    tui.record("markdown-raw-visible", "the selected real assistant Markdown viewer switches from rendered emphasis to literal Markdown source");
    key(&mut tui, b"r");
    assert!(!tui.physical_screen().contains(&format!("**{token}**")));
    key(&mut tui, b"\x06");
    let export = tui.workspace.join("raw-export.txt");
    tui.literal_command(&format!("/copy {}", export.display()));
    wait_file(&mut tui, &export);
    assert_eq!(std::fs::read(export).unwrap(), expected.as_bytes());
    assert_eq!(tui.facts(), baseline);
    tui.record("raw-toggle-canonical-copy", "toggling back and closing restores navigation; assistant export retains exact canonical UTF-8 bytes and generates no backend facts");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_read_block_viewer_reaches_full_tail_and_returns_to_same_selection() {
    let case = "coverage-read-viewer";
    let mut tui = start(case, &[]);
    let token = uuid::Uuid::new_v4().simple().to_string();
    let body = (0..120)
        .map(|i| format!("VIEW-ROW-{i:03} {token}\n"))
        .collect::<String>();
    std::fs::write(tui.workspace.join("viewer.txt"), &body).unwrap();
    let facts = tui.turn("Read viewer.txt exactly once. Reply only VIEWER-DONE, without echoing its contents, calling other tools or modifying files.", None);
    assert_eq!(tui.assistant_text(&facts).trim(), "VIEWER-DONE");
    let baseline = tui.facts();
    tui.command("/find VIEW-ROW-000");
    tui.wait_for("1/1", 8);
    key(&mut tui, b"\r");
    key(&mut tui, b"\x1b");
    key(&mut tui, b"\x06");
    visible(&mut tui, "copy path");
    key(&mut tui, b"\x1b[F");
    visible(&mut tui, "VIEW-ROW-119");
    key(&mut tui, b"\x1b[H");
    visible(&mut tui, "VIEW-ROW-000");
    key(&mut tui, b"\x06");
    key(&mut tui, b"\x06");
    visible(&mut tui, "copy path");
    visible(&mut tui, "VIEW-ROW-000");
    key(&mut tui, b"\x06");
    assert_eq!(
        std::fs::read(tui.workspace.join("viewer.txt")).unwrap(),
        body.as_bytes()
    );
    assert_eq!(tui.facts(), baseline);
    tui.record("full-read-viewer-focus", "the actual read block viewer reaches the 120-line tail and home, closes and reopens the same selected block, with unchanged fixture and backend facts");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair, original kimi/k3.1-pro ACL and public wiki service"]
fn kimi_web_search_explicit_wiki_returns_structured_real_sources() {
    let case = "coverage-web-search-wiki";
    let mut tui = start(case, &[]);
    let facts = request(&mut tui, "wiki-search", "web_search", json!({
        "query":"Rust programming language","engines":["wiki"],"limit":2,"timeout":15,"format":"json"
    }), "Report the actual returned titles and source URLs. Do not retry, switch engines, invent results or call other tools.");
    let output = one(&facts, "web_search");
    assert_eq!(
        output["exit_code"], 0,
        "the explicit public wiki engine did not return successful search results"
    );
    let rows: Vec<Value> = serde_json::from_str(output["text"].as_str().unwrap()).unwrap();
    assert!(!rows.is_empty() && rows.len() <= 2);
    for row in rows {
        assert!(!row["title"].as_str().unwrap().is_empty());
        let url = row["url"].as_str().unwrap();
        assert!(url.starts_with("https://") || url.starts_with("http://"));
    }
    tui.record("real-public-wiki-search", "the explicit anonymous wiki engine returns one or two real structured search rows with titles and HTTP source URLs; failures remain failures without engine substitution");
    finish(tui, case);
}

fn program_calls(output: &Value) -> &[Value] {
    assert_eq!(output["metadata"]["program"]["tool_calls_complete"], true);
    output["metadata"]["program"]["tool_calls"]
        .as_array()
        .unwrap()
}

fn batch_fixture(tui: &LiveTui) {
    std::fs::create_dir(tui.workspace.join("entries")).unwrap();
    for name in ["first.txt", "second.txt"] {
        std::fs::write(tui.workspace.join("entries").join(name), "fixture\n").unwrap();
    }
}

fn theme_row(tui: &mut LiveTui) {
    tui.command("/settings");
    key(tui, b"/");
    key(tui, b"theme");
    key(tui, b"\r");
    visible(tui, "Theme");
}

fn theme_picker(tui: &mut LiveTui) {
    theme_row(tui);
    key(tui, b"\r");
    visible(tui, "revert");
    visible(tui, "select");
}

fn key(tui: &mut LiveTui, bytes: &[u8]) {
    tui.bytes(bytes);
    tui.pump(200);
}

fn visible(tui: &mut LiveTui, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        tui.pump(80);
        let screen = tui.physical_screen();
        if screen.contains(needle) {
            return;
        }
        assert!(Instant::now() < deadline, "expected {needle:?}\n{screen}");
    }
}

fn wait_file(tui: &mut LiveTui, path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !path.is_file() {
        tui.pump(80);
        assert!(Instant::now() < deadline, "expected copied file");
    }
}
