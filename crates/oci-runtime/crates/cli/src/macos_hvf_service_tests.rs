#![cfg(test)]

use std::io::{self, Cursor, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use super::wait_for_stdin_eof;

#[test]
fn lifetime_pipe_flag_is_explicit_and_defaults_to_disabled() {
    for enabled in [false, true] {
        let mut args = vec![
            "a3s-oci",
            "macos-hvf-host-service",
            "--root",
            "/tmp/owner",
            "--shim",
            "/tmp/shim",
            "--system-image-manifest",
            "/tmp/system-image.json",
        ];
        if enabled {
            args.push("--exit-on-stdin-eof");
        }
        let cli = crate::try_parse_cli_for_test(&args).unwrap();
        let crate::Command::MacosHvfHostService {
            exit_on_stdin_eof, ..
        } = cli.command
        else {
            panic!("unexpected command");
        };
        assert_eq!(exit_on_stdin_eof, enabled);
    }
}

#[tokio::test]
async fn lifetime_pipe_waits_for_eof_after_discarding_input() {
    let (input, mut owner) = UnixStream::pair().unwrap();
    let mut waiting = tokio::spawn(wait_for_stdin_eof(input));
    owner.write_all(b"owner remains alive").unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(25), &mut waiting)
            .await
            .is_err()
    );
    drop(owner);
    tokio::time::timeout(Duration::from_secs(1), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn lifetime_pipe_error_is_returned_for_fail_closed_shutdown() {
    struct FailedRead;
    impl Read for FailedRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
    }
    let error = wait_for_stdin_eof(FailedRead).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}

#[tokio::test]
async fn lifetime_pipe_retries_interrupted_reads() {
    struct InterruptedOnce(Option<Cursor<Vec<u8>>>);
    impl Read for InterruptedOnce {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if let Some(input) = &mut self.0 {
                return input.read(bytes);
            }
            self.0 = Some(Cursor::new(b"alive".to_vec()));
            Err(io::Error::from(io::ErrorKind::Interrupted))
        }
    }
    wait_for_stdin_eof(InterruptedOnce(None)).await.unwrap();
}

#[tokio::test]
async fn cancelling_the_wait_does_not_hold_the_runtime_blocking_pool() {
    let (input, owner) = UnixStream::pair().unwrap();
    let waiting = tokio::spawn(wait_for_stdin_eof(input));
    tokio::task::yield_now().await;
    waiting.abort();
    assert!(waiting.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), tokio::task::spawn_blocking(|| 42))
        .await
        .unwrap()
        .unwrap();
    drop(owner);
}
