#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

struct Host {
    child: Child,
    socket: PathBuf,
    _root: tempfile::TempDir,
}

impl Host {
    fn spawn(exit_on_stdin_eof: bool) -> Self {
        let root = tempfile::tempdir_in("/private/var/tmp").unwrap();
        let directory = root.path().canonicalize().unwrap();
        let shim = directory.join("unused-shim");
        std::fs::write(&shim, b"#!/bin/sh\nexit 91\n").unwrap();
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o700)).unwrap();
        let manifest = directory.join("system-image.json");
        std::fs::write(&manifest, b"{}\n").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_a3s-oci"));
        command
            .arg("macos-hvf-host-service")
            .arg("--root")
            .arg(directory.join("owner"))
            .arg("--shim")
            .arg(shim)
            .arg("--system-image-manifest")
            .arg(manifest)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if exit_on_stdin_eof {
            command.arg("--exit-on-stdin-eof");
        }
        let mut host = Self {
            child: command.spawn().unwrap(),
            socket: directory.join("owner/runtime.sock"),
            _root: root,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !host.socket.exists() {
            assert!(
                host.child.try_wait().unwrap().is_none(),
                "Host exited early"
            );
            assert!(Instant::now() < deadline, "Host did not publish its socket");
            std::thread::sleep(Duration::from_millis(10));
        }
        host
    }

    fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "Host did not finish cleanup");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn terminate(&mut self) {
        // SAFETY: this unreaped child still owns its process identity.
        assert_eq!(
            unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) },
            0
        );
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[test]
fn owner_pipe_data_keeps_host_alive_and_eof_cleans_endpoint() {
    let mut host = Host::spawn(true);
    host.child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"alive")
        .unwrap();
    std::thread::sleep(Duration::from_millis(25));
    assert!(host.child.try_wait().unwrap().is_none());
    drop(host.child.stdin.take());
    assert!(host.wait().success());
    assert!(!host.socket.exists());
}

#[test]
fn signals_exit_with_open_stdin_and_eof_monitor_is_opt_in() {
    for monitor in [false, true] {
        let mut host = Host::spawn(monitor);
        if !monitor {
            drop(host.child.stdin.take());
            std::thread::sleep(Duration::from_millis(25));
            assert!(host.child.try_wait().unwrap().is_none());
        }
        host.terminate();
        assert!(host.wait().success());
        assert!(!host.socket.exists());
    }
}
