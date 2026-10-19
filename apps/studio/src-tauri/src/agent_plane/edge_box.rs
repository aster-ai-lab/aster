//! One edge package turn inside a single `a3s-box` MicroVM.
//!
//! The declared millicpu cap is a guest cgroup quota. The declared memory cap
//! is a whole number of mebibytes. The package entry keeps its loopback
//! listener; a studio-owned launcher in the guest proxies the published port
//! to that listener. That port accepts before the proxy is listening, and the
//! first post is reset. A reset before a status line is retried until the
//! turn budget ends. An accepted command is not sent again. The command body
//! is not placed on the command line or in the environment.

use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::task::JoinHandle;

use super::edge_runtime::{EdgeResources, PACKAGE_TIMED_OUT};

pub const BOX_REQUIRED: &str = "a3s-box is required to enforce the declared cpu and memory limits";
pub const IMAGE_UNAVAILABLE: &str = "the edge agent image is not available";
pub const IMAGE_INVALID: &str = "the edge agent image is invalid";
pub const MEMORY_NOT_WHOLE_MIB: &str = "edge package memory must be a whole number of mebibytes";
pub const MEMORY_ABOVE_CEILING: &str = "edge package memory exceeds the enforceable limit";
pub const CPU_CEILING: &str = "edge package cpu exceeds the enforceable vcpu ceiling";
pub const PATH_UNMOUNTABLE: &str = "the edge package path cannot be mounted into a3s-box";
const BOX_CLI_ENV: &str = "A3S_BOX_CLI";
const IMAGE_ENV: &str = "A3S_EDGE_AGENT_IMAGE";
const UNSAFE_HOST_RUN: &str = "A3S_BOX_UNSAFE_HOST_RUN";
pub const DEFAULT_IMAGE: &str = "node:22-bookworm";
pub const BOX_TURN_LIMIT: Duration = Duration::from_secs(120);
const CPU_PERIOD_US: u64 = 100_000;
const MAX_VCPUS: u64 = 255;
const MIB: u64 = 1024 * 1024;
const GUEST_AGENT_ROOT: &str = "/a3s/agent";
const GUEST_LAUNCHER: &str = "/a3s/edge/launch.cjs";
pub const GUEST_PROXY_PORT: u16 = 18080;

/// Guest-side proxy. The package entry still binds `127.0.0.1:0` and prints
/// `port N`. Published traffic arrives on the guest address, so this process
/// listens on `0.0.0.0` and forwards bytes to that loopback port.
const LAUNCH_CJS: &str = r#"
'use strict'
const net = require('net')
const fs = require('fs')
const path = require('path')
const { spawn } = require('child_process')

function extractPackage(data, entry) {
  const dest = '/tmp/a3s-agent'
  fs.rmSync(dest, { recursive: true, force: true })
  fs.mkdirSync(dest, { recursive: true })
  let offset = 0
  while (offset + 512 <= data.length) {
    const header = data.subarray(offset, offset + 512)
    offset += 512
    let blank = true
    for (let i = 0; i < 512; i++) {
      if (header[i] !== 0) {
        blank = false
        break
      }
    }
    if (blank) break
    let sum = 0
    for (let i = 0; i < 512; i++) sum += i >= 148 && i < 156 ? 32 : header[i]
    const stored = parseInt(header.toString('utf8', 148, 156).replace(/\0.*/, '').trim(), 8)
    if (stored !== sum) throw new Error('edge archive checksum mismatch')
    const name = header.toString('utf8', 0, 100).replace(/\0.*/, '')
    const prefix = header.toString('utf8', 345, 500).replace(/\0.*/, '')
    const relative = prefix ? prefix + '/' + name : name
    const parts = relative.split('/')
    if (
      !relative ||
      relative.startsWith('/') ||
      parts.some((part) => part === '' || part === '.' || part === '..')
    ) {
      throw new Error('edge archive path is invalid')
    }
    const kind = header[156]
    const sizeText = header.toString('utf8', 124, 136).replace(/\0.*/, '').trim()
    const size = sizeText ? parseInt(sizeText, 8) : 0
    const target = path.join(dest, relative)
    if (target !== dest && !target.startsWith(dest + path.sep)) {
      throw new Error('edge archive path is invalid')
    }
    if (kind === 53) {
      fs.mkdirSync(target, { recursive: true })
    } else if (kind === 48 || kind === 0) {
      fs.mkdirSync(path.dirname(target), { recursive: true })
      fs.writeFileSync(target, data.subarray(offset, offset + size))
    } else {
      throw new Error('edge archive entry is unsupported')
    }
    offset += size
    if (size % 512 !== 0) offset += 512 - (size % 512)
  }
  const root = '/a3s/agent'
  let rewritten = entry
  if (entry === root || entry.startsWith(root + '/')) rewritten = dest + entry.slice(root.length)
  if (!fs.existsSync(rewritten)) throw new Error('edge archive is missing the entry')
  return rewritten
}

let entry = process.env.A3S_EDGE_ENTRY
const proxyPort = Number(process.env.A3S_EDGE_PROXY_PORT)
if (!entry || !Number.isInteger(proxyPort) || proxyPort < 1 || proxyPort > 65535) {
  process.stderr.write('edge launcher is missing its entry\n')
  process.exit(1)
}
if (process.env.A3S_EDGE_ARCHIVE === 'embedded') {
  try {
    entry = extractPackage(Buffer.from('__ARCHIVE_BASE64__', 'base64'), entry)
  } catch (error) {
    process.stderr.write(String(error && error.message ? error.message : error) + '\n')
    process.exit(1)
  }
}

const child = spawn(process.execPath, [entry], {
  stdio: ['ignore', 'pipe', 'pipe'],
  env: process.env,
})
child.on('error', (error) => {
  process.stderr.write(String(error && error.message ? error.message : error) + '\n')
  process.exit(1)
})
child.stderr.on('data', (chunk) => process.stderr.write(chunk))

let announced = null
const waiting = []
function resolvePort(port) {
  if (announced !== null) return
  announced = port
  for (const pending of waiting) pending(port)
  waiting.length = 0
}
function portReady() {
  if (announced !== null) return Promise.resolve(announced)
  return new Promise((resolve) => waiting.push(resolve))
}

let buffer = ''
child.stdout.on('data', (chunk) => {
  if (announced !== null) return
  buffer += chunk.toString('utf8')
  if (buffer.length > 8192) {
    process.stderr.write('entry process did not announce a port\n')
    child.kill()
    process.exit(1)
  }
  let newline = buffer.indexOf('\n')
  while (newline !== -1) {
    const line = buffer.slice(0, newline).trim()
    buffer = buffer.slice(newline + 1)
    const match = /^port (\d+)$/.exec(line)
    if (match && announced === null) {
      const port = Number(match[1])
      if (port >= 1 && port <= 65535) resolvePort(port)
    }
    newline = buffer.indexOf('\n')
  }
})
child.on('exit', (code) => {
  if (announced === null) {
    process.stderr.write('entry process ended before listening\n')
    process.exit(code === 0 || code == null ? 1 : code)
  }
})

const server = net.createServer((client) => {
  const chunks = []
  let upstream = null
  let ready = false
  let clientEnded = false
  function fail() {
    client.destroy()
    if (upstream) upstream.destroy()
  }
  client.on('data', (chunk) => {
    if (ready && upstream) upstream.write(chunk)
    else chunks.push(chunk)
  })
  client.on('end', () => {
    clientEnded = true
    if (ready && upstream) upstream.end()
  })
  client.on('error', fail)
  portReady().then((port) => {
    upstream = net.connect(port, '127.0.0.1')
    upstream.on('connect', () => {
      ready = true
      for (const chunk of chunks) upstream.write(chunk)
      chunks.length = 0
      if (clientEnded) upstream.end()
    })
    upstream.on('data', (chunk) => client.write(chunk))
    upstream.on('end', () => client.end())
    upstream.on('error', fail)
  })
})
server.on('error', (error) => {
  process.stderr.write(String(error && error.message ? error.message : error) + '\n')
  child.kill()
  process.exit(1)
})
server.listen(proxyPort, '0.0.0.0')
"#;

#[derive(Debug)]
pub struct CpuLimit {
    pub cpus: u32,
    pub quota_us: i64,
    pub period_us: u64,
}

pub fn validate_resources(cpu_millis: u64, memory_bytes: u64) -> Result<(), String> {
    cpu_limit(cpu_millis)?;
    memory_flag(memory_bytes)?;
    Ok(())
}

pub fn cpu_limit(cpu_millis: u64) -> Result<CpuLimit, String> {
    if cpu_millis == 0 {
        return Err(super::edge_runtime::LIMITS_MISSING.into());
    }
    let cpus = cpu_millis.div_ceil(1000);
    if cpus == 0 || cpus > MAX_VCPUS {
        return Err(CPU_CEILING.into());
    }
    let quota = cpu_millis
        .checked_mul(100)
        .ok_or_else(|| CPU_CEILING.to_owned())?;
    let quota_us = i64::try_from(quota).map_err(|_| CPU_CEILING.to_owned())?;
    Ok(CpuLimit {
        cpus: u32::try_from(cpus).map_err(|_| CPU_CEILING.to_owned())?,
        quota_us,
        period_us: CPU_PERIOD_US,
    })
}

pub fn memory_flag(memory_bytes: u64) -> Result<String, String> {
    if memory_bytes == 0 || memory_bytes % MIB != 0 {
        return Err(MEMORY_NOT_WHOLE_MIB.into());
    }
    let mb = memory_bytes / MIB;
    if mb > u32::MAX as u64 {
        return Err(MEMORY_ABOVE_CEILING.into());
    }
    Ok(format!("{mb}m"))
}

pub fn validate_image(image: &str) -> Result<(), String> {
    if image.is_empty() || image.starts_with('-') || image.chars().any(char::is_whitespace) {
        return Err(IMAGE_INVALID.into());
    }
    Ok(())
}

pub fn resolve_image(explicit: Option<&str>) -> Result<String, String> {
    if let Some(image) = explicit {
        validate_image(image)?;
        return Ok(image.to_owned());
    }
    match std::env::var(IMAGE_ENV) {
        Ok(value) => {
            validate_image(&value)?;
            Ok(value)
        }
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_IMAGE.to_owned()),
        Err(std::env::VarError::NotUnicode(_)) => Err(IMAGE_INVALID.into()),
    }
}

/// Explicit path, then `A3S_BOX_CLI`, then the bundled tree, then `PATH`.
/// A set `A3S_BOX_CLI` that is not a file does not fall through to `PATH`.
pub fn resolve_box_executable(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return existing_executable(path);
    }
    if let Some(raw) = std::env::var_os(BOX_CLI_ENV) {
        return existing_executable(Path::new(&raw));
    }
    if let Some(bundled) = discover_bundled_box_executable() {
        return Ok(canonicalize_executable(bundled));
    }
    find_box_on_path()
        .map(canonicalize_executable)
        .ok_or_else(|| BOX_REQUIRED.to_owned())
}

pub fn package_root(installed: &Path, declared: &str) -> Result<PathBuf, String> {
    if !super::edge_runtime::declared_entry_matches(installed, declared) {
        return Err("package entry does not match the installed file".into());
    }
    let declared_len = Path::new(declared).components().count();
    let mut root = installed.to_path_buf();
    for _ in 0..declared_len {
        if !root.pop() {
            return Err("package entry does not match the installed file".into());
        }
    }
    if root.as_os_str().is_empty() {
        return Err("package entry does not match the installed file".into());
    }
    Ok(root)
}

pub fn guest_entry(declared: &str) -> Result<String, String> {
    let path = Path::new(declared);
    if declared.is_empty() || path.is_absolute() {
        return Err("package entry does not match the installed file".into());
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| "package entry does not match the installed file".to_owned())?;
                if part.contains(':') || part.contains('\\') || part.contains('\n') {
                    return Err("package entry does not match the installed file".into());
                }
                parts.push(part);
            }
            _ => return Err("package entry does not match the installed file".into()),
        }
    }
    if parts.is_empty() {
        return Err("package entry does not match the installed file".into());
    }
    Ok(format!("{GUEST_AGENT_ROOT}/{}", parts.join("/")))
}

/// The three hosts a3s-box actually boots. Linux and Windows share the
/// package directory read-only. macOS carries those bytes in the launcher.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HostFamily {
    Macos,
    Linux,
    Windows,
}

fn current_host() -> HostFamily {
    if cfg!(target_os = "windows") {
        HostFamily::Windows
    } else if cfg!(target_os = "macos") {
        HostFamily::Macos
    } else {
        HostFamily::Linux
    }
}

pub fn box_run_arguments(
    resources: &EdgeResources,
    package_root: &Path,
    declared: &str,
    launcher: &Path,
    host_port: u16,
    image: &str,
    timeout_secs: u64,
    host: HostFamily,
) -> Result<Vec<String>, String> {
    validate_resources(resources.cpu_millis, resources.memory_bytes)?;
    validate_image(image)?;
    if timeout_secs == 0 || host_port == 0 {
        return Err("the edge agent publish port is missing".into());
    }
    let cpu = cpu_limit(resources.cpu_millis)?;
    let memory = memory_flag(resources.memory_bytes)?;
    let launcher = mountable_path(launcher, host)?;
    let entry = guest_entry(declared)?;
    let mut args = vec![
        "run".to_owned(),
        "--rm".to_owned(),
        "--no-stdin".to_owned(),
        "--timeout".to_owned(),
        timeout_secs.to_string(),
        "--cpus".to_owned(),
        cpu.cpus.to_string(),
        "--cpu-quota".to_owned(),
        cpu.quota_us.to_string(),
        "--cpu-period".to_owned(),
        cpu.period_us.to_string(),
        "--memory".to_owned(),
        memory,
    ];
    // A directory share is one virtio-fs walk. macOS guest reads of a second
    // file did not return, so that host does not mount the package.
    if host != HostFamily::Macos {
        let root = mountable_path(package_root, host)?;
        args.push("-v".to_owned());
        args.push(volume_mount(&root, GUEST_AGENT_ROOT, host));
    }
    args.extend([
        "-v".to_owned(),
        volume_mount(&launcher, GUEST_LAUNCHER, host),
        "-e".to_owned(),
        "A3S_AGENT_PROTOCOL=1".to_owned(),
        "-e".to_owned(),
        format!("A3S_EDGE_ENTRY={entry}"),
    ]);
    if host == HostFamily::Macos {
        args.push("-e".to_owned());
        args.push("A3S_EDGE_ARCHIVE=embedded".to_owned());
    }
    args.extend([
        "-e".to_owned(),
        format!("A3S_EDGE_PROXY_PORT={GUEST_PROXY_PORT}"),
        "-p".to_owned(),
        format!("{host_port}:{GUEST_PROXY_PORT}"),
        image.to_owned(),
        "--".to_owned(),
        "node".to_owned(),
        GUEST_LAUNCHER.to_owned(),
    ]);
    Ok(args)
}

/// Linux and Windows deny writes on the share. macOS a3s-box refuses `:ro`.
fn volume_mount(host: &str, guest: &str, family: HostFamily) -> String {
    let mut spec = format!("{host}:{guest}");
    if family != HostFamily::Macos {
        spec.push_str(":ro");
    }
    spec
}

struct HostShare {
    launcher: PathBuf,
    #[cfg(target_os = "macos")]
    _dir: tempfile::TempDir,
}

impl HostShare {
    fn launcher(&self) -> &Path {
        &self.launcher
    }
}

/// Linux and Windows mount the installed tree. macOS writes one canonical
/// launcher file; the package bytes are already inside that file.
fn stage_host_share(package: &Path, launcher: &Path) -> Result<HostShare, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = package;
        Ok(HostShare {
            launcher: launcher.to_path_buf(),
        })
    }
    #[cfg(target_os = "macos")]
    {
        let bytes = package_archive_bytes(package)?;
        let script = LAUNCH_CJS.replace("__ARCHIVE_BASE64__", &base64_encode(&bytes));
        let dir = tempfile::Builder::new()
            .prefix("a3s-edge-share-")
            .tempdir()
            .map_err(|error| error.to_string())?;
        let canonical = dir
            .path()
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let launcher_path = canonical.join("launch.cjs");
        std::fs::write(&launcher_path, script).map_err(|error| error.to_string())?;
        let _ = launcher;
        Ok(HostShare {
            launcher: launcher_path,
            _dir: dir,
        })
    }
}

/// The package bytes are carried inside the launcher. A separate virtio-fs
/// file was copied by a3s-box, and the guest produced no output when it tried
/// to read that file. The archive is written once; file bytes are not kept
/// beside it.
fn package_archive_bytes(root: &Path) -> Result<Vec<u8>, String> {
    let mut file = Vec::new();
    append_archive(root, root, &mut file)?;
    file.extend_from_slice(&[0_u8; 1024]);
    Ok(file)
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= input.len() {
        let value = (u32::from(input[index]) << 16)
            | (u32::from(input[index + 1]) << 8)
            | u32::from(input[index + 2]);
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push(TABLE[((value >> 6) & 63) as usize] as char);
        out.push(TABLE[(value & 63) as usize] as char);
        index += 3;
    }
    let rest = input.len() - index;
    if rest == 1 {
        let value = u32::from(input[index]) << 16;
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let value = (u32::from(input[index]) << 16) | (u32::from(input[index + 1]) << 8);
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push(TABLE[((value >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn append_archive(root: &Path, dir: &Path, out: &mut Vec<u8>) -> Result<(), String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
        names.push(entry.map_err(|error| error.to_string())?);
    }
    names.sort_by_key(|entry| entry.file_name());
    for entry in names {
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_symlink() {
            return Err("edge package contains a symlink".into());
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.len() > 100
            || relative
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err("edge package path cannot be archived".into());
        }
        if file_type.is_dir() {
            write_ustar_header(out, &relative, 0, true)?;
            append_archive(root, &entry.path(), out)?;
        } else if file_type.is_file() {
            let bytes = std::fs::read(entry.path()).map_err(|error| error.to_string())?;
            write_ustar_header(out, &relative, bytes.len() as u64, false)?;
            out.extend_from_slice(&bytes);
            let pad = (512 - (bytes.len() % 512)) % 512;
            out.extend(std::iter::repeat_n(0_u8, pad));
        } else {
            return Err("edge package contains an unsupported file".into());
        }
    }
    Ok(())
}

fn write_ustar_header(
    out: &mut impl std::io::Write,
    name: &str,
    size: u64,
    is_dir: bool,
) -> Result<(), String> {
    let bytes = name.as_bytes();
    if bytes.len() > 100 {
        return Err("edge package path cannot be archived".into());
    }
    let mut header = [0_u8; 512];
    header[..bytes.len()].copy_from_slice(bytes);
    put_octal(&mut header[100..108], if is_dir { 0o755 } else { 0o644 });
    put_octal(&mut header[108..116], 0);
    put_octal(&mut header[116..124], 0);
    put_octal(&mut header[124..136], if is_dir { 0 } else { size });
    put_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = if is_dir { b'5' } else { b'0' };
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let sum: u32 = header.iter().map(|byte| u32::from(*byte)).sum();
    put_octal(&mut header[148..156], u64::from(sum));
    out.write_all(&header).map_err(|error| error.to_string())
}

fn put_octal(slot: &mut [u8], value: u64) {
    let width = slot.len() - 1;
    let text = format!("{value:0width$o}");
    let digits = text.as_bytes();
    let count = digits.len().min(width);
    slot[..count].copy_from_slice(&digits[..count]);
    slot[width] = 0;
}

/// macOS virtio-fs returns ENOENT for a directory this process created.

pub(super) struct RunningBox {
    child: Child,
    cli: PathBuf,
    box_name: Arc<Mutex<Option<String>>>,
    pub(super) host_port: u16,
    pub(super) deadline: Instant,
    stderr: Option<JoinHandle<Vec<u8>>>,
    stdout: JoinHandle<()>,
    _launcher: tempfile::NamedTempFile,
    _share: HostShare,
}

impl RunningBox {
    pub(super) async fn finish(mut self) -> String {
        let name = self
            .box_name
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            kill_tree(pid);
        }
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        self.stdout.abort();
        if let Some(name) = name {
            // The shim leaves the CLI process group, so killing the CLI
            // leaves the MicroVM in `starting`. `rm -f` is what removes it.
            let remove = Command::new(&self.cli)
                .args(["rm", "-f", &name])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .status();
            let _ = tokio::time::timeout(Duration::from_secs(20), remove).await;
        }
        match self.stderr.take() {
            Some(task) => stderr_text(task).await,
            None => String::new(),
        }
    }
}

pub(super) async fn start_package_box(
    entry: &Path,
    declared: &str,
    resources: &EdgeResources,
    budget: Duration,
    box_cli: Option<&Path>,
    image: Option<&str>,
) -> Result<RunningBox, String> {
    let deadline = Instant::now() + budget;
    let cli = resolve_box_executable(box_cli)?;
    let image = resolve_image(image)?;
    inspect_image(&cli, &image, deadline).await?;
    let root = package_root(entry, declared)?;
    if !root.join(declared).is_file() {
        return Err(format!(
            "installed package is missing {declared} under {}",
            root.display()
        ));
    }
    let host_port = reserve_host_port()?;
    let launcher = write_launcher()?;
    let share = stage_host_share(&root, launcher.path())?;
    let timeout_secs = budget.as_secs().max(1);
    let args = box_run_arguments(
        resources,
        &root,
        declared,
        share.launcher(),
        host_port,
        &image,
        timeout_secs,
        current_host(),
    )?;
    let mut command = Command::new(&cli);
    command
        .args(&args)
        .env_remove(UNSAFE_HOST_RUN)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(|_| BOX_REQUIRED.to_owned())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "a3s-box has no stdout".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "a3s-box has no stderr".to_owned())?;
    let box_name = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&box_name);
    let stdout = tokio::spawn(async move {
        watch_box_name(stdout, recorded).await;
    });
    let stderr = tokio::spawn(async move { read_bounded(stderr).await });
    let running = RunningBox {
        child,
        cli,
        box_name,
        host_port,
        deadline,
        stderr: Some(stderr),
        stdout,
        _launcher: launcher,
        _share: share,
    };
    if let Err(error) = wait_for_port(host_port, deadline).await {
        let stderr = running.finish().await;
        return Err(with_diagnostic(error, &stderr));
    }
    Ok(running)
}

pub(super) async fn boxed_turn_text(
    entry: &Path,
    declared: &str,
    resources: &EdgeResources,
    body: &Value,
) -> Result<Vec<String>, String> {
    boxed_turn_text_in(entry, declared, resources, body, BOX_TURN_LIMIT, None, None).await
}

pub(super) async fn boxed_turn_text_in(
    entry: &Path,
    declared: &str,
    resources: &EdgeResources,
    body: &Value,
    budget: Duration,
    box_cli: Option<&Path>,
    image: Option<&str>,
) -> Result<Vec<String>, String> {
    match entry.extension().and_then(|extension| extension.to_str()) {
        Some("mjs" | "js" | "cjs") => {}
        _ => return Err("package entry is not a node program".to_owned()),
    }
    let running = start_package_box(entry, declared, resources, budget, box_cli, image).await?;
    let endpoint = format!("http://127.0.0.1:{}", running.host_port);
    let outcome = exchange_until(&endpoint, body, running.deadline).await;
    let stderr = running.finish().await;
    match outcome {
        Ok(deltas) => Ok(deltas),
        Err(error) => Err(with_diagnostic(error, &stderr)),
    }
}

/// The publish socket accepts while the guest proxy is still booting.
/// Retry that gap on the request that has not yet returned a status.
/// A command that already returned 200 is not sent again.
async fn exchange_until(
    endpoint: &str,
    body: &Value,
    deadline: Instant,
) -> Result<Vec<String>, String> {
    post_until(endpoint, "/v1/agent/commands", body, deadline).await?;
    let page = post_until(
        endpoint,
        "/v1/agent/events:page",
        &serde_json::json!({ "after": 0 }),
        deadline,
    )
    .await?;
    super::edge_runtime::text_of_page(&page)
}

async fn post_until(
    endpoint: &str,
    path: &str,
    body: &Value,
    deadline: Instant,
) -> Result<Value, String> {
    let mut pause = Duration::from_millis(50);
    let mut last = super::edge_runtime::PACKAGE_TIMED_OUT.to_owned();
    loop {
        if Instant::now() >= deadline {
            return Err(last);
        }
        let limit = deadline.saturating_duration_since(Instant::now());
        match super::edge_runtime::post_json_within(endpoint, path, body, limit).await {
            Ok(value) => return Ok(value),
            Err(error) if connect_not_ready(&error) => {
                last = error;
                let slice = pause.min(deadline.saturating_duration_since(Instant::now()));
                if slice.is_zero() {
                    return Err(last);
                }
                tokio::time::sleep(slice).await;
                pause = (pause * 2).min(Duration::from_millis(250));
            }
            Err(error) => return Err(error),
        }
    }
}

fn connect_not_ready(error: &str) -> bool {
    let text = error.to_ascii_lowercase();
    text.contains("connection reset")
        || text.contains("connection refused")
        || text.contains("broken pipe")
        || text.contains("returned no status")
        || text.contains("os error 54")
        || text.contains("os error 61")
        || text.contains("os error 32")
}

fn box_label(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix("Box ")?;
    let (label, _) = rest.split_once(" (")?;
    if label.is_empty() || label.contains(' ') {
        return None;
    }
    Some(label.to_owned())
}

fn box_label_in(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    text.lines().find_map(box_label)
}

async fn watch_box_name(mut reader: impl AsyncRead + Unpin, slot: Arc<Mutex<Option<String>>>) {
    let mut pending = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        if slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_none()
        {
            if let Some(label) = box_label_in(&pending) {
                *slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(label);
                pending.clear();
            }
        }
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let known = slot
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .is_some();
                if !known {
                    pending.extend_from_slice(&buf[..n]);
                    if pending.len() > 16_384 {
                        pending.clear();
                    }
                }
            }
        }
    }
}

fn mountable_path(path: &Path, host: HostFamily) -> Result<String, String> {
    let text = path.to_str().ok_or_else(|| PATH_UNMOUNTABLE.to_owned())?;
    if text.is_empty() || text.contains('\n') {
        return Err(PATH_UNMOUNTABLE.into());
    }
    let body = if host == HostFamily::Windows {
        windows_drive_body(text).unwrap_or(text)
    } else {
        text
    };
    if body.contains(':') {
        return Err(PATH_UNMOUNTABLE.into());
    }
    Ok(text.to_owned())
}

/// `C:\pkg` and `C:/pkg` keep the drive colon. The rest of the path must not
/// contain one, or a3s-box cannot split host from guest.
fn windows_drive_body(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        Some(&text[2..])
    } else {
        None
    }
}

fn existing_executable(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        Ok(canonicalize_executable(path.to_path_buf()))
    } else {
        Err(BOX_REQUIRED.into())
    }
}

fn canonicalize_executable(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn discover_bundled_box_executable() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let exe_dir = executable.parent()?;
    let names = ["a3s-box", "a3s-box.exe"];
    let mut candidates = Vec::new();
    for name in names {
        candidates.push(exe_dir.join("a3s-box").join("bin").join(name));
        candidates.push(
            exe_dir
                .join("..")
                .join("Resources")
                .join("a3s-box")
                .join("bin")
                .join(name),
        );
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn find_box_on_path() -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_env) {
        let candidate = directory.join("a3s-box");
        if candidate.is_file() {
            return Some(candidate);
        }
        let with_exe = directory.join("a3s-box.exe");
        if with_exe.is_file() {
            return Some(with_exe);
        }
    }
    None
}

fn reserve_host_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    drop(listener);
    if port == 0 {
        return Err("the edge agent publish port is missing".into());
    }
    Ok(port)
}

fn write_launcher() -> Result<tempfile::NamedTempFile, String> {
    let mut file = tempfile::Builder::new()
        .prefix("a3s-edge-launch-")
        .suffix(".cjs")
        .tempfile()
        .map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut file, LAUNCH_CJS.as_bytes())
        .map_err(|error| error.to_string())?;
    file.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o644))
            .map_err(|error| error.to_string())?;
    }
    Ok(file)
}

async fn inspect_image(cli: &Path, image: &str, deadline: Instant) -> Result<(), String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(PACKAGE_TIMED_OUT.into());
    }
    let mut command = Command::new(cli);
    command
        .arg("image-inspect")
        .arg(image)
        .env_remove(UNSAFE_HOST_RUN)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|_| BOX_REQUIRED.to_owned())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let drain = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            drain(stdout).await;
        }
    });
    let stderr_task = tokio::spawn(async move {
        match stderr {
            Some(stderr) => read_bounded(stderr).await,
            None => Vec::new(),
        }
    });
    let status = match tokio::time::timeout(remaining, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(_)) => {
            let _ = child.kill().await;
            drain.abort();
            return Err(IMAGE_UNAVAILABLE.into());
        }
        Err(_) => {
            let _ = child.kill().await;
            drain.abort();
            return Err(IMAGE_UNAVAILABLE.into());
        }
    };
    drain.abort();
    let stderr = stderr_text(stderr_task).await;
    if status.success() {
        Ok(())
    } else {
        Err(with_diagnostic(IMAGE_UNAVAILABLE.to_owned(), &stderr))
    }
}

async fn wait_for_port(port: u16, deadline: Instant) -> Result<(), String> {
    let mut pause = Duration::from_millis(20);
    loop {
        if Instant::now() >= deadline {
            return Err(PACKAGE_TIMED_OUT.into());
        }
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(stream) => {
                drop(stream);
                return Ok(());
            }
            Err(_) => {
                let slice = pause.min(deadline.saturating_duration_since(Instant::now()));
                if slice.is_zero() {
                    return Err(PACKAGE_TIMED_OUT.into());
                }
                tokio::time::sleep(slice).await;
                pause = (pause * 2).min(Duration::from_millis(200));
            }
        }
    }
}

async fn drain(mut reader: impl AsyncRead + Unpin) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

async fn read_bounded(mut reader: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut buf = [0u8; 1024];
    let mut out = Vec::new();
    while out.len() < 8192 {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let room = 8192 - out.len();
                out.extend_from_slice(&buf[..n.min(room)]);
            }
        }
    }
    out
}

async fn stderr_text(task: JoinHandle<Vec<u8>>) -> String {
    let bytes = match tokio::time::timeout(Duration::from_secs(1), task).await {
        Ok(Ok(bytes)) => bytes,
        _ => Vec::new(),
    };
    let text = String::from_utf8_lossy(&bytes);
    text.trim().chars().take(400).collect()
}

fn with_diagnostic(error: String, stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        error
    } else {
        format!("{error}: {stderr}")
    }
}

#[cfg(unix)]
fn kill_tree(pid: u32) {
    let Some(pid) = rustix::process::Pid::from_raw(pid as i32) else {
        return;
    };
    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resources() -> EdgeResources {
        EdgeResources {
            cpu_millis: 500,
            memory_bytes: 268_435_456,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    const AGENT_VOLUME: &str = "/pkg/weather:/a3s/agent:ro";

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    const LAUNCH_VOLUME: &str = "/tmp/launch.cjs:/a3s/edge/launch.cjs:ro";
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    const LAUNCH_VOLUME: &str = "/tmp/launch.cjs:/a3s/edge/launch.cjs";

    #[test]
    fn a_half_cpu_and_256_mebibytes_become_guest_cgroup_flags() {
        let args = box_run_arguments(
            &resources(),
            Path::new("/pkg/weather"),
            "agent/run.cjs",
            Path::new("/tmp/launch.cjs"),
            18081,
            DEFAULT_IMAGE,
            120,
            current_host(),
        )
        .expect("args");
        let mut expected = vec![
            "run".to_owned(),
            "--rm".to_owned(),
            "--no-stdin".to_owned(),
            "--timeout".to_owned(),
            "120".to_owned(),
            "--cpus".to_owned(),
            "1".to_owned(),
            "--cpu-quota".to_owned(),
            "50000".to_owned(),
            "--cpu-period".to_owned(),
            "100000".to_owned(),
            "--memory".to_owned(),
            "256m".to_owned(),
        ];
        #[cfg(not(target_os = "macos"))]
        {
            expected.push("-v".to_owned());
            expected.push(AGENT_VOLUME.to_owned());
        }
        expected.extend([
            "-v".to_owned(),
            LAUNCH_VOLUME.to_owned(),
            "-e".to_owned(),
            "A3S_AGENT_PROTOCOL=1".to_owned(),
            "-e".to_owned(),
            "A3S_EDGE_ENTRY=/a3s/agent/agent/run.cjs".to_owned(),
        ]);
        if cfg!(target_os = "macos") {
            expected.push("-e".to_owned());
            expected.push("A3S_EDGE_ARCHIVE=embedded".to_owned());
        }
        expected.extend([
            "-e".to_owned(),
            "A3S_EDGE_PROXY_PORT=18080".to_owned(),
            "-p".to_owned(),
            "18081:18080".to_owned(),
            "node:22-bookworm".to_owned(),
            "--".to_owned(),
            "node".to_owned(),
            "/a3s/edge/launch.cjs".to_owned(),
        ]);
        assert_eq!(args, expected);
        assert!(!args.iter().any(|arg| arg.contains("UNSAFE_HOST_RUN")));
        assert!(
            !args
                .iter()
                .any(|arg| arg == "--network" || arg == "--pool" || arg == "--isolation")
        );
    }

    #[test]
    fn memory_that_is_not_a_whole_mebibyte_is_refused() {
        let error = memory_flag(1000).unwrap_err();
        assert_eq!(error, MEMORY_NOT_WHOLE_MIB);
        let error = memory_flag(268_435_457).unwrap_err();
        assert_eq!(error, MEMORY_NOT_WHOLE_MIB);
    }

    #[test]
    fn cpu_above_255_vcpus_is_refused() {
        let error = cpu_limit(256_000).unwrap_err();
        assert_eq!(error, CPU_CEILING);
        let fitted = cpu_limit(255_000).expect("255 cpus");
        assert_eq!(fitted.cpus, 255);
        assert_eq!(fitted.quota_us, 25_500_000);
    }

    #[test]
    fn each_host_mounts_only_the_share_that_host_can_read() {
        let linux = host_args(HostFamily::Linux, "/pkg/weather", "/tmp/launch.cjs");
        assert!(linux.iter().any(|arg| arg == "/pkg/weather:/a3s/agent:ro"));
        assert!(
            linux
                .iter()
                .any(|arg| arg == "/tmp/launch.cjs:/a3s/edge/launch.cjs:ro")
        );
        assert!(!linux.iter().any(|arg| arg == "A3S_EDGE_ARCHIVE=embedded"));

        let windows = host_args(HostFamily::Windows, r"C:\pkg\weather", r"C:\tmp\launch.cjs");
        assert!(
            windows
                .iter()
                .any(|arg| arg == r"C:\pkg\weather:/a3s/agent:ro")
        );
        assert!(
            windows
                .iter()
                .any(|arg| arg == r"C:\tmp\launch.cjs:/a3s/edge/launch.cjs:ro")
        );
        assert!(!windows.iter().any(|arg| arg == "A3S_EDGE_ARCHIVE=embedded"));

        let macos = host_args(HostFamily::Macos, "/pkg/weather", "/tmp/launch.cjs");
        assert!(!macos.iter().any(|arg| arg.contains("/pkg/weather")));
        assert!(!macos.iter().any(|arg| arg.contains(":ro")));
        assert!(macos.iter().any(|arg| arg == "A3S_EDGE_ARCHIVE=embedded"));
        assert!(
            macos
                .iter()
                .any(|arg| arg == "/tmp/launch.cjs:/a3s/edge/launch.cjs")
        );
    }

    #[test]
    fn a_colon_breaks_a_volume_unless_it_is_a_windows_drive() {
        let error = box_run_arguments(
            &resources(),
            Path::new("/pkg/we:ather"),
            "agent/run.cjs",
            Path::new("/tmp/launch.cjs"),
            18081,
            DEFAULT_IMAGE,
            120,
            HostFamily::Linux,
        )
        .unwrap_err();
        assert_eq!(error, PATH_UNMOUNTABLE);
        let error = box_run_arguments(
            &resources(),
            Path::new("/pkg/weather"),
            "agent/run.cjs",
            Path::new("/tmp/lau:nch.cjs"),
            18081,
            DEFAULT_IMAGE,
            120,
            HostFamily::Macos,
        )
        .unwrap_err();
        assert_eq!(error, PATH_UNMOUNTABLE);
        let error = box_run_arguments(
            &resources(),
            Path::new(r"C:\pkg\we:ather"),
            "agent/run.cjs",
            Path::new(r"C:\tmp\launch.cjs"),
            18081,
            DEFAULT_IMAGE,
            120,
            HostFamily::Windows,
        )
        .unwrap_err();
        assert_eq!(error, PATH_UNMOUNTABLE);
        // The package is not a volume on macOS, so a colon in that path is unused.
        box_run_arguments(
            &resources(),
            Path::new("/pkg/we:ather"),
            "agent/run.cjs",
            Path::new("/tmp/launch.cjs"),
            18081,
            DEFAULT_IMAGE,
            120,
            HostFamily::Macos,
        )
        .expect("macos leaves the package path off the command");
    }

    fn host_args(host: HostFamily, package: &str, launcher: &str) -> Vec<String> {
        box_run_arguments(
            &resources(),
            Path::new(package),
            "agent/run.cjs",
            Path::new(launcher),
            18081,
            DEFAULT_IMAGE,
            120,
            host,
        )
        .expect("args")
    }

    #[test]
    fn an_image_override_rejects_a_flag_or_blank() {
        assert_eq!(validate_image("").unwrap_err(), IMAGE_INVALID);
        assert_eq!(validate_image("-node").unwrap_err(), IMAGE_INVALID);
        assert_eq!(validate_image("node 22").unwrap_err(), IMAGE_INVALID);
        assert!(validate_image(DEFAULT_IMAGE).is_ok());
    }

    #[test]
    fn a_package_archive_keeps_the_entry_bytes() {
        let root = std::env::temp_dir().join(format!("a3s-edge-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let package = root.join("pkg");
        std::fs::create_dir_all(package.join("agent")).unwrap();
        let body = b"export {}\n";
        std::fs::write(package.join("agent").join("run.mjs"), body).unwrap();
        let bytes = package_archive_bytes(&package).unwrap();
        let header = &bytes[..512];
        let mut sum = 0_u32;
        for (index, byte) in header.iter().enumerate() {
            sum += if (148..156).contains(&index) {
                32
            } else {
                u32::from(*byte)
            };
        }
        let stored = std::str::from_utf8(&header[148..156])
            .unwrap()
            .trim_end_matches('\0')
            .trim();
        assert_eq!(u32::from_str_radix(stored, 8).unwrap(), sum);
        let name = std::str::from_utf8(&header[0..100])
            .unwrap()
            .trim_end_matches('\0');
        assert_eq!(name, "agent");
        assert_eq!(header[156], b'5');
        let file_header = &bytes[512..1024];
        let file_name = std::str::from_utf8(&file_header[0..100])
            .unwrap()
            .trim_end_matches('\0');
        assert_eq!(file_name, "agent/run.mjs");
        assert_eq!(file_header[156], b'0');
        let size_text = std::str::from_utf8(&file_header[124..136])
            .unwrap()
            .trim_end_matches('\0')
            .trim();
        let size = usize::from_str_radix(size_text, 8).unwrap();
        assert_eq!(&bytes[1024..1024 + size], body);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_box_start_line_names_the_box() {
        assert_eq!(
            box_label("Box vivid_wozniak (5f7eafd3b0e3) started. Press Ctrl-C to stop."),
            Some("vivid_wozniak".to_owned())
        );
        assert_eq!(box_label("Pulling node:22-bookworm..."), None);
        assert!(connect_not_ready("Connection reset by peer (os error 54)"));
        assert!(connect_not_ready("agent runtime returned no status"));
        assert!(!connect_not_ready("agent runtime did not answer"));
        assert!(!connect_not_ready(
            "agent runtime refused the command (404)"
        ));
        assert!(connect_not_ready(&super::super::edge_runtime::io_failure(
            std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "An existing connection was forcibly closed by the remote host.",
            ),
        )));
        assert!(connect_not_ready(&super::super::edge_runtime::io_failure(
            std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "No connection could be made because the target machine actively refused it.",
            ),
        )));
        assert!(!connect_not_ready(
            &std::io::Error::new(std::io::ErrorKind::TimedOut, "a connection attempt failed")
                .to_string()
        ));
    }

    #[tokio::test]
    async fn a_reset_after_the_command_does_not_send_it_again() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listen");
        let port = listener.local_addr().expect("addr").port();
        let commands = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let events = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let command_count = std::sync::Arc::clone(&commands);
        let event_count = std::sync::Arc::clone(&events);
        let server = tokio::spawn(async move {
            let mut reset_events = true;
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let Some(path) = read_request_path(&mut stream).await else {
                    continue;
                };
                if path.contains("/commands") {
                    command_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = write_json(&mut stream, r#"{"accepted":true}"#).await;
                } else if reset_events {
                    reset_events = false;
                    event_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    drop(stream);
                } else {
                    event_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let _ = write_json(
                        &mut stream,
                        r#"{"events":[{"sequence":1,"type":"output","text":"{\"choice\":\"hold\"}"},{"sequence":2,"type":"completed"}]}"#,
                    )
                    .await;
                }
            }
        });
        let text = exchange_until(
            &format!("http://127.0.0.1:{port}"),
            &serde_json::json!({ "prompt": "保持" }),
            Instant::now() + Duration::from_secs(3),
        )
        .await
        .expect("turn");
        server.abort();
        assert_eq!(text, vec!["{\"choice\":\"hold\"}".to_owned()]);
        assert_eq!(commands.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(events.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_slow_read_uses_the_remaining_budget_once() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listen");
        let port = listener.local_addr().expect("addr").port();
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = std::sync::Arc::clone(&hits);
        let server = tokio::spawn(async move {
            let mut held = Vec::new();
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                held.push(stream);
            }
        });
        let error = exchange_until(
            &format!("http://127.0.0.1:{port}"),
            &serde_json::json!({ "prompt": "保持" }),
            Instant::now() + Duration::from_millis(2500),
        )
        .await
        .expect_err("hang");
        server.abort();
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1, "{error}");
        assert!(
            error.contains(super::super::edge_runtime::PACKAGE_TIMED_OUT),
            "{error}"
        );
    }

    #[tokio::test]
    async fn a_fake_box_publishes_the_turn_and_is_removed() {
        let node = Path::new("node");
        if Command::new(node).arg("--version").output().await.is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("a3s-fake-box-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let entry = dir.join("agent").join("run.mjs");
        std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
        std::fs::write(&entry, "process.stdout.write('port 1\\n')\n").unwrap();
        let cli = dir.join("a3s-box.mjs");
        std::fs::write(
            &cli,
            r#"#!/usr/bin/env node
import http from 'node:http'
import { appendFileSync } from 'node:fs'
const args = process.argv.slice(2)
appendFileSync(new URL('./args.log', import.meta.url), args.join('\0') + '\n')
if (args[0] === 'image-inspect') process.exit(0)
const published = args[args.indexOf('-p') + 1].split(':')[0]
let resets = 0
const server = http.createServer((request, response) => {
  if (resets < 1) {
    resets += 1
    request.socket.destroy()
    return
  }
  const chunks = []
  request.on('data', (chunk) => chunks.push(chunk))
  request.on('end', () => {
    response.setHeader('content-type', 'application/json')
    if (request.url === '/v1/agent/commands') {
      response.end(JSON.stringify({ accepted: true }))
      return
    }
    response.end(JSON.stringify({ events: [{ sequence: 1, type: 'output', text: '{"choice":"hold"}' }, { sequence: 2, type: 'completed' }] }))
  })
})
server.listen(Number(published), '127.0.0.1')
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let text = boxed_turn_text_in(
            &entry,
            "agent/run.mjs",
            &resources(),
            &serde_json::json!({ "prompt": "保持" }),
            Duration::from_secs(5),
            Some(&cli),
            None,
        )
        .await
        .expect("turn");
        assert_eq!(text, vec!["{\"choice\":\"hold\"}".to_owned()]);
        let log = std::fs::read_to_string(dir.join("args.log")).unwrap();
        assert!(log.contains("image-inspect"), "{log}");
        assert!(log.contains("--cpu-quota\u{0}50000"), "{log}");
        assert!(log.contains("--memory\u{0}256m"), "{log}");
        assert!(log.contains("--rm"), "{log}");
        assert!(!log.contains("UNSAFE_HOST_RUN"), "{log}");
        #[cfg(target_os = "macos")]
        {
            let source = dir.display().to_string();
            assert!(!log.contains(":ro"), "{log}");
            assert!(!log.contains(&source), "{log}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn read_request_path(stream: &mut tokio::net::TcpStream) -> Option<String> {
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::new();
        let mut tmp = [0_u8; 2048];
        let header_end = loop {
            let n = stream.read(&mut tmp).await.ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&tmp[..n]);
            if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
                break pos;
            }
            if buf.len() > 65_536 {
                return None;
            }
        };
        let header = String::from_utf8_lossy(&buf[..header_end]).into_owned();
        let length = header.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })?;
        while buf.len() < header_end + 4 + length {
            let n = stream.read(&mut tmp).await.ok()?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
        header
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .map(str::to_owned)
    }

    async fn write_json(stream: &mut tokio::net::TcpStream, body: &str) -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).await?;
        stream.shutdown().await
    }
}
