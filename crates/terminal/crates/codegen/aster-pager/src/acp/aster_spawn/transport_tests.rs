#![cfg(all(test, unix))]

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use super::*;

async fn exercise(script: &str, must_refuse: bool) {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("external-agent");
    std::fs::write(&bin, format!("#!/bin/sh\ncd \"$(dirname \"$0\")\" || exit 1\nprintf '%s' $$ > pid\n{script}\nexec /bin/cat\n")).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cancel = CancellationToken::new();
    let (_client, channel) = acp_channels();
    let handle = spawn_bridge(bin, Vec::new(), cancel.clone(), channel).unwrap();
    let started = Instant::now();
    if must_refuse {
        while !handle.is_finished() && started.elapsed() < Duration::from_secs(30) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    } else {
        while !dir.path().join("ready").exists() && started.elapsed() < Duration::from_secs(10) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    let refused_before_cancel = handle.is_finished();
    cancel.cancel();
    let result = tokio::task::spawn_blocking(move || handle.join())
        .await
        .unwrap()
        .unwrap();
    let pid: i32 = std::fs::read_to_string(dir.path().join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(pid > 0);
    // This PID belongs to the fixture, and the joined bridge must have reaped it.
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "fixture survived bridge shutdown"
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    if must_refuse {
        assert!(
            refused_before_cancel,
            "oversized unterminated stdout bypassed the frame limit"
        );
        let error = format!("{:#}", result.unwrap_err());
        assert!(
            error.contains("exceeds") && error.contains("byte limit"),
            "{error}"
        );
    } else {
        assert!(dir.path().join("ready").exists(), "fixture did not start");
        result.unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "partial frame blocked cancellation"
        );
    }
}

#[tokio::test]
async fn oversized_stdout_without_a_newline_is_refused() {
    exercise(
        r#"exec /usr/bin/awk 'BEGIN { for (i=0;i<8192;i++) chunk=chunk "x"; for (i=0;i<8320;i++) printf "%s",chunk; fflush(); getline }'"#,
        true,
    )
    .await;
}

#[tokio::test]
async fn cancellation_joins_a_bridge_waiting_for_a_partial_frame() {
    exercise("printf '{\"jsonrpc\":'; : > ready", false).await;
}

/// Pager teardown cancels, then drops client channels. A bridge that sees the
/// transport close in the same wakeup as cancel must still join cleanly.
#[tokio::test]
async fn cancel_overlapping_client_drop_joins_ok() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("external-agent");
    // Same fixture shape as `exercise`: ready marker then block on stdin.
    std::fs::write(
        &bin,
        format!(
            "#!/bin/sh\ncd \"$(dirname \"$0\")\" || exit 1\nprintf '%s' $$ > pid\nprintf '{{\"jsonrpc\":'; : > ready\nexec /bin/cat\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cancel = CancellationToken::new();
    let (client, channel) = acp_channels();
    let handle = spawn_bridge(bin, Vec::new(), cancel.clone(), channel).unwrap();
    let started = Instant::now();
    while !dir.path().join("ready").exists() && started.elapsed() < Duration::from_secs(10) {
        if handle.is_finished() {
            let result = tokio::task::spawn_blocking(move || handle.join())
                .await
                .unwrap()
                .unwrap();
            panic!("bridge exited before fixture ready: {result:?}");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(dir.path().join("ready").exists(), "fixture did not start");
    // Same order as BridgeShutdownCancel then AppView/acp_rx Drop.
    cancel.cancel();
    drop(client);
    let result = tokio::task::spawn_blocking(move || handle.join())
        .await
        .unwrap()
        .unwrap();
    result.expect("intentional teardown must not surface as bridge failure");
    let pid: i32 = std::fs::read_to_string(dir.path().join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
}
