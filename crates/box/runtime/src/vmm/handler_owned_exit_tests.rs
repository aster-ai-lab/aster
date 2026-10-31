#![cfg(test)]
#![cfg(unix)]

use super::*;
use aster_box_core::vmm::VmExitEvidenceKind;
use std::ops::{Deref, DerefMut};
use std::process::Command;

// Only test-spawned bounded children are ever signalled. This guard also runs
// after assertion failure and does not depend on the syscall fault seam.
struct OwnedFixture(ShimHandler);

impl OwnedFixture {
    fn sleeping() -> Self {
        Self(ShimHandler::from_child(
            Command::new("sleep").arg("10").spawn().unwrap(),
            "owned-close-fixture".to_string(),
        ))
    }

    fn assert_retained(&self, original_pid: u32) {
        assert_eq!(self.process.as_ref().unwrap().id(), original_pid);
        assert!(self.exit_confirmation().is_none());
    }
}

impl Deref for OwnedFixture {
    type Target = ShimHandler;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for OwnedFixture {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for OwnedFixture {
    fn drop(&mut self) {
        if let Some(child) = self.process.as_mut() {
            let _ = child.kill();
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) | Err(_) => break,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Ok(None) => break,
                }
            }
        }
    }
}

// This seam never manufactures a successful status. It leaves the real Child
// alive while controlling only errors and elapsed time; successes use real OS
// waits in the separate tests below.
struct FaultOps {
    now: Instant,
    poll_error_at: Option<usize>,
    signal_error: Option<i32>,
    kill_error: bool,
    polls: usize,
    signals: usize,
    kills: usize,
    sleeps: Vec<Duration>,
}

impl FaultOps {
    fn at(now: Instant) -> Self {
        Self {
            now,
            poll_error_at: None,
            signal_error: None,
            kill_error: false,
            polls: 0,
            signals: 0,
            kills: 0,
            sleeps: Vec::new(),
        }
    }

    fn counts(&self) -> (usize, usize, usize) {
        (self.polls, self.signals, self.kills)
    }
}

impl OwnedChildOps for FaultOps {
    fn now(&mut self) -> Instant {
        self.now
    }

    fn sleep(&mut self, duration: Duration) {
        self.sleeps.push(duration);
        self.now += duration;
    }

    fn poll(&mut self, _process: &mut Child) -> std::io::Result<Option<ExitStatus>> {
        self.polls += 1;
        if self.poll_error_at == Some(self.polls) {
            Err(std::io::Error::other("injected owned wait syscall failure"))
        } else {
            Ok(None)
        }
    }

    fn signal(&mut self, _process: &mut Child, _signal: i32) -> std::io::Result<()> {
        self.signals += 1;
        match self.signal_error {
            Some(error) => Err(std::io::Error::from_raw_os_error(error)),
            None => Ok(()),
        }
    }

    fn force_kill(&mut self, _process: &mut Child) -> std::io::Result<()> {
        self.kills += 1;
        if self.kill_error {
            Err(std::io::Error::other("injected owned kill syscall failure"))
        } else {
            Ok(())
        }
    }
}

fn budget(now: Instant, grace: Duration) -> OwnedCloseBudget {
    OwnedCloseBudget::from_admission(now, grace).unwrap()
}

#[test]
fn owned_signal_error_preserves_child_and_does_not_fall_through_to_kill() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let now = Instant::now();
    let mut ops = FaultOps::at(now);
    ops.signal_error = Some(libc::EPERM);

    let error = handler
        .stop_owned_with(libc::SIGTERM, budget(now, Duration::ZERO), &mut ops)
        .unwrap_err();

    assert!(error.to_string().contains("Failed to signal owned"));
    assert_eq!(ops.counts(), (1, 1, 0));
    handler.assert_retained(pid);
}

#[test]
fn owned_initial_poll_error_preserves_child_without_signalling() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let now = Instant::now();
    let mut ops = FaultOps::at(now);
    ops.poll_error_at = Some(1);

    let error = handler
        .stop_owned_with(libc::SIGTERM, budget(now, Duration::ZERO), &mut ops)
        .unwrap_err();

    assert!(error.to_string().contains("Failed to poll owned"));
    assert_eq!(ops.counts(), (1, 0, 0));
    handler.assert_retained(pid);
}

#[test]
fn owned_grace_poll_error_is_not_success_or_forced_cleanup() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let now = Instant::now();
    let mut ops = FaultOps::at(now);
    ops.poll_error_at = Some(2);

    let error = handler
        .stop_owned_with(libc::SIGTERM, budget(now, Duration::from_secs(1)), &mut ops)
        .unwrap_err();

    assert!(error.to_string().contains("Failed to poll owned"));
    assert_eq!(ops.counts(), (2, 1, 0));
    handler.assert_retained(pid);
}

#[test]
fn owned_kill_error_preserves_child_without_reporting_wait_success() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let now = Instant::now();
    let mut ops = FaultOps::at(now);
    ops.kill_error = true;

    let error = handler
        .stop_owned_with(libc::SIGTERM, budget(now, Duration::ZERO), &mut ops)
        .unwrap_err();

    assert!(error.to_string().contains("Failed to kill owned"));
    assert_eq!(ops.counts(), (2, 1, 1));
    handler.assert_retained(pid);
}

#[test]
fn owned_force_wait_error_preserves_child_and_does_not_hide_failure() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let now = Instant::now();
    let mut ops = FaultOps::at(now);
    ops.poll_error_at = Some(3);

    let error = handler
        .stop_owned_with(libc::SIGTERM, budget(now, Duration::ZERO), &mut ops)
        .unwrap_err();

    assert!(error.to_string().contains("Failed to wait for owned"));
    assert_eq!(ops.counts(), (3, 1, 1));
    handler.assert_retained(pid);
}

#[test]
fn owned_esrch_signal_race_requires_actual_wait_before_completion() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let now = Instant::now();
    let mut ops = FaultOps::at(now);
    ops.signal_error = Some(libc::ESRCH);

    assert!(handler
        .stop_owned_with(libc::SIGTERM, budget(now, Duration::ZERO), &mut ops)
        .is_err());
    assert_eq!(ops.counts(), (1, 1, 0));
    handler.assert_retained(pid);
}

#[test]
fn owned_short_force_remainder_is_not_a_new_five_second_grant() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let admitted = Instant::now();
    let original = budget(admitted, Duration::ZERO);
    let now = original.physical_deadline() - Duration::from_millis(2);
    let mut ops = FaultOps::at(now);

    let error = handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .unwrap_err();

    assert!(error.to_string().contains("original physical deadline"));
    assert_eq!(ops.now, original.physical_deadline());
    assert_eq!(ops.sleeps, [Duration::from_millis(2)]);
    assert_eq!(ops.counts(), (3, 1, 1));
    handler.assert_retained(pid);
    let counts = ops.counts();
    assert!(handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .is_err());
    assert_eq!(ops.counts(), counts);
}

#[test]
fn owned_grace_and_force_share_exact_original_deadline() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let admitted = Instant::now();
    let original = budget(admitted, Duration::from_millis(17));
    let mut ops = FaultOps::at(admitted);

    assert!(handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .is_err());
    assert_eq!(ops.now, original.physical_deadline());
    assert_eq!(
        ops.sleeps.iter().sum::<Duration>(),
        Duration::from_millis(5_017)
    );
    assert_eq!(ops.sleeps.first(), Some(&Duration::from_millis(17)));
    assert!(ops
        .sleeps
        .iter()
        .all(|sleep| *sleep <= Duration::from_millis(50)));
    assert_eq!(ops.signals, 1);
    assert_eq!(ops.kills, 1);
    handler.assert_retained(pid);
}

fn observe_actual_exit(handler: &mut ShimHandler) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        handler.try_wait_exit().unwrap();
        if handler.exit_confirmation().is_some() {
            return;
        }
        assert!(Instant::now() < deadline, "owned fixture failed to exit");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn owned_expired_attempt_stays_failed_after_later_actual_wait_and_explicit_retry() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let admitted = Instant::now();
    let original = budget(admitted, Duration::ZERO);
    let mut ops = FaultOps::at(original.physical_deadline());

    assert!(handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .is_err());
    assert_eq!(ops.counts(), (0, 0, 0));
    handler.assert_retained(pid);

    handler.process.as_mut().unwrap().kill().unwrap();
    observe_actual_exit(&mut handler);
    let proof = handler.exit_confirmation().unwrap();
    assert_eq!(proof.kind(), VmExitEvidenceKind::OwnedReaped);
    assert_eq!(proof.pid(), pid);
    assert!(proof.status().is_some());
    assert!(handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .is_err());
    assert_eq!(ops.counts(), (0, 0, 0));

    // This is a distinct explicit attempt, not a rewrite/reset of original.
    let retry = budget(ops.now, Duration::ZERO);
    handler
        .stop_owned_with(libc::SIGTERM, retry, &mut ops)
        .unwrap();
    assert_eq!(ops.counts(), (0, 0, 0));
    assert_eq!(handler.process.as_ref().unwrap().id(), pid);
}

#[test]
fn owned_real_invalid_signal_error_retains_the_live_original_child() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    let error = handler.stop(-1, 10).unwrap_err();
    assert!(error.to_string().contains("Failed to signal owned"));
    handler.assert_retained(pid);
    assert!(handler
        .process
        .as_mut()
        .unwrap()
        .try_wait()
        .unwrap()
        .is_none());
}

#[test]
fn owned_real_signal_reap_without_numeric_code_is_idempotent_and_never_resignalled() {
    let mut handler = OwnedFixture::sleeping();
    let pid = handler.pid();
    handler.stop(libc::SIGKILL, 200).unwrap();

    let proof = handler.exit_confirmation().unwrap();
    assert_eq!(proof.kind(), VmExitEvidenceKind::OwnedReaped);
    assert_eq!(proof.pid(), pid);
    assert_eq!(proof.status().unwrap().code(), None);
    assert_eq!(handler.exit_code(), None);
    assert_eq!(handler.try_wait_exit().unwrap(), None);
    assert_eq!(handler.process.as_ref().unwrap().id(), pid);

    let now = Instant::now();
    let mut ops = FaultOps::at(now);
    ops.poll_error_at = Some(1);
    ops.signal_error = Some(libc::EPERM);
    ops.kill_error = true;
    handler
        .stop_owned_with(libc::SIGTERM, budget(now, Duration::ZERO), &mut ops)
        .unwrap();
    assert_eq!(ops.counts(), (0, 0, 0));
}

#[test]
fn owned_natural_exit_uses_actual_child_status_before_any_signal() {
    let mut child = Command::new("sh").args(["-c", "exit 7"]).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(status.code(), Some(7));
    let pid = child.id();
    let mut handler = OwnedFixture(ShimHandler::from_child(child, "natural-exit".to_string()));
    assert!(handler.exit_confirmation().is_none());
    // An invalid signal would fail if stop did not first consult its Child.
    handler.stop(-1, 0).unwrap();
    assert_eq!(handler.exit_code(), Some(7));
    assert_eq!(handler.exit_confirmation().unwrap().pid(), pid);
    assert_eq!(
        handler
            .exit_confirmation()
            .unwrap()
            .status()
            .unwrap()
            .code(),
        Some(7)
    );
}

#[test]
fn budgeted_attached_close_remains_unsupported_and_never_claims_owned_reap() {
    let mut handler = ShimHandler::from_pid(999_999_999, "unqualified-attached".to_string());
    let error = handler
        .stop_with_budget(libc::SIGTERM, budget(Instant::now(), Duration::ZERO))
        .unwrap_err();
    assert!(error.to_string().contains("stable identity control"));
    assert!(handler.process.is_none());
    assert!(handler.exit_confirmation().is_none());
}

struct ExitRaceOps {
    system: SystemOwnedChildOps,
    close_after_first_poll: bool,
    expire_after_reap: Option<Instant>,
    now: Instant,
    signals: usize,
}

impl OwnedChildOps for ExitRaceOps {
    fn now(&mut self) -> Instant {
        self.now
    }

    fn sleep(&mut self, duration: Duration) {
        self.system.sleep(duration);
        self.now += duration;
    }

    fn poll(&mut self, child: &mut Child) -> std::io::Result<Option<ExitStatus>> {
        let status = self.system.poll(child)?;
        if status.is_some() {
            if let Some(deadline) = self.expire_after_reap {
                self.now = deadline;
            }
        }
        Ok(status)
    }

    fn signal(&mut self, child: &mut Child, _signal: i32) -> std::io::Result<()> {
        self.signals += 1;
        if self.close_after_first_poll {
            // EOF permits this exact original child to exit naturally between
            // the handler's initial poll and the injected signal failure.
            drop(child.stdin.take());
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if child.try_wait()?.is_some() {
                    return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        panic!("an already reaped Child must never be signalled")
    }

    fn force_kill(&mut self, _child: &mut Child) -> std::io::Result<()> {
        panic!("natural-exit fixture must never be force-killed")
    }
}

#[test]
fn owned_actual_natural_exit_racing_signal_error_remains_observable_for_retry() {
    let child = Command::new("sh")
        .args(["-c", "read ignored; exit 7"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut handler = OwnedFixture(ShimHandler::from_child(child, "exit-race".to_string()));
    let pid = handler.pid();
    let now = Instant::now();
    let original = budget(now, Duration::ZERO);
    let mut ops = ExitRaceOps {
        system: SystemOwnedChildOps,
        close_after_first_poll: true,
        expire_after_reap: None,
        now,
        signals: 0,
    };
    assert!(handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .is_err());
    handler.assert_retained(pid);
    assert_eq!(ops.signals, 1);
    assert_eq!(handler.try_wait_exit().unwrap(), Some(7));
    assert_eq!(
        handler
            .exit_confirmation()
            .unwrap()
            .status()
            .unwrap()
            .code(),
        Some(7)
    );
    assert!(!handler.is_running());
    handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .unwrap();
    assert_eq!(ops.signals, 1);
}

#[test]
fn owned_late_actual_reap_cannot_rewrite_expired_attempt_as_success() {
    let mut child = Command::new("sh").args(["-c", "exit 7"]).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut handler = OwnedFixture(ShimHandler::from_child(child, "late-reap".to_string()));
    let pid = handler.pid();
    let now = Instant::now();
    let original = budget(now, Duration::ZERO);
    let mut ops = ExitRaceOps {
        system: SystemOwnedChildOps,
        close_after_first_poll: false,
        expire_after_reap: Some(original.physical_deadline()),
        now,
        signals: 0,
    };
    assert!(handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .is_err());
    handler.assert_retained(pid);
    assert_eq!(handler.try_wait_exit().unwrap(), Some(7));
    assert!(handler.exit_confirmation().is_some());
    assert!(handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .is_err());
    assert_eq!(ops.signals, 0);
}

struct RealForceOps {
    system: SystemOwnedChildOps,
    signals: usize,
    kills: usize,
}

impl OwnedChildOps for RealForceOps {
    fn now(&mut self) -> Instant {
        self.system.now()
    }

    fn sleep(&mut self, duration: Duration) {
        self.system.sleep(duration);
    }

    fn poll(&mut self, child: &mut Child) -> std::io::Result<Option<ExitStatus>> {
        self.system.poll(child)
    }

    fn signal(&mut self, child: &mut Child, signal: i32) -> std::io::Result<()> {
        self.signals += 1;
        self.system.signal(child, signal)
    }

    fn force_kill(&mut self, child: &mut Child) -> std::io::Result<()> {
        self.kills += 1;
        self.system.force_kill(child)
    }
}

#[test]
fn owned_real_force_kill_waits_for_original_child_and_retains_signal_status() {
    use std::os::unix::process::ExitStatusExt;
    let temp = tempfile::TempDir::new().unwrap();
    let ready = temp.path().join("ready");
    let child = Command::new("sh")
        .args([
            "-c",
            "trap '' TERM; printf ready > \"$1\"; exec sleep 10",
            "owned-force-fixture",
        ])
        .arg(&ready)
        .spawn()
        .unwrap();
    let mut handler = OwnedFixture(ShimHandler::from_child(child, "force-wait".to_string()));
    let pid = handler.pid();
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    while std::fs::read(&ready).ok().as_deref() != Some(b"ready") {
        assert!(Instant::now() < ready_deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut ops = RealForceOps {
        system: SystemOwnedChildOps,
        signals: 0,
        kills: 0,
    };
    let original = budget(Instant::now(), Duration::ZERO);
    handler
        .stop_owned_with(libc::SIGTERM, original, &mut ops)
        .unwrap();

    assert_eq!(ops.signals, 1);
    assert_eq!(ops.kills, 1);
    assert!(Instant::now() < original.physical_deadline());
    assert_eq!(handler.process.as_ref().unwrap().id(), pid);
    let proof = handler.exit_confirmation().unwrap();
    assert_eq!(proof.kind(), VmExitEvidenceKind::OwnedReaped);
    assert_eq!(proof.status().unwrap().signal(), Some(libc::SIGKILL));
    assert_eq!(proof.status().unwrap().code(), None);
    assert!(!handler.is_running());
    assert_eq!(handler.try_wait_exit().unwrap(), None);
}
