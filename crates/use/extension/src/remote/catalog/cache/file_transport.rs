//! Local TUF metadata with native file-URL decoding, including spaces and non-UTF-8 paths.

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tough::{Transport, TransportError, TransportErrorKind, TransportStream};
use url::Url;

use super::MAX_TARGETS_METADATA_BYTES;

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeMetadataTransport;

#[async_trait]
impl Transport for NativeMetadataTransport {
    async fn fetch(&self, url: Url) -> Result<TransportStream, TransportError> {
        // Tough 0.24's file transport uses the escaped URL path as a filesystem path.
        let path = url.to_file_path().map_err(|()| {
            TransportError::new(TransportErrorKind::UnsupportedUrlScheme, url.as_str())
        })?;
        let read = async {
            let mut bytes = Vec::new();
            tokio::fs::File::open(&path)
                .await?
                .take(MAX_TARGETS_METADATA_BYTES + 1)
                .read_to_end(&mut bytes)
                .await?;
            if bytes.len() as u64 > MAX_TARGETS_METADATA_BYTES {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Cached TUF metadata exceeds its byte limit.",
                ));
            }
            Ok(bytes)
        }
        .await;
        let bytes = read.map_err(|error: std::io::Error| {
            let kind = match error.kind() {
                std::io::ErrorKind::NotFound => TransportErrorKind::FileNotFound,
                _ => TransportErrorKind::Other,
            };
            TransportError::new_with_cause(kind, url.as_str(), error)
        })?;
        Ok(Box::pin(futures_util::stream::once(std::future::ready(
            Ok(bytes.into()),
        ))))
    }
}

#[cfg(test)]
mod tests;
