#![cfg(test)]

use super::*;

pub(super) const EXAMPLE: &str = include_str!("../../../examples/smsbao.acl");

#[test]
fn example_and_closed_sets_are_valid() {
    let config = SmsbaoConfig::from_acl(EXAMPLE).unwrap();
    assert_eq!(config.profile_id(), "smsbao-cn");
    for value in SmsbaoAuthentication::ALL {
        assert_eq!(SmsbaoAuthentication::parse(value.as_str()).unwrap(), *value);
    }
    for value in SmsbaoResponseFormat::ALL {
        assert_eq!(SmsbaoResponseFormat::parse(value.as_str()).unwrap(), *value);
    }
    assert!(SmsbaoAuthentication::parse("plain_key").is_err());
    assert!(SmsbaoResponseFormat::parse("automatic").is_err());
}

#[test]
fn unknown_fields_missing_fields_nested_blocks_and_invalid_values_are_refused() {
    let invalid = [
        EXAMPLE.replace("provider \"smsbao\"", "provider \"other\""),
        EXAMPLE.replace("signature = \"Aster\"", "password = \"do-not-echo-this\""),
        EXAMPLE.replace("signature = \"Aster\"", "signature = \"\""),
        EXAMPLE.replace("signature = \"Aster\"", "signature = \"【Aster】\""),
        EXAMPLE.replace("timeout_ms = 5000", "timeout_ms = 0"),
        EXAMPLE.replace("timeout_ms = 5000", "timeout_ms = 60001"),
        EXAMPLE.replace("timeout_ms = 5000", "timeout_ms = 1.5"),
        EXAMPLE.replace("max_message_chars = 500", "max_message_chars = 4097"),
        EXAMPLE.replace("profile_revision = 1", "profile_revision = 0"),
        EXAMPLE.replace("auth_method = \"password\"", "auth_method = \"other\""),
        EXAMPLE.replace("smsbao-password@1", "smsbao-password"),
        format!("{EXAMPLE}\nprovider \"smsbao\" {{}}"),
        EXAMPLE.replace("signature = \"Aster\"", "nested {}"),
    ];
    for source in invalid {
        let error = SmsbaoConfig::from_acl(&source).unwrap_err();
        assert!(!error.to_string().contains("do-not-echo-this"));
    }
}
