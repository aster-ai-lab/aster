//! Install the Aster CLI pair from this repository's `aster-cli` release.
//!
//! The download meter is the ascii.rest `progress-bar` piece
//! (https://ascii.rest/progress-bar/, `src/pieces/progress-bar.ts`).
//! Copyright (c) 2026 bas3line. MIT License.
//!
//! That piece's clock eases, stalls, and restarts on its own. This command
//! keeps the piece's five styles and 48×11 layout, and fills them from the
//! bytes received. The meter stops when the update stops. `--check` and
//! `--json` do not draw it and do not replace either binary.
//!
//! The pinned `aster-cli-v0.1.1` archive contains only the two binaries.
//! Those binaries reject `--build-id`. `BUILD_ID` and `SHA256SUMS` are
//! optional; when both are present the pair identity must match or the
//! previous files stay.

use anyhow::{Context, Result, bail};
use sha2::Digest;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const RELEASE: &str = "aster-cli-v0.1.1";
const OFFICIAL_BASE: &str =
    "https://github.com/aster-ai-lab/aster/releases/download/aster-cli-v0.1.1";
const PAIR_PREFIX: &str = "aster-cli-pair-v1:";
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const READ_CHUNK: usize = 4096;

const COLS: usize = 48;
const ROWS: usize = 11;
const BAR_W: usize = 28;
const LABELS: [&str; 5] = ["fetching", "unpacking", "indexing", "building", "linking"];
const EIGHTHS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];

pub struct Request {
    pub check: bool,
    pub json: bool,
    pub force_reinstall: bool,
    pub version: Option<String>,
    pub background: bool,
}

enum Asset {
    Official { name: &'static str },
    Local(PathBuf),
}

pub fn run(request: Request) -> Result<()> {
    if let Some(version) = request.version.as_deref()
        && !is_pinned_version(version)
    {
        bail!("Aster CLI updates install {RELEASE} only");
    }
    if request.background && !request.check && !request.json {
        bail!("Aster does not apply background updates");
    }
    refuse_foreign_installer_env()?;
    let asset = resolve_asset()?;
    if request.check || request.json {
        return print_status(&asset, request.json);
    }
    // `--force-reinstall` still leaves an identical pair byte-for-byte in place.
    let _ = request.force_reinstall;
    match install_release(&asset)? {
        InstallOutcome::Installed { identity, dir } => {
            let (aster_name, acp_name) = binary_names();
            println!("installed Aster v0.1.1 into {}", dir.display());
            if let Some(identity) = identity {
                println!("identity={identity}");
            }
            println!("aster={}", dir.join(aster_name).display());
            println!("a3s-code-acp={}", dir.join(acp_name).display());
        }
        InstallOutcome::Current { identity } => {
            println!("Aster CLI pair is current");
            println!("release={RELEASE}");
            if let Some(identity) = identity {
                println!("identity={identity}");
            }
        }
    }
    Ok(())
}

fn is_pinned_version(version: &str) -> bool {
    matches!(version, "0.1.1" | "v0.1.1" | "aster-cli-v0.1.1")
}

fn refuse_foreign_installer_env() -> Result<()> {
    if std::env::var_os("ASTER_INSTALL_URL").is_some()
        || std::env::var_os("GROK_INSTALL_URL").is_some()
    {
        bail!("this updater does not download that binary");
    }
    Ok(())
}

fn resolve_asset() -> Result<Asset> {
    let name = asset_name();
    let base = std::env::var("ASTER_RELEASE_BASE").unwrap_or_else(|_| OFFICIAL_BASE.to_string());
    if base.to_ascii_lowercase().contains("x.ai") {
        bail!("this updater does not download that binary");
    }
    if base == OFFICIAL_BASE {
        return Ok(Asset::Official { name });
    }
    if !base.contains("://") {
        let dir = PathBuf::from(&base);
        if dir.is_dir() {
            return Ok(Asset::Local(dir.join(name)));
        }
    }
    bail!("this updater does not download that binary");
}

fn asset_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "aster-cli-v0.1.1-macos.tar.gz"
    }
    #[cfg(target_os = "linux")]
    {
        "aster-cli-v0.1.1-linux.tar.gz"
    }
    #[cfg(windows)]
    {
        "aster-cli-v0.1.1-windows.zip"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        "aster-cli-v0.1.1-unsupported.tar.gz"
    }
}

fn binary_names() -> (&'static str, &'static str) {
    #[cfg(windows)]
    {
        ("aster.exe", "a3s-code-acp.exe")
    }
    #[cfg(not(windows))]
    {
        ("aster", "a3s-code-acp")
    }
}

fn archive_is_zip() -> bool {
    cfg!(windows)
}

fn allowed_member(name: &str) -> bool {
    let (aster_name, acp_name) = binary_names();
    name == aster_name || name == acp_name || name == "BUILD_ID" || name == "SHA256SUMS"
}

fn install_dir() -> Result<PathBuf> {
    if let Some(argv0) = std::env::args_os().next() {
        let raw = PathBuf::from(&argv0);
        let has_parent = raw.components().nth(1).is_some() || raw.is_absolute();
        if has_parent {
            let candidate = if raw.is_absolute() {
                raw
            } else {
                std::env::current_dir()
                    .context("cannot resolve the invoked Aster path")?
                    .join(raw)
            };
            if candidate.is_file()
                && let Some(parent) = candidate.parent()
            {
                if parent.as_os_str().is_empty() {
                    return std::env::current_dir().context("cannot resolve the install directory");
                }
                return Ok(parent.to_path_buf());
            }
        }
    }
    let exe = std::env::current_exe().context("cannot locate the running Aster binary")?;
    exe.parent()
        .context("cannot resolve the install directory")
        .map(Path::to_path_buf)
}

fn print_status(asset: &Asset, json: bool) -> Result<()> {
    let (available, error) = match asset {
        Asset::Official { .. } => (false, None),
        Asset::Local(path) => match compare_local(path) {
            Ok(Compare::Same) => (false, None),
            Ok(Compare::Different) => (true, None),
            Err(error) => (false, Some(error.to_string())),
        },
    };
    let status = a3s_code_update::UpdateStatus {
        current_version: env!("VERSION_WITH_COMMIT").to_string(),
        latest_version: (available || matches!(asset, Asset::Local(_)))
            .then(|| "0.1.1".to_string()),
        update_available: available,
        installer: Some("aster-cli".to_string()),
        channel: a3s_code_update::channel_name()
            .unwrap_or("unknown")
            .to_string(),
        auto_update: Some(false),
        error: error.clone(),
    };
    if json {
        return a3s_code_update::auto_update::print_update_status(&status, true);
    }
    if let Some(error) = error {
        println!("error={error}");
    }
    println!("release={RELEASE}");
    println!("updateAvailable={available}");
    println!("autoUpdate=false");
    Ok(())
}

enum Compare {
    Same,
    Different,
}

fn compare_local(path: &Path) -> Result<Compare> {
    let work = WorkDir::new()?;
    let archive = work.path.join("archive");
    let mut meter = Progress::disabled();
    copy_chunks(path, &archive, &mut meter)?;
    let unpacked = work.path.join("unpack");
    unpack_archive(&archive, &unpacked)?;
    let pair = verify_pair(&unpacked)?;
    let dir = install_dir()?;
    let (aster_name, acp_name) = binary_names();
    if files_identical(&pair.aster, &dir.join(aster_name))
        && files_identical(&pair.acp, &dir.join(acp_name))
    {
        Ok(Compare::Same)
    } else {
        Ok(Compare::Different)
    }
}

enum InstallOutcome {
    Installed {
        identity: Option<String>,
        dir: PathBuf,
    },
    Current {
        identity: Option<String>,
    },
}

fn install_release(asset: &Asset) -> Result<InstallOutcome> {
    let work = WorkDir::new()?;
    let archive = work.path.join("archive");
    let mut meter = Progress::for_terminal();
    let copied = match asset {
        Asset::Official { name } => download_official(name, &archive, &mut meter),
        Asset::Local(path) => copy_chunks(path, &archive, &mut meter),
    };
    meter.finish();
    copied?;
    let unpacked = work.path.join("unpack");
    unpack_archive(&archive, &unpacked).context("release archive is incomplete")?;
    let pair = verify_pair(&unpacked)?;
    let dir = install_dir()?;
    let (aster_name, acp_name) = binary_names();
    let dest_aster = dir.join(aster_name);
    let dest_acp = dir.join(acp_name);
    if files_identical(&pair.aster, &dest_aster) && files_identical(&pair.acp, &dest_acp) {
        return Ok(InstallOutcome::Current {
            identity: pair.identity,
        });
    }
    promote_pair(&dir, &dest_aster, &dest_acp, &pair)?;
    Ok(InstallOutcome::Installed {
        identity: pair.identity,
        dir,
    })
}

struct WorkDir {
    path: PathBuf,
}

impl WorkDir {
    fn new() -> Result<Self> {
        let mut n = 0_u32;
        loop {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!(
                "aster-cli-update-{}-{nanos}-{n}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && n < 100 => {
                    n += 1;
                }
                Err(error) => {
                    return Err(error).context("cannot create a temporary update directory");
                }
            }
        }
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct VerifiedPair {
    aster: PathBuf,
    acp: PathBuf,
    identity: Option<String>,
}

fn verify_pair(unpacked: &Path) -> Result<VerifiedPair> {
    let (aster_name, acp_name) = binary_names();
    let aster = unpacked.join(aster_name);
    let acp = unpacked.join(acp_name);
    if !aster.is_file() || !acp.is_file() {
        bail!("archive did not contain both aster-cli binaries");
    }
    make_executable(&aster)?;
    make_executable(&acp)?;
    let sums = unpacked.join("SHA256SUMS");
    let build_id = unpacked.join("BUILD_ID");
    let sums_present = sums.is_file();
    let id_present = build_id.is_file();
    // The pinned release is the two binaries. Identity checks apply only when
    // the archive carries both metadata files.
    if !sums_present && !id_present {
        return Ok(VerifiedPair {
            aster,
            acp,
            identity: None,
        });
    }
    if !sums_present || !id_present {
        bail!("release is missing paired build identity or checksums");
    }
    let expected = format!(
        "{}  {aster_name}\n{}  {acp_name}\n",
        sha256_hex(&aster)?,
        sha256_hex(&acp)?
    );
    let actual = fs::read(&sums).context("cannot read paired checksums")?;
    if actual != expected.as_bytes() {
        bail!("paired binary checksum mismatch");
    }
    let identity = fs::read_to_string(&build_id).context("cannot read paired build identity")?;
    let identity = identity.trim_end_matches(['\r', '\n']);
    if !identity.starts_with(PAIR_PREFIX) || identity.chars().any(char::is_whitespace) {
        bail!("paired binary source identity mismatch");
    }
    let aster_id = command_build_id(&aster)?;
    let acp_id = command_build_id(&acp)?;
    if aster_id != identity || acp_id != identity {
        bail!("paired binary source identity mismatch");
    }
    Ok(VerifiedPair {
        aster,
        acp,
        identity: Some(identity.to_string()),
    })
}

fn command_build_id(path: &Path) -> Result<String> {
    let mut child = Command::new(path)
        .arg("--build-id")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("cannot read the build identity of {}", path.display()))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > Duration::from_secs(10) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("build identity check timed out");
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => return Err(error).context("build identity check failed"),
        }
    };
    if !status.success() {
        bail!("build identity check failed");
    }
    let stdout = child.stdout.take().context("build identity check failed")?;
    let mut body = String::new();
    stdout
        .take(1024)
        .read_to_string(&mut body)
        .context("build identity check failed")?;
    let id = body.trim();
    if id.is_empty() || id.lines().nth(1).is_some() || !id.starts_with(PAIR_PREFIX) {
        bail!("paired binary source identity mismatch");
    }
    Ok(id.to_string())
}

fn sha256_hex(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0_u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let Some(chunk) = buf.get(..n) else {
            bail!("short read");
        };
        hasher.update(chunk);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn files_identical(left: &Path, right: &Path) -> bool {
    let Ok(left_meta) = fs::metadata(left) else {
        return false;
    };
    let Ok(right_meta) = fs::metadata(right) else {
        return false;
    };
    if !left_meta.is_file() || !right_meta.is_file() || left_meta.len() != right_meta.len() {
        return false;
    }
    let Ok(mut left_file) = File::open(left) else {
        return false;
    };
    let Ok(mut right_file) = File::open(right) else {
        return false;
    };
    let mut left_buf = [0_u8; 8192];
    let mut right_buf = [0_u8; 8192];
    loop {
        let Ok(left_n) = left_file.read(&mut left_buf) else {
            return false;
        };
        let Ok(right_n) = right_file.read(&mut right_buf) else {
            return false;
        };
        if left_n != right_n {
            return false;
        }
        if left_n == 0 {
            return true;
        }
        let (Some(left_chunk), Some(right_chunk)) =
            (left_buf.get(..left_n), right_buf.get(..right_n))
        else {
            return false;
        };
        if left_chunk != right_chunk {
            return false;
        }
    }
}

fn promote_pair(dir: &Path, dest_aster: &Path, dest_acp: &Path, pair: &VerifiedPair) -> Result<()> {
    if dest_aster.is_dir() || dest_acp.is_dir() {
        bail!("binary destination is a directory");
    }
    fs::create_dir_all(dir).context("cannot create the install directory")?;
    let lock = dir.join(".aster-install.lock");
    fs::create_dir(&lock).context("another pair installation needs to finish")?;
    let stage_path = dir.join(format!(".aster-install.{}", std::process::id()));
    if stage_path.exists() {
        let _ = fs::remove_dir(&lock);
        bail!("another pair installation needs to finish");
    }
    if let Err(error) = fs::create_dir(&stage_path) {
        let _ = fs::remove_dir(&lock);
        return Err(error).context("cannot stage the Aster CLI pair");
    }
    let mut stage = Stage {
        dir: stage_path,
        lock,
        dest_aster: dest_aster.to_path_buf(),
        dest_acp: dest_acp.to_path_buf(),
        had_aster: dest_aster.exists(),
        had_acp: dest_acp.exists(),
        aster_promoted: false,
        acp_promoted: false,
        committed: false,
    };
    if stage.had_aster {
        fs::copy(dest_aster, stage.dir.join("old-aster"))
            .context("cannot preserve the current aster")?;
    }
    if stage.had_acp {
        fs::copy(dest_acp, stage.dir.join("old-acp"))
            .context("cannot preserve the current a3s-code-acp")?;
    }
    let new_aster = stage.dir.join("new-aster");
    let new_acp = stage.dir.join("new-acp");
    fs::copy(&pair.aster, &new_aster).context("cannot stage aster")?;
    fs::copy(&pair.acp, &new_acp).context("cannot stage a3s-code-acp")?;
    make_executable(&new_aster)?;
    make_executable(&new_acp)?;
    replace_file(&new_aster, dest_aster).context("cannot install aster")?;
    stage.aster_promoted = true;
    make_executable(dest_aster)?;
    replace_file(&new_acp, dest_acp).context("cannot install a3s-code-acp")?;
    stage.acp_promoted = true;
    make_executable(dest_acp)?;
    stage.committed = true;
    Ok(())
}

struct Stage {
    dir: PathBuf,
    lock: PathBuf,
    dest_aster: PathBuf,
    dest_acp: PathBuf,
    had_aster: bool,
    had_acp: bool,
    aster_promoted: bool,
    acp_promoted: bool,
    committed: bool,
}

impl Drop for Stage {
    fn drop(&mut self) {
        if !self.committed {
            let aster_ok = restore_one(
                self.aster_promoted,
                self.had_aster,
                &self.dir.join("old-aster"),
                &self.dest_aster,
            );
            let acp_ok = restore_one(
                self.acp_promoted,
                self.had_acp,
                &self.dir.join("old-acp"),
                &self.dest_acp,
            );
            if !aster_ok || !acp_ok {
                eprintln!(
                    "error: pair recovery needs attention; backup retained at {}",
                    self.dir.display()
                );
                return;
            }
        }
        let _ = fs::remove_dir_all(&self.dir);
        let _ = fs::remove_dir(&self.lock);
    }
}

fn restore_one(promoted: bool, had_old: bool, backup: &Path, dest: &Path) -> bool {
    if !promoted {
        return true;
    }
    if had_old {
        replace_file(backup, dest).is_ok()
    } else {
        fs::remove_file(dest).is_ok() || !dest.exists()
    }
}

fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_error) if to.exists() => {
            fs::remove_file(to)?;
            match fs::rename(from, to) {
                Ok(()) => Ok(()),
                Err(rename_error) => {
                    let _ = fs::copy(from, to);
                    Err(rename_error)
                }
            }
        }
        Err(error) => Err(error),
    }
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("cannot mark {} executable", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn download_official(name: &str, dest: &Path, meter: &mut Progress) -> Result<()> {
    let url = format!("{OFFICIAL_BASE}/{name}");
    if !url_is_official_asset(&url, name) {
        bail!("this updater does not download that binary");
    }
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("too many redirects");
            }
            if !github_asset_host(attempt.url()) {
                return attempt.error("this updater does not download that binary");
            }
            attempt.follow()
        }))
        .timeout(Duration::from_secs(300))
        .user_agent(concat!("aster-cli/", "0.1.1"))
        .build()
        .context("cannot prepare the release download")?;
    let mut response = client
        .get(&url)
        .send()
        .context("cannot download the Aster CLI release")?;
    if !github_asset_host(response.url()) || !response.status().is_success() {
        bail!("cannot download the Aster CLI release");
    }
    let total = response.content_length().unwrap_or(0);
    if total > MAX_ARCHIVE {
        bail!("release archive is too large");
    }
    meter.push(0, total);
    let mut file = File::create(dest).context("cannot store the release archive")?;
    let mut got = 0_u64;
    let mut buf = [0_u8; READ_CHUNK];
    loop {
        let n = response
            .read(&mut buf)
            .context("release download ended early")?;
        if n == 0 {
            break;
        }
        let Some(chunk) = buf.get(..n) else {
            bail!("short read");
        };
        file.write_all(chunk)?;
        got = got.saturating_add(n as u64);
        if got > MAX_ARCHIVE {
            bail!("release archive is too large");
        }
        let denom = if total == 0 { got } else { total };
        meter.push(got, denom);
    }
    file.sync_all()?;
    if total != 0 && got != total {
        bail!("release download ended early");
    }
    Ok(())
}

fn url_is_official_asset(url: &str, name: &str) -> bool {
    url == format!("{OFFICIAL_BASE}/{name}")
}

fn github_asset_host(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    if host.contains("x.ai") {
        return false;
    }
    matches!(
        host.as_str(),
        "github.com"
            | "release-assets.githubusercontent.com"
            | "objects.githubusercontent.com"
            | "github-releases.githubusercontent.com"
    )
}

fn copy_chunks(src: &Path, dest: &Path, meter: &mut Progress) -> Result<()> {
    let total = fs::metadata(src)
        .with_context(|| format!("release archive is missing: {}", src.display()))?
        .len();
    if total > MAX_ARCHIVE {
        bail!("release archive is too large");
    }
    let mut input = File::open(src).context("release archive is missing")?;
    let mut output = File::create(dest).context("cannot store the release archive")?;
    meter.push(0, total);
    let mut got = 0_u64;
    let mut buf = [0_u8; READ_CHUNK];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let Some(chunk) = buf.get(..n) else {
            bail!("short read");
        };
        output.write_all(chunk)?;
        got = got.saturating_add(n as u64);
        if got > MAX_ARCHIVE {
            bail!("release archive is too large");
        }
        meter.push(got, total);
    }
    output.sync_all()?;
    Ok(())
}

fn unpack_archive(archive: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    if archive_is_zip() {
        unpack_zip(archive, dest)
    } else {
        unpack_tar(archive, dest)
    }
}

fn unpack_tar(archive: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    let mut seen = std::collections::BTreeSet::new();
    for entry in tar.entries().context("release archive is incomplete")? {
        let mut entry = entry.context("release archive is incomplete")?;
        let is_file = entry.header().entry_type().is_file();
        let size = entry.header().size().unwrap_or(MAX_ARCHIVE + 1);
        let name = {
            let path = entry
                .path()
                .context("archive member is not a release file")?;
            let Some(name) = sole_name(&path) else {
                bail!("archive member is not a release file");
            };
            if !allowed_member(name) {
                bail!("archive member is not a release file");
            }
            name.to_string()
        };
        if !is_file {
            bail!("archive member is not a release file");
        }
        if size > MAX_ARCHIVE {
            bail!("release archive is too large");
        }
        let mut out = File::create(dest.join(&name))?;
        std::io::copy(&mut entry, &mut out).context("release archive is incomplete")?;
        seen.insert(name);
    }
    require_members(&seen)
}

fn unpack_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).context("release archive is incomplete")?;
    let mut seen = std::collections::BTreeSet::new();
    let count = zip.len();
    let mut index = 0_usize;
    while index < count {
        let mut entry = zip
            .by_index(index)
            .context("release archive is incomplete")?;
        index += 1;
        if !entry.is_file() {
            bail!("archive member is not a release file");
        }
        let Some(path) = entry.enclosed_name() else {
            bail!("archive member is not a release file");
        };
        let Some(name) = sole_name(&path) else {
            bail!("archive member is not a release file");
        };
        if !allowed_member(name) {
            bail!("archive member is not a release file");
        }
        if entry.size() > MAX_ARCHIVE {
            bail!("release archive is too large");
        }
        let mut out = File::create(dest.join(name))?;
        std::io::copy(&mut entry, &mut out).context("release archive is incomplete")?;
        seen.insert(name.to_string());
    }
    require_members(&seen)
}

fn require_members(seen: &std::collections::BTreeSet<String>) -> Result<()> {
    let (aster_name, acp_name) = binary_names();
    if !seen.contains(aster_name) || !seen.contains(acp_name) {
        bail!("archive did not contain both aster-cli binaries");
    }
    Ok(())
}

fn sole_name(path: &Path) -> Option<&str> {
    let mut found: Option<&str> = None;
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                if found.is_some() {
                    return None;
                }
                found = part.to_str();
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    found
}

struct Progress {
    enabled: bool,
    open: bool,
    last: Option<String>,
}

impl Progress {
    fn for_terminal() -> Self {
        Self {
            enabled: std::io::IsTerminal::is_terminal(&std::io::stderr()),
            open: false,
            last: None,
        }
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            open: false,
            last: None,
        }
    }

    fn push(&mut self, got: u64, total: u64) {
        if !self.enabled {
            return;
        }
        let fraction = if total == 0 {
            1.0
        } else {
            (got as f64 / total as f64).clamp(0.0, 1.0)
        };
        let frame = render_progress_bar(fraction);
        if self.last.as_ref() == Some(&frame) {
            return;
        }
        let mut err = std::io::stderr().lock();
        if self.open {
            let _ = write!(err, "\x1b[{ROWS}A");
        } else {
            let _ = write!(err, "\x1b[?25l");
            self.open = true;
        }
        let _ = writeln!(err, "{frame}");
        let _ = err.flush();
        self.last = Some(frame);
    }

    fn finish(&mut self) {
        if !self.open {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\x1b[?25h");
        let _ = err.flush();
        self.open = false;
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.finish();
    }
}

fn render_progress_bar(fraction: f64) -> String {
    let fraction = fraction.clamp(0.0, 1.0);
    let bars = [
        style_equals(fraction),
        style_block(fraction),
        style_hash(fraction),
        style_lamps(fraction),
        style_rule(fraction),
    ];
    let mut lines = Vec::with_capacity(ROWS);
    for row in 0..ROWS {
        if row % 2 == 0 {
            lines.push(" ".repeat(COLS));
            continue;
        }
        let index = (row - 1) / 2;
        let Some(bar) = bars.get(index) else {
            lines.push(" ".repeat(COLS));
            continue;
        };
        lines.push(pad_cols(format!(
            "  {}{bar}  {}",
            label(index),
            percent(fraction)
        )));
    }
    lines.join("\n")
}

fn pad_cols(mut line: String) -> String {
    while line.chars().count() < COLS {
        line.push(' ');
    }
    line
}

fn label(index: usize) -> String {
    let text = LABELS.get(index).copied().unwrap_or("");
    let cut: String = text.chars().take(9).collect();
    format!("{cut:<10}")
}

fn percent(fraction: f64) -> String {
    let value = (fraction * 100.0).floor() as u32;
    format!("{value:>3}%")
}

fn filled_cells(fraction: f64, width: usize) -> usize {
    (fraction * width as f64).floor() as usize
}

fn style_equals(fraction: f64) -> String {
    let inner = BAR_W - 2;
    let n = filled_cells(fraction, inner);
    let mut line = String::from("[");
    line.push_str(&"=".repeat(n));
    if n < inner {
        line.push('>');
        line.push_str(&" ".repeat(inner - 1 - n));
    }
    line.push(']');
    line
}

fn style_block(fraction: f64) -> String {
    let inner = BAR_W - 2;
    let eighths = (fraction * inner as f64 * 8.0).round() as usize;
    let n = eighths >> 3;
    let mut line = String::from("▕");
    line.push_str(&"█".repeat(n));
    if n < inner {
        line.push(EIGHTHS.get(eighths & 7).copied().unwrap_or(' '));
        line.push_str(&" ".repeat(inner - 1 - n));
    }
    line.push('▏');
    line
}

fn style_hash(fraction: f64) -> String {
    let inner = BAR_W - 2;
    let n = filled_cells(fraction, inner);
    let mut line = String::from("[");
    line.push_str(&"#".repeat(n));
    line.push_str(&".".repeat(inner - n));
    line.push(']');
    line
}

fn style_lamps(fraction: f64) -> String {
    let n = (fraction * 14.0 + 1e-9).floor() as usize;
    let raw = format!("{}{}", "█ ".repeat(n), "░ ".repeat(14 - n));
    let mut line: String = raw.chars().take(BAR_W - 1).collect();
    line.push(' ');
    line
}

fn style_rule(fraction: f64) -> String {
    let steps = (fraction * BAR_W as f64 * 2.0).floor() as usize;
    let n = steps >> 1;
    let mut line = "━".repeat(n);
    if n < BAR_W {
        line.push(if steps & 1 == 1 { '╾' } else { '─' });
        line.push_str(&"─".repeat(BAR_W - 1 - n));
    }
    line
}
