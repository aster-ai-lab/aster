//! The cargo lib harness does not run on the main thread. AppKit window
//! setup from that harness aborts, so this binary is the process main.

fn main() {
    #[cfg(target_os = "macos")]
    if let Err(message) = a3s_studio_lib::probe_applet_titlebar() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
