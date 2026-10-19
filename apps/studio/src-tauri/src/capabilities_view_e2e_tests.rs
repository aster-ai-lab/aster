//! The robot workbench packages (cobot-arm, g1-arm) through Studio, end to
//! end. Each test runs the
//! authority (the loopback host Studio's renderer talks to) over a temporary
//! store and lets `tests/studio.e2e.mjs` in the package play the renderer:
//! install through the catalog, list the views, open the workbench, load it in
//! Chromium from `/aix-view/`, relay agent turns to the warm box, and close.
//!
//!   cargo test --lib cobot_arm_runs_end_to_end -- --ignored --nocapture
//!   cargo test --lib g1_arm_runs_end_to_end -- --ignored --nocapture
//!
//! Hermetic by default: the driver serves the member gateway (installed set,
//! bundle, System One). With `A3S_STUDIO_E2E_GATEWAY=<origin>` and
//! `A3S_STUDIO_E2E_MEMBER_SESSION=<member-session.json>` the run goes through
//! a live gateway instead; the member token stays in this process.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::tests::workbench_bundle;
use crate::authority::StudioAuthority;

/// The member token in hermetic runs. The driver's gateway expects it on every call.
const HERMETIC_MEMBER_TOKEN: &str = "a3s_ms_e2e-hermetic-member";

fn live_member_token() -> String {
    let path = std::env::var("A3S_STUDIO_E2E_MEMBER_SESSION")
        .expect("A3S_STUDIO_E2E_MEMBER_SESSION names a member-session.json");
    let stored: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    stored["token"]
        .as_str()
        .expect("the member session has a token")
        .to_owned()
}

#[test]
#[ignore = "end to end: needs node, Playwright Chromium and a3s-box"]
fn cobot_arm_runs_end_to_end_through_studio() {
    runs_end_to_end_through_studio("cobot-arm");
}

#[test]
#[ignore = "end to end: needs node, Playwright Chromium and a3s-box"]
fn g1_arm_runs_end_to_end_through_studio() {
    runs_end_to_end_through_studio("g1-arm");
}

fn runs_end_to_end_through_studio(name: &str) {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../use-registry/packages")
        .join(name);
    let root = std::env::temp_dir().join(format!("a3s-studio-e2e-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&root).unwrap();
    let bundle = root.join(format!("{name}.zip"));
    std::fs::write(&bundle, workbench_bundle(name)).unwrap();
    let live = std::env::var("A3S_STUDIO_E2E_GATEWAY").ok();
    let mode = if live.is_some() { "live" } else { "hermetic" };

    let mut driver = Command::new("node");
    driver
        .arg(package.join("tests/studio.e2e.mjs"))
        .env("E2E_BUNDLE", &bundle)
        .env("E2E_MODE", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if live.is_none() {
        driver.env("E2E_MEMBER_TOKEN", HERMETIC_MEMBER_TOKEN);
    }
    let mut driver = driver.spawn().expect("node runs the driver");
    let mut lines = BufReader::new(driver.stdout.take().unwrap()).lines();
    let (origin, member_token) = match live {
        Some(origin) => (origin, live_member_token()),
        None => {
            let line = lines.next().unwrap().unwrap();
            let origin = line
                .strip_prefix("gateway ")
                .unwrap_or_else(|| panic!("driver said {line}"))
                .to_owned();
            (origin, HERMETIC_MEMBER_TOKEN.to_owned())
        }
    };

    let mut authority =
        StudioAuthority::start_with_store(&origin, root.join("capabilities")).unwrap();
    authority
        .preference_member_session()
        .save(
            json!({"token": member_token, "display_name": "E2E"})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    writeln!(
        driver.stdin.as_mut().unwrap(),
        "studio {} {}",
        authority.api_base_url(),
        authority.token()
    )
    .unwrap();

    let mut result = None;
    for line in lines {
        let line = line.unwrap();
        match line.strip_prefix("result ") {
            Some(text) => result = Some(serde_json::from_str::<Value>(text).unwrap()),
            None => println!("{line}"),
        }
    }
    let status = driver.wait().unwrap();
    authority.shutdown();
    let _ = std::fs::remove_dir_all(&root);
    assert!(status.success(), "the driver failed: {status}");
    let result = result.expect("the driver reported a result");
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    assert_eq!(result["mode"], json!(mode));
}
