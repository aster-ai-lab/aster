#![cfg(test)]

use std::io::{Read, Write};
use std::num::NonZeroU32;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn guest_monitors_reject_an_owner_that_is_not_the_direct_parent() {
    let current = NonZeroU32::new(std::process::id()).unwrap();
    assert!(super::start_guest_command(current).is_err());
    assert!(super::start_guest_worker(current).is_err());
}

#[test]
fn guest_monitors_pin_the_exact_parent_in_a_private_process_group() {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "owner_process::macos_tests::guest_monitor_child",
        ])
        .env("ASTER_TEST_GUEST_OWNER", std::process::id().to_string())
        .process_group(0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "run only by the parent in an isolated process group"]
fn guest_monitor_child() {
    let owner = std::env::var("ASTER_TEST_GUEST_OWNER")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let owner = NonZeroU32::new(owner).unwrap();
    super::start_guest_command(owner).unwrap();
    super::start_guest_worker(owner).unwrap();
}

struct Family {
    parent: Child,
    neighbor: Child,
}

impl Drop for Family {
    fn drop(&mut self) {
        // The worker owns this pipe and can reap its child if the oracle fails.
        if let Some(input) = self.parent.stdin.as_mut() {
            let _ = input.write_all(b"cleanup");
        }
        for child in [&mut self.parent, &mut self.neighbor] {
            if child.try_wait().is_ok_and(|status| status.is_none()) {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

#[test]
fn owner_death_kills_the_private_worker_group_and_preserves_a_neighbor() {
    for kind in ["command", "worker"] {
        let directory = tempfile::tempdir_in("/private/var/tmp").unwrap();
        let ready = directory.path().join("ready.json");
        let parent = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "owner_process::macos_tests::owner_family_parent",
                "--nocapture",
            ])
            .env("ASTER_TEST_GUEST_READY", &ready)
            .env("ASTER_TEST_GUEST_MONITOR_KIND", kind)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .process_group(0)
            .spawn()
            .unwrap();
        let neighbor = Command::new("/bin/sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let mut family = Family { parent, neighbor };
        let deadline = Instant::now() + Duration::from_secs(5);
        let pids: [u32; 2] = loop {
            if let Ok(bytes) = std::fs::read(&ready) {
                if let Ok(pids) = serde_json::from_slice(&bytes) {
                    break pids;
                }
            }
            assert!(family.parent.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline, "worker did not become ready");
            std::thread::sleep(Duration::from_millis(10));
        };
        let watches = pids.map(exit_watch);
        family.parent.kill().unwrap();
        family.parent.wait().unwrap();
        let mut exited = [false; 2];
        while !exited.into_iter().all(|value| value) {
            for (index, watch) in watches.iter().enumerate() {
                if !exited[index] {
                    exited[index] = observed_exit(watch);
                }
            }
            assert!(
                Instant::now() < deadline,
                "owner death left a live descendant"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(family.neighbor.try_wait().unwrap().is_none());
    }
}

fn exit_watch(pid: u32) -> OwnedFd {
    // SAFETY: a successful kqueue result is a new descriptor owned by this test.
    let descriptor = unsafe { libc::kqueue() };
    assert!(descriptor >= 0);
    let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
    let change = libc::kevent {
        ident: pid as usize,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_ONESHOT,
        fflags: libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    // SAFETY: the initialized change registers one exact live process incarnation.
    assert_eq!(
        unsafe {
            libc::kevent(
                descriptor.as_raw_fd(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        },
        0
    );
    descriptor
}

fn observed_exit(watch: &OwnedFd) -> bool {
    let mut event = std::mem::MaybeUninit::<libc::kevent>::uninit();
    let timeout = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: the output has space for one event and the zero timeout cannot block.
    let count = unsafe {
        libc::kevent(
            watch.as_raw_fd(),
            std::ptr::null(),
            0,
            event.as_mut_ptr(),
            1,
            &timeout,
        )
    };
    assert!(count >= 0);
    count == 1
}

#[test]
#[ignore = "spawned only by the owner-death oracle"]
fn owner_family_parent() {
    let _worker = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "owner_process::macos_tests::owner_family_worker",
            "--nocapture",
        ])
        .env("ASTER_TEST_GUEST_OWNER", std::process::id().to_string())
        .stdin(Stdio::inherit())
        .process_group(0)
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
#[ignore = "spawned only by the owner-death oracle"]
fn owner_family_worker() {
    let owner = std::env::var("ASTER_TEST_GUEST_OWNER")
        .unwrap()
        .parse()
        .unwrap();
    let owner = NonZeroU32::new(owner).unwrap();
    match std::env::var("ASTER_TEST_GUEST_MONITOR_KIND")
        .unwrap()
        .as_str()
    {
        "command" => super::start_guest_command(owner).unwrap(),
        "worker" => super::start_guest_worker(owner).unwrap(),
        _ => panic!("unknown test monitor"),
    }
    let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    let ready = std::env::var_os("ASTER_TEST_GUEST_READY").unwrap();
    std::fs::write(
        ready,
        serde_json::to_vec(&[std::process::id(), child.id()]).unwrap(),
    )
    .unwrap();
    let _ = std::io::stdin().read(&mut [0_u8; 1]);
    let _ = child.kill();
    let _ = child.wait();
}
