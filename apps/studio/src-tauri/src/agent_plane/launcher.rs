//! Locates the a3s-code ACP agent and writes the ACL config it reads model credentials from.
//! a3s-code only loads credentials from ACL, so each launch gets a generated file holding
//! exactly the provider Studio selected.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::providers::ProviderCredentials;

pub const BINARY_ENV: &str = "A3S_ACP_AGENT_BIN";
const BINARY_NAME: &str = if cfg!(windows) {
    "a3s-code-acp.exe"
} else {
    "a3s-code-acp"
};
const BUNDLED_DIR: &str = "agent";
/// Tauri puts Linux bundle resources in `/usr/lib/<productName>`.
const BUNDLED_PRODUCT: &str = "ToWork";

/// A set `A3S_ACP_AGENT_BIN` is the only candidate. An unusable value stops the search.
/// When the variable is unset: workspace debug build, bundled copy, `~/.a3s/bin`, then PATH.
///
/// A local debug build is newer source than a bundled or published file, and only
/// when this process is running from a Cargo `target/` directory.
/// An installed app has no checkout, so it uses the bundled release binary.
/// Finder-launched apps do not inherit the shell PATH, so macOS also looks
/// in Homebrew's default directories.
pub fn find_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok();
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
    select_agent_binary(
        std::env::var_os(BINARY_ENV).map(PathBuf::from),
        &workspace_debug_binary(),
        exe.as_deref()
            .and_then(|path| bundled_agent_from(path, appdir.as_deref())),
        home_dir().as_deref().and_then(installed_binary),
        path_binary(std::env::var_os("PATH"), extra_path_dirs()),
    )
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn workspace_debug_binary() -> PathBuf {
    debug_agent_candidate(
        std::env::current_exe().ok().as_deref(),
        &compile_time_debug_agent(),
    )
}

fn compile_time_debug_agent() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/debug")
        .join(BINARY_NAME)
}

/// Sibling of `current_exe` when that file exists.
/// The compile-time debug path is used only when this process is running from a Cargo `target/` directory.
fn debug_agent_candidate(current_exe: Option<&Path>, compile_time_debug: &Path) -> PathBuf {
    if let Some(executable) = current_exe {
        if let Some(sibling) = sibling_agent(executable) {
            if sibling.is_file() {
                return sibling;
            }
        }
        if running_under_cargo_target(executable) {
            return compile_time_debug.to_path_buf();
        }
        if let Some(directory) = executable.parent() {
            return directory.join(BINARY_NAME);
        }
    }
    compile_time_debug.to_path_buf()
}

fn sibling_agent(executable: &Path) -> Option<PathBuf> {
    let directory = executable.parent()?;
    let directory = if directory.file_name().is_some_and(|name| name == "deps") {
        directory.parent().unwrap_or(directory)
    } else {
        directory
    };
    Some(directory.join(BINARY_NAME))
}

fn running_under_cargo_target(executable: &Path) -> bool {
    let mut dirs = executable.ancestors().skip(1);
    let Some(parent) = dirs.next() else {
        return false;
    };
    let profile = if parent.file_name().is_some_and(|name| name == "deps") {
        let Some(profile) = dirs.next() else {
            return false;
        };
        profile
    } else {
        parent
    };
    profile
        .parent()
        .and_then(|dir| dir.file_name())
        .is_some_and(|name| name == "target")
}

fn bundled_agent_from(exe: &Path, appdir: Option<&Path>) -> Option<PathBuf> {
    bundled_agent_candidates(exe, appdir)
        .into_iter()
        .find(|path| path.is_file())
}

fn bundled_agent_candidates(exe: &Path, appdir: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = exe.parent() else {
        return Vec::new();
    };
    let mut paths = vec![
        dir.join(BUNDLED_DIR).join(BINARY_NAME),
        dir.join("../Resources").join(BUNDLED_DIR).join(BINARY_NAME),
        dir.join("../lib")
            .join(BUNDLED_PRODUCT)
            .join(BUNDLED_DIR)
            .join(BINARY_NAME),
        PathBuf::from("/usr/lib")
            .join(BUNDLED_PRODUCT)
            .join(BUNDLED_DIR)
            .join(BINARY_NAME),
    ];
    if let Some(root) = appdir {
        paths.push(
            root.join("usr/lib")
                .join(BUNDLED_PRODUCT)
                .join(BUNDLED_DIR)
                .join(BINARY_NAME),
        );
    }
    paths
}

fn installed_binary(home: &Path) -> Option<PathBuf> {
    let dir = home.join(".a3s/bin");
    let installed = dir.join(BINARY_NAME);
    if installed.is_file() {
        Some(installed)
    } else {
        newest_versioned(&dir)
    }
}

fn extra_path_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ]
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

fn path_binary(paths: Option<std::ffi::OsString>, extra: Vec<PathBuf>) -> Option<PathBuf> {
    let dirs = paths
        .as_deref()
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .chain(extra);
    dirs.map(|dir| dir.join(BINARY_NAME))
        .find(|path| path.is_file())
}

fn select_agent_binary(
    requested: Option<PathBuf>,
    workspace: &Path,
    bundled: Option<PathBuf>,
    published: Option<PathBuf>,
    path_hit: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(path) = requested {
        return override_is_executable(&path).then_some(path);
    }
    if workspace.is_file() {
        return Some(workspace.to_path_buf());
    }
    if let Some(path) = bundled.filter(|path| path.is_file()) {
        return Some(path);
    }
    published.or(path_hit)
}

fn override_is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn newest_versioned(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<(Vec<u64>, PathBuf)> = fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let version = name.strip_prefix("a3s-code-acp-")?;
            let version = version
                .strip_suffix(std::env::consts::EXE_SUFFIX)
                .unwrap_or(version);
            let parts = version
                .split('.')
                .map(|part| part.parse::<u64>().ok())
                .collect::<Option<Vec<_>>>()?;
            Some((parts, entry.path()))
        })
        .filter(|(_, path)| path.is_file())
        .collect();
    found.sort();
    found.pop().map(|(_, path)| path)
}

/// `providers "<id>" { models "<model>" {} }` with the key inline; written 0600.
pub fn write_config(
    dir: &Path,
    session_id: &str,
    provider_id: &str,
    model_id: &str,
    credentials: &ProviderCredentials,
) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{session_id}.acl"));
    fs::write(&path, render_config(provider_id, model_id, credentials))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

fn render_config(provider_id: &str, model_id: &str, credentials: &ProviderCredentials) -> String {
    let mut out = format!(
        "default_model = \"{}/{}\"\n\nproviders \"{}\" {{\n",
        quote(provider_id),
        quote(model_id),
        quote(provider_id)
    );
    if !credentials.api_key.is_empty() {
        out.push_str(&format!(
            "  api_key = \"{}\"\n",
            quote(&credentials.api_key)
        ));
    }
    if !credentials.base_url.is_empty() {
        out.push_str(&format!(
            "  base_url = \"{}\"\n",
            quote(&credentials.base_url)
        ));
    }
    out.push_str(&format!("\n  models \"{}\" {{}}\n}}\n", quote(model_id)));
    if let Some(search) = search_block(credentials) {
        out.push_str(&search);
    }
    out
}

/// The session uses the unified gateway search route when the model base URL
/// is already that gateway's `/v1` prefix and the caller key is present.
fn search_block(credentials: &ProviderCredentials) -> Option<String> {
    let base = credentials.base_url.trim().trim_end_matches('/');
    if credentials.api_key.is_empty() || !base.ends_with("/v1") {
        return None;
    }
    let endpoint = format!("{base}/search");
    if !search_endpoint_is_local_or_https(&endpoint) {
        return None;
    }
    Some(format!(
        "\nsearch {{\n  engine {{\n    a3s {{\n      enabled = true\n      endpoint = \"{}\"\n      api_key = \"{}\"\n    }}\n  }}\n}}\n",
        quote(&endpoint),
        quote(&credentials.api_key)
    ))
}

fn search_endpoint_is_local_or_https(endpoint: &str) -> bool {
    endpoint.starts_with("https://")
        || endpoint.starts_with("http://127.0.0.1")
        || endpoint.starts_with("http://localhost")
        || endpoint.starts_with("http://[::1]")
}

fn quote(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn mark_executable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let _ = path;
    }

    #[test]
    fn renders_one_provider_with_the_selected_model() {
        let text = render_config(
            "deepseek",
            "deepseek-chat",
            &ProviderCredentials {
                base_url: "https://api.deepseek.com".into(),
                api_key: "sk-\"x\"".into(),
            },
        );
        assert!(text.starts_with("default_model = \"deepseek/deepseek-chat\""));
        assert!(text.contains("providers \"deepseek\" {"));
        assert!(text.contains("api_key = \"sk-\\\"x\\\"\""));
        assert!(text.contains("base_url = \"https://api.deepseek.com\""));
        assert!(text.contains("models \"deepseek-chat\" {}"));
        assert!(!text.contains("search {"));
    }

    #[test]
    fn gateway_credentials_add_the_search_engine() {
        let text = render_config(
            "gateway",
            "deepseek-chat",
            &ProviderCredentials {
                base_url: "http://127.0.0.1:8080/v1".into(),
                api_key: "caller-secret".into(),
            },
        );
        assert!(text.contains("endpoint = \"http://127.0.0.1:8080/v1/search\""));
        assert!(text.contains("engine {\n    a3s {\n      enabled = true"));
        assert!(text.contains("api_key = \"caller-secret\""));
    }

    #[test]
    fn signed_in_gateway_session_includes_search() {
        let credentials = crate::providers::gateway_credentials("member-session");
        let text = render_config("a3s", "studio-model", &credentials);
        assert!(text.contains("/v1/search"), "{text}");
        assert!(text.contains("enabled = true"), "{text}");
        assert!(text.contains("api_key = \"member-session\""), "{text}");
    }

    #[test]
    fn requested_agent_bin_is_the_one_that_launches() {
        let dir = std::env::temp_dir().join(format!("a3s-launcher-env-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a3s-code-acp-9.1.2");
        fs::write(&path, b"").unwrap();
        mark_executable(&path);
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os(BINARY_ENV);
        // The selector reads the process environment. This test is the only writer.
        // SAFETY: env mutation races with other readers. This test owns the variable
        // and the suite runs it on one thread.
        unsafe {
            std::env::set_var(BINARY_ENV, &path);
        }
        let selected = find_binary();
        unsafe {
            match previous {
                Some(value) => std::env::set_var(BINARY_ENV, value),
                None => std::env::remove_var(BINARY_ENV),
            }
        }
        assert_eq!(selected, Some(path));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn picks_the_newest_versioned_binary() {
        let dir = std::env::temp_dir().join(format!("a3s-launcher-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "a3s-code-acp-9.1.0",
            "a3s-code-acp-9.10.0",
            "a3s-code-acp-9.2.3",
            "other",
        ] {
            fs::write(dir.join(name), b"").unwrap();
        }
        assert_eq!(
            newest_versioned(&dir),
            Some(dir.join("a3s-code-acp-9.10.0"))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_debug_build_wins_over_a_newer_published_release() {
        let dir = std::env::temp_dir().join(format!("a3s-launcher-ws-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let workspace = dir.join("a3s-code-acp");
        let published = dir.join("a3s-code-acp-9.10.0");
        fs::write(&workspace, b"").unwrap();
        fs::write(&published, b"").unwrap();
        assert_eq!(
            select_agent_binary(None, &workspace, None, Some(published), None),
            Some(workspace)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn explicit_agent_bin_wins_over_the_workspace_build() {
        let dir =
            std::env::temp_dir().join(format!("a3s-launcher-explicit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let requested = dir.join("requested");
        let workspace = dir.join("workspace");
        fs::write(&requested, b"").unwrap();
        mark_executable(&requested);
        fs::write(&workspace, b"").unwrap();
        assert_eq!(
            select_agent_binary(Some(requested.clone()), &workspace, None, None, None),
            Some(requested)
        );
        let missing = dir.join("missing");
        assert_eq!(
            select_agent_binary(Some(missing), &workspace, None, None, None),
            None
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unusable_override_does_not_select_a_later_candidate() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-launcher-stop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let workspace = dir.join("workspace");
        let bundled = dir.join("bundled");
        let published = dir.join("published");
        let on_path = dir.join("on-path");
        for path in [&workspace, &bundled, &published, &on_path] {
            fs::write(path, b"").unwrap();
        }
        let plain = dir.join("plain");
        fs::write(&plain, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
        }
        assert_eq!(
            select_agent_binary(
                Some(dir.join("missing")),
                &workspace,
                Some(bundled.clone()),
                Some(published.clone()),
                Some(on_path.clone())
            ),
            None
        );
        assert_eq!(
            select_agent_binary(
                Some(PathBuf::new()),
                &workspace,
                Some(bundled.clone()),
                Some(published.clone()),
                Some(on_path.clone())
            ),
            None
        );
        assert_eq!(
            select_agent_binary(
                Some(plain),
                &workspace,
                Some(bundled.clone()),
                Some(published),
                Some(on_path)
            ),
            None
        );
        assert_eq!(
            select_agent_binary(None, &workspace, Some(bundled), None, None),
            Some(workspace)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_binary_rejects_a_set_but_unusable_override() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "a3s-launcher-find-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain");
        fs::write(&plain, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let previous = std::env::var_os(BINARY_ENV);
        for value in [
            dir.join("missing").into_os_string(),
            std::ffi::OsString::new(),
            plain.into_os_string(),
        ] {
            unsafe { std::env::set_var(BINARY_ENV, &value) };
            assert_eq!(
                find_binary(),
                None,
                "override {value:?} must stop the search"
            );
        }
        let executable = dir.join("chosen");
        fs::write(&executable, b"").unwrap();
        mark_executable(&executable);
        unsafe { std::env::set_var(BINARY_ENV, &executable) };
        assert_eq!(find_binary(), Some(executable));
        unsafe {
            match previous {
                Some(value) => std::env::set_var(BINARY_ENV, value),
                None => std::env::remove_var(BINARY_ENV),
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn packaged_host_does_not_prefer_the_compile_time_debug_binary() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-launcher-pkg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        let packaged = dir.join("ToWork.app/Contents/MacOS/towork");
        fs::create_dir_all(packaged.parent().unwrap()).unwrap();
        fs::write(&packaged, b"").unwrap();
        let debug = dir.join("target/debug").join(BINARY_NAME);
        fs::create_dir_all(debug.parent().unwrap()).unwrap();
        fs::write(&debug, b"").unwrap();
        let bundled = dir
            .join("ToWork.app/Contents/Resources/agent")
            .join(BINARY_NAME);
        fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        fs::write(&bundled, b"").unwrap();
        let candidate = debug_agent_candidate(Some(&packaged), &debug);
        assert_ne!(candidate, debug);
        assert_eq!(
            select_agent_binary(None, &candidate, Some(bundled.clone()), None, None),
            Some(bundled.clone())
        );
        let dev_host = dir.join("target/debug/towork");
        fs::write(&dev_host, b"").unwrap();
        let candidate = debug_agent_candidate(Some(&dev_host), &debug);
        assert_eq!(candidate, debug);
        assert_eq!(
            select_agent_binary(None, &candidate, Some(bundled), None, None),
            Some(debug)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn published_release_is_used_when_the_workspace_build_is_absent() {
        let dir = std::env::temp_dir().join(format!("a3s-launcher-rel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing-acp");
        let published = dir.join("a3s-code-acp-9.1.2");
        let on_path = dir.join("path-acp");
        fs::write(&published, b"").unwrap();
        fs::write(&on_path, b"").unwrap();
        assert_eq!(
            select_agent_binary(
                None,
                &missing,
                None,
                Some(published.clone()),
                Some(on_path.clone())
            ),
            Some(published)
        );
        assert_eq!(
            select_agent_binary(None, &missing, None, None, Some(on_path.clone())),
            Some(on_path)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundled_release_wins_when_the_checkout_is_absent() {
        let dir = std::env::temp_dir().join(format!("a3s-launcher-bundle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let exe_dir = dir.join("MacOS");
        fs::create_dir_all(exe_dir.join("agent")).unwrap();
        let bundled = exe_dir.join("agent").join(BINARY_NAME);
        let published = dir.join("a3s-code-acp-9.1.2");
        fs::write(&bundled, b"").unwrap();
        fs::write(&published, b"").unwrap();
        let found = bundled_agent_from(&exe_dir.join("ToWork"), None);
        assert_eq!(
            select_agent_binary(None, &dir.join("missing"), found, Some(published), None),
            Some(bundled)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn standard_install_is_found_without_a_version_suffix() {
        let dir = std::env::temp_dir().join(format!("a3s-launcher-home-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let bin = dir.join(".a3s/bin");
        fs::create_dir_all(&bin).unwrap();
        let installed = bin.join(BINARY_NAME);
        fs::write(&installed, b"").unwrap();
        fs::write(bin.join("a3s-code-acp-9.1.0"), b"").unwrap();
        assert_eq!(installed_binary(&dir), Some(installed));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_lookup_uses_an_extra_directory_when_path_is_empty() {
        let dir = std::env::temp_dir().join(format!("a3s-launcher-path-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let installed = dir.join(BINARY_NAME);
        fs::write(&installed, b"").unwrap();
        assert_eq!(path_binary(None, vec![dir.clone()]), Some(installed));
        let _ = fs::remove_dir_all(&dir);
    }
}
