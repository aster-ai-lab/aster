//! Opt-in, bounded observations of one test-owned workspace's native LSP IO.

use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, OnceLock, Weak},
    task::{Context, Poll},
    time::Instant,
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

const STREAM_LIMIT: usize = 64 * 1024;
const OBSERVATION_LIMIT: usize = 512;

#[derive(Clone, Copy)]
pub(crate) enum Stream {
    Stdin = 0,
    Stdout = 1,
    Stderr = 2,
}

#[derive(Clone, Default)]
pub(crate) struct CapturedStream {
    pub(crate) bytes: Vec<u8>,
    pub(crate) total_bytes: usize,
}

pub(crate) struct Capture {
    root: PathBuf,
    started: Instant,
    data: Mutex<TraceData>,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct Observation {
    pub(crate) stream: usize,
    pub(crate) offset_start: usize,
    pub(crate) offset_end: usize,
    pub(crate) elapsed_micros: u128,
}

#[derive(Default)]
struct TraceData {
    streams: [CapturedStream; 3],
    observations: Vec<Observation>,
    observations_truncated: bool,
}

fn captures() -> &'static Mutex<HashMap<PathBuf, Weak<Capture>>> {
    static CAPTURES: OnceLock<Mutex<HashMap<PathBuf, Weak<Capture>>>> = OnceLock::new();
    CAPTURES.get_or_init(Mutex::default)
}

impl Capture {
    pub(crate) fn register(canonical_root: &Path) -> Arc<Self> {
        let capture = Arc::new(Self {
            root: canonical_root.to_path_buf(),
            started: Instant::now(),
            data: Mutex::default(),
        });
        let previous = captures()
            .lock()
            .unwrap()
            .insert(capture.root.clone(), Arc::downgrade(&capture));
        assert!(previous.and_then(|previous| previous.upgrade()).is_none());
        capture
    }

    pub(crate) fn snapshot(&self) -> [CapturedStream; 3] {
        self.data.lock().unwrap().streams.clone()
    }

    pub(crate) fn observations(&self) -> (Vec<Observation>, bool) {
        let data = self.data.lock().unwrap();
        (data.observations.clone(), data.observations_truncated)
    }

    fn append(&self, stream: Stream, bytes: &[u8]) {
        let mut data = self.data.lock().unwrap();
        let target = &mut data.streams[stream as usize];
        let offset_start = target.total_bytes;
        target.total_bytes = target.total_bytes.saturating_add(bytes.len());
        let retain = bytes.len().min(STREAM_LIMIT - target.bytes.len());
        target.bytes.extend_from_slice(&bytes[..retain]);
        let offset_end = target.total_bytes;
        if data.observations.len() < OBSERVATION_LIMIT {
            data.observations.push(Observation {
                stream: stream as usize,
                offset_start,
                offset_end,
                elapsed_micros: self.started.elapsed().as_micros(),
            });
        } else {
            data.observations_truncated = true;
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        captures().lock().unwrap().remove(&self.root);
    }
}

fn for_root(root: &Path) -> Option<Arc<Capture>> {
    captures().lock().unwrap().get(root).and_then(Weak::upgrade)
}

pub(crate) struct ObservedRead<R> {
    inner: R,
    capture: Option<Arc<Capture>>,
    stream: Stream,
}

pub(crate) fn observe_read<R>(inner: R, root: &Path, stream: Stream) -> ObservedRead<R> {
    ObservedRead {
        inner,
        capture: for_root(root),
        stream,
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ObservedRead<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Ok(()))) {
            if let Some(capture) = &self.capture {
                capture.append(self.stream, &buffer.filled()[before..]);
            }
        }
        result
    }
}

pub(crate) struct ObservedWrite<W> {
    inner: W,
    capture: Option<Arc<Capture>>,
}

pub(crate) fn observe_write<W>(inner: W, root: &Path) -> ObservedWrite<W> {
    ObservedWrite {
        inner,
        capture: for_root(root),
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ObservedWrite<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, bytes);
        if let Poll::Ready(Ok(written)) = result {
            if let Some(capture) = &self.capture {
                capture.append(Stream::Stdin, &bytes[..written]);
            }
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[tokio::test]
async fn bounded_observation_preserves_all_transmitted_bytes() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let workspace = tempfile::tempdir().unwrap();
    let capture = Capture::register(workspace.path());
    let (reader, writer) = tokio::io::duplex(2048);
    let mut reader = observe_read(reader, workspace.path(), Stream::Stdout);
    let mut writer = observe_write(writer, workspace.path());
    let expected = vec![b'x'; STREAM_LIMIT * 16 + 17];
    let sent = expected.clone();
    let task = tokio::spawn(async move {
        writer.write_all(&sent).await.unwrap();
        writer.shutdown().await.unwrap();
    });
    let mut received = Vec::new();
    reader.read_to_end(&mut received).await.unwrap();
    task.await.unwrap();

    assert_eq!(received, expected);
    let streams = capture.snapshot();
    for stream in &streams[..2] {
        assert_eq!(stream.total_bytes, expected.len());
        assert_eq!(stream.bytes, expected[..STREAM_LIMIT]);
    }
    assert_eq!(streams[2].total_bytes, 0);
    let (observations, truncated) = capture.observations();
    assert_eq!(observations.len(), OBSERVATION_LIMIT);
    assert!(truncated);
    assert!(observations
        .windows(2)
        .all(|pair| pair[0].elapsed_micros <= pair[1].elapsed_micros));
}
