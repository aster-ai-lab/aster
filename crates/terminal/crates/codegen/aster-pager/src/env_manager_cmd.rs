//! `aster tool|exec|env|activate|use`: project environment manager (mise-like).
//!
//! Tools and vars come from `.aster/env.acl` (and `$ASTER_HOME/env.acl`).
//! Installs go through `aster-use-tool-acquire` under `$ASTER_HOME/tools`.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use aster_use_tool_acquire::{
    AcquireRequest, EnvConfig, FetchPolicy, HttpSource, ToolStore, catalog_by_name,
    load_env_config, project_env_path, read_env_acl, resolve_tools, user_env_path, write_env_acl,
};
use clap::Subcommand;
use serde::Serialize;

// ── CLI args ────────────────────────────────────────────────────────

#[derive(Debug, clap::Args, Clone)]
pub struct ToolArgs {
    #[command(subcommand)]
    pub command: ToolCommand,
}

#[derive(Debug, Subcommand, Clone)]
pub enum ToolCommand {
    /// Install tools declared in env.acl (or one named tool)
    Install {
        /// Tool name, optionally `name@version`. Omit to install every declared tool.
        tool: Option<String>,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Pin a tool into the project env.acl and install it
    Use {
        /// Tool name, optionally `name@version` (version defaults to latest).
        tool: String,
        /// Write the user env.acl instead of the project file.
        #[arg(long)]
        global: bool,
        #[arg(long)]
        json: bool,
    },
    /// List installed tools in the local store
    #[command(name = "ls", visible_alias = "list")]
    Ls {
        #[arg(long)]
        json: bool,
    },
    /// Print the project-selected executable, or an undeclared tool's shim
    Which {
        tool: String,
        #[arg(long)]
        json: bool,
    },
    /// Remove a tool from the store (and optionally from env.acl)
    Uninstall {
        tool: String,
        /// Also drop the tool pin from the project env.acl.
        #[arg(long)]
        unpin: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, clap::Args, Clone)]
pub struct EnvArgs {
    #[command(subcommand)]
    pub command: EnvCommand,
}

#[derive(Subcommand, Clone)]
pub enum EnvCommand {
    /// Show merged tools and vars from env.acl layers
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Set a project env var in `.aster/env.acl`
    Set {
        /// `NAME=value`
        assignment: String,
        #[arg(long)]
        global: bool,
        #[arg(long)]
        json: bool,
    },
    /// Remove a project env var from env.acl
    Unset {
        name: String,
        #[arg(long)]
        global: bool,
        #[arg(long)]
        json: bool,
    },
}

impl std::fmt::Debug for EnvCommand {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Show { json } => formatter.debug_struct("Show").field("json", json).finish(),
            Self::Set { global, json, .. } => formatter
                .debug_struct("Set")
                .field("assignment", &"<redacted>")
                .field("global", global)
                .field("json", json)
                .finish(),
            Self::Unset { name, global, json } => formatter
                .debug_struct("Unset")
                .field("name", name)
                .field("global", global)
                .field("json", json)
                .finish(),
        }
    }
}

#[derive(clap::Args, Clone)]
pub struct ExecArgs {
    /// Skip installing missing tools declared in env.acl.
    #[arg(long = "no-install")]
    pub no_install: bool,
    /// Command and arguments to run with the managed PATH and env vars.
    #[arg(
        required = true,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "CMD"
    )]
    pub command: Vec<OsString>,
}

impl std::fmt::Debug for ExecArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecArgs")
            .field("no_install", &self.no_install)
            .field(
                "command",
                &format_args!("{} redacted arguments", self.command.len()),
            )
            .finish()
    }
}

#[derive(Debug, clap::Args, Clone)]
pub struct ActivateArgs {
    /// Shell to emit activation for. Defaults to `$SHELL` basename, else bash.
    #[arg(value_name = "SHELL")]
    pub shell: Option<String>,
}

#[derive(clap::Args, Clone)]
pub struct UseForwardArgs {
    /// Arguments forwarded to the `aster-use` binary.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub args: Vec<OsString>,
}

impl std::fmt::Debug for UseForwardArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UseForwardArgs")
            .field(
                "args",
                &format_args!("{} redacted arguments", self.args.len()),
            )
            .finish()
    }
}

// ── Dispatch ────────────────────────────────────────────────────────

pub async fn run_tool(args: ToolArgs) -> Result<()> {
    match args.command {
        ToolCommand::Install { tool, json } => install_cmd(tool.as_deref(), json).await,
        ToolCommand::Use { tool, global, json } => use_cmd(&tool, global, json).await,
        ToolCommand::Ls { json } => ls_cmd(json),
        ToolCommand::Which { tool, json } => which_cmd(&tool, json),
        ToolCommand::Uninstall { tool, unpin, json } => uninstall_cmd(&tool, unpin, json),
    }
}

pub async fn run_env(args: EnvArgs) -> Result<()> {
    match args.command {
        EnvCommand::Show { json } => show_env(json),
        EnvCommand::Set {
            assignment,
            global,
            json,
        } => set_env(&assignment, global, json),
        EnvCommand::Unset { name, global, json } => unset_env(&name, global, json),
    }
}

pub async fn run_exec(args: ExecArgs) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let mut config = load_env_config(&project, &aster_home).map_err(anyhow::Error::new)?;
    if !args.no_install {
        for receipt in install_resolved(&store, &config).await? {
            config.tools.insert(receipt.name, receipt.version);
        }
    }
    let mut command = args.command;
    if command.is_empty() {
        bail!("aster exec requires a command");
    }
    let program = command.remove(0);
    let mut child = ProcessCommand::new(&program);
    child.args(&command);
    child.envs(runtime_env(&store, &config)?);
    child.stdin(Stdio::inherit());
    child.stdout(Stdio::inherit());
    child.stderr(Stdio::inherit());
    run_inherited_command(child).await.with_context(|| {
        format!(
            "failed to exec {}",
            std::path::Path::new(&program).display()
        )
    })
}

pub fn run_activate(args: ActivateArgs) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let config = load_env_config(&project, &aster_home).map_err(anyhow::Error::new)?;
    let shell = args
        .shell
        .or_else(|| {
            env::var_os("SHELL").and_then(|value| {
                PathBuf::from(value)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
        })
        .unwrap_or_else(|| "bash".to_string());
    print!("{}", activation_script(&shell, &store, &config)?);
    Ok(())
}

pub async fn run_use_forward(args: UseForwardArgs) -> Result<()> {
    let binary = resolve_aster_use()?;
    let mut command = ProcessCommand::new(&binary);
    command
        .args(&args.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    run_inherited_command(command)
        .await
        .with_context(|| format!("failed to run {}", binary.display()))
}

// ── Commands ────────────────────────────────────────────────────────

async fn install_cmd(tool: Option<&str>, json: bool) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let mut config = load_env_config(&project, &aster_home).map_err(anyhow::Error::new)?;
    let receipts = if let Some(raw) = tool {
        let (name, version) = split_tool_pin(raw)?;
        config.tools.insert(name.clone(), version.clone());
        let resolved = resolve_tools(&EnvConfig {
            tools: BTreeMap::from([(name, version)]),
            vars: BTreeMap::new(),
        })
        .map_err(anyhow::Error::new)?;
        acquire_all(&store, &resolved).await?
    } else {
        install_resolved(&store, &config).await?
    };
    emit_install(json, &receipts);
    Ok(())
}

async fn use_cmd(raw: &str, global: bool, json: bool) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let (name, version) = split_tool_pin(raw)?;
    let path = if global {
        user_env_path(&aster_home)
    } else {
        project_env_path(&project)
    };
    let mut file = read_env_acl(&path)?.unwrap_or_default();
    let resolved = resolve_tools(&EnvConfig {
        tools: BTreeMap::from([(name, version)]),
        vars: BTreeMap::new(),
    })
    .map_err(anyhow::Error::new)?;
    let receipts = acquire_all(&store, &resolved).await?;
    for receipt in &receipts {
        file.tools
            .insert(receipt.name.clone(), receipt.version.clone());
    }
    write_env_acl(&path, &file)?;
    emit_install(json, &receipts);
    Ok(())
}

fn ls_cmd(json: bool) -> Result<()> {
    let (_aster_home, _project, store) = open_context()?;
    let receipts = store.list_sync().map_err(anyhow::Error::new)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&receipts)?);
    } else if receipts.is_empty() {
        println!("No tools installed under {}.", store.root_path().display());
    } else {
        for receipt in receipts {
            println!(
                "{}\t{}\t{}",
                receipt.name, receipt.version, receipt.executable
            );
        }
    }
    Ok(())
}

fn which_cmd(tool: &str, json: bool) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let mut config = load_env_config(&project, &aster_home)?;
    config.tools.retain(|name, _| name == tool);
    let path = if let Some(selected) = resolve_tools(&config)?.first() {
        let receipt = installed_tool(&store, selected)?
            .ok_or_else(|| anyhow!("tool not installed: {tool}"))?;
        store
            .root_path()
            .join("installs")
            .join(receipt.name)
            .join(receipt.version)
            .join(receipt.executable)
    } else {
        store
            .which(tool)?
            .ok_or_else(|| anyhow!("tool not installed: {tool}"))?
    };
    if json {
        println!("{}", serde_json::json!({ "name": tool, "path": path }));
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

fn uninstall_cmd(tool: &str, unpin: bool, json: bool) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    store.remove_sync(tool).map_err(anyhow::Error::new)?;
    if unpin {
        let path = project_env_path(&project);
        if let Some(mut file) = read_env_acl(&path)? {
            file.tools.remove(tool);
            write_env_acl(&path, &file).map_err(anyhow::Error::new)?;
        }
        let _ = aster_home;
    }
    if json {
        println!("{}", serde_json::json!({ "removed": tool, "unpin": unpin }));
    } else {
        println!("Removed {tool}.");
    }
    Ok(())
}

fn show_env(json: bool) -> Result<()> {
    let (aster_home, project, _store) = open_context()?;
    let config = load_env_config(&project, &aster_home).map_err(anyhow::Error::new)?;
    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            tools: &'a BTreeMap<String, String>,
            vars: &'a BTreeMap<String, String>,
            project: PathBuf,
            user: PathBuf,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&Out {
                tools: &config.tools,
                vars: &config.vars,
                project: project_env_path(&project),
                user: user_env_path(&aster_home),
            })?
        );
    } else {
        println!("# tools");
        if config.tools.is_empty() {
            println!("(none)");
        } else {
            for (name, version) in &config.tools {
                println!("{name} = {version}");
            }
        }
        println!("# vars");
        if config.vars.is_empty() {
            println!("(none)");
        } else {
            for (name, value) in &config.vars {
                println!("{name}={value}");
            }
        }
    }
    Ok(())
}

fn set_env(assignment: &str, global: bool, json: bool) -> Result<()> {
    let (name, value) = split_assignment(assignment)?;
    let (aster_home, project, _store) = open_context()?;
    let path = if global {
        user_env_path(&aster_home)
    } else {
        project_env_path(&project)
    };
    let mut file = read_env_acl(&path)?.unwrap_or_default();
    file.vars.insert(name.clone(), value.clone());
    write_env_acl(&path, &file).map_err(anyhow::Error::new)?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "name": name, "value": value, "path": path })
        );
    } else {
        println!("Set {name} in {}.", path.display());
    }
    Ok(())
}

fn unset_env(name: &str, global: bool, json: bool) -> Result<()> {
    let (aster_home, project, _store) = open_context()?;
    let path = if global {
        user_env_path(&aster_home)
    } else {
        project_env_path(&project)
    };
    let mut file =
        read_env_acl(&path)?.ok_or_else(|| anyhow!("env acl not found: {}", path.display()))?;
    if file.vars.remove(name).is_none() {
        bail!("variable not set: {name}");
    }
    write_env_acl(&path, &file).map_err(anyhow::Error::new)?;
    if json {
        println!("{}", serde_json::json!({ "unset": name, "path": path }));
    } else {
        println!("Unset {name} in {}.", path.display());
    }
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────

async fn run_inherited_command(command: ProcessCommand) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut command = command;
        Err(command.exec().into())
    }
    #[cfg(windows)]
    {
        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        let scope = aster_tty_utils::ProcessScope::new();
        // Keep the inherited console group so Ctrl-C still reaches the command.
        let mut child = command.spawn()?;
        let _group = scope.enroll(&child)?;
        let status = child.wait().await?;
        match status.code() {
            Some(0) => Ok(()),
            Some(code) => std::process::exit(code),
            None => bail!("command terminated without an exit code"),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = command;
        bail!("command execution is unsupported on this platform")
    }
}

fn open_context() -> Result<(PathBuf, PathBuf, ToolStore)> {
    let aster_home = aster_dirs::resolve_grok_home()
        .ok_or_else(|| anyhow!("cannot resolve ASTER_HOME / home directory"))?;
    let project = env::current_dir().context("current directory")?;
    let store = ToolStore::open(aster_home.join("tools")).map_err(anyhow::Error::new)?;
    Ok((aster_home, project, store))
}

async fn install_resolved(store: &ToolStore, config: &EnvConfig) -> Result<Vec<InstallRecord>> {
    let resolved = resolve_tools(config).map_err(anyhow::Error::new)?;
    acquire_all(store, &resolved).await
}

async fn acquire_all(
    store: &ToolStore,
    resolved: &[aster_use_tool_acquire::ResolvedTool],
) -> Result<Vec<InstallRecord>> {
    if resolved.is_empty() {
        return Ok(Vec::new());
    }
    let store = store.clone();
    let resolved = resolved.to_vec();
    tokio::task::spawn_blocking(move || {
        let http = HttpSource::with_policy(FetchPolicy::default()).map_err(anyhow::Error::new)?;
        let mut out = Vec::with_capacity(resolved.len());
        for tool in resolved {
            let installed = if tool.version.is_some() {
                installed_tool(&store, &tool)?
            } else {
                None
            };
            let receipt = if let Some(receipt) = installed {
                receipt
            } else {
                store
                    .acquire_with(
                        AcquireRequest {
                            name: tool.name.clone(),
                            spec: tool.spec.clone(),
                            version: tool.version.clone(),
                            executable_name: tool.executable.clone(),
                        },
                        &http,
                    )
                    .map_err(anyhow::Error::new)?
            };
            out.push(InstallRecord {
                name: receipt.name,
                version: receipt.version,
                executable: receipt.executable,
            });
        }
        Ok(out)
    })
    .await
    .context("tool install task")?
}

fn runtime_env(store: &ToolStore, config: &EnvConfig) -> Result<Vec<(OsString, OsString)>> {
    let mut pairs = Vec::new();
    let mut paths = Vec::new();
    for tool in resolve_tools(config)? {
        let receipt = installed_tool(store, &tool)?.ok_or_else(|| {
            anyhow!(
                "tool not installed: {}@{}",
                tool.name,
                tool.version.as_deref().unwrap_or("latest")
            )
        })?;
        paths.push(
            store
                .root_path()
                .join("installs")
                .join(receipt.name)
                .join(receipt.version),
        );
    }
    if let Some(existing) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing));
    }
    pairs.push((
        OsString::from("PATH"),
        env::join_paths(paths).context("managed PATH")?,
    ));
    for (name, value) in &config.vars {
        // PATH must retain the project's selected installations ahead of inherited commands.
        if name == "PATH" || (cfg!(windows) && name.eq_ignore_ascii_case("PATH")) {
            continue;
        }
        pairs.push((OsString::from(name), OsString::from(value)));
    }
    Ok(pairs)
}

fn installed_tool(
    store: &ToolStore,
    tool: &aster_use_tool_acquire::ResolvedTool,
) -> Result<Option<aster_use_tool_acquire::ToolReceipt>> {
    let receipt = store.installed_receipt(&tool.name, tool.version.as_deref())?;
    if let Some(receipt) = &receipt {
        if receipt.spec != tool.spec || receipt.executable != tool.executable {
            return Err(
                aster_use_tool_acquire::ToolAcquireError::InstallationIdentityMismatch {
                    name: receipt.name.clone(),
                    version: receipt.version.clone(),
                }
                .into(),
            );
        }
    }
    Ok(receipt)
}

fn activation_script(shell: &str, store: &ToolStore, config: &EnvConfig) -> Result<String> {
    let shell = shell.to_ascii_lowercase();
    if !matches!(
        shell.as_str(),
        "sh" | "bash" | "zsh" | "fish" | "pwsh" | "powershell" | "powershell.exe" | "pwsh.exe"
    ) {
        bail!("unsupported activation shell: {shell}; expected sh, bash, zsh, fish, or PowerShell");
    }
    let mut out = String::new();
    for (name, value) in runtime_env(store, config)? {
        let name = name
            .to_str()
            .context("activation variable name is not UTF-8")?;
        let value = value
            .to_str()
            .context("activation variable value is not UTF-8")?;
        match shell.as_str() {
            "fish" => out.push_str(&format!("set -gx {name} {}\n", fish_quote(value))),
            "pwsh" | "powershell" | "powershell.exe" | "pwsh.exe" => {
                out.push_str(&format!("$env:{name} = {}\n", pwsh_quote(value)));
            }
            _ => out.push_str(&format!("export {name}={}\n", sh_quote(value))),
        }
    }
    Ok(out)
}

fn resolve_aster_use() -> Result<PathBuf> {
    if let Some(explicit) = env::var_os("ASTER_USE_BIN") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Ok(path);
        }
        bail!("ASTER_USE_BIN is not a file: {}", path.display());
    }
    if let Ok(current) = env::current_exe() {
        if let Some(dir) = current.parent() {
            for name in ["aster-use", "aster-use.exe"] {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
    }
    which_on_path("aster-use")
        .ok_or_else(|| anyhow!("aster-use not found next to aster or on PATH; set ASTER_USE_BIN"))
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = directory.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

fn split_tool_pin(raw: &str) -> Result<(String, String)> {
    let (name, version) = match raw.split_once('@') {
        Some((name, version)) if !name.is_empty() && !version.is_empty() => {
            (name.to_string(), version.to_string())
        }
        Some(_) => bail!("invalid tool pin: {raw}"),
        None => (raw.to_string(), "latest".to_string()),
    };
    if catalog_by_name(&name).is_none() {
        bail!("unknown tool: {name}");
    }
    Ok((name, version))
}

fn split_assignment(raw: &str) -> Result<(String, String)> {
    let Some((name, value)) = raw.split_once('=') else {
        bail!("expected NAME=value, got {raw}");
    };
    if name.is_empty() {
        bail!("empty environment variable name");
    }
    Ok((name.to_string(), value.to_string()))
}

fn emit_install(json: bool, receipts: &[InstallRecord]) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(receipts).unwrap_or_else(|_| "[]".into())
        );
        return;
    }
    if receipts.is_empty() {
        println!("Nothing to install.");
        return;
    }
    for receipt in receipts {
        println!("installed {} {}", receipt.name, receipt.version);
    }
}

fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn fish_quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn pwsh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[derive(Debug, Clone, Serialize)]
struct InstallRecord {
    name: String,
    version: String,
    executable: String,
}

#[cfg(test)]
mod tests;
