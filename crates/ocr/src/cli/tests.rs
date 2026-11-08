use super::*;

#[test]
fn document_profile_is_an_explicit_mcp_argument() {
    let cli =
        Cli::try_parse_from(["aster-use-ocr", "serve", "--mcp", "--profile", "document"]).unwrap();
    assert_eq!(cli.profile, PpOcrV6Profile::Document);
    assert!(matches!(cli.command, Command::Serve { mcp: true }));
}

#[cfg(unix)]
#[test]
fn extraction_arguments_preserve_non_utf8_paths() {
    use std::os::unix::ffi::OsStringExt;
    let path = OsString::from_vec(b"image-\xff.png".to_vec());
    let cli = Cli::try_parse_from([
        OsString::from("aster-use-ocr"),
        OsString::from("extract"),
        path.clone(),
    ])
    .unwrap();
    let Command::Extract { path: parsed } = cli.command else {
        panic!("expected extraction command");
    };
    assert_eq!(parsed.as_os_str(), path);
}

#[tokio::test]
async fn doctor_is_versioned_even_when_no_provider_is_ready() {
    let output = run(vec!["doctor".to_string(), "--json".to_string()])
        .await
        .unwrap();
    assert_eq!(output.json["schemaVersion"], 1);
    assert_eq!(output.json["ok"], true);
    assert!(output.json["data"]["readiness"].is_string());
}

#[tokio::test]
async fn serve_requires_an_explicit_protocol() {
    let error = run(vec!["serve".to_string()]).await.unwrap_err();
    assert_eq!(error.code, "use.ocr.usage_invalid");
}
