#![cfg(test)]

use super::*;
use std::time::Duration;

#[cfg(unix)]
fn alive(pid: &str) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", pid.trim()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[tokio::test]
#[cfg(unix)]
async fn timeout_reaps_the_exact_child_and_keeps_runtime_responsive() {
    let work = super::super::TempWork::create().unwrap();
    let pid_file = work.path.join("pid");
    let pid_arg = pid_file.to_str().unwrap();
    let started = Instant::now();
    let args = [
        "-c",
        "printf '%s' $$ > \"$1\"; exec sleep 60",
        "office-test",
        pid_arg,
    ];
    let job = run(
        Path::new("/bin/sh"),
        &args,
        started + Duration::from_millis(600),
        1024,
    );
    let heartbeat = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(started.elapsed() < Duration::from_millis(550));
    };
    let (result, ()) = tokio::join!(job, heartbeat);
    assert_eq!(result.unwrap_err(), "office-timeout");
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    assert!(
        !alive(&pid),
        "the owned hung child must be reaped before failure is reported"
    );
    let dir = work.path.clone();
    drop(work);
    assert!(!dir.exists());
}

#[tokio::test]
#[cfg(unix)]
async fn output_limit_kills_and_reaps_a_peer_that_never_closes_stdout() {
    let work = super::super::TempWork::create().unwrap();
    let pid_file = work.path.join("pid");
    let result = run(
        Path::new("/bin/sh"),
        &[
            "-c",
            "printf '%s' $$ > \"$1\"; exec /usr/bin/yes",
            "office-test",
            pid_file.to_str().unwrap(),
        ],
        Instant::now() + Duration::from_secs(5),
        1024,
    )
    .await;
    assert_eq!(result.unwrap_err(), "office-output-limit");
    assert!(!alive(&std::fs::read_to_string(pid_file).unwrap()));
}

#[tokio::test]
#[cfg(unix)]
async fn consumes_stderr_without_retaining_it_and_refuses_invalid_output() {
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = run(
        Path::new("/bin/sh"),
        &["-c", "head -c 100000 /dev/zero >&2; printf 'ok'"],
        deadline,
        2,
    )
    .await
    .unwrap();
    assert_eq!(result, "ok");
    assert_eq!(
        run(
            Path::new("/bin/sh"),
            &["-c", "printf '\\377'"],
            deadline,
            1024
        )
        .await
        .unwrap_err(),
        "unreadable"
    );
    assert_eq!(
        run(Path::new("/bin/sh"), &["-c", "exit 1"], deadline, 1024)
            .await
            .unwrap_err(),
        "unreadable"
    );
}

#[tokio::test]
#[cfg(unix)]
async fn expired_shared_deadline_refuses_the_next_process_before_spawn() {
    let work = super::super::TempWork::create().unwrap();
    let marker = work.path.join("marker");
    let result = run(
        Path::new("/bin/sh"),
        &[
            "-c",
            "touch \"$1\"",
            "office-test",
            marker.to_str().unwrap(),
        ],
        Instant::now(),
        1024,
    )
    .await;
    assert_eq!(result.unwrap_err(), "office-timeout");
    assert!(!marker.exists());
}
