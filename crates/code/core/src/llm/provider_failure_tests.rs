use super::*;

#[test]
fn only_typed_transient_exhaustion_is_retryable() {
    for status in [408, 429, 500, 503, 529, 400, 401, 403] {
        let error = anyhow::Error::new(crate::retry::RetryExhaustedError::from_status(
            2, status, "fixture",
        ))
        .context("provider call");
        let failure = RetryableProviderFailure::from_error(&error);
        if matches!(status, 408 | 429 | 500..=599) {
            let failure = failure.unwrap();
            assert_eq!(failure.status_code(), status);
            assert_eq!(failure.attempts(), 2);
        } else {
            assert!(failure.is_none());
        }
    }
    let quota = anyhow::Error::new(NonRetryableLlmError::from_status(
        "fixture",
        429,
        "quota exhausted",
    ));
    assert!(RetryableProviderFailure::from_error(&quota).is_none());
    assert!(
        RetryableProviderFailure::from_error(&anyhow::anyhow!("HTTP 503 in task text")).is_none()
    );
}
