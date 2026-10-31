//! `aster tool|exec|env|activate|use`: project environment manager (mise-like).
//!
//! Tools and vars come from `.aster/env.acl` (and `$ASTER_HOME/env.acl`).
//! Installs go through `aster-use-tool-acquire` under `$ASTER_HOME/tools`.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{anyhow, bail, Context, Result};
use aster_use_tool_acquire::{
    catalog_by_name, load_env_config, project_env_path, resolve_tools, user_env_path, write_env_acl,
    AcquireRequest, EnvConfig, FetchPolicy, HttpSource, ToolStore,
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
    /// Print the shim path for an installed tool
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

#[derive(Debug, Subcommand, Clone)]
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

#[derive(Debug, clap::Args, Clone)]
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
    pub command: Vec<String>,
}

#[derive(Debug, clap::Args, Clone)]
pub struct ActivateArgs {
    /// Shell to emit activation for. Defaults to `$SHELL` basename, else bash.
    #[arg(value_name = "SHELL")]
    pub shell: Option<String>,
}

#[derive(Debug, clap::Args, Clone)]
pub struct UseForwardArgs {
    /// Arguments forwarded to the `aster-use` binary.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub args: Vec<String>,
}

// ── Dispatch ────────────────────────────────────────────────────────

pub async fn run_tool(args: ToolArgs) -> Result<()> {
    match args.command {
        ToolCommand::Install { tool, json } => install_cmd(tool.as_deref(), json).await,
        ToolCommand::Use {
            tool,
            global,
            json,
        } => use_cmd(&tool, global, json).await,
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
    let config = load_env_config(&project, &aster_home).map_err(anyhow::Error::msg)?;
    if !args.no_install {
        install_resolved(&store, &config).await?;
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
    let status = child
        .status()
        .with_context(|| format!("failed to exec {program}"))?;
    if let Some(code) = status.code() {
        if code != 0 {
            std::process::exit(code);
        }
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            std::process::exit(128 + signal);
        }
    }
    bail!("command terminated without an exit code");
}

pub fn run_activate(args: ActivateArgs) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let config = load_env_config(&project, &aster_home).map_err(anyhow::Error::msg)?;
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
    let status = ProcessCommand::new(&binary)
        .args(&args.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to run {}", binary.display()))?;
    if let Some(code) = status.code() {
        if code != 0 {
            std::process::exit(code);
        }
        return Ok(());
    }
    bail!("aster-use terminated without an exit code");
}

// ── Commands ────────────────────────────────────────────────────────

async fn install_cmd(tool: Option<&str>, json: bool) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let mut config = load_env_config(&project, &aster_home).map_err(anyhow::Error::msg)?;
    let receipts = if let Some(raw) = tool {
        let (name, version) = split_tool_pin(raw)?;
        config.tools.insert(name.clone(), version.clone());
        let resolved = resolve_tools(&EnvConfig {
            tools: BTreeMap::from([(name, version)]),
            vars: BTreeMap::new(),
        })
        .map_err(anyhow::Error::msg)?;
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
    let _ = catalog_by_name(&name).ok_or_else(|| anyhow!("unknown tool: {name}"))?;
    let path = if global {
        user_env_path(&aster_home)
    } else {
        project_env_path(&project)
    };
    let mut file = if path.exists() {
        aster_use_tool_acquire::parse_env_acl(
            &std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?,
        )
        .map_err(anyhow::Error::msg)?
    } else {
        EnvConfig::default()
    };
    file.tools.insert(name.clone(), version.clone());
    write_env_acl(&path, &file).map_err(anyhow::Error::msg)?;
    let resolved = resolve_tools(&EnvConfig {
        tools: BTreeMap::from([(name, version)]),
        vars: BTreeMap::new(),
    })
    .map_err(anyhow::Error::msg)?;
    let receipts = acquire_all(&store, &resolved).await?;
    emit_install(json, &receipts);
    Ok(())
}

fn ls_cmd(json: bool) -> Result<()> {
    let (_aster_home, _project, store) = open_context()?;
    let receipts = store.list_sync().map_err(anyhow::Error::msg)?;
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
    let (_aster_home, _project, store) = open_context()?;
    let path = store
        .which(tool)
        .map_err(anyhow::Error::msg)?
        .ok_or_else(|| anyhow!("tool not installed: {tool}"))?;
    if json {
        println!(
            "{}",
            serde_json::json!({ "name": tool, "path": path })
        );
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

fn uninstall_cmd(tool: &str, unpin: bool, json: bool) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    store.remove_sync(tool).map_err(anyhow::Error::msg)?;
    if unpin {
        let path = project_env_path(&project);
        if path.exists() {
            let mut file = aster_use_tool_acquire::parse_env_acl(
                &std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?,
            )
            .map_err(anyhow::Error::msg)?;
            file.tools.remove(tool);
            write_env_acl(&path, &file).map_err(anyhow::Error::msg)?;
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
    let config = load_env_config(&project, &aster_home).map_err(anyhow::Error::msg)?;
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
    let mut file = if path.exists() {
        aster_use_tool_acquire::parse_env_acl(
            &std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?,
        )
        .map_err(anyhow::Error::msg)?
    } else {
        EnvConfig::default()
    };
    file.vars.insert(name.clone(), value.clone());
    write_env_acl(&path, &file).map_err(anyhow::Error::msg)?;
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
    if !path.exists() {
        bail!("env acl not found: {}", path.display());
    }
    let mut file = aster_use_tool_acquire::parse_env_acl(
        &std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?,
    )
    .map_err(anyhow::Error::msg)?;
    if file.vars.remove(name).is_none() {
        bail!("variable not set: {name}");
    }
    write_env_acl(&path, &file).map_err(anyhow::Error::msg)?;
    if json {
        println!("{}", serde_json::json!({ "unset": name, "path": path }));
    } else {
        println!("Unset {name} in {}.", path.display());
    }
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────

fn open_context() -> Result<(PathBuf, PathBuf, ToolStore)> {
    let aster_home = aster_dirs::resolve_grok_home()
        .ok_or_else(|| anyhow!("cannot resolve ASTER_HOME / home directory"))?;
    let project = env::current_dir().context("current directory")?;
    let store = ToolStore::open(aster_home.join("tools")).map_err(anyhow::Error::msg)?;
    Ok((aster_home, project, store))
}

async fn install_resolved(store: &ToolStore, config: &EnvConfig) -> Result<Vec<InstallRecord>> {
    let resolved = resolve_tools(config).map_err(anyhow::Error::msg)?;
    acquire_all(store, &resolved).await
}

async fn acquire_all(
    store: &ToolStore,
    resolved: &[aster_use_tool_acquire::ResolvedTool],
) -> Result<Vec<InstallRecord>> {
    if resolved.is_empty() {
        return Ok(Vec::new());
    }
    let http = HttpSource::with_policy(FetchPolicy::default()).map_err(anyhow::Error::msg)?;
    let store = store.clone();
    let resolved = resolved.to_vec();
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::with_capacity(resolved.len());
        for tool in resolved {
            let receipt = store
                .acquire_with(
                    AcquireRequest {
                        name: tool.name.clone(),
                        spec: tool.spec.clone(),
                        version: tool.version.clone(),
                        executable_name: tool.executable.clone(),
                    },
                    &http,
                )
                .map_err(|error| anyhow!("{error}"))?;
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
    let shim = store.shim_dir();
    let mut path = OsString::from(shim.as_os_str());
    path.push(if cfg!(windows) { ";" } else { ":" });
    if let Some(existing) = env::var_os("PATH") {
        path.push(existing);
    }
    pairs.push((OsString::from("PATH"), path));
    for (name, value) in &config.vars {
        pairs.push((OsString::from(name), OsString::from(value)));
    }
    Ok(pairs)
}

fn activation_script(shell: &str, store: &ToolStore, config: &EnvConfig) -> Result<String> {
    let shim = store.shim_dir();
    let shim_display = shim.display().to_string();
    let shell = shell.to_ascii_lowercase();
    let mut out = String::new();
    match shell.as_str() {
        "fish" => {
            out.push_str(&format!("set -gx PATH \"{shim_display}\" $PATH\n"));
            for (name, value) in &config.vars {
                out.push_str(&format!(
                    "set -gx {} {}\n",
                    name,
                    fish_quote(value)
                ));
            }
        }
        "pwsh" | "powershell" | "powershell.exe" | "pwsh.exe" => {
            out.push_str(&format!(
                "$env:PATH = '{};' + $env:PATH\n",
                shim_display.replace('\'', "''")
            ));
            for (name, value) in &config.vars {
                out.push_str(&format!(
                    "$env:{} = {}\n",
                    name,
                    pwsh_quote(value)
                ));
            }
        }
        // bash, zsh, sh, and unknown shells
        _ => {
            out.push_str(&format!("export PATH=\"{shim_display}:$PATH\"\n"));
            for (name, value) in &config.vars {
                out.push_str(&format!("export {}={}\n", name, sh_quote(value)));
            }
        }
    }
    Ok(out)
}

fn resolve_aster_use() -> Result<PathBuf> {
    if let Ok(explicit) = env::var("ASTER_USE_BIN") {
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
    which_on_path("aster-use").ok_or_else(|| {
        anyhow!("aster-use not found next to aster or on PATH; set ASTER_USE_BIN")
    })
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
        println!("{}", serde_json::to_string_pretty(receipts).unwrap_or_else(|_| "[]".into()));
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
    format!("'{}'", value.replace('\'', "\\'"))
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
mod tests {
    use super::*;

    #[test]
    fn activation_bash_exports_shim_and_vars() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ToolStore::open(temporary.path().join("tools")).unwrap();
        let config = EnvConfig {
            tools: BTreeMap::new(),
            vars: BTreeMap::from([("NODE_ENV".into(), "development".into())]),
        };
        let script = activation_script("bash", &store, &config).unwrap();
        assert!(script.contains("export PATH="));
        assert!(script.contains("export NODE_ENV='development'"));
        assert!(script.contains(store.shim_dir().to_str().unwrap()));
    }

    #[test]
    fn split_tool_pin_defaults_latest() {
        let (name, version) = split_tool_pin("uv").unwrap();
        assert_eq!(name, "uv");
        assert_eq!(version, "latest");
        let (name, version) = split_tool_pin("uv@0.6.0").unwrap();
        assert_eq!(name, "uv");
        assert_eq!(version, "0.6.0");
    }

    #[test]
    fn clap_accepts_tool_env_exec_activate_use() {
        use crate::app::{Command, PagerArgs};
        use clap::Parser;

        assert!(matches!(
            PagerArgs::try_parse_from(["aster", "tool", "ls", "--json"])
                .unwrap()
                .command,
            Some(Command::Tool(_))
        ));
        assert!(matches!(
            PagerArgs::try_parse_from(["aster", "env", "show"])
                .unwrap()
                .command,
            Some(Command::Env(_))
        ));
        assert!(matches!(
            PagerArgs::try_parse_from(["aster", "exec", "--", "uv", "--version"])
                .unwrap()
                .command,
            Some(Command::Exec(_))
        ));
        assert!(matches!(
            PagerArgs::try_parse_from(["aster", "activate", "zsh"])
                .unwrap()
                .command,
            Some(Command::Activate(_))
        ));
        assert!(matches!(
            PagerArgs::try_parse_from(["aster", "use", "plugin", "list"])
                .unwrap()
                .command,
            Some(Command::Use(_))
        ));
    }
}
