#![cfg(target_os = "macos")]

use super::fact_evidence::{
    completed_narrative, facts, facts_for_session, receipt_for, settled_turns,
};
use super::tui_coding::turns::{selected_model, trace_events, trace_pair};
use super::*;
use serde_json::{Value, json};

fn reply_is_visible(reply: &str, screen: &str) -> bool {
    let (rendered, _) = a3s_code_markdown::render_markdown_ratatui_full(
        reply,
        a3s_code_pager::theme::md_style::style(),
        true,
        None,
    );
    let expected: String = rendered
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .flat_map(|span| span.content.chars())
        .filter(|ch| !ch.is_whitespace())
        .collect();
    let shown: String = screen.chars().filter(|ch| !ch.is_whitespace()).collect();
    !expected.is_empty() && shown.contains(&expected)
}

#[test]
fn reply_visibility_uses_rendered_markdown_and_keeps_the_actual_text() {
    for (reply, shown) in [
        ("The function is `probe`.", "The function is probe."),
        ("The function is **probe**.", "The function is probe."),
        (
            "The function is [probe](https://example.com/).",
            "The function is probe (https://example.com/).",
        ),
    ] {
        assert!(reply_is_visible(reply, shown));
        assert!(!reply_is_visible(reply, "The function is different."));
    }
    assert!(!reply_is_visible(
        "The function is [probe](https://example.com/).",
        "The function is probe."
    ));
    assert!(!reply_is_visible("", "unrelated earlier output"));
}

#[test]
fn wrapped_reply_visibility_keeps_words_and_the_link_destination() {
    let reply = "A source is [docs](https://example.com/docs).";
    let mut parser = vt100::Parser::new(4, 120, 0);
    parser.process(
        b"\x1b[1;3HA source is docs (https://example.com/\x1b[1;111H5:40 PM\x1b[2;3Hdocs).",
    );
    assert!(!reply_is_visible(reply, &parser.screen().contents()));
    let viewport = tui_read::read_message_viewport(parser.screen());
    assert!(reply_is_visible(reply, &viewport));
    assert!(!reply_is_visible(
        &reply.replace("source", "reference"),
        &viewport
    ));
    parser.process(b"\x1b[2;1H\x1b[2K");
    assert!(!reply_is_visible(
        reply,
        &tui_read::read_message_viewport(parser.screen())
    ));
    parser.process(b"\x1b[2J\x1b[1;3HA source is docs.");
    assert!(!reply_is_visible(
        reply,
        &tui_read::read_message_viewport(parser.screen())
    ));
    parser.process(b"\x1b[2J\x1b[1;111Hdocs");
    assert!(!reply_is_visible(
        "docs",
        &tui_read::read_message_viewport(parser.screen())
    ));
}

struct Evidence {
    workspace: PathBuf,
    home: PathBuf,
    trace: PathBuf,
    case: &'static str,
}

impl Evidence {
    fn new(session: &LiveSession, case: &'static str) -> Self {
        Self {
            workspace: session.workspace.clone(),
            home: session.home.clone(),
            trace: session.prompt_trace.clone().expect("request relay"),
            case,
        }
    }
}

impl Drop for Evidence {
    fn drop(&mut self) {
        let Some(directory) = std::env::var_os("A3S_CODE_TUI_CAPABILITY_EVIDENCE_DIR") else {
            return;
        };
        let directory = PathBuf::from(directory);
        let artifact = json!({
            "schema":"a3s.test.tui-capability-evidence.v1", "case":self.case,
            "test_panicked":std::thread::panicking(),
            "facts":facts(&self.workspace, &self.home), "requests":trace_events(&self.trace),
        });
        let result = std::fs::create_dir_all(&directory).and_then(|()| {
            std::fs::write(
                directory.join(format!("{}.json", self.case)),
                artifact.to_string(),
            )
        });
        if let Err(error) = result {
            eprintln!("capability evidence retention failed: {error}");
            assert!(
                std::thread::panicking(),
                "cannot pass without requested evidence"
            );
        }
    }
}

fn turn(session: &mut LiveSession, prompt: &str, tool: &str, turns: usize, exit: i64) -> Value {
    let trace = session.prompt_trace.clone().expect("request relay");
    let before_event = trace_events(&trace)
        .last()
        .and_then(|event| event["seq"].as_u64())
        .unwrap_or(0);
    submit_prompt(&mut session.pty, prompt);
    let workspace = session.workspace.clone();
    let home = session.home.clone();
    let model = session.model_id.clone();
    drive_until_screen(
        &mut session.pty,
        Duration::from_secs(420),
        |screen, cells| {
            let events = trace_events(&trace);
            let current: Vec<_> = events
                .iter()
                .filter(|event| event["seq"].as_u64().is_some_and(|seq| seq > before_event))
                .cloned()
                .collect();
            let Some((request, response)) = trace_pair(&current, prompt) else {
                return false;
            };
            let id = request["session_id"].as_str().expect("session identity");
            assert_eq!(selected_model(&events, id), Some(model.as_str()));
            assert!(
                response["error_code"].is_null(),
                "prompt failed: {response}"
            );
            assert_eq!(response["stop_reason"], "end_turn", "{response}");
            let observed = facts_for_session(&workspace, &home, id);
            assert_eq!(settled_turns(&observed), turns);
            completed_narrative(&observed).is_some_and(|reply| {
                !screen_is_busy(screen)
                    && reply_is_visible(reply, &tui_read::read_message_viewport(cells))
            })
        },
    );
    let events: Vec<_> = trace_events(&trace)
        .into_iter()
        .filter(|event| event["seq"].as_u64().is_some_and(|seq| seq > before_event))
        .collect();
    let (request, _) = trace_pair(&events, prompt).expect("completed prompt");
    let observed = facts_for_session(&workspace, &home, request["session_id"].as_str().unwrap());
    let receipts = receipt_for(&observed, tool);
    assert_eq!(receipts.len(), turns, "exactly one {tool} call per turn");
    assert_eq!(
        observed
            .iter()
            .filter(|f| f["type"] == "tool.prepared")
            .count(),
        turns
    );
    let receipt = receipts.last().unwrap();
    assert_eq!(receipt["exit_code"], exit, "{receipt}");
    assert_eq!(
        receipt["outcome"],
        if exit == 0 { "succeeded" } else { "failed" }
    );
    assert_eq!(receipt["effects"]["completeness"], "complete");
    assert_eq!(receipt["effects"]["changes"], json!([]));
    assert_eq!(
        std::fs::read(workspace.join("sentinel.txt")).unwrap(),
        b"unchanged"
    );
    assert_acl_unchanged(session);
    serde_json::from_str(receipt["output"].as_str().expect("raw receipt output")).unwrap()
}

#[test]
#[ignore = "requires actual macOS pager/ACP and the configured real provider"]
fn real_llm_tui_program_reads_and_refuses_an_unlisted_write() {
    let mut session = open_live().expect("A3S_CONFIG_FILE is required");
    let _evidence = Evidence::new(&session, "tui-program-read-and-denial");
    let marker = uuid::Uuid::new_v4().to_string();
    std::fs::write(session.workspace.join("program-note.txt"), &marker).unwrap();
    let prompt = r#"Call program exactly once with type script, allowed_tools ["read"], and source `async function run(ctx, inputs) { return await ctx.tool("read", {file_path:"program-note.txt"}); }`. Do not use other tools. Finish with one short sentence quoting its returned file text."#;
    let result = turn(&mut session, prompt, "program", 1, 0);
    assert_eq!(result["metadata"]["script_result"]["exitCode"], 0);
    assert!(
        result["metadata"]["script_result"]["output"]
            .as_str()
            .unwrap()
            .contains(&marker)
    );
    let prompt = r#"Call program exactly once with type script, allowed_tools ["read"], and source `async function run(ctx, inputs) { return await ctx.tool("write", {file_path:"sentinel.txt", content:"changed"}); }`. Do not retry, change allowed_tools, or use other tools. Finish with one short sentence reporting its actual refusal."#;
    let result = turn(&mut session, prompt, "program", 2, 1);
    assert!(
        result["text"]
            .as_str()
            .unwrap()
            .contains("not allowed for this PTC script")
    );
    assert_eq!(
        std::fs::read_to_string(session.workspace.join("program-note.txt")).unwrap(),
        marker
    );
    eprintln!(
        "ORACLE case=tui-program-read-and-denial actual_read=true denied_write=true unchanged=true"
    );
}

#[test]
#[ignore = "requires actual macOS pager/ACP and the configured real provider"]
fn real_llm_tui_task_child_reads_the_source() {
    let mut session = open_live().expect("A3S_CONFIG_FILE is required");
    let _evidence = Evidence::new(&session, "tui-task-child-read");
    let marker = uuid::Uuid::new_v4().to_string();
    std::fs::write(session.workspace.join("child-note.txt"), &marker).unwrap();
    let prompt = r#"Call task exactly once with one item: agent explore, description Read the fixture, prompt Read child-note.txt using read and return its exact full text. Do not read the file yourself or call other tools. After the child finishes, give one short sentence quoting the child's returned text."#;
    let result = turn(&mut session, prompt, "task", 1, 0);
    assert!(result["text"].as_str().unwrap().contains(&marker));
    let observed = facts(&session.workspace, &session.home);
    assert!(
        receipt_for(&observed, "read").iter().any(|receipt| {
            receipt["exit_code"] == 0
                && receipt["output"]
                    .as_str()
                    .is_some_and(|text| text.contains(&marker))
        }),
        "child must actually read the source"
    );
    eprintln!(
        "ORACLE case=tui-task-child-read parent_task=true child_read=true source_matched=true"
    );
}

#[test]
#[ignore = "requires actual macOS pager/ACP and the configured real provider"]
fn real_llm_tui_plan_update_reaches_the_current_tool() {
    let mut session = open_live().expect("A3S_CONFIG_FILE is required");
    let _evidence = Evidence::new(&session, "tui-plan-update");
    let prompt = r#"Call update_plan exactly once with plan [{"step":"Inspect source","status":"completed"},{"step":"Verify result","status":"in_progress"}]. Do not call other tools. Finish with one short sentence reporting the actual plan state."#;
    let result = turn(&mut session, prompt, "update_plan", 1, 0);
    assert_eq!(
        result["text"],
        "Updated plan: 1/2 done · in progress: Verify result"
    );
    eprintln!("ORACLE case=tui-plan-update completed=1 total=2 active=Verify-result");
}

#[test]
#[ignore = "requires actual macOS pager/ACP and the configured real provider"]
fn real_llm_tui_skill_returns_its_unprompted_body() {
    let mut session = open_live().expect("A3S_CONFIG_FILE is required");
    let _evidence = Evidence::new(&session, "tui-skill-body");
    let marker = uuid::Uuid::new_v4().to_string();
    let skill = session.workspace.join(".agents/skills/capability-kimi");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(skill.join("SKILL.md"), format!("---\nname: capability-kimi\ndescription: Read the fixture body.\nkind: instruction\n---\nThe fixture body is {marker}.\n")).unwrap();
    let prompt = "Invoke Skill exactly once with skill_name capability-kimi. Do not call other tools. Finish with one short sentence quoting the fixture body returned by that tool.";
    let result = turn(&mut session, prompt, "Skill", 1, 0);
    assert!(result["text"].as_str().unwrap().contains(&marker));
    eprintln!("ORACLE case=tui-skill-body actual_skill=true body_matched=true");
}

#[test]
#[ignore = "requires actual macOS pager/ACP, installed TypeScript language service and real provider"]
fn real_llm_tui_code_symbols_uses_the_language_service() {
    let mut session = open_live().expect("A3S_CONFIG_FILE is required");
    let _evidence = Evidence::new(&session, "tui-code-symbols");
    let typescript = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/intelligent-ui/node_modules/typescript");
    assert!(typescript.join("lib/tsserver.js").is_file());
    std::fs::create_dir_all(session.workspace.join("node_modules")).unwrap();
    std::os::unix::fs::symlink(
        typescript,
        session.workspace.join("node_modules/typescript"),
    )
    .unwrap();
    std::fs::create_dir_all(session.workspace.join("src")).unwrap();
    std::fs::write(
        session.workspace.join("package.json"),
        r#"{"private":true}"#,
    )
    .unwrap();
    std::fs::write(
        session.workspace.join("tsconfig.json"),
        r#"{"compilerOptions":{"noEmit":true},"include":["src"]}"#,
    )
    .unwrap();
    let symbol = format!("sourceProbe_{}", uuid::Uuid::new_v4().simple());
    std::fs::write(
        session.workspace.join("src/probe.ts"),
        format!("export function {symbol}(value: number): number {{ return value + 1; }}\n"),
    )
    .unwrap();
    let prompt = r#"Call code_symbols exactly once with operation outline and path src/probe.ts. Do not read the file or call other tools. Finish with one short sentence naming the returned function."#;
    let result = turn(&mut session, prompt, "code_symbols", 1, 0);
    assert!(
        result["metadata"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["name"] == symbol)
    );
    eprintln!("ORACLE case=tui-code-symbols actual_language_service=true symbol_matched=true");
}

#[test]
#[ignore = "requires actual macOS pager/ACP, configured real provider and public HTTPS access"]
fn real_llm_tui_web_fetch_returns_bounded_public_content() {
    let mut session = open_live().expect("A3S_CONFIG_FILE is required");
    let _evidence = Evidence::new(&session, "tui-web-fetch");
    let prompt = r#"Call web_fetch exactly once with url https://example.com/ and max_chars 1000. Do not call other tools or retry. Finish with one short sentence naming the title returned by that tool."#;
    let result = turn(&mut session, prompt, "web_fetch", 1, 0);
    let text = result["text"].as_str().unwrap();
    assert!(!text.trim().is_empty());
    assert_eq!(
        result["metadata"]["range"]["returned_chars"],
        text.chars().count()
    );
    assert!(
        result["metadata"]["range"]["returned_chars"]
            .as_u64()
            .unwrap()
            <= 1000
    );
    assert!(
        result["metadata"]["source_anchors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|url| url == "https://example.com/")
    );
    eprintln!("ORACLE case=tui-web-fetch fetched_public_source=true bounded=true");
}

#[test]
#[ignore = "requires actual macOS pager/ACP, configured real provider and baseline search engines"]
fn real_llm_tui_web_search_returns_actual_candidates() {
    let mut session = open_live().expect("A3S_CONFIG_FILE is required");
    let _evidence = Evidence::new(&session, "tui-web-search");
    let prompt = r#"Call web_search exactly once with query Python 3 official documentation, limit 3, timeout 20 and format json. Do not call other tools or retry. Finish with one short sentence naming one candidate actually returned by that tool."#;
    let result = turn(&mut session, prompt, "web_search", 1, 0);
    assert!(
        result["metadata"]["returned_result_count"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        result["metadata"]["returned_result_count"]
            .as_u64()
            .unwrap()
            <= 3
    );
    assert!(
        !result["metadata"]["source_anchors"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    eprintln!("ORACLE case=tui-web-search actual_candidates=true sources_present=true");
}
