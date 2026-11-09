//! Install the Aster CLI pair from this repository's `aster-cli` releases.
//!
//! Download progress measures received bytes only. Verification, extraction,
//! and installation report their own stages. Non-interactive output uses
//! plain stage lines; `--check` and `--json` do not draw progress or install.
//!
//! Releases are discovered at run time from the channel manifest each
//! release publishes as `latest.json`. `aster update` reads the newest
//! manifest, `aster update --version X.Y.Z` reads that release's manifest.
//! The manifest supplies the version, the asset name, and its SHA-256
//! digest; every download URL is built from the constants below, so a
//! manifest can never point the updater at another host. An unpinned
//! update installs strictly newer releases only; an explicit `--version`
//! may pin or roll back. `ASTER_RELEASE_BASE` names a local directory
//! holding `latest.json` and the assets for hermetic checks.
//!
//! The archive contains the two binaries, `BUILD_ID`, `SHA256SUMS`, and the
//! `embedding/` Model2Vec asset files used by `aster-acp` `local_cpu` retrieval.
//! The two-file form without metadata stays installable; the manifest
//! digest still bounds it. Archives must not contain a `sandbox/` tree.

use anyhow::{Context, Result, bail};
use sha2::Digest;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const OFFICIAL_RELEASES: &str = "https://github.com/aster-ai-lab/aster/releases";
const MANIFEST_NAME: &str = "latest.json";
const MAX_MANIFEST: u64 = 64 * 1024;
const PAIR_PREFIX: &str = "aster-cli-pair-v1:";
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const READ_CHUNK: usize = 4096;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

pub struct Request {
    pub check: bool,
    pub json: bool,
    pub force_reinstall: bool,
    pub version: Option<String>,
    pub background: bool,
}

enum ChannelSource {
    Official { url: String },
    Local(PathBuf),
}

/// The release the updater resolved: where to fetch the archive from and
/// what its digest must be.
struct Channel {
    version: Version,
    tag: String,
    asset_name: String,
    sha256: String,
    source: ChannelSource,
}

/// `major.minor.patch`, the only version shape this channel publishes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Version(u32, u32, u32);

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.');
        let mut numbers = [0_u32; 3];
        for slot in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            *slot = part.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        Some(Self(numbers[0], numbers[1], numbers[2]))
    }

    fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("package version is a release triple")
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Accepts `0.1.2`, `v0.1.2`, and `aster-cli-v0.1.2` for `--version`.
fn normalize_requested_version(text: &str) -> Option<Version> {
    let text = text
        .strip_prefix("aster-cli-v")
        .or_else(|| text.strip_prefix('v'))
        .unwrap_or(text);
    Version::parse(text)
}

/// The release channel this host must install.
fn channel_for(os: &str, arch: &str) -> &'static str {
    match (os, arch) {
        ("macos", _) => "macos",
        ("windows", _) => "windows",
        ("linux", "x86_64") => "linux",
        ("linux", "aarch64") => "linux-arm64",
        _ => "unsupported",
    }
}

fn platform_key() -> &'static str {
    channel_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn asset_name_for(version: Version, platform: &str) -> String {
    let extension = if platform == "windows" {
        "zip"
    } else {
        "tar.gz"
    };
    format!("aster-cli-v{version}-{platform}.{extension}")
}

fn expected_asset_name(version: Version) -> String {
    asset_name_for(version, platform_key())
}

pub fn run(request: Request) -> Result<()> {
    if request.background && !request.check && !request.json {
        bail!("Aster does not apply background updates");
    }
    let requested = match request.version.as_deref() {
        None => None,
        Some(text) => match normalize_requested_version(text) {
            Some(version) => Some(version),
            None => bail!("--version wants a release triple such as 0.1.2"),
        },
    };
    refuse_foreign_installer_env()?;
    let mut meter = Progress::new(!request.check && !request.json);
    meter.show("Checking for updates...");
    let channel = resolve_channel(requested)?;
    if request.check || request.json {
        return print_status(&channel, request.json);
    }
    // An unpinned update installs strictly newer releases only. An explicit
    // `--version` (and `--force-reinstall`) may reinstall the same release.
    if requested.is_none() && !request.force_reinstall && channel.version <= Version::current() {
        meter.finish();
        print_current(&channel.version, &channel.tag, None);
        return Ok(());
    }
    meter.finish();
    eprintln!(
        "Updating Aster {} -> {}",
        Version::current(),
        channel.version
    );
    let outcome = install_release(&channel, request.force_reinstall, &mut meter)?;
    meter.finish();
    match outcome {
        InstallOutcome::Installed {
            identity,
            dir,
            version,
        } => {
            let (aster_name, acp_name) = binary_names();
            println!("installed Aster v{version} into {}", dir.display());
            if let Some(identity) = identity {
                println!("identity={identity}");
            }
            println!("aster={}", dir.join(aster_name).display());
            println!("aster-acp={}", dir.join(acp_name).display());
        }
        InstallOutcome::Current {
            identity,
            version,
            tag,
        } => print_current(&version, &tag, identity),
    }
    Ok(())
}

fn print_current(version: &Version, tag: &str, identity: Option<String>) {
    println!("Aster CLI pair is current");
    println!("release={tag}");
    println!("latest={version}");
    if let Some(identity) = identity {
        println!("identity={identity}");
    }
}

fn refuse_foreign_installer_env() -> Result<()> {
    if std::env::var_os("ASTER_INSTALL_URL").is_some()
        || std::env::var_os("GROK_INSTALL_URL").is_some()
    {
        bail!("this updater does not download that binary");
    }
    Ok(())
}

/// Reads the channel manifest: the newest release by default, the named
/// release for an explicit `--version`, or a local directory override.
fn resolve_channel(requested: Option<Version>) -> Result<Channel> {
    if let Ok(base) = std::env::var("ASTER_RELEASE_BASE") {
        if base.contains("://") {
            bail!("this updater does not download that binary");
        }
        let dir = PathBuf::from(&base);
        if !dir.is_dir() {
            bail!("ASTER_RELEASE_BASE is not a local release directory");
        }
        let manifest = read_local_manifest(&dir)?;
        return channel_from_manifest(manifest, requested, |name| {
            let path = dir.join(name);
            if !path.is_file() {
                bail!("local release is missing {name}");
            }
            Ok(ChannelSource::Local(path))
        });
    }
    let (url, tag_hint) = match requested {
        Some(version) => (
            format!("{OFFICIAL_RELEASES}/download/aster-cli-v{version}/{MANIFEST_NAME}"),
            format!("aster-cli-v{version}"),
        ),
        None => (
            format!("{OFFICIAL_RELEASES}/download/aster-cli-stable/{MANIFEST_NAME}"),
            String::new(),
        ),
    };
    let bytes = fetch_manifest(&url)?;
    let manifest = parse_manifest(&bytes, &tag_hint)?;
    let release_tag = manifest.tag.clone();
    channel_from_manifest(manifest, requested, move |name| {
        Ok(ChannelSource::Official {
            url: format!("{OFFICIAL_RELEASES}/download/{release_tag}/{name}"),
        })
    })
}

fn read_local_manifest(dir: &Path) -> Result<Manifest> {
    let path = dir.join(MANIFEST_NAME);
    let file = File::open(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let bytes = read_manifest_body(file)?;
    parse_manifest(&bytes, "")
}

fn read_manifest_body(reader: impl Read) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    reader
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut body)
        .context("cannot read the channel manifest")?;
    if body.len() as u64 > MAX_MANIFEST {
        bail!("channel manifest is too large");
    }
    Ok(body)
}

fn channel_from_manifest(
    manifest: Manifest,
    requested: Option<Version>,
    source: impl Fn(&str) -> Result<ChannelSource>,
) -> Result<Channel> {
    if let Some(version) = requested
        && manifest.version != version
    {
        bail!(
            "release {} does not match the requested version {version}",
            manifest.tag
        );
    }
    let asset_name = expected_asset_name(manifest.version);
    let asset = manifest
        .assets
        .into_iter()
        .find(|asset| asset.name == asset_name)
        .with_context(|| format!("channel manifest has no {asset_name}"))?;
    Ok(Channel {
        version: manifest.version,
        tag: manifest.tag,
        asset_name,
        sha256: asset.sha256,
        source: source(&asset.name)?,
    })
}

struct ManifestAsset {
    name: String,
    sha256: String,
}

struct Manifest {
    version: Version,
    tag: String,
    assets: Vec<ManifestAsset>,
}

/// Strict parse of the channel manifest. Unknown keys are ignored so a
/// newer release cannot strand an older updater; every known field must
/// still be exact, and asset names must be the canonical per-release names
/// so a manifest cannot smuggle in an arbitrary filename.
fn parse_manifest(bytes: &[u8], tag_hint: &str) -> Result<Manifest> {
    let body = serde_json::from_slice::<serde_json::Value>(bytes)
        .context("channel manifest is not JSON")?;
    let Some(object) = body.as_object() else {
        bail!("channel manifest is not an object");
    };
    let version = object
        .get("version")
        .and_then(|v| v.as_str())
        .and_then(Version::parse)
        .context("channel manifest has no release version")?;
    let tag = object
        .get("tag")
        .and_then(|v| v.as_str())
        .context("channel manifest has no tag")?;
    if tag != format!("aster-cli-v{version}") {
        bail!("channel manifest tag does not match its version");
    }
    if !tag_hint.is_empty() && tag != tag_hint {
        bail!("channel manifest tag does not match the requested release");
    }
    let Some(assets) = object.get("assets").and_then(|v| v.as_object()) else {
        bail!("channel manifest has no assets");
    };
    let mut parsed = Vec::new();
    for (platform, entry) in assets {
        // Ignore unknown platforms so a newer channel cannot strand an older
        // updater. Known platforms stay exact (name + sha256 shape).
        if !matches!(
            platform.as_str(),
            "macos" | "linux" | "linux-arm64" | "windows"
        ) {
            continue;
        }
        let Some(entry) = entry.as_object() else {
            bail!("channel manifest asset {platform} is not an object");
        };
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let sha256 = entry
            .get("sha256")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let canonical = format!(
            "aster-cli-v{version}-{platform}.{}",
            if platform == "windows" {
                "zip"
            } else {
                "tar.gz"
            }
        );
        if name != canonical || sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            bail!("channel manifest asset {platform} is malformed");
        }
        parsed.push(ManifestAsset {
            name,
            sha256: sha256.to_ascii_lowercase(),
        });
    }
    if parsed.is_empty() {
        bail!("channel manifest has no assets");
    }
    Ok(Manifest {
        version,
        tag: tag.to_string(),
        assets: parsed,
    })
}

fn binary_names() -> (&'static str, &'static str) {
    #[cfg(windows)]
    {
        ("aster.exe", "aster-acp.exe")
    }
    #[cfg(not(windows))]
    {
        ("aster", "aster-acp")
    }
}

fn archive_is_zip() -> bool {
    cfg!(windows)
}

const EMBEDDING_MEMBERS: &[&str] = &[
    "embedding/potion-code-16M-v2.safetensors",
    "embedding/potion-code-16M-v2.tokenizer.json",
    "embedding/potion-code-16M-v2.config.json",
];

fn allowed_member(name: &str) -> bool {
    let (aster_name, acp_name) = binary_names();
    name == aster_name
        || name == acp_name
        || name == "BUILD_ID"
        || name == "SHA256SUMS"
        || EMBEDDING_MEMBERS.contains(&name)
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

fn print_status(channel: &Channel, json: bool) -> Result<()> {
    let available = channel.version > Version::current();
    let status = aster_update::UpdateStatus {
        current_version: env!("VERSION_WITH_COMMIT").to_string(),
        latest_version: Some(channel.version.to_string()),
        update_available: available,
        installer: Some("aster-cli".to_string()),
        channel: aster_update::channel_name()
            .unwrap_or("unknown")
            .to_string(),
        auto_update: Some(false),
        error: None,
    };
    if json {
        return aster_update::auto_update::print_update_status(&status, true);
    }
    println!("release={}", channel.tag);
    println!("latest={}", channel.version);
    println!("updateAvailable={available}");
    println!("autoUpdate=false");
    Ok(())
}

enum InstallOutcome {
    Installed {
        identity: Option<String>,
        dir: PathBuf,
        version: Version,
    },
    Current {
        identity: Option<String>,
        version: Version,
        tag: String,
    },
}

fn install_release(
    channel: &Channel,
    force_reinstall: bool,
    meter: &mut Progress,
) -> Result<InstallOutcome> {
    let work = WorkDir::new()?;
    let archive = work.path.join("archive");
    meter.show("Downloading...");
    match &channel.source {
        ChannelSource::Official { url } => {
            download_official(url, &channel.asset_name, &archive, meter)
        }
        ChannelSource::Local(path) => copy_chunks(path, &archive, meter),
    }?;
    meter.show("Verifying archive...");
    let digest = sha256_hex(&archive).context("cannot digest the release archive")?;
    if digest != channel.sha256 {
        bail!("channel checksum mismatch");
    }
    let unpacked = work.path.join("unpack");
    meter.show("Extracting...");
    unpack_archive(&archive, &unpacked).context("release archive is incomplete")?;
    meter.show("Verifying components...");
    let pair = verify_pair(&unpacked)?;
    let dir = install_dir()?;
    let (aster_name, acp_name) = binary_names();
    let dest_aster = dir.join(aster_name);
    let dest_acp = dir.join(acp_name);
    if !force_reinstall
        && files_identical(&pair.aster, &dest_aster)
        && files_identical(&pair.acp, &dest_acp)
    {
        return Ok(InstallOutcome::Current {
            identity: pair.identity,
            version: channel.version,
            tag: channel.tag.clone(),
        });
    }
    meter.show("Installing...");
    promote_pair(&dir, &dest_aster, &dest_acp, &pair)?;
    Ok(InstallOutcome::Installed {
        identity: pair.identity,
        dir,
        version: channel.version,
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
    embedding: PathBuf,
    identity: Option<String>,
}

fn verify_embedding_dir(unpacked: &Path) -> Result<PathBuf> {
    let embedding = unpacked.join("embedding");
    for name in EMBEDDING_MEMBERS {
        let file = name
            .strip_prefix("embedding/")
            .expect("embedding member prefix");
        let path = embedding.join(file);
        if !path.is_file() {
            bail!("archive did not contain embedding assets");
        }
    }
    Ok(embedding)
}

fn verify_pair(unpacked: &Path) -> Result<VerifiedPair> {
    let (aster_name, acp_name) = binary_names();
    let aster = unpacked.join(aster_name);
    let acp = unpacked.join(acp_name);
    if !aster.is_file() || !acp.is_file() {
        bail!("archive did not contain both aster-cli binaries");
    }
    if unpacked.join("sandbox").exists() {
        bail!("archive contains unexpected sandbox companions");
    }
    let embedding = verify_embedding_dir(unpacked)?;
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
            embedding,
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
    if fs::metadata(&sums)?.len() > MAX_MANIFEST || fs::metadata(&build_id)?.len() > 1024 {
        bail!("paired release metadata is too large");
    }
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
        embedding,
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if left_meta.permissions().mode() & 0o111 != right_meta.permissions().mode() & 0o111 {
            return false;
        }
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
            .context("cannot preserve the current aster-acp")?;
    }
    let new_aster = stage.dir.join("new-aster");
    let new_acp = stage.dir.join("new-acp");
    fs::copy(&pair.aster, &new_aster).context("cannot stage aster")?;
    fs::copy(&pair.acp, &new_acp).context("cannot stage aster-acp")?;
    make_executable(&new_aster)?;
    make_executable(&new_acp)?;
    replace_file(&new_aster, dest_aster).context("cannot install aster")?;
    stage.aster_promoted = true;
    make_executable(dest_aster)?;
    replace_file(&new_acp, dest_acp).context("cannot install aster-acp")?;
    stage.acp_promoted = true;
    make_executable(dest_acp)?;
    install_embedding_dir(dir, &pair.embedding)?;
    stage.committed = true;
    Ok(())
}

fn install_embedding_dir(install_root: &Path, source: &Path) -> Result<()> {
    let dest = install_root.join("embedding");
    fs::create_dir_all(&dest).context("cannot create the embedding asset directory")?;
    for name in EMBEDDING_MEMBERS {
        let file = name
            .strip_prefix("embedding/")
            .expect("embedding member prefix");
        let from = source.join(file);
        let to = dest.join(file);
        fs::copy(&from, &to)
            .with_context(|| format!("cannot install embedding asset {}", to.display()))?;
    }
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

fn https_client(timeout: Duration) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("too many redirects");
            }
            if !github_asset_host(attempt.url()) {
                return attempt.error("this updater does not download that binary");
            }
            attempt.follow()
        }))
        .timeout(timeout)
        .user_agent(concat!("aster-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("cannot prepare the release download")
}

fn fetch_manifest(url: &str) -> Result<Vec<u8>> {
    let client = https_client(Duration::from_secs(30))?;
    let response = client
        .get(url)
        .send()
        .context("cannot read the channel manifest")?;
    if !github_asset_host(response.url()) || !response.status().is_success() {
        bail!("cannot read the channel manifest");
    }
    read_manifest_body(response)
}

fn download_official(url: &str, name: &str, dest: &Path, meter: &mut Progress) -> Result<()> {
    let prefix = format!("{OFFICIAL_RELEASES}/download/aster-cli-v");
    if !url.starts_with(&prefix) || !url.ends_with(&format!("/{name}")) {
        bail!("this updater does not download that binary");
    }
    let client = https_client(Duration::from_secs(300))?;
    let mut response = client
        .get(url)
        .send()
        .context("cannot download the Aster CLI release")?;
    if !github_asset_host(response.url()) || !response.status().is_success() {
        bail!("cannot download the Aster CLI release");
    }
    let total = response.content_length();
    if total.is_some_and(|total| total > MAX_ARCHIVE) {
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
        meter.push(got, total);
    }
    file.sync_all()?;
    if total.is_some_and(|total| got != total) {
        bail!("release download ended early");
    }
    Ok(())
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
    meter.push(0, Some(total));
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
        meter.push(got, Some(total));
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
    let mut total = 0_u64;
    for entry in tar.entries().context("release archive is incomplete")? {
        let mut entry = entry.context("release archive is incomplete")?;
        let is_file = entry.header().entry_type().is_file();
        let size = entry.header().size().unwrap_or(MAX_ARCHIVE + 1);
        let name = {
            let path = entry
                .path()
                .context("archive member is not a release file")?;
            archive_member(&path).context("archive member is not a release file")?
        };
        if !is_file {
            bail!("archive member is not a release file");
        }
        total = total
            .checked_add(size)
            .context("release archive is too large")?;
        if total > MAX_ARCHIVE {
            bail!("release archive is too large");
        }
        if !seen.insert(name.clone()) {
            bail!("duplicate archive member");
        }
        if let Some(parent) = dest.join(&name).parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(dest.join(&name))?;
        std::io::copy(&mut entry, &mut out).context("release archive is incomplete")?;
    }
    require_members(&seen)
}

fn unpack_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).context("release archive is incomplete")?;
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0_u64;
    let count = zip.len();
    let mut index = 0_usize;
    while index < count {
        let mut entry = zip
            .by_index(index)
            .context("release archive is incomplete")?;
        index += 1;
        if !entry.is_file()
            || entry
                .unix_mode()
                .is_some_and(|mode| !matches!(mode & 0o170_000, 0 | 0o100_000))
        {
            bail!("archive member is not a release file");
        }
        let Some(path) = entry.enclosed_name() else {
            bail!("archive member is not a release file");
        };
        let name = archive_member(&path).context("archive member is not a release file")?;
        total = total
            .checked_add(entry.size())
            .context("release archive is too large")?;
        if total > MAX_ARCHIVE {
            bail!("release archive is too large");
        }
        if !seen.insert(name.clone()) {
            bail!("duplicate archive member");
        }
        if let Some(parent) = dest.join(&name).parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(dest.join(&name))?;
        std::io::copy(&mut entry, &mut out).context("release archive is incomplete")?;
    }
    require_members(&seen)
}

fn require_members(seen: &std::collections::BTreeSet<String>) -> Result<()> {
    let (aster_name, acp_name) = binary_names();
    if !seen.contains(aster_name) || !seen.contains(acp_name) {
        bail!("archive did not contain both aster-cli binaries");
    }
    for member in EMBEDDING_MEMBERS {
        if !seen.contains(*member) {
            bail!("archive did not contain embedding assets");
        }
    }
    if seen
        .iter()
        .any(|name| name.starts_with("sandbox/") || name == "sandbox")
    {
        bail!("archive contains unexpected sandbox companions");
    }
    Ok(())
}

fn archive_member(path: &Path) -> Option<String> {
    if let Some(name) = sole_name(path) {
        return allowed_member(name).then(|| name.to_owned());
    }
    let name = path.to_str()?;
    allowed_member(name).then(|| name.to_owned())
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
    interactive: bool,
    open: bool,
    started: Instant,
    last_draw: Option<Instant>,
}

impl Progress {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            interactive: std::io::IsTerminal::is_terminal(&std::io::stderr())
                && std::env::var_os("TERM").as_deref() != Some(std::ffi::OsStr::new("dumb")),
            open: false,
            started: Instant::now(),
            last_draw: None,
        }
    }

    fn show(&mut self, text: &str) {
        if !self.enabled {
            return;
        }
        let mut err = std::io::stderr().lock();
        if self.interactive {
            let text: String = text.chars().take(progress_width()).collect();
            let _ = write!(err, "\r\x1b[2K{text}");
            self.open = true;
        } else {
            let _ = writeln!(err, "{text}");
        }
        let _ = err.flush();
    }

    fn push(&mut self, got: u64, total: Option<u64>) {
        if !self.enabled || !self.interactive {
            return;
        }
        if got == 0 {
            self.started = Instant::now();
        } else if total != Some(got)
            && self
                .last_draw
                .is_some_and(|last| last.elapsed() < PROGRESS_INTERVAL)
        {
            return;
        }
        let frame = render_download(got, total, self.started.elapsed(), progress_width());
        self.show(&frame);
        self.last_draw = Some(Instant::now());
    }

    fn finish(&mut self) {
        if self.open {
            let mut err = std::io::stderr().lock();
            let _ = write!(err, "\r\x1b[2K");
            let _ = err.flush();
            self.open = false;
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.finish();
    }
}

fn progress_width() -> usize {
    // Leave the last column unused so a redraw cannot trigger automatic wrapping.
    crossterm::terminal::size().map_or(79, |(columns, _)| usize::from(columns).saturating_sub(1))
}

fn render_download(got: u64, total: Option<u64>, elapsed: Duration, width: usize) -> String {
    let received = got as f64 / (1024.0 * 1024.0);
    let speed = received / elapsed.as_secs_f64().max(0.001);
    let mut line = match total {
        Some(total) => {
            let fraction = (got as f64 / total.max(1) as f64).clamp(0.0, 1.0);
            let percent = (fraction * 100.0).floor() as u32;
            let size = format!("{received:.1}/{:.1} MiB", total as f64 / (1024.0 * 1024.0));
            let suffix = format!("{percent:>3}%  {size}  {speed:.1} MiB/s");
            let bar_width = width
                .saturating_sub("Downloading  ".len() + suffix.len())
                .min(28);
            if bar_width >= 8 {
                let inner = bar_width - 2;
                let filled = (fraction * inner as f64).floor() as usize;
                let bar = if filled == inner {
                    "=".repeat(inner)
                } else {
                    format!("{}>{}", "=".repeat(filled), " ".repeat(inner - filled - 1))
                };
                format!("Downloading [{bar}] {suffix}")
            } else {
                let compact = format!("Downloading {percent:>3}%  {size}");
                if compact.len() <= width {
                    compact
                } else {
                    format!("Downloading {percent:>3}%")
                }
            }
        }
        None => format!("Downloading {received:.1} MiB  {speed:.1} MiB/s"),
    };
    line.truncate(width);
    line
}

#[cfg(test)]
mod tests;
