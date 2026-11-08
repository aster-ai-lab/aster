use super::*;
use aster_use_core::UseError;

#[tokio::test]
async fn embedded_help_uses_the_host_invocation_and_preserves_result_data() {
    let embedded = crate::cli::run_embedded_os(vec!["help".into()])
        .await
        .unwrap();
    let standalone = crate::cli::run_os(vec!["help".into()]).await.unwrap();
    assert!(embedded.human.contains("aster use registry package check"));
    assert!(embedded.human.contains("aster use registry package build"));
    assert!(!embedded.human.contains("aster-use registry"));
    assert!(standalone
        .human
        .contains("aster-use registry package check"));
    assert_eq!(embedded.json, standalone.json);
}

#[tokio::test]
async fn native_cli_output_uses_the_same_json_and_human_contract() {
    let output = crate::cli::run_os(vec!["version".into(), "--json".into()]).await;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(
        write_result(true, output, &mut stdout, &mut stderr).unwrap(),
        0
    );
    let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(value["data"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(stderr.is_empty());
    stdout.clear();
    let error =
        UseError::new("use.cli.fixture", "fixture error").with_suggestion("fixture suggestion");
    assert_eq!(
        write_result(false, Err(error), &mut stdout, &mut stderr).unwrap(),
        1
    );
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "aster-use: fixture error\nsuggestion: fixture suggestion\n"
    );
}

#[tokio::test]
async fn native_cli_errors_keep_the_machine_error_envelope() {
    let output = crate::cli::run_os(vec!["unknown-route".into(), "--json".into()]).await;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(
        write_result(true, output, &mut stdout, &mut stderr).unwrap(),
        1
    );
    let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "use.route_unknown");
    assert!(stderr.is_empty());
}

#[test]
fn rendering_suppressed_output_preserves_the_command_exit_code() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let output = CommandOutput {
        human: "not printed".into(),
        json: serde_json::Value::Null,
        should_print: false,
        exit_code: 37,
    };
    assert_eq!(
        write_result(true, Ok(output), &mut stdout, &mut stderr).unwrap(),
        37
    );
    assert!(stdout.is_empty() && stderr.is_empty());
}

#[test]
fn a_closed_output_returns_an_io_error_without_panicking() {
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for json in [false, true] {
        let output = CommandOutput {
            human: "fixture".into(),
            json: serde_json::Value::Null,
            should_print: true,
            exit_code: 0,
        };
        assert_eq!(
            write_result(json, Ok(output), &mut Closed, &mut Vec::new())
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn non_utf8_command_text_returns_a_typed_encoding_error() {
    use std::os::unix::ffi::OsStringExt;
    let error = match crate::cli::run_os(vec![std::ffi::OsString::from_vec(vec![0xff])]).await {
        Ok(_) => panic!("invalid command text was admitted"),
        Err(error) => error,
    };
    assert_eq!(error.code, "use.cli.argument_encoding");
}
