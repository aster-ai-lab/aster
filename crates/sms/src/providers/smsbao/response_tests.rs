#![cfg(test)]

use super::*;

#[test]
fn acceptance_preserves_handle_and_never_claims_delivery() {
    assert_eq!(
        decode_submit(
            SmsbaoResponseFormat::Json,
            br#"{"code":0,"data":{"taskId":"task-123"}}"#
        ),
        SubmissionOutcome::Accepted {
            provider_message_id: Some("task-123".into())
        }
    );
    assert_eq!(
        decode_submit(SmsbaoResponseFormat::Text, b"0\r\n"),
        SubmissionOutcome::Accepted {
            provider_message_id: None
        }
    );
    assert_eq!(
        decode_submit(SmsbaoResponseFormat::Json, br#"{"code":0,"data":null}"#),
        SubmissionOutcome::Accepted {
            provider_message_id: None
        }
    );
}

#[test]
fn format_specific_rejection_codes_are_not_conflated() {
    for (format, code, reason) in [
        (
            SmsbaoResponseFormat::Text,
            30,
            SmsReason::AuthenticationFailed,
        ),
        (SmsbaoResponseFormat::Json, 30, SmsReason::ProviderRejected),
        (SmsbaoResponseFormat::Text, 51, SmsReason::InvalidRecipient),
        (SmsbaoResponseFormat::Json, 51, SmsReason::InvalidContent),
        (SmsbaoResponseFormat::Json, 52, SmsReason::InvalidRecipient),
        (SmsbaoResponseFormat::Json, 71, SmsReason::PolicyDenied),
        (
            SmsbaoResponseFormat::Json,
            40,
            SmsReason::AuthenticationFailed,
        ),
        (SmsbaoResponseFormat::Text, 60, SmsReason::QuotaExceeded),
        (SmsbaoResponseFormat::Json, 41, SmsReason::QuotaExceeded),
    ] {
        let text = if format == SmsbaoResponseFormat::Text {
            code.to_string()
        } else {
            format!("{{\"code\":{code}}}")
        };
        assert_eq!(
            decode_submit(format, text.as_bytes()),
            SubmissionOutcome::DefinitelyNotAccepted {
                reason,
                provider_code: Some(code)
            }
        );
    }
}

#[test]
fn unknown_truncated_negative_or_wrong_format_responses_stay_indeterminate() {
    for (format, text) in [
        (SmsbaoResponseFormat::Json, "0"),
        (SmsbaoResponseFormat::Json, "{\"code\":"),
        (SmsbaoResponseFormat::Json, "{\"code\":\"0\"}"),
        (SmsbaoResponseFormat::Json, "{\"code\":999}"),
        (SmsbaoResponseFormat::Text, "{\"code\":0}"),
        (SmsbaoResponseFormat::Text, "0\n30"),
        (SmsbaoResponseFormat::Text, "-1"),
        (SmsbaoResponseFormat::Text, "private-response-secret"),
    ] {
        let outcome = decode_submit(format, text.as_bytes());
        assert!(matches!(outcome, SubmissionOutcome::Indeterminate { .. }));
        assert!(!format!("{outcome:?}").contains("private-response-secret"));
    }
}

#[test]
fn balances_validate_integer_quantities_and_do_not_echo_wire_messages() {
    assert_eq!(
        decode_balance(SmsbaoResponseFormat::Text, b"0\r\n100,998\r\n").unwrap(),
        SmsbaoBalance {
            remaining: 998,
            sent: Some(100)
        }
    );
    assert_eq!(
        decode_balance(
            SmsbaoResponseFormat::Json,
            br#"{"code":0,"data":{"balance":998}}"#
        )
        .unwrap()
        .remaining,
        998
    );
    for body in [
        br#"{"code":0,"data":{"balance":-1}}"#.as_slice(),
        br#"{"code":40,"msg":"private-response-secret"}"#,
        b"not json",
    ] {
        let error = decode_balance(SmsbaoResponseFormat::Json, body).unwrap_err();
        assert!(!error.to_string().contains("private-response-secret"));
    }
}
