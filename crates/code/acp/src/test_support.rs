#![cfg(test)]

pub(crate) fn successful_tool_json(text: &str) -> serde_json::Value {
    let body = text
        .strip_prefix("Tool exit code: 0\n")
        .unwrap_or_else(|| panic!("expected an actual successful tool result: {text}"));
    serde_json::from_str(body).expect("successful native tool payload is JSON")
}

pub(crate) fn assert_failed_tool_text(text: &str) {
    let body = text
        .strip_prefix("Tool exit code: 1\n")
        .unwrap_or_else(|| panic!("expected an actual failed tool result: {text}"));
    assert!(
        serde_json::from_str::<serde_json::Value>(body).is_err(),
        "refused document content must not enter the model as accepted JSON: {body}"
    );
}

#[cfg(windows)]
pub(crate) fn native_executable_fixture(
    root: &std::path::Path,
    name: &str,
    source: &str,
) -> std::path::PathBuf {
    let input = root.join(format!("{name}.rs"));
    let output = root.join(format!("{name}.exe"));
    std::fs::write(&input, source).unwrap();
    let result = std::process::Command::new("rustc")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--edition=2021", "--crate-name", name])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "native fixture compilation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    output
}
