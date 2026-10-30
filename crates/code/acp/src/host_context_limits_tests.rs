#![cfg(test)]

use super::*;

#[test]
fn host_guidelines_keep_a_fixed_unicode_character_bound() {
    let input = serde_json::json!({"a3sHost": {
        "guidelines": format!(" \0{}\0 ", "界".repeat(13_000))
    }});
    let host = studio_host_from_meta(input.as_object()).unwrap();
    assert_eq!(host.guidelines.chars().count(), 12_288);
    assert!(!host.guidelines.contains('\0'));
    assert!(host.guidelines.starts_with('界'));
    assert!(host.guidelines.ends_with('界'));
}
