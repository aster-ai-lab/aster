//! Desktop family for the window shell.
//!
//! Program selection is a pure function of the family so one test process
//! covers macOS, Linux, and Windows. Spawning the program stays at the call site.

use std::path::Path;

pub const CAPABILITY_UNAVAILABLE: &str = "desktop-capability-unavailable";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostFamily {
    Macos,
    Linux,
    Windows,
}

pub fn current_family() -> HostFamily {
    if cfg!(target_os = "windows") {
        HostFamily::Windows
    } else if cfg!(target_os = "macos") {
        HostFamily::Macos
    } else {
        HostFamily::Linux
    }
}

pub fn screenshot_supported(family: HostFamily) -> bool {
    family == HostFamily::Macos
}

pub fn updates_supported(family: HostFamily) -> bool {
    family == HostFamily::Macos
}

pub struct ShellLaunch {
    pub program: &'static str,
    pub args: Vec<String>,
}

pub fn open_http_launch(family: HostFamily, url: &str) -> ShellLaunch {
    match family {
        HostFamily::Macos => ShellLaunch {
            program: "open",
            args: vec![url.to_owned()],
        },
        HostFamily::Linux => ShellLaunch {
            program: "xdg-open",
            args: vec![url.to_owned()],
        },
        HostFamily::Windows => ShellLaunch {
            program: "rundll32.exe",
            args: vec!["url.dll,FileProtocolHandler".to_owned(), url.to_owned()],
        },
    }
}

pub fn open_path_launch(family: HostFamily, path: &Path, reveal: bool) -> ShellLaunch {
    let text = path.display().to_string();
    match family {
        HostFamily::Macos => {
            let mut args = Vec::new();
            if reveal {
                args.push("-R".to_owned());
            }
            args.push(text);
            ShellLaunch {
                program: "open",
                args,
            }
        }
        HostFamily::Linux => {
            let target = if reveal {
                path.parent().unwrap_or(path).display().to_string()
            } else {
                text
            };
            ShellLaunch {
                program: "xdg-open",
                args: vec![target],
            }
        }
        HostFamily::Windows => {
            if reveal {
                ShellLaunch {
                    program: "explorer.exe",
                    args: vec![format!("/select,{text}")],
                }
            } else {
                ShellLaunch {
                    program: "explorer.exe",
                    args: vec![text],
                }
            }
        }
    }
}

pub fn spawn_launch(launch: &ShellLaunch) -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new(launch.program)
        .args(&launch.args)
        .status()
}

/// JavaScript that freezes `__A3S_STUDIO_DESKTOP__` on the current document.
pub(crate) fn desktop_capabilities_assignment() -> String {
    let value = desktop_capabilities();
    format!(
        "try{{Object.defineProperty(globalThis,'__A3S_STUDIO_DESKTOP__',{{value:Object.freeze({value}),configurable:false,enumerable:false,writable:false}});}}catch(e){{}}"
    )
}

pub fn desktop_capabilities() -> serde_json::Value {
    let family = current_family();
    let platform = match family {
        HostFamily::Macos => "darwin",
        HostFamily::Windows => "win32",
        HostFamily::Linux => "linux",
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let chrome = if family == HostFamily::Macos {
        "overlay"
    } else {
        "native"
    };
    serde_json::json!({
        "platform": platform,
        "arch": arch,
        "chrome": chrome,
        "openExternal": true,
        "revealInFolder": true,
        "chooseDirectory": true,
        "screenshot": screenshot_supported(family),
        "singleInstance": family != HostFamily::Windows,
        "autoUpdate": updates_supported(family),
        "chromeCookieImport": family == HostFamily::Macos || family == HostFamily::Windows,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        HostFamily, desktop_capabilities, open_http_launch, open_path_launch, screenshot_supported,
        updates_supported,
    };
    use std::path::Path;

    #[test]
    fn http_open_uses_the_family_program() {
        let page = "https://docs.example.com/guide";
        assert_eq!(open_http_launch(HostFamily::Macos, page).program, "open");
        assert_eq!(
            open_http_launch(HostFamily::Linux, page).program,
            "xdg-open"
        );
        let windows = open_http_launch(HostFamily::Windows, page);
        assert_eq!(windows.program, "rundll32.exe");
        assert_eq!(
            windows.args,
            vec!["url.dll,FileProtocolHandler".to_owned(), page.to_owned()]
        );
        assert!(!windows.args.iter().any(|arg| arg.contains("xdg-open")));
    }

    #[test]
    fn reveal_selects_the_file_on_windows_and_the_parent_on_linux() {
        let path = Path::new("/work/notes/today.md");
        let mac = open_path_launch(HostFamily::Macos, path, true);
        assert_eq!(mac.program, "open");
        assert_eq!(mac.args, vec!["-R".to_owned(), path.display().to_string()]);
        let linux = open_path_launch(HostFamily::Linux, path, true);
        assert_eq!(linux.program, "xdg-open");
        let parent = path.parent().expect("parent").display().to_string();
        assert!(parent.ends_with("notes"));
        assert!(!parent.contains("today.md"));
        assert_eq!(linux.args, vec![parent]);
        let windows = open_path_launch(HostFamily::Windows, path, true);
        assert_eq!(windows.program, "explorer.exe");
        assert_eq!(windows.args, vec![format!("/select,{}", path.display())]);
    }

    #[test]
    fn screenshot_and_updates_stay_on_macos() {
        assert!(screenshot_supported(HostFamily::Macos));
        assert!(!screenshot_supported(HostFamily::Linux));
        assert!(!screenshot_supported(HostFamily::Windows));
        assert!(updates_supported(HostFamily::Macos));
        assert!(!updates_supported(HostFamily::Linux));
        assert!(!updates_supported(HostFamily::Windows));
    }

    #[test]
    fn capabilities_match_the_compiled_family() {
        let caps = desktop_capabilities();
        let platform = caps["platform"].as_str().unwrap();
        assert!(matches!(platform, "darwin" | "win32" | "linux"));
        let overlay = platform == "darwin";
        assert_eq!(caps["chrome"], if overlay { "overlay" } else { "native" });
        assert_eq!(caps["screenshot"], overlay);
        assert_eq!(caps["autoUpdate"], overlay);
        assert_eq!(caps["singleInstance"], platform != "win32");
        assert_eq!(
            caps["chromeCookieImport"],
            platform == "darwin" || platform == "win32"
        );
    }
}
