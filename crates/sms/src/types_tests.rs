#![cfg(test)]

use crate::*;

#[test]
fn closed_sets_round_trip_and_refuse_unknown_without_echo() {
    for value in SmsReason::ALL {
        assert_eq!(SmsReason::parse(value.as_str()).unwrap(), *value);
    }
    for value in SmsCapability::ALL {
        assert_eq!(SmsCapability::parse(value.as_str()).unwrap(), *value);
    }
    for value in SmsContentKind::ALL {
        assert_eq!(SmsContentKind::parse(value.as_str()).unwrap(), *value);
    }
    for value in SubmissionOutcomeKind::ALL {
        assert_eq!(
            SubmissionOutcomeKind::parse(value.as_str()).unwrap(),
            *value
        );
    }
    let error = SmsCapability::parse("private-input-value")
        .unwrap_err()
        .to_string();
    assert!(error.contains("SMS capability"));
    assert!(!error.contains("private-input-value"));
}

#[test]
fn phone_numbers_are_explicit_e164_and_debug_is_redacted() {
    let phone = PhoneNumber::parse("+8613800138000").unwrap();
    assert_eq!(phone.as_str(), "+8613800138000");
    assert!(!format!("{phone:?}").contains("13800138000"));
    for invalid in [
        "13800138000",
        "+0138",
        "+",
        "+86,13800138000",
        "+1234567890123456",
        "+86abc",
    ] {
        assert!(PhoneNumber::parse(invalid).is_err());
    }
}

#[test]
fn secrets_content_and_errors_do_not_expose_sensitive_values() {
    let secret = SecretValue::new("fake-secret-value".into()).unwrap();
    assert_eq!(secret.expose_secret(), "fake-secret-value");
    let content = SmsContent::text("fake-verification-code".into());
    assert!(!format!("{secret:?}").contains("fake-secret-value"));
    assert!(!format!("{content:?}").contains("fake-verification-code"));
    assert!(SecretValue::new(String::new()).is_err());
    assert!(SecretValue::new("x\n".into()).is_err());
}

#[test]
fn references_and_bindings_require_explicit_positive_versions() {
    assert!(SecretReference::parse("smsbao-password@1").is_ok());
    for invalid in [
        "password",
        "@1",
        "password@0",
        "password@01",
        "a@-1",
        "a@+1",
        "a/b@1",
    ] {
        assert!(SecretReference::parse(invalid).is_err(), "{invalid}");
    }
    let mut binding = PreparedBinding {
        profile_id: "profile".into(),
        profile_revision: 1,
        attempt_id: "attempt".into(),
    };
    assert!(binding.validate().is_ok());
    binding.profile_revision = 0;
    assert!(binding.validate().is_err());
}
