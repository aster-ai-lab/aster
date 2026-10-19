#![cfg(test)]

//! Desktop capability, directory results, and boot scripts.
//! This crate does not link the window shell or the OCR runtime.

#[path = "../../src/host_family.rs"]
mod host_family;

#[path = "../../src/folder_dialog.rs"]
mod folder_dialog;

#[path = "../../src/boot_contract.rs"]
mod boot_contract;

#[cfg(test)]
mod qualification {
    use std::io::Write;
    use std::process::{Command, Stdio};

    use super::boot_contract::{
        boot_initialization_script, is_main_renderer_document, main_renderer_resume_script,
    };

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
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut child = Command::new("node")
            .arg("tests/helpers/nativeBootScripts.cjs")
            .current_dir(&root)
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
    fn native_boot_scripts_keep_runtime_on_trusted_documents() {
        execute_scripts("trusted", qualification_runtime());
        execute_scripts("trusted-plain", qualification_runtime());
    }

    #[test]
    fn native_boot_scripts_refuse_guest_documents() {
        execute_scripts("denied", qualification_runtime());
    }

    #[test]
    fn native_boot_scripts_refuse_invalid_runtime_addresses() {
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
    fn main_renderer_document_matches_the_owned_urls() {
        assert_eq!(
            is_main_renderer_document(
                "http://127.0.0.1:4318/windows/main/index.html?window=mail-compose#compose"
            ),
            cfg!(debug_assertions),
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
}
