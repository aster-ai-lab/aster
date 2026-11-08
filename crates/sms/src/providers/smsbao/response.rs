use serde_json::Value;

use crate::{SmsError, SmsReason, SubmissionOutcome};

use super::{SmsbaoBalance, SmsbaoResponseFormat};

pub(super) fn decode_submit(format: SmsbaoResponseFormat, body: &[u8]) -> SubmissionOutcome {
    let decoded = match format {
        SmsbaoResponseFormat::Json => {
            serde_json::from_slice::<Value>(body)
                .ok()
                .and_then(|value| {
                    let code = value.as_object()?.get("code")?.as_i64()?;
                    let id = value
                        .get("data")
                        .and_then(|data| data.get("taskId"))
                        .and_then(Value::as_str)
                        .filter(|id| {
                            !id.is_empty()
                                && id.len() <= 128
                                && id.bytes().all(|b| {
                                    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':')
                                })
                        })
                        .map(str::to_owned);
                    Some((code, id))
                })
        }
        SmsbaoResponseFormat::Text => std::str::from_utf8(body)
            .ok()
            .and_then(|body| body.trim().parse::<i64>().ok())
            .map(|code| (code, None)),
    };
    match decoded {
        Some((0, provider_message_id)) => SubmissionOutcome::Accepted {
            provider_message_id,
        },
        Some((code, _)) => match rejection(format, code) {
            Some(reason) => SubmissionOutcome::DefinitelyNotAccepted {
                reason,
                provider_code: Some(code),
            },
            None => unknown(),
        },
        None => unknown(),
    }
}

pub(super) fn decode_balance(
    format: SmsbaoResponseFormat,
    body: &[u8],
) -> Result<SmsbaoBalance, SmsError> {
    let result = match format {
        SmsbaoResponseFormat::Json => {
            let value: Value = serde_json::from_slice(body).map_err(|_| unknown_error())?;
            let code = value
                .get("code")
                .and_then(Value::as_i64)
                .ok_or_else(unknown_error)?;
            if code != 0 {
                return Err(rejection_error(format, code));
            }
            value
                .get("data")
                .and_then(|data| data.get("balance"))
                .and_then(Value::as_u64)
                .map(|remaining| SmsbaoBalance {
                    remaining,
                    sent: None,
                })
        }
        SmsbaoResponseFormat::Text => {
            let text = std::str::from_utf8(body).map_err(|_| unknown_error())?;
            let mut lines = text.trim().lines();
            let code = lines
                .next()
                .and_then(|line| line.trim().parse::<i64>().ok())
                .ok_or_else(unknown_error)?;
            if code != 0 {
                return Err(rejection_error(format, code));
            }
            let (sent, remaining) = lines
                .next()
                .and_then(|line| line.trim().split_once(','))
                .ok_or_else(unknown_error)?;
            if lines.next().is_some() {
                return Err(unknown_error());
            }
            match (sent.parse::<u64>(), remaining.parse::<u64>()) {
                (Ok(sent), Ok(remaining)) => Some(SmsbaoBalance {
                    remaining,
                    sent: Some(sent),
                }),
                _ => None,
            }
        }
    };
    result.ok_or_else(unknown_error)
}

fn rejection(format: SmsbaoResponseFormat, code: i64) -> Option<SmsReason> {
    use SmsReason::*;
    match (format, code) {
        (SmsbaoResponseFormat::Text, 30 | 40) | (SmsbaoResponseFormat::Json, 40 | 55) => {
            Some(AuthenticationFailed)
        }
        (_, 41) | (SmsbaoResponseFormat::Text, 60) => Some(QuotaExceeded),
        (_, 42 | 43) | (SmsbaoResponseFormat::Json, 44 | 54 | 71) => Some(PolicyDenied),
        (SmsbaoResponseFormat::Text, 50) | (SmsbaoResponseFormat::Json, 51) => Some(InvalidContent),
        (SmsbaoResponseFormat::Text, 51) | (SmsbaoResponseFormat::Json, 52) => {
            Some(InvalidRecipient)
        }
        (SmsbaoResponseFormat::Json, 30 | 70) => Some(ProviderRejected),
        (SmsbaoResponseFormat::Json, 53) => Some(ProviderUnavailable),
        _ => None,
    }
}
fn rejection_error(format: SmsbaoResponseFormat, code: i64) -> SmsError {
    match rejection(format, code) {
        Some(reason) => SmsError::Provider { reason, code },
        None => unknown_error(),
    }
}
fn unknown() -> SubmissionOutcome {
    SubmissionOutcome::Indeterminate {
        reason: SmsReason::UnrecognizedProviderResponse,
    }
}
fn unknown_error() -> SmsError {
    SmsError::Refused {
        reason: SmsReason::UnrecognizedProviderResponse,
    }
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
