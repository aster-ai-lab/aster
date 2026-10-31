use std::path::PathBuf;

use super::{parse_config, RunnerConfig, ATTRIBUTE_SET};

const FULL: &str = r#"
runner {
  api_url           = "http://127.0.0.1:3000/"
  state_file        = "/var/lib/aster-runner/state.acl"
  work_dir          = "/var/lib/aster-runner/work"
  image             = "alpine:3.20"
  box_cli           = "/usr/local/bin/aster-box"
  cpus              = 4
  memory            = "1024m"
  poll_seconds      = 2
  heartbeat_seconds = 15
}
"#;

fn minimal(extra: &str) -> String {
    format!(
        r#"runner {{
  api_url    = "https://api.example.test"
  state_file = "/state.acl"
  work_dir   = "/work"
  image      = "alpine:3.20"
  {extra}
}}"#
    )
}

fn refused(text: &str) -> String {
    parse_config(text).expect_err("refused")
}

#[test]
fn the_contract_example_parses() {
    assert_eq!(
        parse_config(FULL).expect("parse"),
        RunnerConfig {
            api_url: "http://127.0.0.1:3000".into(),
            state_file: PathBuf::from("/var/lib/aster-runner/state.acl"),
            work_dir: PathBuf::from("/var/lib/aster-runner/work"),
            image: "alpine:3.20".into(),
            box_cli: Some(PathBuf::from("/usr/local/bin/aster-box")),
            cpus: 4,
            memory: "1024m".into(),
            poll_seconds: 2,
            heartbeat_seconds: 15,
        }
    );
}

#[test]
fn optional_attributes_take_their_defaults() {
    let config = parse_config(&minimal("")).expect("parse");
    assert_eq!(config.box_cli, None);
    assert_eq!(
        (
            config.cpus,
            config.memory.as_str(),
            config.poll_seconds,
            config.heartbeat_seconds
        ),
        (2, "2g", 5, 30)
    );
}

#[test]
fn the_shape_is_closed() {
    assert_eq!(
        refused(&minimal("labels = [\"x\"]")),
        format!("runner only accepts {ATTRIBUTE_SET}")
    );
    assert_eq!(
        refused(&minimal("box { }")),
        format!("runner only accepts {ATTRIBUTE_SET}")
    );
    assert_eq!(
        refused("other {\n}\n"),
        "runner acl must hold exactly one `runner` block"
    );
    assert_eq!(
        refused(&format!("{}\n{}", minimal(""), minimal(""))),
        "runner acl must hold exactly one `runner` block"
    );
}

#[test]
fn values_are_checked() {
    for (extra, refusal) in [
        ("cpus = 0", "runner cpus must be an integer 1..=16"),
        ("cpus = 2.5", "runner cpus must be an integer 1..=16"),
        ("cpus = \"2\"", "runner cpus must be an integer 1..=16"),
        (
            "heartbeat_seconds = 90",
            "runner heartbeat_seconds must be an integer 5..=60",
        ),
        (
            "poll_seconds = 0",
            "runner poll_seconds must be an integer 1..=60",
        ),
        (
            "memory = \"2G\"",
            "runner memory must match ^[1-9][0-9]{0,5}[mg]$",
        ),
        (
            "memory = \"0g\"",
            "runner memory must match ^[1-9][0-9]{0,5}[mg]$",
        ),
        (
            "memory = \"g\"",
            "runner memory must match ^[1-9][0-9]{0,5}[mg]$",
        ),
        (
            "box_cli = \"aster-box\"",
            "runner box_cli must be an absolute path",
        ),
    ] {
        assert_eq!(refused(&minimal(extra)), refusal, "{extra}");
    }
    let text = minimal("").replace("\"alpine:3.20\"", "\"-v\"");
    assert_eq!(
        refused(&text),
        "runner image must be an image reference without whitespace"
    );
    let text = minimal("").replace("https://api.example.test", "ftp://api");
    assert_eq!(
        refused(&text),
        "runner api_url must be an http:// or https:// URL"
    );
    let text = minimal("").replace("\"/work\"", "\"work\"");
    assert_eq!(refused(&text), "runner work_dir must be an absolute path");
    let text = minimal("").replace("state_file = \"/state.acl\"", "");
    assert_eq!(refused(&text), "runner state_file is required");
}
