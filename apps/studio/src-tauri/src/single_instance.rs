//! One process per tenant. A second launch asks the running one to show its
//! window and exits before it starts an authority or writes the runtime file.
//! Different tenants keep separate locks and run side by side.

/// The fork launcher tries the next id when this process lost the lock race.
pub const FORK_CLAIM_LOST: i32 = 75;

#[cfg(unix)]
pub use unix::{Claim, InstanceLock, claim, claim_quiet, is_held};

#[cfg(not(unix))]
pub enum Claim {
    Owned(InstanceLock),
    Busy,
    Unavailable(std::io::Error),
}

#[cfg(not(unix))]
pub struct InstanceLock;

#[cfg(not(unix))]
pub fn claim(_slug: &str) -> Claim {
    Claim::Owned(InstanceLock)
}

#[cfg(not(unix))]
pub fn claim_quiet(slug: &str) -> Claim {
    claim(slug)
}

#[cfg(not(unix))]
pub fn is_held(_slug: &str) -> bool {
    false
}

#[cfg(not(unix))]
impl InstanceLock {
    pub fn listen(self, _on_activate: impl Fn() + Send + 'static) {}
}

#[cfg(unix)]
mod unix {
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::thread;

    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    const LOCK_UN: i32 = 8;

    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }

    /// Held for the life of the process. The kernel drops the `flock` when the
    /// process exits, so a crash never leaves the tenant locked.
    pub struct InstanceLock {
        lock: Option<File>,
        listener: Option<UnixListener>,
    }

    pub enum Claim {
        Owned(InstanceLock),
        Busy,
        Unavailable(std::io::Error),
    }

    /// The lock lives in the tenant directory, not `TMPDIR`.
    fn lock_parent(slug: &str) -> std::io::Result<PathBuf> {
        let Some(dir) = crate::providers::tenant_data_dir(slug) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "tenant data directory is unavailable",
            ));
        };
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn claim(slug: &str) -> Claim {
        match lock_parent(slug) {
            Ok(dir) => claim_in(&dir, slug, true),
            Err(error) => Claim::Unavailable(error),
        }
    }

    /// Like [`claim`], but a busy tenant is left alone. Used while picking a free fork.
    pub fn claim_quiet(slug: &str) -> Claim {
        match lock_parent(slug) {
            Ok(dir) => claim_in(&dir, slug, false),
            Err(error) => Claim::Unavailable(error),
        }
    }

    /// `true` when a live process holds this tenant's lock.
    pub fn is_held(slug: &str) -> bool {
        match lock_parent(slug) {
            Ok(dir) => is_held_in(&dir, slug),
            Err(_) => false,
        }
    }

    pub(super) fn claim_in(dir: &Path, slug: &str, activate: bool) -> Claim {
        let (lock_path, socket_path) = paths(dir, slug);
        let lock = match OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
        {
            Ok(file) => file,
            Err(error) => return Claim::Unavailable(error),
        };
        if unsafe { flock(lock.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
            if activate {
                if let Ok(mut stream) = UnixStream::connect(&socket_path) {
                    let _ = stream.write_all(b"show\n");
                }
            }
            return Claim::Busy;
        }
        let _ = fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path)
            .map_err(|error| {
                eprintln!(
                    "studio: single-instance socket {}: {error}",
                    socket_path.display()
                );
            })
            .ok();
        Claim::Owned(InstanceLock {
            lock: Some(lock),
            listener,
        })
    }

    pub(super) fn is_held_in(dir: &Path, slug: &str) -> bool {
        let (lock_path, _) = paths(dir, slug);
        let Ok(lock) = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
        else {
            return false;
        };
        let held = unsafe { flock(lock.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0;
        if !held {
            let _ = unsafe { flock(lock.as_raw_fd(), LOCK_UN) };
        }
        held
    }

    fn paths(dir: &Path, slug: &str) -> (PathBuf, PathBuf) {
        (
            dir.join(format!("dev.a3s.studio.{slug}.lock")),
            dir.join(format!("dev.a3s.studio.{slug}.sock")),
        )
    }

    impl InstanceLock {
        /// Calls `on_activate` for every later launch of the same tenant.
        pub fn listen(self, on_activate: impl Fn() + Send + 'static) {
            let InstanceLock { lock, listener } = self;
            // The lock must outlive this call: dropping the file releases the flock.
            if let Some(lock) = lock {
                std::mem::forget(lock);
            }
            let Some(listener) = listener else {
                return;
            };
            let spawned = thread::Builder::new()
                .name("studio-single-instance".to_owned())
                .spawn(move || {
                    for stream in listener.incoming().flatten() {
                        let mut request = [0u8; 5];
                        let mut stream = stream;
                        if stream.read_exact(&mut request).is_ok() && &request == b"show\n" {
                            on_activate();
                        }
                    }
                });
            if let Err(error) = spawned {
                eprintln!("studio: single-instance listener: {error}");
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::unix::{Claim, claim, claim_in, claim_quiet, is_held, is_held_in};

    #[test]
    fn second_claim_is_busy_and_asks_the_first_window_to_show_itself() {
        let dir = std::env::temp_dir().join(format!("studio-si-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let Claim::Owned(first) = claim_in(&dir, "toyou", true) else {
            panic!("first launch owns the tenant");
        };
        let (tx, rx) = mpsc::channel();
        first.listen(move || {
            let _ = tx.send(());
        });

        assert!(matches!(claim_in(&dir, "toyou", true), Claim::Busy));
        rx.recv_timeout(Duration::from_secs(2))
            .expect("the running instance is asked to show itself");

        assert!(
            matches!(claim_in(&dir, "a3s", true), Claim::Owned(_)),
            "another tenant is not blocked"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unopenable_lock_path_is_not_owned() {
        let dir = std::env::temp_dir().join(format!("studio-si-blocked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir(dir.join("dev.a3s.studio.toyou.lock")).unwrap();

        assert!(matches!(
            claim_in(&dir, "toyou", true),
            Claim::Unavailable(_)
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quiet_claim_does_not_notify() {
        let dir = std::env::temp_dir().join(format!("studio-fork-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let Claim::Owned(first) = claim_in(&dir, "toyou-fork1", true) else {
            panic!("fork1");
        };
        let (tx, rx) = mpsc::channel();
        first.listen(move || {
            let _ = tx.send(());
        });

        assert!(is_held_in(&dir, "toyou-fork1"));
        assert!(!is_held_in(&dir, "toyou-fork2"));
        assert!(matches!(claim_in(&dir, "toyou-fork1", false), Claim::Busy));
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "a quiet claim must not ask the running window to show itself"
        );
        assert!(matches!(
            claim_in(&dir, "toyou-fork2", false),
            Claim::Owned(_)
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_tenant_directory_does_not_fall_back_to_temp() {
        let slug = "toyou";
        let temp_lock = std::env::temp_dir().join(format!("dev.a3s.studio.{slug}.lock"));
        let _ = std::fs::remove_file(&temp_lock);
        match claim(slug) {
            Claim::Unavailable(error) => {
                assert!(
                    error
                        .to_string()
                        .contains("tenant data directory is unavailable")
                );
            }
            Claim::Owned(_) | Claim::Busy => panic!("a missing tenant directory is not ownership"),
        }
        match claim_quiet(slug) {
            Claim::Unavailable(error) => {
                assert!(
                    error
                        .to_string()
                        .contains("tenant data directory is unavailable")
                );
            }
            Claim::Owned(_) | Claim::Busy => panic!("a quiet claim does not use a temp lock"),
        }
        assert!(!is_held(slug));
        assert!(!temp_lock.exists());
    }
}
