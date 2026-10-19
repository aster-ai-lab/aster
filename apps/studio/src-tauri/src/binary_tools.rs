//! Environment tool installs through a3s-use tool-acquire.
//!
//! The shell resolves a bare name from the closed catalog, then publishes the
//! executable under the host tool root. Python packages go through `uv`.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_use_tool_acquire::{
    AcquireRequest, FetchPolicy, HttpSource, ToolAcquireError, ToolBackend, ToolPayload, ToolQuery,
    ToolSource, ToolSpec, ToolStore, catalog, catalog_by_name, parse_tool_spec,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const CUSTOM_SCHEMA: &str = "a3s.studio.binary-tools.v1";
const INSTALL_SCHEMA: &str = "a3s.studio.binary-install.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CustomTool {
    name: String,
    tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    requested_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomFile {
    schema: String,
    tools: Vec<CustomTool>,
}

#[derive(Debug, Clone)]
struct Recipe {
    spec: String,
    executable: String,
}

#[derive(Debug)]
struct BinaryState {
    root: PathBuf,
    store: ToolStore,
    http: HttpSource,
    custom: Vec<CustomTool>,
    operations: HashMap<String, Value>,
    latest: HashMap<String, String>,
}

struct ShellSource<'a> {
    http: &'a HttpSource,
    uv: Option<PathBuf>,
    pip_index: Option<String>,
}

impl ToolSource for ShellSource<'_> {
    fn fetch(&self, query: &ToolQuery) -> Result<ToolPayload, ToolAcquireError> {
        if let ToolBackend::Pipx { package } = query.spec.backend() {
            let extras = catalog()
                .iter()
                .find(|entry| entry.spec == query.spec.raw())
                .and_then(|entry| entry.pipx_extras);
            return pipx_with_uv(
                self.uv.as_deref(),
                package,
                extras,
                query,
                self.pip_index.as_deref(),
            );
        }
        self.http.fetch(query)
    }

    fn latest(&self, spec: &ToolSpec) -> Result<String, ToolAcquireError> {
        if matches!(spec.backend(), ToolBackend::Pipx { .. }) {
            return Err(ToolAcquireError::LatestUnavailable);
        }
        self.http.latest(spec)
    }
}

fn with_state<T>(body: impl FnOnce(&mut BinaryState) -> Result<T, String>) -> Result<T, String> {
    static STATE: OnceLock<Mutex<Result<BinaryState, String>>> = OnceLock::new();
    let slot = STATE.get_or_init(|| Mutex::new(BinaryState::open()));
    let mut guard = slot
        .lock()
        .map_err(|_| "The tool install lock is busy.".to_string())?;
    match guard.as_mut() {
        Ok(state) => body(state),
        Err(error) => Err(error.clone()),
    }
}

impl BinaryState {
    fn open() -> Result<Self, String> {
        let home = crate::user_home();
        if home.is_empty() {
            return Err("The home directory is missing.".to_string());
        }
        let root = PathBuf::from(home).join(".a3s").join("tools");
        let store = ToolStore::open(&root).map_err(|error| error.to_string())?;
        let http = HttpSource::new().map_err(|error| error.to_string())?;
        let custom = read_custom(&root)?;
        Ok(Self {
            root,
            store,
            http,
            custom,
            operations: HashMap::new(),
            latest: HashMap::new(),
        })
    }
}

#[tauri::command]
pub async fn binary_ipc(route: String, input: Option<Value>) -> Result<Value, String> {
    let input = input.unwrap_or(Value::Null);
    tokio::task::spawn_blocking(move || with_state(|state| dispatch(state, &route, &input)))
        .await
        .map_err(|_| "The tool install task stopped.".to_string())?
}

fn dispatch(state: &mut BinaryState, route: &str, input: &Value) -> Result<Value, String> {
    match route {
        "binary.install_tool" => {
            let name = required_name(input)?;
            let version = optional_version(input.get("targetVersion"))?;
            install_named(state, &name, version)?;
            Ok(Value::Null)
        }
        "binary.add_custom_tool" => add_custom(state, input).map(|_| Value::Null),
        "binary.remove_tool" => remove_named(state, input),
        "binary.get_tool_snapshots" => snapshots(state, input),
        "binary.search_registry" => {
            let query = input.as_str().unwrap_or("");
            Ok(Value::Array(search_catalog(query)))
        }
        "binary.get_latest_versions" => latest_versions(state, input.as_bool().unwrap_or(false)),
        "binary.apply_install_settings" => apply_install_settings(&state.root, input),
        _ => Err("The binary route is unknown.".to_string()),
    }
}

fn install_named(
    state: &mut BinaryState,
    name: &str,
    version: Option<String>,
) -> Result<(), String> {
    let recipe = recipe_for(name, &state.custom).map_err(|error| error.to_string())?;
    state
        .operations
        .insert(name.to_string(), json!({ "status": "installing" }));
    let (http, pip_index) = match load_http(&state.root) {
        Ok(loaded) => loaded,
        Err(error) => {
            state.operations.insert(
                name.to_string(),
                json!({
                    "status": "failed",
                    "action": "install",
                    "error": error.clone()
                }),
            );
            return Err(error);
        }
    };
    let uv = locate_uv(&state.root);
    let source = ShellSource {
        http: &http,
        uv,
        pip_index,
    };
    let result = state.store.acquire_with(
        AcquireRequest {
            name: name.to_string(),
            spec: recipe.spec,
            version,
            executable_name: recipe.executable,
        },
        &source,
    );
    match result {
        Ok(_) => {
            state.operations.remove(name);
            Ok(())
        }
        Err(error) => {
            state.operations.insert(
                name.to_string(),
                json!({
                    "status": "failed",
                    "action": "install",
                    "error": error.to_string()
                }),
            );
            Err(error.to_string())
        }
    }
}

fn add_custom(state: &mut BinaryState, input: &Value) -> Result<(), String> {
    let name = input
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "The tool name is missing.".to_string())?;
    let tool = input
        .get("tool")
        .and_then(Value::as_str)
        .ok_or_else(|| "The tool spec is missing.".to_string())?;
    let requested =
        optional_version(input.get("requestedVersion")).map_err(|error| error.to_string())?;
    if catalog_by_name(name).is_some() || state.custom.iter().any(|entry| entry.name == name) {
        return Err("The tool name is already defined.".to_string());
    }
    parse_tool_spec(tool).map_err(|error| error.to_string())?;
    if state.custom.iter().any(|entry| entry.tool == tool)
        || catalog().iter().any(|entry| entry.spec == tool)
    {
        return Err("The tool spec is already provided.".to_string());
    }
    state.custom.push(CustomTool {
        name: name.to_string(),
        tool: tool.to_string(),
        requested_version: requested.clone(),
    });
    write_custom(&state.root, &state.custom)?;
    if let Err(error) = install_named(state, name, requested) {
        // The definition stays so the card can retry.
        let _ = error;
    }
    Ok(())
}

fn remove_named(state: &mut BinaryState, input: &Value) -> Result<Value, String> {
    let name = required_name(input)?;
    let definition_only = input
        .get("definitionOnly")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let custom = state.custom.iter().any(|entry| entry.name == name);
    let fixed = catalog_by_name(&name).is_some();
    if definition_only {
        if !custom || fixed {
            return Err("A fixed tool has no definition to remove.".to_string());
        }
        state.custom.retain(|entry| entry.name != name);
        write_custom(&state.root, &state.custom)?;
        state.operations.remove(&name);
        return Ok(json!({ "status": "removed" }));
    }
    if let Some(blocked) = dependency_block(state, &name) {
        return Ok(blocked);
    }
    state
        .operations
        .insert(name.clone(), json!({ "status": "removing" }));
    match state.store.remove_sync(&name) {
        Ok(()) => {
            state.custom.retain(|entry| entry.name != name);
            write_custom(&state.root, &state.custom)?;
            state.operations.remove(&name);
            Ok(json!({ "status": "removed" }))
        }
        Err(error) => {
            state.operations.remove(&name);
            Ok(json!({
                "status": "cleanup_blocked",
                "reason": "cleanup_failed",
                "message": error.to_string()
            }))
        }
    }
}

fn dependency_block(state: &BinaryState, name: &str) -> Option<Value> {
    let prefix = if name == "node" {
        "npm:"
    } else if name == "python" {
        "pipx:"
    } else {
        return None;
    };
    let dependents: Vec<String> = state
        .store
        .list_sync()
        .ok()?
        .into_iter()
        .filter(|receipt| receipt.spec.starts_with(prefix))
        .map(|receipt| receipt.name)
        .collect();
    if dependents.is_empty() {
        None
    } else {
        Some(json!({
            "status": "cleanup_blocked",
            "reason": "dependency_blocked",
            "dependents": dependents
        }))
    }
}

fn snapshots(state: &BinaryState, input: &Value) -> Result<Value, String> {
    let names = input
        .as_array()
        .ok_or_else(|| "The tool list is missing.".to_string())?;
    let mut wanted = Vec::new();
    for name in names {
        let name = name
            .as_str()
            .ok_or_else(|| "The tool name is missing.".to_string())?;
        parse_tool_name_boundary(name)?;
        wanted.push(name.to_string());
    }
    for tool in &state.custom {
        if !wanted.iter().any(|name| name == &tool.name) {
            wanted.push(tool.name.clone());
        }
    }
    let mut record = serde_json::Map::new();
    for name in wanted {
        record.insert(name.clone(), one_snapshot(state, &name));
    }
    Ok(Value::Object(record))
}

fn one_snapshot(state: &BinaryState, name: &str) -> Value {
    let definition = state
        .custom
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| {
            json!({
                "name": entry.name,
                "tool": entry.tool,
                "requestedVersion": entry.requested_version
            })
        });
    let recipe = recipe_for(name, &state.custom).ok();
    let listed = state.store.list_sync();
    let receipt = listed
        .as_ref()
        .ok()
        .and_then(|rows| rows.iter().find(|row| row.name == name));
    let shim = state.store.which(name).ok().flatten();
    let application = match (&recipe, &receipt, &shim) {
        (Some(recipe), Some(receipt), Some(_)) if receipt.spec == recipe.spec => {
            json!({ "status": "applied", "version": receipt.version })
        }
        (Some(recipe), Some(receipt), _) if receipt.spec != recipe.spec => {
            json!({ "status": "conflict" })
        }
        (Some(_), Some(receipt), None) => json!({ "status": "broken", "version": receipt.version }),
        (Some(_), None, _) => json!({ "status": "absent" }),
        _ => Value::Null,
    };
    let availability = if let Some(path) = shim {
        let mut availability = json!({
            "source": "use",
            "path": path.display().to_string()
        });
        if let Some(version) = receipt.as_ref().map(|row| row.version.clone()) {
            availability["version"] = json!(version);
        }
        availability
    } else if let Some(path) = system_executable(name, &state.root) {
        json!({ "source": "system", "path": path })
    } else {
        json!({ "source": "none" })
    };
    let mut snapshot = json!({
        "name": name,
        "availability": availability
    });
    if let Some(definition) = definition {
        snapshot["definition"] = definition;
    }
    if !application.is_null() {
        snapshot["application"] = application;
    }
    if let Some(operation) = state.operations.get(name) {
        snapshot["operation"] = operation.clone();
    }
    snapshot
}

fn latest_versions(state: &mut BinaryState, force: bool) -> Result<Value, String> {
    if !force {
        return Ok(serde_json::to_value(&state.latest).unwrap_or_else(|_| json!({})));
    }
    let names: Vec<String> = catalog()
        .iter()
        .map(|entry| entry.name.to_string())
        .chain(state.custom.iter().map(|entry| entry.name.clone()))
        .collect();
    let (http, _) = load_http(&state.root)?;
    let before = fs_fingerprint(&state.root);
    for name in names {
        let Some(recipe) = recipe_for(&name, &state.custom).ok() else {
            continue;
        };
        let applied = state.store.which(&name).ok().flatten().is_some();
        if !applied {
            continue;
        }
        let source = ShellSource {
            http: &http,
            uv: None,
            pip_index: None,
        };
        match state.store.latest_with(&recipe.spec, &source) {
            Ok(version) => {
                state.latest.insert(name, version);
            }
            Err(_) => {
                // A latest-version failure leaves the install and the previous hint.
            }
        }
    }
    let after = fs_fingerprint(&state.root);
    if before != after {
        return Err("The latest-version read changed the tool root.".to_string());
    }
    Ok(serde_json::to_value(&state.latest).unwrap_or_else(|_| json!({})))
}

fn fs_fingerprint(root: &Path) -> String {
    let current = root.join("current");
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(&current) {
        for entry in entries.flatten() {
            if let Ok(bytes) = fs::read(entry.path()) {
                names.push(format!(
                    "{}:{}",
                    entry.file_name().to_string_lossy(),
                    String::from_utf8_lossy(&bytes)
                ));
            }
        }
    }
    names.sort();
    names.join("\n")
}

fn search_catalog(query: &str) -> Vec<Value> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let lowered = trimmed.to_ascii_lowercase();
    let mut rows: Vec<Value> = catalog()
        .iter()
        .filter(|entry| {
            entry.name.to_ascii_lowercase().contains(&lowered)
                || entry.spec.to_ascii_lowercase().contains(&lowered)
        })
        .map(|entry| json!({ "name": entry.name, "tool": entry.spec }))
        .collect();
    if rows.is_empty() {
        if let Ok(spec) = parse_tool_spec(trimmed) {
            if let Some(name) = spec_name(spec.raw()) {
                rows.push(json!({ "name": name, "tool": spec.raw() }));
            }
        }
    }
    rows
}

fn spec_name(spec: &str) -> Option<String> {
    let tail = spec.rsplit(['/', ':']).next().unwrap_or(spec);
    let mut name = String::new();
    for character in tail.chars() {
        if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
            name.push(character);
        }
    }
    if name.is_empty() || !name.chars().next()?.is_ascii_alphabetic() {
        None
    } else {
        Some(name)
    }
}

fn recipe_for(name: &str, custom: &[CustomTool]) -> Result<Recipe, ToolAcquireError> {
    if let Some(entry) = catalog_by_name(name) {
        return Ok(Recipe {
            spec: entry.spec.to_string(),
            executable: entry.executable.to_string(),
        });
    }
    let Some(tool) = custom.iter().find(|entry| entry.name == name) else {
        return Err(ToolAcquireError::InvalidName(name.to_string()));
    };
    parse_tool_spec(&tool.tool)?;
    Ok(Recipe {
        spec: tool.tool.clone(),
        executable: name.to_string(),
    })
}

fn required_name(input: &Value) -> Result<String, String> {
    let name = input
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "The tool name is missing.".to_string())?;
    parse_tool_name_boundary(name)?;
    Ok(name.to_string())
}

fn optional_version(value: Option<&Value>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let version = value
        .as_str()
        .ok_or_else(|| "The tool version is invalid.".to_string())?;
    if version.is_empty() {
        return Ok(None);
    }
    if version.contains('/')
        || version.contains('\\')
        || version.contains(':')
        || version.contains("..")
    {
        return Err("The tool version is invalid.".to_string());
    }
    Ok(Some(version.to_string()))
}

fn parse_tool_name_boundary(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("The tool name is missing.".to_string());
    };
    if first.is_ascii_alphabetic()
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        Ok(())
    } else {
        Err("The tool name is invalid.".to_string())
    }
}

fn read_custom(root: &Path) -> Result<Vec<CustomTool>, String> {
    let path = root.join("custom-tools.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(&path).map_err(|_| "The custom tool file cannot be read.".to_string())?;
    let file: CustomFile = serde_json::from_slice(&bytes)
        .map_err(|_| "The custom tool file is invalid.".to_string())?;
    if file.schema != CUSTOM_SCHEMA {
        return Err("The custom tool file is invalid.".to_string());
    }
    Ok(file.tools)
}

fn write_custom(root: &Path, tools: &[CustomTool]) -> Result<(), String> {
    let file = CustomFile {
        schema: CUSTOM_SCHEMA.to_string(),
        tools: tools.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|_| "The custom tool file cannot be written.".to_string())?;
    let path = root.join("custom-tools.json");
    let temporary = root.join(format!(
        ".custom-tools.{}.tmp",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    {
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "The custom tool file cannot be written.".to_string())?;
        output
            .write_all(&bytes)
            .map_err(|_| "The custom tool file cannot be written.".to_string())?;
    }
    fs::rename(&temporary, &path).map_err(|_| "The custom tool file cannot be written.".to_string())
}

fn system_executable(name: &str, root: &Path) -> Option<String> {
    let finder = if cfg!(windows) {
        "where.exe"
    } else {
        "/usr/bin/which"
    };
    let output = Command::new(finder).arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(text.lines().next()?.trim());
    if path.starts_with(root) || !path.is_file() {
        return None;
    }
    Some(path.display().to_string())
}

fn locate_uv(root: &Path) -> Option<PathBuf> {
    let shim = root
        .join("shims")
        .join(if cfg!(windows) { "uv.exe" } else { "uv" });
    if shim.is_file() {
        return Some(shim);
    }
    system_executable("uv", root).map(PathBuf::from)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallInput {
    github_mirror: String,
    github_token: String,
    npm_registry: String,
    pip_index_url: String,
    verify_signatures: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstallFile {
    schema: String,
    github_mirror: String,
    github_token: String,
    npm_registry: String,
    pip_index_url: String,
}

fn apply_install_settings(root: &Path, input: &Value) -> Result<Value, String> {
    let parsed: InstallInput = serde_json::from_value(input.clone())
        .map_err(|_| "The install settings are invalid.".to_string())?;
    let file = InstallFile {
        schema: INSTALL_SCHEMA.to_string(),
        github_mirror: canonical_mirror(&parsed.github_mirror)?,
        github_token: canonical_token(&parsed.github_token)?,
        npm_registry: canonical_npm(&parsed.npm_registry)?,
        pip_index_url: canonical_pip(&parsed.pip_index_url)?,
    };
    // Checksums stay required. verifySignatures cannot turn them off.
    let _ = parsed.verify_signatures;
    write_install_file(root, &file)?;
    Ok(Value::Null)
}

fn load_http(root: &Path) -> Result<(HttpSource, Option<String>), String> {
    let file = read_install_file(root)?;
    let http = HttpSource::with_policy(FetchPolicy {
        github_mirror: file.github_mirror,
        npm_registry: file.npm_registry,
        github_token: file.github_token,
    })
    .map_err(|_| "The install source is not allowed.".to_string())?;
    let pip_index = if file.pip_index_url.is_empty() {
        None
    } else {
        Some(file.pip_index_url)
    };
    Ok((http, pip_index))
}

fn read_install_file(root: &Path) -> Result<InstallFile, String> {
    let path = root.join("install-settings.json");
    if !path.exists() {
        return Ok(InstallFile {
            schema: INSTALL_SCHEMA.to_string(),
            github_mirror: String::new(),
            github_token: String::new(),
            npm_registry: String::new(),
            pip_index_url: String::new(),
        });
    }
    let bytes = fs::read(&path).map_err(|_| "The install settings cannot be read.".to_string())?;
    let file: InstallFile = serde_json::from_slice(&bytes)
        .map_err(|_| "The install settings are invalid.".to_string())?;
    if file.schema != INSTALL_SCHEMA {
        return Err("The install settings are invalid.".to_string());
    }
    let github_mirror = canonical_mirror(&file.github_mirror)?;
    let github_token = canonical_token(&file.github_token)?;
    let npm_registry = canonical_npm(&file.npm_registry)?;
    let pip_index_url = canonical_pip(&file.pip_index_url)?;
    Ok(InstallFile {
        schema: file.schema,
        github_mirror,
        github_token,
        npm_registry,
        pip_index_url,
    })
}

fn write_install_file(root: &Path, file: &InstallFile) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(file)
        .map_err(|_| "The install settings cannot be written.".to_string())?;
    let path = root.join("install-settings.json");
    let temporary = root.join(format!(
        ".install-settings.{}.tmp",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    {
        let mut output = install_file_options()
            .open(&temporary)
            .map_err(|_| "The install settings cannot be written.".to_string())?;
        output
            .write_all(&bytes)
            .map_err(|_| "The install settings cannot be written.".to_string())?;
    }
    restrict_install_file(&temporary)?;
    fs::rename(&temporary, &path)
        .map_err(|_| "The install settings cannot be written.".to_string())?;
    restrict_install_file(&path)
}

fn install_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

fn restrict_install_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| "The install settings cannot be written.".to_string())?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn canonical_mirror(value: &str) -> Result<String, String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    match trimmed {
        "https://ghfast.top" | "https://ghproxy.net" => Ok(trimmed.to_string()),
        _ => Err("The install source is not allowed.".to_string()),
    }
}

fn canonical_npm(value: &str) -> Result<String, String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    match trimmed {
        "https://registry.npmjs.org" | "https://registry.npmmirror.com" => Ok(trimmed.to_string()),
        _ => Err("The install source is not allowed.".to_string()),
    }
}

fn canonical_pip(value: &str) -> Result<String, String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    match trimmed {
        "https://pypi.tuna.tsinghua.edu.cn/simple" => {
            Ok("https://pypi.tuna.tsinghua.edu.cn/simple".to_string())
        }
        "https://mirrors.aliyun.com/pypi/simple" => {
            Ok("https://mirrors.aliyun.com/pypi/simple/".to_string())
        }
        "https://pypi.org/simple" => Ok("https://pypi.org/simple".to_string()),
        _ => Err("The install source is not allowed.".to_string()),
    }
}

fn canonical_token(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.len() > 256
        || !trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
    {
        return Err("The GitHub token is invalid.".to_string());
    }
    Ok(trimmed.to_string())
}

fn pip_install_args(python: &Path, requirement: &str, index: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "pip".to_string(),
        "install".to_string(),
        "--python".to_string(),
        python.display().to_string(),
    ];
    if let Some(index) = index {
        args.push("--index-url".to_string());
        args.push(index.to_string());
    }
    args.push(requirement.to_string());
    args
}

fn pipx_with_uv(
    uv: Option<&Path>,
    package: &str,
    extras: Option<&str>,
    query: &ToolQuery,
    index: Option<&str>,
) -> Result<ToolPayload, ToolAcquireError> {
    let Some(uv) = uv else {
        return Err(ToolAcquireError::RuntimeMissing("uv".to_string()));
    };
    let requirement = pipx_requirement(package, extras, query.version.as_deref())?;
    let parent = std::env::temp_dir().join(format!(
        "a3s-pipx-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&parent)
        .map_err(|error| ToolAcquireError::io("create python workspace", &parent, error))?;
    let venv = parent.join("venv");
    let created = Command::new(uv)
        .args(["venv", "--python", "3.12"])
        .arg(&venv)
        .status()
        .map_err(|_| ToolAcquireError::RuntimeMissing("uv".to_string()))?;
    if !created.success() {
        let _ = fs::remove_dir_all(&parent);
        return Err(ToolAcquireError::DownloadRejected);
    }
    let python = if cfg!(windows) {
        venv.join("Scripts").join("python.exe")
    } else {
        venv.join("bin").join("python")
    };
    let installed = Command::new(uv)
        .args(pip_install_args(&python, &requirement, index))
        .status()
        .map_err(|_| ToolAcquireError::RuntimeMissing("uv".to_string()))?;
    if !installed.success() {
        let _ = fs::remove_dir_all(&parent);
        return Err(ToolAcquireError::DownloadRejected);
    }
    let bin_dir = if cfg!(windows) {
        venv.join("Scripts")
    } else {
        venv.join("bin")
    };
    let executable = bin_dir.join(&query.executable);
    let bytes = fs::read(&executable)
        .map_err(|error| ToolAcquireError::io("read python tool", &executable, error))?;
    let _ = fs::remove_dir_all(&parent);
    if bytes.is_empty() {
        return Err(ToolAcquireError::NotExecutable);
    }
    use sha2::{Digest, Sha256};
    let sha256 = hex_sha256(&Sha256::digest(&bytes));
    let version = query
        .version
        .clone()
        .unwrap_or_else(|| "installed".to_string());
    Ok(ToolPayload::new(version, sha256, bytes))
}

fn pipx_requirement(
    package: &str,
    extras: Option<&str>,
    version: Option<&str>,
) -> Result<String, ToolAcquireError> {
    let requirement = match (extras, version) {
        (Some(extra), Some(version)) => format!("{package}[{extra}]=={version}"),
        (Some(extra), None) => format!("{package}[{extra}]"),
        (None, Some(version)) => format!("{package}=={version}"),
        (None, None) => package.to_string(),
    };
    if requirement.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || matches!(character, '-' | '_' | '.' | '[' | ']' | '=' | ',')
    }) {
        Ok(requirement)
    } else {
        Err(ToolAcquireError::InvalidSpec(requirement))
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_use_tool_acquire::{StaticSource, ToolPayload};
    use sha2::{Digest, Sha256};

    const NONE_TOKENS: &[&str] = &["none"];

    fn digest(bytes: &[u8]) -> String {
        hex_sha256(&Sha256::digest(bytes))
    }

    fn temp_state() -> (tempfile_dir::Temp, BinaryState) {
        let directory = tempfile_dir::Temp::new();
        let store = ToolStore::open(directory.path()).unwrap();
        let http =
            HttpSource::for_test("http://127.0.0.1:9", "http://127.0.0.1:9", NONE_TOKENS).unwrap();
        let state = BinaryState {
            root: directory.path().to_path_buf(),
            store,
            http,
            custom: Vec::new(),
            operations: HashMap::new(),
            latest: HashMap::new(),
        };
        (directory, state)
    }

    #[test]
    fn empty_install_settings_use_the_official_sources() {
        let (_directory, state) = temp_state();
        let (http, pip) = load_http(&state.root).unwrap();
        assert_eq!(http.npm_registry(), "https://registry.npmjs.org");
        assert_eq!(http.github_mirror(), None);
        assert!(pip.is_none());
    }

    #[test]
    fn saved_sources_are_the_ones_the_installer_uses() {
        let (directory, state) = temp_state();
        let token = "ghp_exampletoken";
        apply_install_settings(
            &state.root,
            &json!({
                "githubMirror": "https://ghfast.top/",
                "githubToken": token,
                "npmRegistry": "https://registry.npmmirror.com",
                "pipIndexUrl": "https://pypi.tuna.tsinghua.edu.cn/simple",
                "verifySignatures": false
            }),
        )
        .unwrap();
        let (http, pip) = load_http(&state.root).unwrap();
        assert_eq!(http.github_mirror(), Some("https://ghfast.top"));
        assert_eq!(http.npm_registry(), "https://registry.npmmirror.com");
        assert_eq!(
            pip.as_deref(),
            Some("https://pypi.tuna.tsinghua.edu.cn/simple")
        );
        let error = apply_install_settings(
            &state.root,
            &json!({
                "githubMirror": "https://evil.example",
                "githubToken": token,
                "npmRegistry": "",
                "pipIndexUrl": "",
                "verifySignatures": true
            }),
        )
        .unwrap_err();
        assert_eq!(error, "The install source is not allowed.");
        assert!(!error.contains(token));
        let (http, _) = load_http(&state.root).unwrap();
        assert_eq!(http.github_mirror(), Some("https://ghfast.top"));
        let mode = fs::metadata(directory.path().join("install-settings.json"))
            .unwrap()
            .permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(mode.mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn pip_install_args_include_the_saved_index() {
        let args = pip_install_args(
            Path::new("/venv/bin/python"),
            "babeldoc==1.2.3",
            Some("https://pypi.tuna.tsinghua.edu.cn/simple"),
        );
        assert_eq!(
            args,
            vec![
                "pip",
                "install",
                "--python",
                "/venv/bin/python",
                "--index-url",
                "https://pypi.tuna.tsinghua.edu.cn/simple",
                "babeldoc==1.2.3"
            ]
        );
    }

    #[test]
    fn remove_clears_the_published_tool() {
        let (directory, mut state) = temp_state();
        let bytes = b"#!/bin/sh\necho uv\n";
        let mut source = StaticSource::new();
        source.insert(
            "github:astral-sh/uv",
            ToolPayload::new("1.0.0", digest(bytes), bytes.to_vec()),
        );
        install_with_source(&mut state, &source).unwrap();
        let removed = remove_named(&mut state, &json!({ "name": "uv" })).unwrap();
        assert_eq!(removed["status"], "removed");
        assert!(!directory.path().join("current/uv").exists());
        assert!(!directory.path().join("shims/uv").exists());
    }

    #[test]
    fn node_removal_stays_blocked_while_an_npm_tool_is_installed() {
        let (_directory, mut state) = temp_state();
        let bytes = b"#!/usr/bin/env node\n";
        let mut source = StaticSource::new();
        source.insert(
            "npm:left-pad",
            ToolPayload::new("1.0.0", digest(bytes), bytes.to_vec()),
        );
        state
            .store
            .acquire_with(
                AcquireRequest {
                    name: "leftpad".into(),
                    spec: "npm:left-pad".into(),
                    version: None,
                    executable_name: "leftpad".into(),
                },
                &source,
            )
            .unwrap();
        let blocked = remove_named(&mut state, &json!({ "name": "node" })).unwrap();
        assert_eq!(blocked["status"], "cleanup_blocked");
        assert_eq!(blocked["reason"], "dependency_blocked");
        assert!(directory_has_current(&state, "leftpad"));
    }

    fn directory_has_current(state: &BinaryState, name: &str) -> bool {
        state.root.join("current").join(name).is_file()
    }

    #[test]
    fn shell_source_does_not_name_the_replaced_installer() {
        let source = include_str!("binary_tools.rs");
        let retired = format!("{}{}", "mi", "se");
        assert!(!source.to_ascii_lowercase().contains(&retired));
    }

    #[test]
    fn fixed_tool_install_publishes_a_use_snapshot() {
        let (_directory, state) = temp_state();
        let bytes = b"#!/bin/sh\necho uv\n";
        let mut source = StaticSource::new();
        source.insert(
            "github:astral-sh/uv",
            ToolPayload::new("1.0.0", digest(bytes), bytes.to_vec()),
        );
        state
            .store
            .acquire_with(
                AcquireRequest {
                    name: "uv".into(),
                    spec: "github:astral-sh/uv".into(),
                    version: None,
                    executable_name: "uv".into(),
                },
                &source,
            )
            .unwrap();
        let snapshot = one_snapshot(&state, "uv");
        assert_eq!(snapshot["availability"]["source"], "use");
        assert_eq!(snapshot["availability"]["version"], "1.0.0");
        assert_eq!(snapshot["application"]["status"], "applied");
    }

    #[test]
    fn checksum_mismatch_keeps_the_published_tool() {
        let (directory, mut state) = temp_state();
        let good = b"#!/bin/sh\necho uv\n";
        let mut source = StaticSource::new();
        source.insert(
            "github:astral-sh/uv",
            ToolPayload::new("1.0.0", digest(good), good.to_vec()),
        );
        install_with_source(&mut state, &source).unwrap();
        let mut bad = ToolPayload::new("2.0.0", digest(good), b"broken".to_vec());
        bad.sha256 = digest(good);
        source.insert("github:astral-sh/uv", bad);
        let error = install_with_source(&mut state, &source).unwrap_err();
        assert_eq!(error.code(), "use.tool_acquire.checksum_mismatch");
        assert_eq!(
            fs::read_to_string(directory.path().join("current/uv")).unwrap(),
            "1.0.0\n"
        );
    }

    #[test]
    fn pipx_without_uv_leaves_current_absent() {
        let (directory, state) = temp_state();
        let source = ShellSource {
            http: &state.http,
            uv: None,
            pip_index: None,
        };
        let error = state
            .store
            .acquire_with(
                AcquireRequest {
                    name: "babeldoc-stream".into(),
                    spec: "pipx:babeldoc-stream".into(),
                    version: None,
                    executable_name: "babeldoc-stream".into(),
                },
                &source,
            )
            .unwrap_err();
        assert_eq!(error.code(), "use.tool_acquire.runtime_missing");
        assert!(!directory.path().join("current/babeldoc-stream").exists());
    }

    #[test]
    fn latest_failure_does_not_change_current() {
        let (directory, state) = temp_state();
        let bytes = b"#!/bin/sh\necho agy\n";
        let mut source = StaticSource::new();
        source.insert(
            "aqua:google-antigravity/antigravity-cli",
            ToolPayload::new("1.0.0", digest(bytes), bytes.to_vec()),
        );
        source.fail_latest("aqua:google-antigravity/antigravity-cli");
        state
            .store
            .acquire_with(
                AcquireRequest {
                    name: "agy".into(),
                    spec: "aqua:google-antigravity/antigravity-cli".into(),
                    version: None,
                    executable_name: "agy".into(),
                },
                &source,
            )
            .unwrap();
        let before = fs::read(directory.path().join("current/agy")).unwrap();
        let error = state
            .store
            .latest_with("aqua:google-antigravity/antigravity-cli", &source)
            .unwrap_err();
        assert_eq!(error.code(), "use.tool_acquire.latest_unavailable");
        assert_eq!(
            fs::read(directory.path().join("current/agy")).unwrap(),
            before
        );
    }

    fn install_with_source(
        state: &mut BinaryState,
        source: &StaticSource,
    ) -> Result<(), ToolAcquireError> {
        let recipe = recipe_for("uv", &state.custom)?;
        state.store.acquire_with(
            AcquireRequest {
                name: "uv".into(),
                spec: recipe.spec,
                version: None,
                executable_name: recipe.executable,
            },
            source,
        )?;
        Ok(())
    }

    /// Tiny directory guard so the shell tests do not add a dev-dependency.
    mod tempfile_dir {
        use std::path::{Path, PathBuf};

        pub struct Temp {
            path: PathBuf,
        }

        impl Temp {
            pub fn new() -> Self {
                use std::sync::atomic::{AtomicU64, Ordering};
                static NEXT: AtomicU64 = AtomicU64::new(0);
                let path = std::env::temp_dir().join(format!(
                    "a3s-binary-tools-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }

            pub fn path(&self) -> &Path {
                &self.path
            }
        }

        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }
}
