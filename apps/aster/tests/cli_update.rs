//! Hermetic `aster update` checks. Each run starts the shipped binary.
//! The previous `aster` in the install directory is a fixture that execs
//! `CARGO_BIN_EXE_aster` and keeps that path as argv[0], so the install
//! directory is the fixture directory and the installer is the shipped command.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

const OLD_ID: &str = "aster-cli-pair-v1:fixture-old:local";
const NEW_ID: &str = "aster-cli-pair-v1:fixture-newer:local";
const OTHER_ID: &str = "aster-cli-pair-v1:fixture-other:local";

fn shipped_aster() -> &'static str {
    env!("CARGO_BIN_EXE_aster")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn asset_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "aster-cli-v0.1.1-macos.tar.gz"
    } else if cfg!(target_os = "linux") {
        "aster-cli-v0.1.1-linux.tar.gz"
    } else {
        "aster-cli-v0.1.1-windows.zip"
    }
}

fn temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "aster-update-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temp");
    path
}

fn write_pair(dir: &Path, identity: &str, forward_update: bool, pad: bool) {
    fs::create_dir_all(dir).expect("pair dir");
    let aster = if forward_update {
        format!(
            "#!/bin/bash\nif [ \"$1\" = \"--build-id\" ]; then\n  printf '%s\\n' '{identity}'\n  exit 0\nfi\nexec -a \"$0\" '{}' \"$@\"\nexit 127\n",
            shipped_aster().replace('\'', "")
        )
    } else {
        format!(
            "#!/bin/bash\nif [ \"$1\" = \"--build-id\" ]; then\n  printf '%s\\n' '{identity}'\n  exit 0\nfi\nexit 0\n"
        )
    };
    let mut aster = aster;
    if pad {
        aster.push_str(&entropy_comment(96_000));
    }
    let acp = format!(
        "#!/bin/bash\nif [ \"$1\" = \"--build-id\" ]; then\n  printf '%s\\n' '{identity}'\n  exit 0\nfi\nexit 0\n"
    );
    let aster_path = dir.join("aster");
    let acp_path = dir.join("a3s-code-acp");
    fs::write(&aster_path, aster).expect("aster fixture");
    fs::write(&acp_path, acp).expect("acp fixture");
    chmod(&aster_path);
    chmod(&acp_path);
}

fn entropy_comment(bytes: usize) -> String {
    let mut text = String::from("\n# ");
    let mut state = 0xA3C5_19E7_u32;
    for index in 0..bytes {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let unit = (state >> 16) as u8;
        text.push(char::from(b'!' + (unit % 93)));
        if index % 72 == 71 {
            text.push_str("\n# ");
        }
    }
    text.push('\n');
    text
}

fn chmod(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("mode").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }
}

fn package(bin_dir: &Path, archive: &Path) {
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent).expect("assets");
    }
    let status = Command::new("bash")
        .arg(repo_root().join("scripts/package-aster-cli.sh"))
        .arg(if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "windows"
        })
        .arg(bin_dir)
        .arg(archive)
        .env("COPYFILE_DISABLE", "1")
        .status()
        .expect("package");
    assert!(status.success(), "package-aster-cli.sh failed");
    let len = fs::metadata(archive).expect("archive").len();
    assert!(
        len > 12_288,
        "fixture archive is {len} bytes; progress frames need more than one chunk"
    );
}

fn prepare_home(root: &Path) -> PathBuf {
    let home = root.join("home");
    fs::create_dir_all(home.join(".a3s")).expect("a3s");
    fs::create_dir_all(home.join(".grok")).expect("grok");
    fs::write(home.join(".a3s/marker"), b"keep-a3s").expect("marker");
    fs::write(home.join(".grok/marker"), b"keep-grok").expect("marker");
    home
}

fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    files
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, files);
        } else {
            let rel = path
                .strip_prefix(root)
                .expect("prefix")
                .to_string_lossy()
                .into_owned();
            files.push((rel, fs::read(&path).expect("read")));
        }
    }
}

fn base_command(program: &Path, home: &Path, release: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin:/usr/local/bin")
        .env("TERM", "xterm-256color")
        .env("ASTER_RELEASE_BASE", release)
        .env("COPYFILE_DISABLE", "1")
        .current_dir(home)
        .stdin(Stdio::null());
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
    command
}

fn capture_pipe(
    program: &Path,
    args: &[&str],
    home: &Path,
    release: &Path,
) -> (i32, String, String) {
    let mut command = base_command(program, home, release);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn aster");
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("aster update timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    (
        status.code().unwrap_or(1),
        stdout_thread.join().expect("stdout thread"),
        stderr_thread.join().expect("stderr thread"),
    )
}

fn capture_terminal(program: &Path, home: &Path, release: &Path) -> (i32, String) {
    let system = portable_pty::native_pty_system();
    let pair = system
        .openpty(portable_pty::PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("pty");
    let mut command = portable_pty::CommandBuilder::new(program);
    command.arg("update");
    command.env_clear();
    command.env("HOME", home);
    command.env("PATH", "/usr/bin:/bin:/usr/local/bin");
    command.env("TERM", "xterm-256color");
    command.env("ASTER_RELEASE_BASE", release);
    command.env("COPYFILE_DISABLE", "1");
    command.cwd(home);
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
    let mut child = pair.slave.spawn_command(command).expect("spawn pty");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("pty reader");
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut tmp = [0_u8; 4096];
        loop {
            match reader.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Some(chunk) = tmp.get(..n) {
                        buf.extend_from_slice(chunk);
                    }
                }
            }
        }
        let _ = sender.send(buf);
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("pty wait") {
            break status;
        }
        if started.elapsed() > Duration::from_secs(180) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("terminal aster update timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    drop(pair.master);
    let bytes = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("pty output");
    (
        status.exit_code() as i32,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn fetching_fills(text: &str) -> Vec<usize> {
    strip_ansi(text)
        .lines()
        .filter(|line| line.contains("fetching"))
        .map(|line| line.chars().filter(|ch| *ch == '=').count())
        .collect()
}

/// ascii.rest 0.4.0 `progress-bar` frames at one shared fill.
/// Styles and the 48×11 layout are `src/pieces/progress-bar.ts`. The piece's
/// own clock is not used: every bar shows the same received-byte fraction.
const PROGRESS_BAR_EMPTY: &str = "                                                \n  fetching  [>                         ]    0%  \n                                                \n  unpacking ▕                          ▏    0%  \n                                                \n  indexing  [..........................]    0%  \n                                                \n  building  ░ ░ ░ ░ ░ ░ ░ ░ ░ ░ ░ ░ ░ ░     0%  \n                                                \n  linking   ────────────────────────────    0%  \n                                                ";
const PROGRESS_BAR_FULL: &str = "                                                \n  fetching  [==========================]  100%  \n                                                \n  unpacking ▕██████████████████████████▏  100%  \n                                                \n  indexing  [##########################]  100%  \n                                                \n  building  █ █ █ █ █ █ █ █ █ █ █ █ █ █   100%  \n                                                \n  linking   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━  100%  \n                                                ";

fn assert_progress(text: &str) {
    // A PTY records the piece with CRLF. The frame itself is the 48-column text.
    let plain = strip_ansi(text).replace('\r', "");
    assert!(
        plain.contains(PROGRESS_BAR_EMPTY),
        "missing the ascii.rest progress-bar frame at 0%\n{plain}"
    );
    assert!(
        plain.contains(PROGRESS_BAR_FULL),
        "missing the ascii.rest progress-bar frame at 100%\n{plain}"
    );
    for label in ["fetching", "unpacking", "indexing", "building", "linking"] {
        assert!(
            plain.contains(label),
            "missing progress-bar label {label}\n{plain}"
        );
    }
    assert!(plain.contains('█'), "missing block style\n{plain}");
    assert!(plain.contains('#'), "missing hash style\n{plain}");
    assert!(plain.contains('━'), "missing rule style\n{plain}");
    let fills = fetching_fills(text);
    assert!(
        fills.len() >= 2,
        "expected successive progress-bar frames, got {fills:?}\n{plain}"
    );
    assert!(
        fills.windows(2).all(|pair| pair[0] <= pair[1]),
        "progress-bar fill went backwards: {fills:?}"
    );
    assert!(
        fills.first().copied().unwrap_or(0) < fills.last().copied().unwrap_or(0),
        "progress-bar fill did not grow: {fills:?}"
    );
    assert!(!plain.to_ascii_lowercase().contains("press any key"));
    assert!(plain.contains(NEW_ID), "{plain}");
    assert!(plain.contains("installed Aster v0.1.1"), "{plain}");
}

fn assert_no_frames(text: &str) {
    assert!(!text.contains("fetching"), "{text}");
    assert!(!text.contains('█'), "{text}");
    assert!(!text.contains('━'), "{text}");
    assert!(!text.contains("unpacking"), "{text}");
}

fn build_id(path: &Path) -> String {
    let output = Command::new(path)
        .arg("--build-id")
        .stdin(Stdio::null())
        .output()
        .expect("build-id");
    assert!(output.status.success(), "{}", path.display());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn sha256_hex(path: &Path) -> String {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("shasum");
    assert!(output.status.success(), "shasum");
    String::from_utf8(output.stdout)
        .expect("hex")
        .split_whitespace()
        .next()
        .expect("hash")
        .to_string()
}

#[test]
fn shipped_update_installs_newer_pair_with_progress_bar() {
    for index in 0..2 {
        let root = temp_root(&format!("install-{index}"));
        let home = prepare_home(&root);
        let before_home = snapshot(&home);
        let bins = root.join("bins");
        write_pair(&bins, NEW_ID, true, true);
        let assets = root.join("assets");
        let archive = assets.join(asset_name());
        package(&bins, &archive);
        let install = root.join("install");
        write_pair(&install, OLD_ID, true, false);
        let old_aster = fs::read(install.join("aster")).expect("old aster");
        let old_acp = fs::read(install.join("a3s-code-acp")).expect("old acp");
        let (code, text) = capture_terminal(&install.join("aster"), &home, &assets);
        println!("CAPTURE install-{index}\n{text}");
        assert_eq!(code, 0, "{text}");
        assert_progress(&text);
        assert_eq!(
            snapshot(&home),
            before_home,
            "update changed ~/.a3s or ~/.grok"
        );
        assert_eq!(
            fs::read(install.join("aster")).expect("new aster"),
            fs::read(bins.join("aster")).expect("fixture aster")
        );
        assert_eq!(
            fs::read(install.join("a3s-code-acp")).expect("new acp"),
            fs::read(bins.join("a3s-code-acp")).expect("fixture acp")
        );
        assert_ne!(fs::read(install.join("aster")).expect("new"), old_aster);
        assert_ne!(
            fs::read(install.join("a3s-code-acp")).expect("new"),
            old_acp
        );
        assert_eq!(build_id(&install.join("aster")), NEW_ID);
        assert_eq!(build_id(&install.join("a3s-code-acp")), NEW_ID);
        let names: Vec<_> = fs::read_dir(&install)
            .expect("install dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        let _ = root;
    }
}

#[test]
fn shipped_update_check_and_current_leave_bytes() {
    let root = temp_root("check");
    let home = prepare_home(&root);
    let before_home = snapshot(&home);
    let bins = root.join("bins");
    write_pair(&bins, NEW_ID, true, true);
    let assets = root.join("assets");
    package(&bins, &assets.join(asset_name()));
    let install = root.join("install");
    write_pair(&install, OLD_ID, true, false);
    let aster_before = fs::read(install.join("aster")).expect("aster");
    let acp_before = fs::read(install.join("a3s-code-acp")).expect("acp");

    let (code, stdout, stderr) = capture_pipe(
        &install.join("aster"),
        &["update", "--check", "--json"],
        &home,
        &assets,
    );
    println!("CAPTURE check-json\n{stdout}\n{stderr}");
    assert_eq!(code, 0, "{stdout}\n{stderr}");
    assert!(stdout.contains("\"updateAvailable\":true"), "{stdout}");
    assert!(stdout.contains("\"autoUpdate\":false"), "{stdout}");
    assert_no_frames(&stdout);
    assert_no_frames(&stderr);
    assert_eq!(
        fs::read(install.join("aster")).expect("aster"),
        aster_before
    );
    assert_eq!(
        fs::read(install.join("a3s-code-acp")).expect("acp"),
        acp_before
    );

    let (json_code, json_stdout, json_stderr) = capture_pipe(
        &install.join("aster"),
        &["update", "--json"],
        &home,
        &assets,
    );
    println!("CAPTURE json\n{json_stdout}\n{json_stderr}");
    assert_eq!(json_code, 0, "{json_stdout}\n{json_stderr}");
    assert!(
        json_stdout.contains("\"updateAvailable\":true"),
        "{json_stdout}"
    );
    assert_no_frames(&json_stdout);
    assert_no_frames(&json_stderr);
    assert_eq!(
        fs::read(install.join("aster")).expect("aster"),
        aster_before
    );
    assert_eq!(
        fs::read(install.join("a3s-code-acp")).expect("acp"),
        acp_before
    );
    assert_eq!(snapshot(&home), before_home);

    let current_home = prepare_home(&root.join("current-home"));
    let current_before = snapshot(&current_home);
    let current = root.join("current");
    fs::create_dir_all(&current).expect("current");
    fs::copy(bins.join("aster"), current.join("aster")).expect("copy aster");
    fs::copy(bins.join("a3s-code-acp"), current.join("a3s-code-acp")).expect("copy acp");
    chmod(&current.join("aster"));
    chmod(&current.join("a3s-code-acp"));
    let current_aster = fs::read(current.join("aster")).expect("current aster");
    let current_acp = fs::read(current.join("a3s-code-acp")).expect("current acp");
    let (current_code, current_stdout, current_stderr) =
        capture_pipe(&current.join("aster"), &["update"], &current_home, &assets);
    println!("CAPTURE current\n{current_stdout}\n{current_stderr}");
    assert_eq!(current_code, 0, "{current_stdout}\n{current_stderr}");
    assert!(!current_stdout.contains("installed"), "{current_stdout}");
    assert!(!current_stderr.contains("installed"), "{current_stderr}");
    assert_no_frames(&current_stdout);
    assert_no_frames(&current_stderr);
    assert_eq!(
        fs::read(current.join("aster")).expect("aster"),
        current_aster
    );
    assert_eq!(
        fs::read(current.join("a3s-code-acp")).expect("acp"),
        current_acp
    );
    assert_eq!(snapshot(&current_home), current_before);
}

#[test]
fn shipped_update_failure_leaves_the_previous_pair() {
    let root = temp_root("rollback");
    let home = prepare_home(&root);
    let before_home = snapshot(&home);
    let bins = root.join("bins");
    write_pair(&bins, NEW_ID, true, true);
    let assets = root.join("assets");
    let archive = assets.join(asset_name());
    package(&bins, &archive);

    let truncated_assets = root.join("truncated");
    fs::create_dir_all(&truncated_assets).expect("truncated assets");
    let bytes = fs::read(&archive).expect("archive");
    let end = (bytes.len() / 2).max(1).min(bytes.len().saturating_sub(1));
    let Some(prefix) = bytes.get(..end) else {
        panic!("archive prefix");
    };
    fs::write(truncated_assets.join(asset_name()), prefix).expect("truncate");
    let truncated_install = root.join("truncated-install");
    write_pair(&truncated_install, OLD_ID, true, false);
    let truncated_aster = fs::read(truncated_install.join("aster")).expect("aster");
    let truncated_acp = fs::read(truncated_install.join("a3s-code-acp")).expect("acp");
    let (truncated_code, truncated_stdout, truncated_stderr) = capture_pipe(
        &truncated_install.join("aster"),
        &["update"],
        &home,
        &truncated_assets,
    );
    println!("CAPTURE truncated\n{truncated_stdout}\n{truncated_stderr}");
    assert_ne!(truncated_code, 0, "{truncated_stdout}");
    assert!(
        !truncated_stdout.contains("installed"),
        "{truncated_stdout}"
    );
    assert_eq!(
        fs::read(truncated_install.join("aster")).expect("aster"),
        truncated_aster
    );
    assert_eq!(
        fs::read(truncated_install.join("a3s-code-acp")).expect("acp"),
        truncated_acp
    );
    assert_eq!(build_id(&truncated_install.join("aster")), OLD_ID);
    assert_eq!(build_id(&truncated_install.join("a3s-code-acp")), OLD_ID);

    let mismatch_assets = root.join("mismatch");
    fs::create_dir_all(&mismatch_assets).expect("mismatch assets");
    let unpack = root.join("unpack");
    fs::create_dir_all(&unpack).expect("unpack");
    let extracted = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .args(["-C"])
        .arg(&unpack)
        .arg("-xzf")
        .arg(&archive)
        .status()
        .expect("tar");
    assert!(extracted.success(), "extract");
    let mut aster_text = fs::read_to_string(unpack.join("aster")).expect("aster text");
    assert!(aster_text.contains(NEW_ID));
    aster_text = aster_text.replacen(NEW_ID, OTHER_ID, 1);
    fs::write(unpack.join("aster"), aster_text).expect("rewrite aster");
    chmod(&unpack.join("aster"));
    let sums = format!(
        "{}  aster\n{}  a3s-code-acp\n",
        sha256_hex(&unpack.join("aster")),
        sha256_hex(&unpack.join("a3s-code-acp"))
    );
    fs::write(unpack.join("SHA256SUMS"), sums).expect("sums");
    let mismatch_archive = mismatch_assets.join(asset_name());
    let packed = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .current_dir(&unpack)
        .args(["-czf"])
        .arg(&mismatch_archive)
        .args(["aster", "a3s-code-acp", "BUILD_ID", "SHA256SUMS"])
        .status()
        .expect("repack");
    assert!(packed.success(), "repack");
    let mismatch_install = root.join("mismatch-install");
    write_pair(&mismatch_install, OLD_ID, true, false);
    let mismatch_aster = fs::read(mismatch_install.join("aster")).expect("aster");
    let mismatch_acp = fs::read(mismatch_install.join("a3s-code-acp")).expect("acp");
    let (mismatch_code, mismatch_stdout, mismatch_stderr) = capture_pipe(
        &mismatch_install.join("aster"),
        &["update"],
        &home,
        &mismatch_assets,
    );
    println!("CAPTURE mismatch\n{mismatch_stdout}\n{mismatch_stderr}");
    assert_ne!(mismatch_code, 0, "{mismatch_stdout}\n{mismatch_stderr}");
    assert!(!mismatch_stdout.contains("installed"));
    assert_eq!(
        fs::read(mismatch_install.join("aster")).expect("aster"),
        mismatch_aster
    );
    assert_eq!(
        fs::read(mismatch_install.join("a3s-code-acp")).expect("acp"),
        mismatch_acp
    );
    assert_eq!(build_id(&mismatch_install.join("aster")), OLD_ID);
    assert_eq!(build_id(&mismatch_install.join("a3s-code-acp")), OLD_ID);
    let acp = Command::new(mismatch_install.join("a3s-code-acp"))
        .stdin(Stdio::null())
        .status()
        .expect("acp");
    assert!(acp.success(), "previous acp is not runnable");
    assert_eq!(
        snapshot(&home),
        before_home,
        "failed update changed ~/.a3s or ~/.grok"
    );
}

/// The pinned aster-cli v0.1.1 archive contains only the two binaries.
/// Published aster rejects `--build-id` as an unexpected clap argument.
/// Published `a3s-code-acp` prints `unknown argument` and exits 1.
fn published_style_binary(which: &str) -> String {
    if which == "aster" {
        "#!/bin/bash\nif [ \"$1\" = \"--build-id\" ]; then\n  printf '%s\\n' \"error: unexpected argument '--build-id' found\" >&2\n  exit 2\nfi\nexit 0\n".to_string()
    } else {
        "#!/bin/bash\nif [ \"$1\" = \"--build-id\" ]; then\n  printf '%s\\n' 'Error: unknown argument: --build-id' >&2\n  exit 1\nfi\nexit 0\n".to_string()
    }
}

fn package_two_binaries(bin_dir: &Path, archive: &Path) {
    if let Some(parent) = archive.parent() {
        fs::create_dir_all(parent).expect("assets");
    }
    let status = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .current_dir(bin_dir)
        .arg("-czf")
        .arg(archive)
        .arg("aster")
        .arg("a3s-code-acp")
        .status()
        .expect("tar");
    assert!(status.success(), "two-file archive");
    let listed = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .args(["-tzf"])
        .arg(archive)
        .output()
        .expect("list archive");
    assert!(listed.status.success(), "list archive");
    let mut names: Vec<_> = String::from_utf8(listed.stdout)
        .expect("names")
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["a3s-code-acp".to_string(), "aster".to_string()],
        "pinned-style archive must be only the two binaries"
    );
}

#[test]
fn shipped_update_installs_two_file_release_without_build_id() {
    let root = temp_root("two-file");
    let home = prepare_home(&root);
    let before_home = snapshot(&home);
    let bins = root.join("bins");
    fs::create_dir_all(&bins).expect("bins");
    let aster_body = published_style_binary("aster");
    let acp_body = published_style_binary("acp");
    fs::write(bins.join("aster"), &aster_body).expect("aster stub");
    fs::write(bins.join("a3s-code-acp"), &acp_body).expect("acp stub");
    chmod(&bins.join("aster"));
    chmod(&bins.join("a3s-code-acp"));
    let assets = root.join("assets");
    let archive = assets.join(asset_name());
    package_two_binaries(&bins, &archive);

    let install = root.join("install");
    write_pair(&install, OLD_ID, true, false);
    let old_aster = fs::read(install.join("aster")).expect("old aster");
    let old_acp = fs::read(install.join("a3s-code-acp")).expect("old acp");
    let (code, stdout, stderr) = capture_pipe(&install.join("aster"), &["update"], &home, &assets);
    println!("CAPTURE two-file\n{stdout}\n{stderr}");
    assert_eq!(code, 0, "{stdout}\n{stderr}");
    assert!(stdout.contains("installed Aster v0.1.1"), "{stdout}");
    assert!(!stdout.contains("identity="), "{stdout}");
    assert!(!stderr.contains("identity mismatch"), "{stderr}");
    assert_no_frames(&stdout);
    assert_no_frames(&stderr);
    assert_eq!(
        fs::read(install.join("aster")).expect("new aster"),
        aster_body.as_bytes()
    );
    assert_eq!(
        fs::read(install.join("a3s-code-acp")).expect("new acp"),
        acp_body.as_bytes()
    );
    assert_ne!(
        fs::read(install.join("aster")).expect("replaced"),
        old_aster
    );
    assert_ne!(
        fs::read(install.join("a3s-code-acp")).expect("replaced"),
        old_acp
    );

    let replaced_aster = Command::new(install.join("aster"))
        .stdin(Stdio::null())
        .status()
        .expect("run replaced aster");
    assert!(replaced_aster.success(), "replaced aster is not runnable");
    let replaced_acp = Command::new(install.join("a3s-code-acp"))
        .stdin(Stdio::null())
        .status()
        .expect("run replaced acp");
    assert!(replaced_acp.success(), "replaced acp is not runnable");

    let aster_id = Command::new(install.join("aster"))
        .arg("--build-id")
        .stdin(Stdio::null())
        .output()
        .expect("aster --build-id");
    assert_eq!(aster_id.status.code(), Some(2));
    let aster_err = String::from_utf8_lossy(&aster_id.stderr);
    assert!(
        aster_err.contains("unexpected argument '--build-id'"),
        "{aster_err}"
    );
    let acp_id = Command::new(install.join("a3s-code-acp"))
        .arg("--build-id")
        .stdin(Stdio::null())
        .output()
        .expect("acp --build-id");
    assert_eq!(acp_id.status.code(), Some(1));
    let acp_err = String::from_utf8_lossy(&acp_id.stderr);
    assert!(
        acp_err.contains("unknown argument: --build-id"),
        "{acp_err}"
    );
    assert_eq!(snapshot(&home), before_home);
}
