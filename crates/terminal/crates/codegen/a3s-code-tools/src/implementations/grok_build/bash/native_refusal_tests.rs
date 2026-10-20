//! Pager bash and core's native sandbox both refuse `true` before spawn.
//!
//! Core covers `true` in `crates/code/core/src/sandbox/native/lifetime_tests.rs`.
//! This side is the env gate Aster sets on macOS. The gate does not read the
//! command. With the env set, `run` returns before a process exists.

const COMMAND: &str = "true";
const ENV: &str = "A3S_REFUSE_UNQUALIFIED_NATIVE_SHELL";

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EnvOn;

impl EnvOn {
    fn set() -> Self {
        unsafe { std::env::set_var(ENV, "1") };
        Self
    }
}

impl Drop for EnvOn {
    fn drop(&mut self) {
        unsafe { std::env::remove_var(ENV) };
    }
}

#[test]
fn true_is_refused_before_spawn_when_the_env_is_set() {
    let _lock = lock();
    assert_eq!(COMMAND, "true");
    let _guard = EnvOn::set();
    let error = super::unqualified_native_shell_error().expect("env refuses before spawn");
    let text = error.to_string();
    assert!(
        text.contains("native_process_lifetime_unqualified"),
        "{text}"
    );
}

#[test]
fn true_is_not_refused_by_this_gate_when_the_env_is_unset() {
    let _lock = lock();
    assert_eq!(COMMAND, "true");
    unsafe { std::env::remove_var(ENV) };
    assert!(super::unqualified_native_shell_error().is_none());
}
