use super::*;
use futures_util::TryStreamExt;

async fn read(path: &std::path::Path) -> Vec<u8> {
    NativeMetadataTransport
        .fetch(Url::from_file_path(path).unwrap())
        .await
        .unwrap()
        .try_fold(Vec::new(), |mut all, bytes| {
            all.extend_from_slice(&bytes);
            std::future::ready(Ok(all))
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn metadata_urls_decode_native_paths_once_and_preserve_missing_file_classification() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("目录 with spaces %25 #");
    tokio::fs::create_dir(&directory).await.unwrap();
    let path = directory.join("timestamp.json");
    tokio::fs::write(&path, b"\x00native bytes\xff")
        .await
        .unwrap();
    assert_eq!(read(&path).await, b"\x00native bytes\xff");
    let missing = NativeMetadataTransport
        .fetch(Url::from_file_path(directory.join("2.root.json")).unwrap())
        .await
        .err()
        .unwrap();
    assert_eq!(missing.kind(), TransportErrorKind::FileNotFound);
    let remote = NativeMetadataTransport
        .fetch(Url::parse("https://example.org/timestamp.json").unwrap())
        .await
        .err()
        .unwrap();
    assert_eq!(remote.kind(), TransportErrorKind::UnsupportedUrlScheme);
}

// This macOS filesystem rejects non-UTF-8 names; Linux retains their native bytes.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn local_metadata_root_keeps_non_utf8_filesystem_identity() {
    use std::os::unix::ffi::OsStringExt;
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary
        .path()
        .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
    tokio::fs::create_dir(&directory).await.unwrap();
    let path = directory.join("targets.json");
    tokio::fs::write(&path, b"native identity").await.unwrap();
    assert_eq!(read(&path).await, b"native identity");
}

#[cfg(unix)]
#[tokio::test]
async fn unavailable_non_utf8_metadata_keeps_the_native_io_error() {
    use std::error::Error as _;
    use std::os::unix::ffi::OsStringExt;
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary
        .path()
        .join(std::ffi::OsString::from_vec(b"missing-\xff".to_vec()));
    let error = NativeMetadataTransport
        .fetch(Url::from_file_path(path).unwrap())
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error.kind(),
        TransportErrorKind::FileNotFound | TransportErrorKind::Other
    ));
    assert!(error
        .source()
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .is_some());
}

#[tokio::test]
async fn local_metadata_reads_refuse_bytes_above_the_tuf_metadata_bound() {
    use std::error::Error as _;
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("targets.json");
    let file = tokio::fs::File::create(&path).await.unwrap();
    file.set_len(MAX_TARGETS_METADATA_BYTES + 1).await.unwrap();
    drop(file);
    let error = NativeMetadataTransport
        .fetch(Url::from_file_path(path).unwrap())
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind(), TransportErrorKind::Other);
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
}
