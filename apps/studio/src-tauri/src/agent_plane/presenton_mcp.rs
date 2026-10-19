//! Built-in MCP tools a3s-code calls for the current task's 演示文稿.
//! The view stores the deck. This process checks the closed sets, returns
//! the last snapshot the renderer posted, and tells the model to stop.
//! The tools exist only while the 演示文稿 plugin is installed.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};

pub(crate) const SLUG: &str = "a3s-presenton";
pub(crate) const PACKAGE_ID: &str = "use/a3s/presenton";

/// The unpacked plugin, pinned to this package, and not a link out of the tree.
pub(crate) fn plugin_installed(caps: &Path) -> bool {
    if crate::capabilities::installed_package_id(caps, SLUG).as_deref() != Some(PACKAGE_ID) {
        return false;
    }
    let dir = caps.join("plugins").join(SLUG);
    let Ok(canonical_dir) = dir.canonicalize() else {
        return false;
    };
    let Ok(plugins) = caps.join("plugins").canonicalize() else {
        return false;
    };
    canonical_dir == plugins.join(SLUG) && canonical_dir.join("a3s-use-extension.acl").is_file()
}

/// Drop the posted deck text. Uninstall calls this so a later install starts empty.
pub(crate) fn release_installed() {
    *snapshot_lock() = String::new();
}

#[allow(unused_imports)]
use a3s_boot::{Result, body, controller, post};

pub(crate) fn presenton_mcp_server(base_url: &str, token: &str) -> Value {
    let base = base_url.trim_end_matches('/');
    json!({
        "type": "http",
        "name": "presenton",
        "url": format!("{base}/mcp/presenton"),
        "headers": [{ "name": "Authorization", "value": format!("Bearer {token}") }],
    })
}

pub struct PresentonMcpController {
    caps: Option<PathBuf>,
}

impl PresentonMcpController {
    pub(crate) fn new(caps: Option<PathBuf>) -> Self {
        Self { caps }
    }

    fn installed(&self) -> bool {
        self.caps.as_deref().is_some_and(plugin_installed)
    }
}

#[controller("/mcp/presenton")]
impl PresentonMcpController {
    #[post("/")]
    async fn rpc(&self, #[body] body: Value) -> Result<Value> {
        Ok(handle_rpc(self.installed(), &body))
    }

    #[post("/snapshot")]
    async fn snapshot(&self, #[body] body: Value) -> Result<Value> {
        Ok(store_snapshot(self.installed(), &body))
    }
}

fn handle_rpc(installed: bool, body: &Value) -> Value {
    if !installed {
        release_installed();
    }
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    let id = body.get("id").cloned();
    if id.is_none() {
        return Value::Null;
    }
    let result = match method {
        "initialize" => json!({
            "protocolVersion": body
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "presenton", "version": "1.0.0" }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": if installed { tools() } else { json!([]) } }),
        "tools/call" => {
            if installed {
                call_tool(body.get("params").unwrap_or(&Value::Null))
            } else {
                json!({ "content": [{ "type": "text", "text": "演示文稿未安装。" }], "isError": true })
            }
        }
        _ => {
            return json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "method not found" }
            });
        }
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn tools() -> Value {
    json!([
        {
            "name": "generate_deck",
            "description": "生成当前任务右侧的演示文稿。成员要求幻灯片或 PPT 时调用，content 写入文稿依据的正文。调用后大纲和每一页会在右侧生成。不要让成员填写上传页、选择模板或点击生成。不要另写 pptx，也不要调用别的文稿技能。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "文稿依据的正文，最多 12000 字。" },
                    "n_slides": { "type": ["integer", "null"], "description": "1 到 20。省略或 null 为自动页数。" },
                    "language": { "type": "string", "description": "省略时为简体中文。必须是演示文稿语言清单中的一项。" },
                    "tone": { "type": "string", "description": "default、casual、professional、funny、educational、sales_pitch。省略时为 default。" },
                    "verbosity": { "type": "string", "description": "concise、standard、text-heavy。省略时为 standard。" },
                    "include_title_slide": { "type": "boolean", "description": "省略时为 true。" },
                    "instructions": { "type": "string", "description": "补充说明，最多 4000 字。" }
                },
                "required": ["content"]
            }
        },
        {
            "name": "read_deck",
            "description": "读取当前任务右侧演示文稿的大纲、每一页的可见文字和备注。不要另写文件。",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "edit_outline",
            "description": "替换当前演示文稿的一节大纲。index 从 0 开始。其他节和每一页保持原样。不要另写文件。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "index": { "type": "integer", "description": "0 到 19。" },
                    "content": { "type": "string", "description": "这一节的新大纲，1 到 4000 字。" }
                },
                "required": ["index", "content"]
            }
        },
        {
            "name": "edit_slide",
            "description": "替换当前演示文稿一页的第一处可见文字，并设置该页备注。其他页保持原样。不要另写文件。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "index": { "type": "integer", "description": "0 到 19。" },
                    "text": { "type": "string", "description": "这一页的新可见文字，1 到 4000 字。" },
                    "note": { "type": "string", "description": "这一页的备注，最多 4000 字，可以是空字符串。" }
                },
                "required": ["index", "text", "note"]
            }
        }
    ])
}

fn call_tool(params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let outcome = match name {
        "generate_deck" => accept_deck(&arguments).map(|_| {
            "已交给当前任务的演示文稿。大纲和每一页会在右侧生成。不要再写文件，不要调用别的文稿技能。".to_owned()
        }),
        "read_deck" => {
            let text = current_snapshot();
            if text.is_empty() {
                Err("当前任务还没有演示文稿正文。".to_owned())
            } else {
                Ok(text)
            }
        }
        // shortcut: the view stores the edit after this reply, upgrade when the tool call can wait for the snapshot.
        "edit_outline" => accept_outline(&arguments).map(|_| {
            "已修改当前演示文稿的一节大纲。其他节和每一页保持原样。不要另写文件。".to_owned()
        }),
        "edit_slide" => accept_slide(&arguments).map(|_| {
            "已修改当前演示文稿的一页正文和备注。其他页保持原样。不要另写文件。".to_owned()
        }),
        _ => Err(format!("unknown tool {name}")),
    };
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(message) => {
            json!({ "content": [{ "type": "text", "text": message }], "isError": true })
        }
    }
}

const TONES: [&str; 6] = [
    "default",
    "casual",
    "professional",
    "funny",
    "educational",
    "sales_pitch",
];
const VERBOSITIES: [&str; 3] = ["concise", "standard", "text-heavy"];
const DEFAULT_LANGUAGE: &str = "Chinese (Simplified - 中文, 汉语)";
const MAX_CONTENT: usize = 12_000;
const MAX_INSTRUCTIONS: usize = 4_000;
const MAX_TEXT: usize = 4_000;
const MAX_SLIDES: i64 = 20;
const MAX_SNAPSHOT: usize = 64_000;

fn snapshot_slot() -> &'static Mutex<String> {
    static SLOT: Mutex<String> = Mutex::new(String::new());
    &SLOT
}

fn snapshot_lock() -> std::sync::MutexGuard<'static, String> {
    snapshot_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn current_snapshot() -> String {
    snapshot_lock().clone()
}

fn store_snapshot(installed: bool, body: &Value) -> Value {
    if !installed {
        release_installed();
        return json!({ "ok": false });
    }
    let text = body.get("text").and_then(Value::as_str).unwrap_or("");
    if text.chars().count() > MAX_SNAPSHOT {
        return json!({ "ok": false });
    }
    *snapshot_lock() = text.to_owned();
    json!({ "ok": true })
}

fn integer_in(value: &Value, min: i64, max: i64) -> bool {
    value
        .as_i64()
        .filter(|n| value.as_f64().is_some_and(|f| f == *n as f64))
        .is_some_and(|n| (min..=max).contains(&n))
}

fn text_in(value: Option<&Value>, min: usize, max: usize) -> bool {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|text| {
            let count = text.chars().count();
            count >= min && count <= max
        })
}

fn accept_deck(arguments: &Value) -> std::result::Result<(), String> {
    let content = arguments
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let content_len = content.chars().count();
    if content.is_empty() || content_len > MAX_CONTENT {
        return Err("content 必须是 1 到 12000 字的正文。".to_owned());
    }
    if let Some(slides) = arguments.get("n_slides") {
        if !slides.is_null() {
            let n = slides
                .as_i64()
                .filter(|n| slides.as_f64().is_some_and(|f| f == *n as f64))
                .ok_or_else(|| "n_slides 必须是 1 到 20 的整数，或 null。".to_owned())?;
            if !(1..=MAX_SLIDES).contains(&n) {
                return Err("n_slides 必须是 1 到 20 的整数，或 null。".to_owned());
            }
        }
    }
    let language = arguments
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_LANGUAGE);
    if !languages().contains(&language) {
        return Err("language 不在演示文稿的语言清单里。".to_owned());
    }
    let tone = arguments
        .get("tone")
        .and_then(Value::as_str)
        .unwrap_or("default");
    if !TONES.contains(&tone) {
        return Err(
            "tone 必须是 default、casual、professional、funny、educational 或 sales_pitch。"
                .to_owned(),
        );
    }
    let verbosity = arguments
        .get("verbosity")
        .and_then(Value::as_str)
        .unwrap_or("standard");
    if !VERBOSITIES.contains(&verbosity) {
        return Err("verbosity 必须是 concise、standard 或 text-heavy。".to_owned());
    }
    if let Some(title) = arguments.get("include_title_slide") {
        if !title.is_boolean() {
            return Err("include_title_slide 必须是布尔值。".to_owned());
        }
    }
    let instructions = arguments
        .get("instructions")
        .and_then(Value::as_str)
        .unwrap_or("");
    if instructions.chars().count() > MAX_INSTRUCTIONS {
        return Err("instructions 最多 4000 字。".to_owned());
    }
    Ok(())
}

fn accept_outline(arguments: &Value) -> std::result::Result<(), String> {
    if !integer_in(arguments.get("index").unwrap_or(&Value::Null), 0, 19)
        || !text_in(arguments.get("content"), 1, MAX_TEXT)
    {
        return Err("大纲修改需要 0 到 19 的序号，以及 1 到 4000 字的新内容。".to_owned());
    }
    Ok(())
}

fn accept_slide(arguments: &Value) -> std::result::Result<(), String> {
    let note = arguments.get("note").and_then(Value::as_str);
    if !integer_in(arguments.get("index").unwrap_or(&Value::Null), 0, 19)
        || !text_in(arguments.get("text"), 1, MAX_TEXT)
        || note.is_none_or(|text| text.chars().count() > MAX_TEXT)
    {
        return Err(
            "页面修改需要 0 到 19 的序号、1 到 4000 字的正文，以及不超过 4000 字的备注。"
                .to_owned(),
        );
    }
    Ok(())
}

/// Quoted strings in the package language list. That file is the closed set.
fn languages() -> Vec<&'static str> {
    quoted(include_str!(
        "../../../../use-registry/packages/presenton/agent/languages.mjs"
    ))
}

fn quoted(source: &'static str) -> Vec<&'static str> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index] != b'"' {
            if bytes[index] == b'\\' {
                index += 2;
                if index > bytes.len() {
                    return out;
                }
                continue;
            }
            index += 1;
        }
        if index >= bytes.len() {
            return out;
        }
        out.push(&source[start..index]);
        index += 1;
    }
    out
}

#[cfg(test)]
pub(crate) fn set_snapshot_for_tests(text: &str) {
    *snapshot_lock() = text.to_owned();
}

#[cfg(test)]
pub(crate) fn snapshot_for_tests() -> String {
    current_snapshot()
}

#[cfg(test)]
pub(crate) fn snapshot_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
mod tests {
    use super::{accept_deck, handle_rpc, plugin_installed, release_installed, store_snapshot};
    use serde_json::json;
    use std::fs;

    fn rpc(body: serde_json::Value) -> serde_json::Value {
        handle_rpc(true, &body)
    }

    #[test]
    fn generate_deck_accepts_a_topic_and_refuses_a_language_outside_the_list() {
        accept_deck(&json!({ "content": "项目总结" })).expect("defaults");
        let refused = accept_deck(&json!({ "content": "项目总结", "language": "中文" }));
        assert!(refused.is_err(), "{refused:?}");
        assert!(accept_deck(&json!({ "content": "  " })).is_err());
        assert!(accept_deck(&json!({ "content": "项目总结", "n_slides": 21 })).is_err());
        accept_deck(&json!({
            "content": "项目总结",
            "n_slides": null,
            "tone": "professional",
            "verbosity": "concise",
            "include_title_slide": false,
            "instructions": "面向管理层"
        }))
        .expect("closed sets");
    }

    #[test]
    fn tools_call_returns_the_stop_sentence() {
        let _guard = super::snapshot_test_lock();
        let response = rpc(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "generate_deck", "arguments": { "content": "项目总结" } }
        }));
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("不要再写文件"));
        assert_eq!(response["result"]["isError"], false);
    }

    #[test]
    fn read_deck_returns_only_the_posted_snapshot_and_edits_do_not_echo_a_body() {
        let _guard = super::snapshot_test_lock();
        release_installed();
        let empty = rpc(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "read_deck", "arguments": {} }
        }));
        assert_eq!(empty["result"]["isError"], true);
        assert!(
            empty["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("还没有")
        );

        let listed = rpc(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/list"
        }));
        let names: Vec<&str> = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert_eq!(
            names,
            vec!["generate_deck", "read_deck", "edit_outline", "edit_slide"]
        );

        let snapshot = r#"{"id":"deck-1","outlines":["甲","乙"],"slides":[{"index":2,"text":"可见","note":"备注"}]}"#;
        assert_eq!(
            store_snapshot(true, &json!({ "text": snapshot }))["ok"],
            true
        );
        let read = rpc(json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": { "name": "read_deck", "arguments": {} }
        }));
        assert_eq!(read["result"]["isError"], false);
        assert_eq!(read["result"]["content"][0]["text"], snapshot);

        let outline = rpc(json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": { "name": "edit_outline", "arguments": { "index": 1, "content": "新的一节" } }
        }));
        let outline_text = outline["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(outline["result"]["isError"], false);
        assert!(outline_text.contains("其他节"));
        assert!(!outline_text.contains("新的一节"));

        let slide = rpc(json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": { "name": "edit_slide", "arguments": { "index": 2, "text": "新的一页", "note": "" } }
        }));
        let slide_text = slide["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(slide["result"]["isError"], false);
        assert!(slide_text.contains("其他页"));
        assert!(!slide_text.contains("新的一页"));
        assert_eq!(
            rpc(json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "tools/call",
                "params": { "name": "read_deck", "arguments": {} }
            }))["result"]["content"][0]["text"],
            snapshot
        );

        let refused = rpc(json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": { "name": "edit_slide", "arguments": { "index": 1.5, "text": "新的一页", "note": "备注" } }
        }));
        assert_eq!(refused["result"]["isError"], true);
        assert_eq!(
            store_snapshot(true, &json!({ "text": "字".repeat(64_001) }))["ok"],
            false
        );
        assert_eq!(super::current_snapshot(), snapshot);
    }

    #[test]
    fn an_uninstalled_plugin_lists_no_tools_and_drops_the_snapshot() {
        let _guard = super::snapshot_test_lock();
        let root =
            std::env::temp_dir().join(format!("a3s-presenton-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let caps = root.join("capabilities");
        assert!(!plugin_installed(&caps));
        store_snapshot(true, &json!({ "text": "旧文稿" }));
        let gone = handle_rpc(
            false,
            &json!({
                "jsonrpc": "2.0",
                "id": 9,
                "method": "tools/call",
                "params": { "name": "read_deck", "arguments": {} }
            }),
        );
        assert_eq!(gone["result"]["isError"], true);
        assert_eq!(gone["result"]["content"][0]["text"], "演示文稿未安装。");
        println!("演示文稿未安装。");
        assert_eq!(super::current_snapshot(), "");
        let listed = handle_rpc(
            false,
            &json!({ "jsonrpc": "2.0", "id": 10, "method": "tools/list" }),
        );
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 0);
        assert_eq!(
            store_snapshot(false, &json!({ "text": "卸掉之后" }))["ok"],
            false
        );
        assert_eq!(super::current_snapshot(), "");

        let plugin = caps.join("plugins").join(super::SLUG);
        fs::create_dir_all(&plugin).unwrap();
        fs::write(plugin.join(".a3s-package-id"), super::PACKAGE_ID).unwrap();
        fs::write(
            plugin.join("a3s-use-extension.acl"),
            "extension \"a3s/presenton\" {}\n",
        )
        .unwrap();
        assert!(plugin_installed(&caps));
        fs::write(plugin.join(".a3s-package-id"), "use/other/presenton").unwrap();
        assert!(!plugin_installed(&caps));
        let _ = fs::remove_dir_all(&root);
    }
}
