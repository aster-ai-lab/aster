//! Range-preserving upgrades through the existing native acquisition store.

use super::*;

#[derive(Debug, Default, Clone, clap::Args)]
pub struct UpgradeArgs {
    /// Configured tool names, optionally overriding the version request.
    tool: Vec<String>,
    /// Upgrade beyond declared ranges and update the owning declarations.
    #[arg(long, short = 'b')]
    bump: bool,
    /// With --bump, write exact resolved versions.
    #[arg(long, requires = "bump")]
    pin: bool,
    #[arg(long, short = 'x')]
    exclude: Vec<String>,
    /// Remove upgraded versions now when no tracked project or process needs them.
    #[arg(long, conflicts_with = "no_prune")]
    prune: bool,
    /// Retain previous installations.
    #[arg(long)]
    no_prune: bool,
    #[arg(long, short = 'n')]
    dry_run: bool,
    /// Preview and exit 1 when a version or declaration would change.
    #[arg(long)]
    dry_run_code: bool,
    /// Resolve cached metadata and use complete installed versions only.
    #[arg(long)]
    offline: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Serialize)]
struct Upgrade {
    name: String,
    previous: Option<String>,
    version: String,
    request: String,
    changed: bool,
    #[serde(skip)]
    tool: aster_use_tool_acquire::ResolvedTool,
}

pub(super) async fn run(args: UpgradeArgs) -> Result<()> {
    let context = open_env_context().await?;
    let preview = args.dry_run || args.dry_run_code;
    let json = args.json;
    let dry_run_code = args.dry_run_code;
    let (context, upgrades) = tokio::task::spawn_blocking(move || {
        let source = if args.offline {
            None
        } else {
            Some(HttpSource::with_policy(FetchPolicy::default())?)
        };
        let upgrades = upgrade(
            &context,
            &args,
            source.as_ref().map(|s| s as &dyn ToolSource),
        )?;
        Ok::<_, anyhow::Error>((context, upgrades))
    })
    .await
    .context("upgrade tools task")??;
    if !preview {
        refresh_dispatch_shims(&context).await?;
    }
    let mut stdout = std::io::stdout().lock();
    if json {
        writeln!(stdout, "{}", serde_json::to_string_pretty(&upgrades)?)?;
    } else {
        for item in upgrades.iter().filter(|item| item.changed) {
            writeln!(
                stdout,
                "{}\t{} → {}{}",
                item.name,
                item.previous.as_deref().unwrap_or("missing"),
                item.version,
                if preview { " (preview)" } else { "" }
            )?;
        }
        if !upgrades.iter().any(|item| item.changed) {
            writeln!(stdout, "No upgrades available within the selected ranges.")?;
        }
    }
    if dry_run_code && upgrades.iter().any(|item| item.changed) {
        std::process::exit(1);
    }
    Ok(())
}

fn upgrade(
    context: &EnvContext,
    args: &UpgradeArgs,
    source: Option<&dyn ToolSource>,
) -> Result<Vec<Upgrade>> {
    let mut selected = BTreeMap::<String, Vec<(String, bool)>>::new();
    for raw in &args.tool {
        let (name, request) = split_tool_pin(raw)?;
        let requests = if raw.contains('@') {
            vec![(request, true)]
        } else {
            context
                .declarations
                .config()
                .tools
                .get(&name)
                .with_context(|| format!("tool has no active declaration: {name}"))?
                .iter()
                .map(|request| (request.clone(), false))
                .collect()
        };
        let values = selected.entry(name).or_default();
        for request in requests {
            if !values.contains(&request) {
                values.push(request);
            }
        }
    }
    if args.tool.is_empty() {
        selected.extend(
            context
                .declarations
                .config()
                .tools
                .iter()
                .map(|(name, requests)| {
                    (
                        name.clone(),
                        requests.iter().map(|r| (r.clone(), false)).collect(),
                    )
                }),
        );
    }
    for name in &args.exclude {
        // Exclusions use the same closed catalog boundary as tool arguments.
        let (name, _) = split_tool_pin(name)?;
        selected.remove(&name);
    }
    let mut upgrades = Vec::new();
    let mut writes = BTreeMap::<PathBuf, EnvDeclarations>::new();
    for (name, requests) in selected {
        let mut pins = Vec::new();
        for (raw, explicit) in requests {
            let original = VersionRequest::parse(&raw)?;
            if matches!(
                original.kind(),
                VersionRequestKind::Path(_) | VersionRequestKind::System
            ) {
                pins.push(raw);
                continue;
            }
            let selection = resolve_env_tool(
                &name,
                &raw,
                Some(&context.project),
                context.user_home.as_deref(),
            )?;
            let request = if args.bump && !explicit {
                VersionRequest::parse("latest")?
            } else {
                original.clone()
            };
            let tool = selection.tool().clone();
            let listing = context
                .store
                .remote_versions_with(&parse_tool_spec(&tool.spec)?, source)?;
            let version = listing.resolve(&request)?;
            let previous = context.store.installed_for(&tool)?.map(|r| r.version);
            let pin = if !args.bump {
                raw.clone()
            } else if args.pin {
                version.clone()
            } else if explicit {
                request.canonical()
            } else {
                bumped_request(&original, &version)?
            };
            let changed = previous.as_deref() != Some(&version) || (args.bump && pin != raw);
            upgrades.push(Upgrade {
                name: name.clone(),
                previous,
                version,
                request: pin.clone(),
                changed,
                tool,
            });
            if !pins.contains(&pin) {
                pins.push(pin);
            }
        }
        if args.bump && !pins.is_empty() {
            let path = context
                .declarations
                .declaration_path(&name, &context.aster_home)
                .unwrap_or_else(|| project_env_path(&context.project));
            let file = match writes.entry(path) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let file = read_env_declarations(entry.key())?.unwrap_or_default();
                    entry.insert(file)
                }
            };
            file.tools.insert(name, pins);
        }
    }
    if args.dry_run || args.dry_run_code {
        return Ok(upgrades);
    }
    // Resolve and preflight every offline installation before publishing any selection.
    if source.is_none() {
        let mut declarations = EnvDeclarations::default();
        for item in &upgrades {
            declarations
                .tools
                .entry(item.name.clone())
                .or_default()
                .push(item.version.clone());
        }
        EnvLock::capture_declarations(&declarations, &context.store)?;
    }
    for item in &upgrades {
        if installed_target(&context.store, item)?.is_some() {
            context.store.activate_version(&item.name, &item.version)?;
        } else {
            let source = source.context("tool source unavailable")?;
            context.store.acquire_with(
                AcquireRequest {
                    name: item.name.clone(),
                    spec: item.tool.spec.clone(),
                    version: Some(item.version.clone()),
                    executable_name: item.tool.executable.clone(),
                },
                source,
            )?;
        }
    }
    for (path, file) in writes {
        if read_env_declarations(&path)?.as_ref() != Some(&file) {
            write_env_declarations(path, &file)?;
        }
    }
    load_env_declarations_with_sources(&context.project, &context.aster_home)?
        .track_sources(&context.store)?;
    if args.prune && !upgrades.is_empty() {
        let names = upgrades
            .iter()
            .map(|item| item.name.clone())
            .collect::<Vec<_>>();
        let unused = context.store.plan_prune(&context.aster_home, &names)?;
        for receipt in unused.tools() {
            if upgrades.iter().any(|item| {
                item.name == receipt.name
                    && item.previous.as_deref() == Some(&receipt.version)
                    && item.version != receipt.version
            }) {
                context
                    .store
                    .remove_version_sync(&receipt.name, &receipt.version)?;
            }
        }
    }
    // shortcut: default old-version grace-period cleanup needs the lifecycle scheduler in T004.
    Ok(upgrades)
}

fn installed_target(store: &ToolStore, item: &Upgrade) -> Result<Option<ToolReceipt>> {
    let mut tool = item.tool.clone();
    tool.version = Some(item.version.clone());
    Ok(store.installed_for(&tool)?)
}

fn bumped_request(request: &VersionRequest, version: &str) -> Result<String> {
    match request.kind() {
        VersionRequestKind::Latest => Ok("latest".into()),
        VersionRequestKind::Version(value)
            if value
                .split('.')
                .all(|v| !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit())) =>
        {
            let precision = value.split('.').count();
            let components = version
                .trim_start_matches('v')
                .split('.')
                .take(precision)
                .collect::<Vec<_>>();
            if components.len() != precision {
                bail!("cannot preserve version precision for {value} → {version}");
            }
            Ok(components.join("."))
        }
        _ => bail!(
            "--bump needs an explicit tool@request for {}",
            request.canonical()
        ),
    }
}

#[cfg(test)]
mod tests;
