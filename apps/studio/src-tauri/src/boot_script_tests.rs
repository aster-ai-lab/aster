#![cfg(test)]

use std::io::Write;
use std::process::{Command, Stdio};

use super::boot_contract::page_reload_marker;
use super::{boot_initialization_script, is_main_renderer_document, main_renderer_resume_script};

fn qualification_runtime() -> serde_json::Value {
    serde_json::json!({
        "apiBaseUrl": "http://127.0.0.1:9",
        "token": "qualification-shell-credential",
        "homePath": "/qualification/home",
    })
}

fn execute_scripts(mode: &str, runtime: serde_json::Value) {
    let reserve_lights = mode != "trusted-plain";
    let input = serde_json::json!({
        "mode": mode,
        "development": cfg!(debug_assertions),
        "boot": boot_initialization_script(true, reserve_lights, "toyou", "ToWork", &runtime, reserve_lights),
        "resume": main_renderer_resume_script(&runtime, "toyou", "ToWork"),
        "runtime": runtime,
    });
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Studio root");
    let mut child = Command::new("node")
        .arg("tests/helpers/nativeBootScripts.cjs")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node qualification engine must be installed");
    child
        .stdin
        .take()
        .expect("qualification input")
        .write_all(&serde_json::to_vec(&input).expect("qualification scripts"))
        .expect("write qualification scripts");
    let output = child.wait_with_output().expect("qualification result");
    assert!(
        output.status.success(),
        "actual generated script execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn page_reload_marker_is_present_only_when_the_drive_asks_for_it() {
    assert_eq!(page_reload_marker(false), "");
    assert_eq!(
        page_reload_marker(true),
        "document.documentElement.dataset.a3sPageReload='1';document.documentElement.dataset.a3sBoot=String(Date.now());"
    );
}

#[test]
fn trusted_host_documents_keep_runtime_and_window_boot_behavior() {
    execute_scripts("trusted", qualification_runtime());
    execute_scripts("trusted-plain", qualification_runtime());
}

#[test]
fn guest_and_nested_documents_never_obtain_host_credentials_or_window_effects() {
    execute_scripts("denied", qualification_runtime());
}

#[test]
fn invalid_runtime_addresses_refuse_host_activation() {
    for address in [
        serde_json::Value::Null,
        serde_json::json!(9),
        serde_json::json!("not a URL"),
        serde_json::json!("https://127.0.0.1:9"),
        serde_json::json!("http://localhost:9"),
        serde_json::json!("http://user:password@127.0.0.1:9"),
        serde_json::json!("http://127.0.0.1:9/aix-view/guest.html"),
        serde_json::json!("http://127.0.0.1:9/?guest=true"),
        serde_json::json!("http://127.0.0.1:9/#guest"),
    ] {
        let mut runtime = qualification_runtime();
        runtime["apiBaseUrl"] = address;
        execute_scripts("invalid-runtime", runtime);
    }
    execute_scripts("invalid-runtime", serde_json::json!({}));
}

#[test]
fn page_load_resume_only_selects_owned_main_documents() {
    assert_eq!(
        is_main_renderer_document(
            "http://127.0.0.1:4318/windows/main/index.html?window=mail-compose#compose"
        ),
        cfg!(debug_assertions),
        "the development renderer must not acquire host authority in release builds"
    );
    for url in [
        "tauri://localhost/windows/main/index.html",
        "http://tauri.localhost/windows/main/index.html",
        "https://tauri.localhost/windows/main/index.html",
    ] {
        assert!(is_main_renderer_document(url), "owned renderer: {url}");
    }
    for url in [
        "http://127.0.0.1:4318/aix-view/token/ui/page.html",
        "http://127.0.0.1:4318/windows/main/index.html/guest",
        "tauri://localhost/aix-view/token/ui/page.html",
        "https://tauri.localhost/aix-view/token/ui/page.html",
        "http://127.0.0.1:4319/windows/main/index.html",
        "http://user:password@127.0.0.1:4318/windows/main/index.html",
        "https://tauri.localhost.evil.invalid/windows/main/index.html",
        "https://tauri.localhost:444/windows/main/index.html",
        "http://127.0.0.1:9/",
        "about:blank",
    ] {
        assert!(!is_main_renderer_document(url), "guest renderer: {url}");
    }
}
