use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn executable() -> &'static Path {
    static PYTHON: OnceLock<PathBuf> = OnceLock::new();
    PYTHON.get_or_init(|| {
        // macOS's /usr/bin/python3 is an Xcode launcher, not the interpreter.
        let output = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .expect("resolve test Python interpreter");
        assert!(output.status.success(), "test Python discovery failed");
        Path::new(
            String::from_utf8(output.stdout)
                .expect("test Python path must be UTF-8")
                .trim(),
        )
        .canonicalize()
        .expect("resolved test Python interpreter must exist")
    })
}
