//! `aster tool|exec|env|activate|use`: project environment manager (mise-like).
//!
//! Tools and vars come from `.aster/env.acl` (and `$ASTER_HOME/env.acl`).
//! Installs go through `aster-use-tool-acquire` under `$ASTER_HOME/tools`.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use aster_use_tool_acquire::{
    AcquireRequest, EnvDeclarations, EnvLock, EnvToolOrigin, FetchPolicy, HttpSource,
    LoadedEnvDeclarations, ResolvedEnvTool, ToolAcquireError, ToolReceipt, ToolSource, ToolStore,
    VersionRequest, VersionRequestKind, catalog_by_name, find_env_executable,
    load_env_declarations, load_env_declarations_with_sources, nearest_project_env_declarations,
    parse_tool_spec, project_env_lock_path, project_env_path, read_env_declarations,
    resolve_declarations, resolve_env_tool, user_env_path, write_env_declarations,
};
#[cfg(test)]
use aster_use_tool_acquire::{EnvConfig, resolve_tools, write_env_acl};
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
    /// Rebuild project-aware command shims from every installed version (Unix)
    Reshim,
    /// Remove versions unused by tracked projects, global declarations or running tools
    Prune {
        /// Restrict tool cleanup to these installed tool names.
        tool: Vec<String>,
        /// Clean missing configuration tracking only, unless --tools is also given.
        #[arg(long)]
        configs: bool,
        /// Clean installed tools only, unless --configs is also given.
        #[arg(long)]
        tools: bool,
        #[arg(long, short = 'y')]
        yes: bool,
        #[arg(long, short = 'n')]
        dry_run: bool,
        #[arg(long)]
        dry_run_code: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(hide = true)]
    ShimExec(ShimExecArgs),
    /// Install declared tools or the requested tool versions
    Install {
        /// Tool names with optional versions. Bare names use their declarations.
        /// Omit to install every declared tool.
        tool: Vec<String>,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
        /// Refuse downloads and use installed versions only.
        #[arg(long)]
        offline: bool,
        /// Require a matching project lock and verify every installed file.
        #[arg(long, conflicts_with = "tool")]
        locked: bool,
    },
    /// Lock declared tools and their complete installed file sets for this platform
    Lock {
        /// Generate the lock from installed tools without downloading.
        #[arg(long)]
        offline: bool,
        #[arg(long)]
        json: bool,
    },
    /// List provider versions for a catalog tool, or read a cached listing offline
    #[command(name = "ls-remote")]
    LsRemote {
        tool: String,
        #[arg(long)]
        offline: bool,
        #[arg(long)]
        json: bool,
    },
    /// Install and pin tools into the project env.acl
    Use {
        /// Tool names with optional versions. Bare names prefer installed versions.
        #[arg(required = true)]
        tool: Vec<String>,
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
    /// Remove installed versions; use --all for multiple matches or every tool
    Uninstall {
        /// Tool names with optional versions. Omit with --all to remove every installed tool.
        #[arg(required_unless_present_any = ["all", "project"])]
        tool: Vec<String>,
        /// Remove every matching installed version, or every tool when none is named.
        #[arg(long, short)]
        all: bool,
        /// Remove versions requested by the nearest project, preserving its declarations.
        /// Other projects using these shared installations must reinstall them.
        #[arg(long, conflicts_with_all = ["all", "tool", "unpin"])]
        project: bool,
        /// Confirm project removal without an interactive prompt.
        #[arg(long, short = 'y', requires = "project")]
        yes: bool,
        /// Show the selected versions without removing files or editing declarations.
        #[arg(long, short = 'n')]
        dry_run: bool,
        /// Preview removal and exit with status 1 when matching versions exist.
        #[arg(long)]
        dry_run_code: bool,
        /// Also drop the project pin; an exact version only drops the same literal pin.
        #[arg(long)]
        unpin: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(clap::Args, Clone)]
pub struct ShimExecArgs {
    home: PathBuf,
    tool: String,
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    command: Vec<OsString>,
}

impl std::fmt::Debug for ShimExecArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ShimExecArgs")
            .field("home", &self.home)
            .field("tool", &self.tool)
            .field(
                "command",
                &format_args!("{} redacted arguments", self.command.len()),
            )
            .finish()
    }
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
    /// Select tools for this execution without writing env.acl.
    /// Repeat a tool to select its versions in argument order.
    #[arg(
        short = 't',
        long = "tool",
        value_name = "TOOL@REQUEST",
        conflicts_with = "locked"
    )]
    pub tools: Vec<String>,
    /// Skip installing missing tools declared in env.acl.
    #[arg(long = "no-install")]
    pub no_install: bool,
    /// Refuse downloads and use installed versions only.
    #[arg(long)]
    pub offline: bool,
    /// Require a matching project lock and verify every installed file.
    #[arg(long)]
    pub locked: bool,
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
            .field(
                "tools",
                &format_args!("{} redacted requests", self.tools.len()),
            )
            .field("no_install", &self.no_install)
            .field("offline", &self.offline)
            .field("locked", &self.locked)
            .field(
                "command",
                &format_args!("{} redacted arguments", self.command.len()),
            )
            .finish()
    }
}

#[derive(Debug, clap::Args, Clone)]
pub struct ActivateArgs {
    /// Shell to emit activation for. Defaults to `$SHELL`, then the platform shell.
    #[arg(value_name = "SHELL")]
    pub shell: Option<String>,
}

#[derive(clap::Args, Clone)]
pub struct UseForwardArgs {
    /// Package, plugin and registry command arguments.
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

/// Manage installed tools and project pins, returning install, validation, and I/O errors.
/// Active blocking I/O continues if the calling future is canceled.
/// `uninstall --dry-run-code` exits with status 1 when matching versions exist.
pub async fn run_tool(args: ToolArgs) -> Result<()> {
    match args.command {
        ToolCommand::Reshim => tokio::task::spawn_blocking(|| {
            let context = open_env_context_sync()?;
            let count = reshim_store(&context.aster_home, &context.store)?;
            writeln!(std::io::stdout().lock(), "Rebuilt {count} command shims.")?;
            Ok(())
        })
        .await
        .context("rebuild command shims task")?,
        ToolCommand::ShimExec(ShimExecArgs {
            home,
            tool,
            command,
        }) => {
            let context = tokio::task::spawn_blocking(move || {
                let project = env::current_dir()?;
                let store = ToolStore::open(home.join("tools"))?;
                load_env_context(home, project, store)
            })
            .await
            .context("load shim environment task")??;
            if !context.declarations.config().tools.contains_key(&tool) {
                bail!("no version is configured for shim command {tool}");
            }
            run_exec_in_context(
                ExecArgs {
                    tools: Vec::new(),
                    no_install: false,
                    offline: false,
                    locked: false,
                    command,
                },
                context,
                BTreeMap::new(),
            )
            .await
        }
        ToolCommand::Install {
            tool,
            json,
            offline,
            locked,
        } => install_cmd(tool, json, offline, locked).await,
        ToolCommand::Lock { offline, json } => lock_cmd(offline, json).await,
        ToolCommand::LsRemote {
            tool,
            offline,
            json,
        } => ls_remote_cmd(&tool, offline, json).await,
        ToolCommand::Use { tool, global, json } => use_cmd(tool, global, json).await,
        ToolCommand::Prune {
            tool,
            configs,
            tools,
            yes,
            dry_run,
            dry_run_code,
            json,
        } => tokio::task::spawn_blocking(move || {
            prune_cmd(&tool, configs, tools, yes, dry_run, dry_run_code, json)
        })
        .await
        .context("prune unused tool versions task")?,
        ToolCommand::Ls { json } => ls_cmd(json),
        ToolCommand::Which { tool, json } => {
            tokio::task::spawn_blocking(move || which_cmd(&tool, json))
                .await
                .context("locate tool task")?
        }
        ToolCommand::Uninstall {
            tool,
            all,
            project,
            yes,
            dry_run,
            dry_run_code,
            unpin,
            json,
        } => tokio::task::spawn_blocking(move || {
            let scope = if project {
                UninstallScope::Project { yes }
            } else {
                UninstallScope::Named { all }
            };
            uninstall_cmd(&tool, scope, dry_run, dry_run_code, unpin, json)
        })
        .await
        .context("uninstall tool task")?,
    }
}

/// Read or edit ACL environment declarations. Invalid declarations and I/O errors are returned.
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

/// Execute with project-selected tools and variables, refusing missing or mismatched installations.
/// Unix replaces this process; Windows waits asynchronously with an owned process-tree job.
pub async fn run_exec(args: ExecArgs) -> Result<()> {
    if args.locked && !args.tools.is_empty() {
        bail!("temporary tool overrides cannot be used with a project lock");
    }
    let mut overrides: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for raw in &args.tools {
        let (name, request) = split_tool_pin(raw)?;
        overrides.entry(name).or_default().push(request);
    }
    let context = open_env_context().await?;
    run_exec_in_context(args, context, overrides).await
}

async fn run_exec_in_context(
    args: ExecArgs,
    context: EnvContext,
    overrides: BTreeMap<String, Vec<String>>,
) -> Result<()> {
    let config = context.declarations.config();
    let receipts = if args.locked {
        install_locked(
            &context.store,
            &context.project,
            config,
            args.offline || args.no_install,
        )
        .await?
    } else {
        install_env_tools(&context, args.offline, !args.no_install, overrides).await?
    };
    let mut command = args.command;
    if command.is_empty() {
        bail!("aster exec requires a command");
    }
    let program = command.remove(0);
    let environment = runtime_env_from_installs(&context.store, config, &receipts)?;
    if !args.no_install && !args.offline && receipts.iter().any(|receipt| receipt.origin.is_none())
    {
        refresh_dispatch_shims(&context).await?;
    }
    let search = environment
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .or_else(|| env::var_os("PATH"));
    let lookup_program = program.clone();
    let cwd = context.project.clone();
    let executable = tokio::task::spawn_blocking(move || {
        find_env_executable(&lookup_program, search.as_deref(), &cwd)?.ok_or_else(|| {
            ToolAcquireError::io(
                "locate exec command",
                Path::new(&lookup_program),
                std::io::Error::from(std::io::ErrorKind::NotFound),
            )
        })
    })
    .await
    .context("locate exec command task")??;
    let mut child = ProcessCommand::new(executable);
    track_env_sources(&context).await?;
    child.args(&command);
    child.envs(environment);
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

/// Write one environment snapshot for a supported shell; unsupported values return an error.
pub fn run_activate(args: ActivateArgs) -> Result<()> {
    let context = open_env_context_sync()?;
    let shell = args
        .shell
        .or_else(|| {
            env::var_os("SHELL").and_then(|value| {
                PathBuf::from(value)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
        })
        .unwrap_or_else(|| if cfg!(windows) { "powershell" } else { "bash" }.to_owned());
    std::io::stdout().write_all(activation_script(&shell, &context)?.as_bytes())?;
    Ok(())
}

/// Forward native arguments to `aster-use`, preserving its exit status and reporting launch errors.
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

async fn install_cmd(tools: Vec<String>, json: bool, offline: bool, locked: bool) -> Result<()> {
    if locked && !tools.is_empty() {
        bail!("explicit tool selections cannot be used with a project lock");
    }
    let context = open_env_context().await?;
    let receipts = if locked {
        install_locked(
            &context.store,
            &context.project,
            context.declarations.config(),
            offline,
        )
        .await?
    } else if !tools.is_empty() {
        let resolved = resolve_tool_arguments(&context, tools, ToolArgumentMode::Install).await?;
        acquire_context_tools(&context.store, &resolved, offline, true).await?
    } else {
        install_env_tools(&context, offline, true, BTreeMap::new()).await?
    };
    refresh_dispatch_shims(&context).await?;
    track_env_sources(&context).await?;
    emit_install(json, &receipts)?;
    Ok(())
}

async fn lock_cmd(offline: bool, json: bool) -> Result<()> {
    let (aster_home, project, store) = open_context()?;
    let config = load_env_declarations(&project, &aster_home)?;
    let receipts = install_resolved(&store, &config, offline).await?;
    let path = project_env_lock_path(project);
    let destination = path.clone();
    tokio::task::spawn_blocking(move || {
        EnvLock::capture_declarations(&config, &store)?.write(destination)
    })
    .await
    .context("capture tool lock task")??;
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(
            stdout,
            "{}",
            serde_json::json!({ "path": path, "tools": receipts })
        )?;
    } else {
        writeln!(
            stdout,
            "Locked {} tools in {}.",
            receipts.len(),
            path.display()
        )?;
    }
    Ok(())
}

async fn ls_remote_cmd(tool: &str, offline: bool, json: bool) -> Result<()> {
    let (_aster_home, _project, store) = open_context()?;
    let catalog = catalog_by_name(tool).ok_or_else(|| anyhow!("unknown tool: {tool}"))?;
    let spec = parse_tool_spec(catalog.spec)?;
    let listing = tokio::task::spawn_blocking(move || {
        let http = if offline {
            None
        } else {
            Some(HttpSource::with_policy(FetchPolicy::default())?)
        };
        store.remote_versions_with(&spec, http.as_ref().map(|source| source as &dyn ToolSource))
    })
    .await
    .context("list remote tool versions task")??;
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(stdout, "{}", serde_json::to_string_pretty(&listing)?)?;
    } else {
        for version in listing.versions() {
            writeln!(stdout, "{}", version.version())?;
        }
    }
    Ok(())
}

async fn use_cmd(tools: Vec<String>, global: bool, json: bool) -> Result<()> {
    if tools.is_empty() {
        bail!("specify at least one tool");
    }
    let context = open_env_context().await?;
    let path = if global {
        user_env_path(&context.aster_home)
    } else {
        project_env_path(&context.project)
    };
    let read_path = path.clone();
    let mut file = tokio::task::spawn_blocking(move || read_env_declarations(read_path))
        .await
        .context("read environment pin task")??
        .unwrap_or_default();
    let resolved = resolve_tool_arguments(&context, tools, ToolArgumentMode::Use).await?;
    let path_pins = resolved
        .iter()
        .map(|selection| match selection.origin() {
            EnvToolOrigin::Path(paths) => paths
                .root()
                .to_str()
                .context("external tool path cannot be represented in UTF-8 ACL")
                .map(|root| Some(format!("path:{root}"))),
            _ => Ok(None),
        })
        .collect::<Result<Vec<_>>>()?;
    let receipts = acquire_context_tools(&context.store, &resolved, false, true).await?;
    let mut selected = BTreeMap::<String, Vec<String>>::new();
    for (receipt, path_pin) in receipts.iter().zip(path_pins) {
        let pin = path_pin.unwrap_or_else(|| receipt.version.clone());
        let pins = selected.entry(receipt.name.clone()).or_default();
        if !pins.contains(&pin) {
            pins.push(pin);
        }
    }
    file.tools.extend(selected);
    let project = context.project.clone();
    let home = context.aster_home.clone();
    let store = context.store.clone();
    tokio::task::spawn_blocking(move || {
        write_env_declarations(&path, &file)?;
        load_env_declarations_with_sources(&project, &home)?.track_sources(&store)
    })
    .await
    .context("write environment pin task")??;
    refresh_dispatch_shims(&context).await?;
    emit_install(json, &receipts)?;
    Ok(())
}

fn ls_cmd(json: bool) -> Result<()> {
    let (_aster_home, _project, store) = open_context()?;
    let receipts = store.list_sync().map_err(anyhow::Error::new)?;
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(stdout, "{}", serde_json::to_string_pretty(&receipts)?)?;
    } else if receipts.is_empty() {
        writeln!(
            stdout,
            "No tools installed under {}.",
            store.root_path().display()
        )?;
    } else {
        for receipt in receipts {
            writeln!(
                stdout,
                "{}\t{}\t{}",
                receipt.name, receipt.version, receipt.executable
            )?;
        }
    }
    Ok(())
}

fn which_cmd(tool: &str, json: bool) -> Result<()> {
    let context = open_env_context_sync()?;
    let path = if let Some(selected) = context
        .declarations
        .resolve_tool_versions(tool, context.user_home.as_deref())?
    {
        let mut executable = None;
        for selected in selected {
            if let Some(path) = selected.which(
                &context.store,
                env::var_os("PATH").as_deref(),
                &context.project,
            )? {
                executable = Some(path);
                break;
            }
        }
        executable.ok_or_else(|| anyhow!("tool executable not found: {tool}"))?
    } else {
        let declared = if catalog_by_name(tool).is_none() {
            let paths = installed_context_tools(&context)?
                .into_iter()
                .flat_map(|record| record.bin_dirs)
                .collect::<Vec<_>>();
            let search = env::join_paths(paths).context("declared executable search path")?;
            find_env_executable(OsStr::new(tool), Some(&search), &context.project)?
        } else {
            None
        };
        match declared {
            Some(path) => path,
            None => context
                .store
                .which(tool)?
                .ok_or_else(|| anyhow!("tool not installed: {tool}"))?,
        }
    };
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(
            stdout,
            "{}",
            serde_json::json!({ "name": tool, "path": serde_json::to_value(&path)? })
        )?;
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            stdout.write_all(path.as_os_str().as_bytes())?;
            stdout.write_all(b"\n")?;
        }
        #[cfg(not(unix))]
        writeln!(
            stdout,
            "{}",
            path.to_str()
                .context("tool path cannot be represented in UTF-8 text")?
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum UninstallScope {
    Named { all: bool },
    Project { yes: bool },
}

fn uninstall_cmd(
    tools: &[String],
    scope: UninstallScope,
    dry_run: bool,
    dry_run_code: bool,
    unpin: bool,
    json: bool,
) -> Result<()> {
    let all = matches!(scope, UninstallScope::Named { all: true });
    if tools.is_empty() && matches!(scope, UninstallScope::Named { all: false }) {
        bail!("specify at least one tool, --all or --project");
    }
    let (home, project, store) = open_context()?;
    let path = project_env_path(&project);
    let config = if unpin {
        read_env_declarations(&path)?
    } else {
        None
    };
    let mut selected_project = None;
    let selections = if matches!(scope, UninstallScope::Project { .. }) {
        let (root, declarations) = nearest_project_env_declarations(&project)?
            .ok_or_else(|| anyhow!("no project environment declaration found"))?;
        selected_project = Some(root);
        select_project_uninstall(&store, &declarations)?
    } else if tools.is_empty() {
        let mut grouped = BTreeMap::<String, Vec<ToolReceipt>>::new();
        for receipt in store.all_uninstall_candidates()? {
            grouped
                .entry(receipt.name.clone())
                .or_default()
                .push(receipt);
        }
        grouped
            .into_iter()
            .map(|(name, receipts)| UninstallSelection {
                name,
                version: None,
                receipts,
            })
            .collect()
    } else {
        tools
            .iter()
            .map(|tool| select_uninstall(&store, tool, all))
            .collect::<Result<Vec<_>>>()?
    };
    let mut seen = BTreeSet::new();
    let selected = selections
        .iter()
        .flat_map(|selection| &selection.receipts)
        .filter(|receipt| seen.insert((&receipt.name, &receipt.version)))
        .collect::<Vec<_>>();
    let preview = dry_run || dry_run_code;
    let project_json = if json {
        selected_project
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?
    } else {
        None
    };
    if !preview
        && !selected.is_empty()
        && let (Some(root), UninstallScope::Project { yes }) = (&selected_project, scope)
        && !confirm_project_uninstall(root, &selected, yes)?
    {
        writeln!(std::io::stderr().lock(), "Cancelled.")?;
        if json {
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::json!({"cancelled": true, "project": project_json, "tools": []})
            )?;
        }
        return Ok(());
    }
    if !preview {
        let whole_tools = selections
            .iter()
            .filter(|selection| all && selection.version.is_none())
            .map(|selection| selection.name.as_str())
            .collect::<BTreeSet<_>>();
        for name in &whole_tools {
            store.remove_sync(name)?;
        }
        for receipt in &selected {
            if !whole_tools.contains(receipt.name.as_str()) {
                store.remove_version_sync(&receipt.name, &receipt.version)?;
            }
        }
        if let Some(mut file) = config {
            let mut changed = false;
            for selection in &selections {
                if selection.version.is_none() {
                    changed |= file.tools.remove(&selection.name).is_some();
                } else if let Some(pins) = file.tools.get_mut(&selection.name) {
                    let before = pins.len();
                    pins.retain(|pin| {
                        Some(pin.as_str()) != selection.version.as_deref()
                            && !selection
                                .receipts
                                .iter()
                                .any(|receipt| &receipt.version == pin)
                    });
                    changed |= pins.len() != before;
                    if pins.is_empty() {
                        file.tools.remove(&selection.name);
                    }
                }
            }
            if changed {
                write_env_declarations(&path, &file)?;
            }
        }
    }
    #[cfg(unix)]
    if !preview {
        reshim_store(&home, &store)?;
    }
    #[cfg(not(unix))]
    let _ = home;
    let mut stdout = std::io::stdout().lock();
    if json {
        let mut report = serde_json::Map::from_iter([
            (
                "tools".into(),
                serde_json::json!(
                    selected
                        .iter()
                        .map(|receipt| serde_json::json!({
                            "name": receipt.name,
                            "version": receipt.version
                        }))
                        .collect::<Vec<_>>()
                ),
            ),
            ("unpin".into(), serde_json::json!(unpin)),
            ("dryRun".into(), serde_json::json!(preview)),
        ]);
        if let Some(project) = project_json {
            report.insert("project".into(), project);
        }
        if let ([tool], [selection]) = (tools, selections.as_slice()) {
            let versions = selection
                .receipts
                .iter()
                .map(|receipt| &receipt.version)
                .collect::<Vec<_>>();
            if preview {
                report.insert("tool".into(), serde_json::json!(tool));
                report.insert("wouldRemove".into(), serde_json::json!(versions));
            } else {
                report.insert("removed".into(), serde_json::json!(tool));
                report.insert("versions".into(), serde_json::json!(versions));
            }
        }
        writeln!(stdout, "{}", serde_json::Value::Object(report))?;
    } else if selected.is_empty() {
        if let [tool] = tools {
            writeln!(stdout, "No installed version matches {tool}.")?;
        } else {
            writeln!(stdout, "No installed versions match the requested tools.")?;
        }
    } else {
        for receipt in &selected {
            writeln!(
                stdout,
                "{} {}@{}.",
                if preview { "Would remove" } else { "Removed" },
                receipt.name,
                receipt.version
            )?;
        }
    }
    if dry_run_code && !selected.is_empty() {
        stdout.flush()?;
        std::process::exit(1);
    }
    Ok(())
}

struct UninstallSelection {
    name: String,
    version: Option<String>,
    receipts: Vec<ToolReceipt>,
}

fn select_project_uninstall(
    store: &ToolStore,
    declarations: &EnvDeclarations,
) -> Result<Vec<UninstallSelection>> {
    let mut selections = Vec::new();
    for (name, requests) in &declarations.tools {
        for request in requests {
            if matches!(
                VersionRequest::parse(request)?.kind(),
                VersionRequestKind::Path(_) | VersionRequestKind::System
            ) {
                continue;
            }
            let selection = resolve_env_tool(name, request, None, None)?;
            if let Some(receipt) = store.uninstall_candidate_for(selection.tool())? {
                selections.push(UninstallSelection {
                    name: name.clone(),
                    version: Some(request.clone()),
                    receipts: vec![receipt],
                });
            }
        }
    }
    Ok(selections)
}

fn confirm_project_uninstall(root: &Path, selected: &[&ToolReceipt], yes: bool) -> Result<bool> {
    let mut stderr = std::io::stderr().lock();
    for receipt in selected {
        writeln!(
            stderr,
            "{}@{} requested by {}",
            receipt.name,
            receipt.version,
            root.display()
        )?;
    }
    drop(stderr);
    confirm_tool_removal(selected.len(), yes, "project removal")
}

fn confirm_tool_removal(count: usize, yes: bool, operation: &str) -> Result<bool> {
    let mut stderr = std::io::stderr().lock();
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        bail!("{operation} requires confirmation; pass --yes to uninstall non-interactively");
    }
    write!(
        stderr,
        "Remove {} installed version{}? [y/N] ",
        count,
        if count == 1 { "" } else { "s" }
    )?;
    stderr.flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().take(16).read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn prune_cmd(
    names: &[String],
    configs: bool,
    tools: bool,
    yes: bool,
    dry_run: bool,
    dry_run_code: bool,
    json: bool,
) -> Result<()> {
    let (home, _project, store) = open_context()?;
    let clean_tools = tools || !configs;
    let clean_configs = configs || !tools;
    let selected = if clean_tools {
        store.plan_prune(&home, names)?.tools().to_vec()
    } else {
        Vec::new()
    };
    let stale = if clean_configs {
        store.stale_env_sources()?
    } else {
        Vec::new()
    };
    let preview = dry_run || dry_run_code;
    if !preview && !selected.is_empty() {
        for receipt in &selected {
            writeln!(
                std::io::stderr().lock(),
                "Unused {}@{}",
                receipt.name,
                receipt.version
            )?;
        }
        if !confirm_tool_removal(selected.len(), yes, "unused-version cleanup")? {
            writeln!(std::io::stderr().lock(), "Cancelled.")?;
            return Ok(());
        }
    }
    if !preview {
        for receipt in &selected {
            store.remove_version_sync(&receipt.name, &receipt.version)?;
        }
        if clean_configs {
            store.prune_env_sources()?;
        }
        #[cfg(unix)]
        if !selected.is_empty() {
            reshim_store(&home, &store)?;
        }
    }
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(
            stdout,
            "{}",
            serde_json::json!({"dryRun":preview,"configs":stale.len(),"tools":selected.iter().map(|receipt| serde_json::json!({"name":receipt.name,"version":receipt.version})).collect::<Vec<_>>() })
        )?;
    } else {
        for receipt in &selected {
            writeln!(
                stdout,
                "{} {}@{}.",
                if preview { "Would remove" } else { "Removed" },
                receipt.name,
                receipt.version
            )?;
        }
        writeln!(
            stdout,
            "{} {} missing configuration record(s).",
            if preview { "Would forget" } else { "Forgot" },
            stale.len()
        )?;
    }
    if dry_run_code && !selected.is_empty() {
        stdout.flush()?;
        std::process::exit(1);
    }
    Ok(())
}

fn select_uninstall(store: &ToolStore, tool: &str, all: bool) -> Result<UninstallSelection> {
    let (name, version) = match tool.split_once('@') {
        Some((name, version)) => (name, Some(version)),
        None => (tool, None),
    };
    let receipts = match store.uninstall_candidates(name, version) {
        Err(ToolAcquireError::VersionMetadataMissing { spec }) => {
            let spec = parse_tool_spec(&spec)?;
            let source = HttpSource::with_policy(FetchPolicy::default())?;
            store.remote_versions_with(&spec, Some(&source))?;
            store.uninstall_candidates(name, version)?
        }
        result => result?,
    };
    if !all && receipts.len() > 1 {
        bail!(
            "multiple installed versions match {tool}; use --all to remove all matching versions"
        );
    }
    Ok(UninstallSelection {
        name: name.into(),
        version: version.map(str::to_owned),
        receipts,
    })
}

fn show_env(json: bool) -> Result<()> {
    let (aster_home, project, _store) = open_context()?;
    let config = load_env_declarations(&project, &aster_home).map_err(anyhow::Error::new)?;
    let mut stdout = std::io::stdout().lock();
    if json {
        #[derive(Serialize)]
        struct Out<'a> {
            tools: BTreeMap<&'a str, serde_json::Value>,
            vars: &'a BTreeMap<String, String>,
            project: PathBuf,
            user: PathBuf,
        }
        writeln!(
            stdout,
            "{}",
            serde_json::to_string_pretty(&Out {
                tools: config
                    .tools
                    .iter()
                    .map(|(name, versions)| {
                        let value = match versions.as_slice() {
                            [version] => serde_json::json!(version),
                            versions => serde_json::json!(versions),
                        };
                        (name.as_str(), value)
                    })
                    .collect(),
                vars: &config.vars,
                project: project_env_path(&project),
                user: user_env_path(&aster_home),
            })?
        )?;
    } else {
        writeln!(stdout, "# tools")?;
        if config.tools.is_empty() {
            writeln!(stdout, "(none)")?;
        } else {
            for (name, versions) in &config.tools {
                for version in versions {
                    writeln!(stdout, "{name} = {version}")?;
                }
            }
        }
        writeln!(stdout, "# vars")?;
        if config.vars.is_empty() {
            writeln!(stdout, "(none)")?;
        } else {
            for (name, value) in &config.vars {
                writeln!(stdout, "{name}={value}")?;
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
    let mut file = read_env_declarations(&path)?.unwrap_or_default();
    file.vars.insert(name.clone(), value.clone());
    write_env_declarations(&path, &file).map_err(anyhow::Error::new)?;
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(
            stdout,
            "{}",
            serde_json::json!({ "name": name, "value": value, "path": path })
        )?;
    } else {
        writeln!(stdout, "Set {name} in {}.", path.display())?;
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
    let mut file = read_env_declarations(&path)?
        .ok_or_else(|| anyhow!("env acl not found: {}", path.display()))?;
    if file.vars.remove(name).is_none() {
        bail!("variable not set: {name}");
    }
    write_env_declarations(&path, &file).map_err(anyhow::Error::new)?;
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(
            stdout,
            "{}",
            serde_json::json!({ "unset": name, "path": path })
        )?;
    } else {
        writeln!(stdout, "Unset {name} in {}.", path.display())?;
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

struct EnvContext {
    aster_home: PathBuf,
    project: PathBuf,
    store: ToolStore,
    declarations: LoadedEnvDeclarations,
    user_home: Option<PathBuf>,
}

fn open_env_context_sync() -> Result<EnvContext> {
    let (aster_home, project, store) = open_context()?;
    load_env_context(aster_home, project, store)
}

fn load_env_context(aster_home: PathBuf, project: PathBuf, store: ToolStore) -> Result<EnvContext> {
    let declarations = load_env_declarations_with_sources(&project, &aster_home)?;
    Ok(EnvContext {
        aster_home,
        project,
        store,
        declarations,
        user_home: aster_dirs::home_dir(),
    })
}

async fn refresh_dispatch_shims(context: &EnvContext) -> Result<()> {
    #[cfg(unix)]
    {
        let home = context.aster_home.clone();
        let store = context.store.clone();
        tokio::task::spawn_blocking(move || reshim_store(&home, &store))
            .await
            .context("refresh command shims task")??;
    }
    #[cfg(not(unix))]
    let _ = context;
    Ok(())
}

async fn track_env_sources(context: &EnvContext) -> Result<()> {
    let sources = context.declarations.clone();
    let store = context.store.clone();
    tokio::task::spawn_blocking(move || sources.track_sources(&store))
        .await
        .context("track admitted environment sources task")??;
    Ok(())
}

fn reshim_store(home: &Path, store: &ToolStore) -> Result<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let executable = env::current_exe().context("current Aster executable")?;
        store
            .reshim_with(|tool, command| {
                let mut body = b"exec ".to_vec();
                for argument in [
                    executable.as_os_str(),
                    OsStr::new("--no-auto-update"),
                    OsStr::new("tool"),
                    OsStr::new("shim-exec"),
                    home.as_os_str(),
                    OsStr::new(tool),
                    OsStr::new(command),
                ] {
                    body.push(b'\'');
                    for byte in argument.as_bytes() {
                        if *byte == b'\'' {
                            body.extend_from_slice(b"'\"'\"'");
                        } else {
                            body.push(*byte);
                        }
                    }
                    body.extend_from_slice(b"' ");
                }
                body.extend_from_slice(b"\"$@\"\n");
                Ok(body)
            })
            .map_err(anyhow::Error::new)
    }
    #[cfg(not(unix))]
    {
        let _ = (home, store);
        bail!("project dispatch shims are not supported on this platform")
    }
}

async fn open_env_context() -> Result<EnvContext> {
    tokio::task::spawn_blocking(open_env_context_sync)
        .await
        .context("load environment context task")?
}

#[derive(Clone, Copy)]
enum ToolArgumentMode {
    Install,
    Use,
}

async fn resolve_tool_arguments(
    context: &EnvContext,
    arguments: Vec<String>,
    mode: ToolArgumentMode,
) -> Result<Vec<ResolvedEnvTool>> {
    let root = context.project.clone();
    let home = context.user_home.clone();
    let declarations = context.declarations.clone();
    let store = context.store.clone();
    tokio::task::spawn_blocking(move || {
        let mut seen = BTreeSet::new();
        let mut resolved = Vec::new();
        for raw in arguments {
            if !seen.insert(raw.clone()) {
                continue;
            }
            let (name, request) = split_tool_pin(&raw)?;
            let implicit = !raw.contains('@');
            let declared = if implicit && matches!(mode, ToolArgumentMode::Install) {
                declarations.resolve_tool_versions(&name, home.as_deref())?
            } else {
                None
            };
            let selections = match declared {
                Some(selections) => selections,
                None => vec![resolve_env_tool(
                    &name,
                    &request,
                    Some(&root),
                    home.as_deref(),
                )?],
            };
            for mut selection in selections {
                if implicit
                    && matches!(mode, ToolArgumentMode::Use)
                    && matches!(selection.origin(), EnvToolOrigin::Managed)
                    && let Some(receipt) = store.latest_installed_for(selection.tool())?
                {
                    selection =
                        resolve_env_tool(&name, &receipt.version, Some(&root), home.as_deref())?;
                }
                resolved.push(selection);
            }
        }
        Ok::<_, anyhow::Error>(resolved)
    })
    .await
    .context("resolve tool arguments task")?
}

async fn install_env_tools(
    context: &EnvContext,
    offline: bool,
    allow_install: bool,
    overrides: BTreeMap<String, Vec<String>>,
) -> Result<Vec<InstallRecord>> {
    let declarations = context.declarations.clone();
    let home = context.user_home.clone();
    let root = context.project.clone();
    let tools = tokio::task::spawn_blocking(move || {
        declarations.resolve_tools_with_version_overrides(&overrides, &root, home.as_deref())
    })
    .await
    .context("resolve environment tools task")??;
    acquire_context_tools(&context.store, &tools, offline, allow_install).await
}

async fn acquire_context_tools(
    store: &ToolStore,
    tools: &[ResolvedEnvTool],
    offline: bool,
    allow_install: bool,
) -> Result<Vec<InstallRecord>> {
    let managed: Vec<_> = tools
        .iter()
        .filter(|tool| matches!(tool.origin(), EnvToolOrigin::Managed))
        .map(|tool| tool.tool().clone())
        .collect();
    let records = if allow_install {
        acquire_all(store, &managed, offline).await?
    } else {
        let store = store.clone();
        tokio::task::spawn_blocking(move || select_installed_tools(&store, &managed))
            .await
            .context("select installed tools task")??
    };
    context_install_records(tools, records)
}

fn context_install_records(
    tools: &[ResolvedEnvTool],
    managed: Vec<InstallRecord>,
) -> Result<Vec<InstallRecord>> {
    let mut managed = managed.into_iter();
    tools
        .iter()
        .map(|selection| {
            let tool = selection.tool();
            let (origin, bin_dirs) = match selection.origin() {
                EnvToolOrigin::Managed => {
                    return managed.next().ok_or_else(|| {
                        anyhow!("managed tool selection has no installation record")
                    });
                }
                EnvToolOrigin::System => (ExternalOrigin::System, Vec::new()),
                EnvToolOrigin::Path(paths) => (ExternalOrigin::Path, paths.bin_dirs().to_vec()),
            };
            Ok(InstallRecord {
                name: tool.name.clone(),
                version: tool
                    .version
                    .clone()
                    .ok_or_else(|| anyhow!("external tool selection has no request"))?,
                executable: tool.executable.clone(),
                origin: Some(origin),
                bin_dirs,
            })
        })
        .collect()
}

async fn install_resolved(
    store: &ToolStore,
    config: &EnvDeclarations,
    offline: bool,
) -> Result<Vec<InstallRecord>> {
    let resolved = resolve_declarations(config).map_err(anyhow::Error::new)?;
    acquire_all(store, &resolved, offline).await
}

async fn install_locked(
    store: &ToolStore,
    project: &Path,
    config: &EnvDeclarations,
    offline: bool,
) -> Result<Vec<InstallRecord>> {
    let store = store.clone();
    let project = project.to_path_buf();
    let config = config.clone();
    tokio::task::spawn_blocking(move || {
        let (_path, lock) = EnvLock::discover(&project)?
            .ok_or_else(|| anyhow!("project tool lock missing; run `aster tool lock`"))?;
        lock.resolve_declarations(&config)?;
        let http = if offline {
            None
        } else {
            Some(HttpSource::with_policy(FetchPolicy::default())?)
        };
        lock.install_with(
            &store,
            http.as_ref().map(|source| source as &dyn ToolSource),
        )?
        .into_iter()
        .map(|receipt| install_record(&store, receipt))
        .collect::<Result<Vec<_>>>()
    })
    .await
    .context("locked tool install task")?
}

async fn acquire_all(
    store: &ToolStore,
    resolved: &[aster_use_tool_acquire::ResolvedTool],
    offline: bool,
) -> Result<Vec<InstallRecord>> {
    if resolved.is_empty() {
        return Ok(Vec::new());
    }
    let store = store.clone();
    let resolved = resolved.to_vec();
    tokio::task::spawn_blocking(move || {
        let http = if offline {
            None
        } else {
            Some(HttpSource::with_policy(FetchPolicy::default())?)
        };
        let mut out = Vec::with_capacity(resolved.len());
        for tool in resolved {
            let installed = if tool.version.is_some() || offline {
                installed_tool(&store, &tool)?
            } else {
                None
            };
            let receipt = if let Some(receipt) = installed {
                receipt
            } else {
                let http = http
                    .as_ref()
                    .ok_or_else(|| ToolAcquireError::ToolNotInstalled {
                        name: tool.name.clone(),
                        version: tool.version.clone().unwrap_or_else(|| "latest".into()),
                    })?;
                store
                    .acquire_requested_with(
                        AcquireRequest {
                            name: tool.name.clone(),
                            spec: tool.spec.clone(),
                            version: tool.version.clone(),
                            executable_name: tool.executable.clone(),
                        },
                        http,
                    )
                    .map_err(anyhow::Error::new)?
            };
            out.push(install_record(&store, receipt)?);
        }
        Ok(out)
    })
    .await
    .context("tool install task")?
}

fn runtime_env(context: &EnvContext) -> Result<Vec<(OsString, OsString)>> {
    let records = installed_context_tools(context)?;
    runtime_env_from_installs(&context.store, context.declarations.config(), &records)
}

fn installed_context_tools(context: &EnvContext) -> Result<Vec<InstallRecord>> {
    let tools = context
        .declarations
        .resolve_tools(context.user_home.as_deref())?;
    let managed: Vec<_> = tools
        .iter()
        .filter(|tool| matches!(tool.origin(), EnvToolOrigin::Managed))
        .map(|tool| tool.tool().clone())
        .collect();
    context_install_records(&tools, select_installed_tools(&context.store, &managed)?)
}

fn select_installed_tools(
    store: &ToolStore,
    tools: &[aster_use_tool_acquire::ResolvedTool],
) -> Result<Vec<InstallRecord>> {
    let mut installed = Vec::new();
    for tool in tools {
        let receipt = installed_tool(store, tool)?.ok_or_else(|| {
            anyhow!(
                "tool not installed: {}@{}",
                tool.name,
                tool.version.as_deref().unwrap_or("latest")
            )
        })?;
        installed.push(install_record(store, receipt)?);
    }
    Ok(installed)
}

fn runtime_env_from_installs(
    store: &ToolStore,
    config: &EnvDeclarations,
    installed: &[InstallRecord],
) -> Result<Vec<(OsString, OsString)>> {
    let mut pairs = Vec::new();
    let mut paths: Vec<_> = installed
        .iter()
        .flat_map(|receipt| receipt.bin_dirs.iter().cloned())
        .collect();
    let inherited = env::var_os("PATH");
    paths.extend(store.unmanaged_path_dirs(inherited.as_deref()));
    if !paths.is_empty() || inherited.is_some() {
        pairs.push((
            OsString::from("PATH"),
            env::join_paths(paths).context("managed PATH")?,
        ));
    }
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
    store.installed_for(tool).map_err(anyhow::Error::new)
}

fn activation_script(shell: &str, context: &EnvContext) -> Result<String> {
    let shell = shell.to_ascii_lowercase();
    if !matches!(
        shell.as_str(),
        "sh" | "bash" | "zsh" | "fish" | "pwsh" | "powershell" | "powershell.exe" | "pwsh.exe"
    ) {
        bail!("unsupported activation shell: {shell}; expected sh, bash, zsh, fish, or PowerShell");
    }
    let mut out = String::new();
    for (name, value) in runtime_env(context)? {
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
    if let Ok(current) = env::current_exe()
        && let Some(dir) = current.parent()
    {
        for name in ["aster-use", "aster-use.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    which_on_path("aster-use")?
        .ok_or_else(|| anyhow!("aster-use not found next to aster or on PATH; set ASTER_USE_BIN"))
}

fn which_on_path(name: &str) -> Result<Option<PathBuf>> {
    find_env_executable(
        OsStr::new(name),
        env::var_os("PATH").as_deref(),
        &env::current_dir()?,
    )
    .map_err(anyhow::Error::new)
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

fn emit_install(json: bool, receipts: &[InstallRecord]) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(stdout, "{}", serde_json::to_string_pretty(receipts)?)?;
        return Ok(());
    }
    if receipts.is_empty() {
        writeln!(stdout, "Nothing to install.")?;
        return Ok(());
    }
    for receipt in receipts {
        match receipt.origin {
            None => writeln!(stdout, "installed {} {}", receipt.name, receipt.version)?,
            Some(ExternalOrigin::System) => writeln!(stdout, "Using system {}.", receipt.name)?,
            Some(ExternalOrigin::Path) => {
                writeln!(stdout, "Using {} from {}.", receipt.name, receipt.version)?
            }
        }
    }
    Ok(())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ExternalOrigin {
    System,
    Path,
}

#[derive(Debug, Clone, Serialize)]
struct InstallRecord {
    name: String,
    version: String,
    executable: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<ExternalOrigin>,
    #[serde(skip)]
    bin_dirs: Vec<PathBuf>,
}

fn install_record(
    store: &ToolStore,
    receipt: aster_use_tool_acquire::ToolReceipt,
) -> Result<InstallRecord> {
    let paths = store.installation_paths(&receipt)?;
    Ok(InstallRecord {
        name: receipt.name,
        version: receipt.version,
        executable: receipt.executable,
        origin: None,
        bin_dirs: paths.bin_dirs().to_vec(),
    })
}

#[cfg(test)]
mod tests;
