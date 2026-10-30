#![cfg(test)]

use super::*;

#[test]
fn retained_streams_round_trip_unicode_empty_and_unterminated_text() {
    for (stdout, stderr) in [
        ("", ""),
        ("中文 e\u{301}\r\n", "error"),
        ("no final newline", "\nstderr\n"),
        ("", "only stderr"),
    ] {
        let streams = capture(stdout, stderr);
        let combined = format!("{stdout}{stderr}");
        assert_eq!(split(&combined, &streams), Some((stdout, stderr)));
        assert!(streams["stdout"].get("text").is_none());
        assert!(streams["stderr"].get("text").is_none());
    }
}

#[test]
fn changed_redacted_or_malformed_text_never_acquires_a_stream_identity() {
    let streams = capture("中文\n", "error\n");
    for changed in [
        "[redacted]\nerror\n",
        "中文\nother\n",
        "中文\nerror\nannotation",
    ] {
        assert!(split(changed, &streams).is_none());
    }
    for (pointer, value) in [
        ("/schema", json!("unknown")),
        ("/stdout/text_bytes", json!(1)),
        ("/stdout/text_bytes", json!(u64::MAX)),
        ("/stderr/text_bytes", json!(0)),
        ("/stdout/sha256", json!("foreign")),
        ("/stderr/sha256", Value::Null),
    ] {
        let mut invalid = streams.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(split("中文\nerror\n", &invalid).is_none());
    }
}
