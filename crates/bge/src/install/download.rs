//! Verified, resumable downloads for manifest files.
//!
//! Source authenticity comes from the manifest's sha256 pin, not from a
//! host allowlist: the URL is a presigned object-store address the
//! operator's platform issued. The transfer still bounds itself to the
//! pinned byte length, resumes from the verified prefix on interruption,
//! and rejects any range response that does not continue exactly where
//! the previous attempt stopped.

use std::path::Path;
use std::time::Duration;

use aster_use_core::{UseError, UseResult};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::bge_error;

const DOWNLOAD_ATTEMPTS: usize = 8;
const DOWNLOAD_RETRY_DELAY: Duration = Duration::from_secs(2);
const DOWNLOAD_MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(super) struct Downloaded {
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

pub(super) fn client() -> UseResult<reqwest::Client> {
    let redirects = reqwest::redirect::Policy::custom(|attempt| {
        let approved = attempt.previous().len() < 5
            && matches!(attempt.url().scheme(), "http" | "https")
            && attempt.url().host_str().is_some();
        if approved {
            attempt.follow()
        } else {
            attempt.error("BGE model download redirected to an unusable host")
        }
    });
    reqwest::Client::builder()
        .user_agent(concat!("aster-use-bge/", env!("CARGO_PKG_VERSION")))
        .redirect(redirects)
        .connect_timeout(Duration::from_secs(30))
        // Bound stalled reads, not the complete transfer: a 2 GiB weight on
        // a healthy slow link legitimately takes more than a minute.
        .read_timeout(Duration::from_secs(90))
        .build()
        .map_err(|error| {
            bge_error(
                "use.bge.download_failed",
                format!("Failed to create the BGE download client: {error}"),
            )
        })
}

pub(super) async fn verified(
    client: &reqwest::Client,
    source: &str,
    destination: &Path,
    expected_bytes: u64,
    expected_sha256: &str,
) -> UseResult<Downloaded> {
    let url = reqwest::Url::parse(source).map_err(|error| {
        bge_error(
            "use.bge.download_source_invalid",
            format!("Invalid BGE model download URL: {error}"),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(bge_error(
            "use.bge.download_source_invalid",
            "BGE model download source must be a credential-free http(s) URL.",
        ));
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .await
        .map_err(|error| {
            bge_error(
                "use.bge.install_failed",
                format!(
                    "Failed to create the BGE download '{}': {error}",
                    destination.display()
                ),
            )
        })?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut completed = false;

    for attempt in 1..=DOWNLOAD_ATTEMPTS {
        if attempt > 1 {
            let multiplier = 1_u32 << (attempt - 2);
            tokio::time::sleep(
                DOWNLOAD_RETRY_DELAY
                    .saturating_mul(multiplier)
                    .min(DOWNLOAD_MAX_RETRY_DELAY),
            )
            .await;
        }

        let mut request = client
            .get(url.clone())
            .header(reqwest::header::ACCEPT_ENCODING, "identity");
        if total > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={total}-"));
        }
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(_error) if attempt < DOWNLOAD_ATTEMPTS => continue,
            Err(error) => {
                return Err(attempts_exhausted(
                    attempt,
                    total,
                    describe_reqwest_error(&error),
                ));
            }
        };

        let status = response.status();
        if status == reqwest::StatusCode::OK {
            if total > 0 {
                // The server ignored the range; the verified prefix cannot
                // be trusted against a fresh body, so restart from zero.
                file = tokio::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(destination)
                    .await
                    .map_err(|error| {
                        bge_error(
                            "use.bge.install_failed",
                            format!(
                                "Failed to restart the BGE download '{}': {error}",
                                destination.display()
                            ),
                        )
                    })?;
                total = 0;
                hasher = Sha256::new();
            }
        } else if status == reqwest::StatusCode::PARTIAL_CONTENT {
            validate_content_range(response.headers(), total, expected_bytes)?;
        } else if retryable_status(status) && attempt < DOWNLOAD_ATTEMPTS {
            continue;
        } else {
            return Err(bge_error(
                "use.bge.download_failed",
                format!("BGE model download failed with HTTP status {status}."),
            ));
        }

        if response.content_length().is_some_and(|length| {
            total
                .checked_add(length)
                .is_none_or(|response_end| response_end > expected_bytes)
        }) {
            return Err(bge_error(
                "use.bge.integrity_mismatch",
                "BGE model response exceeds the pinned file length.",
            ));
        }

        let mut interrupted = None;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    total = total.checked_add(chunk.len() as u64).ok_or_else(|| {
                        bge_error("use.bge.download_too_large", "Download size overflowed.")
                    })?;
                    if total > expected_bytes {
                        return Err(bge_error(
                            "use.bge.integrity_mismatch",
                            "BGE model response exceeds the pinned file length.",
                        ));
                    }
                    hasher.update(&chunk);
                    file.write_all(&chunk).await.map_err(|error| {
                        bge_error(
                            "use.bge.install_failed",
                            format!(
                                "Failed to write the BGE download '{}': {error}",
                                destination.display()
                            ),
                        )
                    })?;
                    if total == expected_bytes {
                        completed = true;
                        break;
                    }
                }
                Ok(None) => {
                    interrupted = Some(format!(
                        "response ended at byte {total} before the pinned {expected_bytes}-byte length"
                    ));
                    break;
                }
                Err(error) => {
                    interrupted = Some(describe_reqwest_error(&error));
                    break;
                }
            }
        }
        if completed {
            break;
        }
        if attempt == DOWNLOAD_ATTEMPTS {
            return Err(attempts_exhausted(
                attempt,
                total,
                interrupted.unwrap_or_else(|| "response was interrupted".to_string()),
            ));
        }
    }

    if !completed {
        return Err(attempts_exhausted(
            DOWNLOAD_ATTEMPTS,
            total,
            "response was interrupted".to_string(),
        ));
    }
    let sha256 = format!("{:x}", hasher.finalize());
    if sha256 != expected_sha256 {
        return Err(bge_error(
            "use.bge.integrity_mismatch",
            format!(
                "The download of '{}' hashed to {sha256}, expected {expected_sha256}.",
                destination.display()
            ),
        ));
    }
    file.flush().await.map_err(|error| {
        bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to flush the BGE download '{}': {error}",
                destination.display()
            ),
        )
    })?;
    file.sync_all().await.map_err(|error| {
        bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to sync the BGE download '{}': {error}",
                destination.display()
            ),
        )
    })?;
    Ok(Downloaded {
        bytes: total,
        sha256,
    })
}

fn retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn validate_content_range(
    headers: &reqwest::header::HeaderMap,
    expected_start: u64,
    expected_total: u64,
) -> UseResult<()> {
    let value = headers
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            bge_error(
                "use.bge.download_failed",
                "BGE model range response omitted Content-Range.",
            )
        })?;
    let (range, total) = value
        .strip_prefix("bytes ")
        .and_then(|value| value.split_once('/'))
        .ok_or_else(|| {
            bge_error(
                "use.bge.download_failed",
                "BGE model range response has an invalid Content-Range.",
            )
        })?;
    let (start, end) = range.split_once('-').ok_or_else(|| {
        bge_error(
            "use.bge.download_failed",
            "BGE model range response has an invalid byte range.",
        )
    })?;
    let start = start.parse::<u64>().map_err(|_| {
        bge_error(
            "use.bge.download_failed",
            "BGE model range response has an invalid start offset.",
        )
    })?;
    let end = end.parse::<u64>().map_err(|_| {
        bge_error(
            "use.bge.download_failed",
            "BGE model range response has an invalid end offset.",
        )
    })?;
    let total = total.parse::<u64>().map_err(|_| {
        bge_error(
            "use.bge.download_failed",
            "BGE model range response has an invalid total length.",
        )
    })?;
    if start != expected_start || end < start || end >= total || total != expected_total {
        return Err(bge_error(
            "use.bge.download_failed",
            format!(
                "BGE model range response '{value}' does not continue byte {expected_start} of the pinned {expected_total}-byte file."
            ),
        ));
    }
    Ok(())
}

fn describe_reqwest_error(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        let cause_message = cause.to_string();
        if !message.contains(&cause_message) {
            message.push_str(": ");
            message.push_str(&cause_message);
        }
        source = std::error::Error::source(cause);
    }
    message
}

fn attempts_exhausted(attempts: usize, offset: u64, cause: String) -> UseError {
    bge_error(
        "use.bge.download_failed",
        format!("Failed to read the BGE model download after {attempts} attempts at byte {offset}: {cause}"),
    )
}
