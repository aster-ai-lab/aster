use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

use super::*;

/// Records its arguments; the image picks what the "box" does.
const FAKE_BOX: &str = r#"#!/bin/sh
trap 'on_interrupt' INT
on_interrupt() {
  kill "$sleeper" 2> /dev/null
  [ "$image" = fake-stubborn ] && return
  echo "Stopping box"
  exit 130
}
dir=$(dirname "$0")
case "$1" in
  warm) exit 0 ;;
  kill|rm) echo "$*" >> "$dir/stops"; exit 0 ;;
esac
printf '%s\n' "$@" > "$dir/args"
image=""; previous=""
for arg in "$@"; do
  if [ "$arg" = "--" ]; then image="$previous"; break; fi
  previous="$arg"
done
case "$image" in
  fake-ok) printf 'out \303\251\n'; echo "err line" >&2; exit 0 ;;
  fake-exit-3) echo failing; exit 3 ;;
  fake-sleep|fake-stubborn)
    while :; do
      sleep 30 > /dev/null 2>&1 &
      sleeper=$!
      wait "$sleeper"
    done ;;
esac
exit 99
"#;

fn settings(image: &str) -> BoxSettings {
    BoxSettings {
        image: image.into(),
        cpus: 2,
        memory: "2g".into(),
    }
}

/// Run once before use: macOS assesses a new executable on its first exec,
/// which under load can outlast a timed test's interrupt.
fn fake_box(dir: &Path) -> PathBuf {
    let path = dir.join("aster-box");
    std::fs::write(&path, FAKE_BOX).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let warmed = std::process::Command::new(&path)
        .arg("warm")
        .status()
        .unwrap();
    assert!(warmed.success());
    path
}

fn cancel() -> (watch::Sender<bool>, watch::Sender<bool>, Cancel) {
    let (ended_tx, ended) = watch::channel(false);
    let (shutdown_tx, shutdown) = watch::channel(false);
    (ended_tx, shutdown_tx, Cancel { ended, shutdown })
}

async fn run_box(
    dir: &Path,
    image: &str,
    deadline: Duration,
    cancel: Cancel,
) -> (Result<StepExit, String>, String) {
    let executor = BoxExecutor::new(fake_box(dir), settings(image))
        .with_stop_grace(Duration::from_millis(800));
    let checkout = dir.join("checkout");
    let env_dir = dir.join("env");
    std::fs::create_dir_all(&checkout).unwrap();
    std::fs::create_dir_all(&env_dir).unwrap();
    let step = StepRun {
        box_name: "aster-job-0b9d6c0e-1".into(),
        checkout: &checkout,
        env_dir: &env_dir,
        deadline: Instant::now() + deadline,
    };
    let (output, mut received) = mpsc::unbounded_channel();
    let exit = executor.run(&step, output, cancel).await;
    let mut text = String::new();
    while let Ok(chunk) = received.try_recv() {
        text.push_str(&chunk);
    }
    (exit, text)
}

#[test]
fn the_box_command_is_constant_and_mounts_the_job() {
    let step = StepRun {
        box_name: "aster-job-0b9d6c0e-2".into(),
        checkout: Path::new("/work/job/checkout"),
        env_dir: Path::new("/work/job/env"),
        deadline: Instant::now(),
    };
    let args = box_run_arguments(&settings("alpine:3.20"), &step, 610).unwrap();
    assert_eq!(
        args,
        [
            "run",
            "--rm",
            "--no-stdin",
            "--log-driver",
            "none",
            "--name",
            "aster-job-0b9d6c0e-2",
            "--cpus",
            "2",
            "--memory",
            "2g",
            "--timeout",
            "610",
            "-v",
            "/work/job/checkout:/workspace",
            "-v",
            "/work/job/env:/run/aster",
            "-w",
            "/workspace",
            "alpine:3.20",
            "--",
            "sh",
            "-c",
            ". /run/aster/step.sh && exec sh -c \"$ASTER_STEP\"",
        ]
    );
    let colon = StepRun {
        checkout: Path::new("/work/a:b"),
        ..step
    };
    assert!(box_run_arguments(&settings("alpine:3.20"), &colon, 1).is_err());
    let relative = StepRun {
        checkout: Path::new("work/job"),
        ..colon
    };
    assert!(box_run_arguments(&settings("alpine:3.20"), &relative, 1).is_err());
}

#[test]
fn the_step_script_quotes_every_value() {
    let script = render_step_script(&[
        ("DEPLOY_TOKEN".into(), "s3cr'et $HOME `id`".into()),
        ("MULTI".into(), "line one\nline two".into()),
        ("ASTER_STEP".into(), "echo \"$DEPLOY_TOKEN\"".into()),
    ])
    .unwrap();
    assert_eq!(
        script,
        "export DEPLOY_TOKEN='s3cr'\\''et $HOME `id`'\n\
         export MULTI='line one\nline two'\n\
         export ASTER_STEP='echo \"$DEPLOY_TOKEN\"'\n"
    );
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "{script}printf '%s|%s' \"$DEPLOY_TOKEN\" \"$MULTI\""
        ))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "s3cr'et $HOME `id`|line one\nline two"
    );
    for name in ["1ABC", "A-B", "", "A B"] {
        assert!(
            render_step_script(&[(name.into(), "x".into())]).is_err(),
            "{name}"
        );
    }
    assert!(render_step_script(&[("NUL".into(), "a\0b".into())]).is_err());
}

#[test]
fn the_step_script_is_owner_only_and_replaced() {
    let dir = tempfile::tempdir().unwrap();
    write_step_script(dir.path(), "export A='1'\n").unwrap();
    write_step_script(dir.path(), "export A='2'\n").unwrap();
    let path = dir.path().join(STEP_SCRIPT);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "export A='2'\n");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn a_split_character_waits_for_its_bytes() {
    let mut carry = "a\u{e9}".as_bytes()[..2].to_vec();
    assert_eq!(take_utf8(&mut carry), "a");
    assert_eq!(carry, [0xc3]);
    carry.push(0xa9);
    assert_eq!(take_utf8(&mut carry), "\u{e9}");
    assert!(carry.is_empty());
    let mut invalid = vec![b'x', 0xff, b'y'];
    assert_eq!(take_utf8(&mut invalid), "x\u{fffd}y");
}

#[test]
fn the_box_cli_resolves_to_the_real_binary() {
    let dir = tempfile::tempdir().unwrap();
    let real = fake_box(dir.path());
    let link = dir.path().join("linked-box");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(
        resolve_box_cli(Some(&link)).unwrap(),
        std::fs::canonicalize(&real).unwrap()
    );
    assert!(resolve_box_cli(Some(&dir.path().join("missing"))).is_err());
}

#[tokio::test]
async fn a_step_streams_both_outputs_and_reports_its_exit() {
    let dir = tempfile::tempdir().unwrap();
    let (_ended, _shutdown, signal) = cancel();
    let (exit, text) = run_box(dir.path(), "fake-ok", Duration::from_secs(60), signal).await;
    assert_eq!(exit.unwrap(), StepExit::Exited(0));
    assert!(text.contains("out \u{e9}\n"), "{text}");
    assert!(text.contains("err line\n"), "{text}");
    let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
    assert!(args.starts_with("run\n--rm\n--no-stdin\n"), "{args}");
    assert!(args.contains("--timeout\n6"), "{args}");
    assert!(!dir.path().join("stops").exists());

    let dir = tempfile::tempdir().unwrap();
    let (_ended, _shutdown, signal) = cancel();
    let (exit, text) = run_box(dir.path(), "fake-exit-3", Duration::from_secs(60), signal).await;
    assert_eq!(exit.unwrap(), StepExit::Exited(3));
    assert_eq!(text, "failing\n");
}

#[tokio::test]
async fn the_deadline_interrupts_the_box() {
    let dir = tempfile::tempdir().unwrap();
    let (_ended, _shutdown, signal) = cancel();
    let started = Instant::now();
    let (exit, text) = run_box(dir.path(), "fake-sleep", Duration::from_millis(500), signal).await;
    assert_eq!(exit.unwrap(), StepExit::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(text.contains("Stopping box"), "{text}");
    assert!(!dir.path().join("stops").exists());

    let dir = tempfile::tempdir().unwrap();
    let (_ended, _shutdown, signal) = cancel();
    let (exit, _) = run_box(dir.path(), "fake-ok", Duration::ZERO, signal).await;
    assert_eq!(exit.unwrap(), StepExit::TimedOut);
    assert!(!dir.path().join("args").exists());
}

#[tokio::test]
async fn a_cancel_stops_the_box() {
    for ended in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let (ended_tx, shutdown_tx, signal) = cancel();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let sender = if ended { ended_tx } else { shutdown_tx };
            let _ = sender.send(true);
            tokio::time::sleep(Duration::from_secs(30)).await;
        });
        let (exit, text) = run_box(dir.path(), "fake-sleep", Duration::from_secs(60), signal).await;
        assert_eq!(exit.unwrap(), StepExit::Cancelled);
        assert!(text.contains("Stopping box"), "{text}");
        assert!(!dir.path().join("stops").exists());
    }
}

#[tokio::test]
async fn a_box_that_ignores_ctrl_c_is_killed() {
    let dir = tempfile::tempdir().unwrap();
    let (_ended, _shutdown, signal) = cancel();
    let started = Instant::now();
    let (exit, _) = run_box(
        dir.path(),
        "fake-stubborn",
        Duration::from_millis(300),
        signal,
    )
    .await;
    assert_eq!(exit.unwrap(), StepExit::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
    let stops = std::fs::read_to_string(dir.path().join("stops")).unwrap();
    assert_eq!(stops, "kill aster-job-0b9d6c0e-1\nrm -f aster-job-0b9d6c0e-1\n");
}
