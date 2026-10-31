use aster_use_core::{DomainDiagnostic, Readiness, UseError, UseResult};

use crate::capability_registry::{
    snapshot as capability_registry_snapshot, wait_for_change as wait_for_capability_change,
};
use crate::extension_cli::{
    extension_capabilities, extension_inspect, extension_list, extension_operation_diagnostic,
    extension_planning_evidence, extension_snapshot, extension_watch, external_component_value,
    external_package_id, install_remote_extension, installed_extension_for_id,
    installed_extensions, uninstall_extension, upgrade_remote_extension,
};
use std::time::Duration;

mod component;
mod knowledge;
#[cfg(feature = "extensions")]
mod plugin;
#[cfg(not(feature = "extensions"))]
mod plugin {
    use aster_use_core::{UseError, UseResult};

    use super::CommandOutput;

    pub(super) async fn run(_args: &[String]) -> UseResult<CommandOutput> {
        Err(UseError::new(
            "use.extension.disabled",
            "Plugin Manager commands require the 'extensions' feature.",
        ))
    }
}
#[cfg(feature = "extensions")]
mod registry;
#[cfg(feature = "extensions")]
mod state;
#[cfg(not(feature = "extensions"))]
mod registry {
    use aster_use_core::{UseError, UseResult};

    use super::CommandOutput;

    pub(super) async fn run(_args: &[String]) -> UseResult<CommandOutput> {
        Err(UseError::new(
            "use.extension.disabled",
            "Registry source and cache operations require the 'extensions' feature.",
        ))
    }
}
#[cfg(not(feature = "extensions"))]
mod state {
    use aster_use_core::{UseError, UseResult};

    use super::CommandOutput;

    pub(super) async fn run(_args: &[String]) -> UseResult<CommandOutput> {
        Err(UseError::new(
            "use.state_backup_disabled",
            "Coordinated state backup requires the 'extensions' feature.",
        ))
    }
}

pub struct CommandOutput {
    pub human: String,
    pub json: serde_json::Value,
    pub exit_code: u8,
    pub should_print: bool,
}

impl CommandOutput {
    pub(crate) fn success(human: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            human: human.into(),
            json: serde_json::json!({
                "schemaVersion": 1,
                "ok": true,
                "data": data,
            }),
            exit_code: 0,
            should_print: true,
        }
    }
}

pub async fn run(args: Vec<String>) -> UseResult<CommandOutput> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(help());
    };
    match command {
        "-V" | "--version" | "version" => Ok(version()),
        "-h" | "--help" | "help" => Ok(help()),
        "capabilities" => capabilities().await,
        "capability" => capability(&args[1..]).await,
        "doctor" => doctor(args.get(1).map(String::as_str)).await,
        "install" => Box::pin(package_command_alias("install", &args[1..])).await,
        "upgrade" => Box::pin(package_command_alias("upgrade", &args[1..])).await,
        "uninstall" => Box::pin(package_command_alias("uninstall", &args[1..])).await,
        "component" => Box::pin(component::run(&args[1..])).await,
        "plugin" => Box::pin(plugin::run(&args[1..])).await,
        "knowledge" => knowledge::run(&args[1..]).await,
        "registry" => registry::run(&args[1..]).await,
        "state" => state::run(&args[1..]).await,
        "browser" | "ocr" | "box" => Err(removed_builtin(command)),
        "extension" => extension(&args[1..]).await,
        "mcp" => mcp(&args[1..]).await,
        route => Err(
            UseError::new("use.route_unknown", format!("Unknown Use route '{route}'."))
                .with_suggestion("Run 'aster use capabilities --json'."),
        ),
    }
}

fn version() -> CommandOutput {
    CommandOutput {
        human: format!("aster-use {}", env!("CARGO_PKG_VERSION")),
        json: serde_json::json!({
            "schemaVersion": 1,
            "ok": true,
            "version": env!("CARGO_PKG_VERSION"),
            "data": {
                "version": env!("CARGO_PKG_VERSION"),
            },
        }),
        exit_code: 0,
        should_print: true,
    }
}

fn help() -> CommandOutput {
    CommandOutput::success(
        concat!(
            "aster-use — AI Native Package Manager\n\n",
            "usage:\n",
            "  aster-use capabilities [--json]\n",
            "  aster-use capability snapshot [--json]\n",
            "  aster-use capability watch [--after-generation <n>] [--after-revision <sha256>] [--timeout-ms <ms>] [--json]\n",
            "  aster-use doctor [--json]\n",
            "  aster-use install <publisher/name> --scope-id <id> [--scope-kind <user|workspace>] [--registry-name <name>] [--offline] [--json]\n",
            "  aster-use upgrade <publisher/name> --scope-id <id> [--scope-kind <user|workspace>] [--registry-name <name>] [--offline] [--json]\n",
            "  aster-use uninstall <publisher/name> --scope-id <id> [--scope-kind <user|workspace>] [--json]\n",
            "  aster-use component list|status|install|upgrade|uninstall [args] [--json]\n",
            "  aster-use plugin search|inspect [args] [--json]\n",
            "  aster-use plugin list-installed|status --scope-id <id> [--scope-kind <user|workspace>] [--json]\n",
            "  aster-use plugin plan-install|plan-upgrade|plan-uninstall --scope-id <id> [--scope-kind <user|workspace>] [args] [--json]\n",
            "  aster-use plugin plan-enable|plan-disable <publisher/name> --scope-id <id> [--scope-kind <user|workspace>] [--json]\n",
            "  aster-use plugin apply-plan --operation-id <id> --plan-digest <sha256> --scope-id <id> [--scope-kind <user|workspace>] --yes [--json]\n",
            "  aster-use knowledge search <query> [--limit <n>] [--json]\n",
            "  aster-use knowledge usage [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge audit [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge backup <path> [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge verify-backup <path> [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge backup-retention <directory> [--max-backups <n>] [--max-bytes <n>] [--plan-digest <sha256> --yes] [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge plan-restore <path> [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge restore <path> --plan-digest <sha256> --yes [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge restore-status [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use knowledge repair-search-index --yes [--scope-kind <user|workspace>] [--scope-id <id>] [--json]\n",
            "  aster-use registry source list [--json]\n",
            "  aster-use registry source add <name> (--url <https-url> | --github <owner/repository>) --trust-root <sha256> [source options] [--json]\n",
            "  aster-use registry source replace <name> (--url <https-url> | --github <owner/repository>) --trust-root <sha256> --expected-revision <sha256> --yes [source options] [--json]\n",
            "  aster-use registry source default|enable|disable|remove <name> --expected-revision <sha256> --yes [--json]\n",
            "  aster-use registry cache usage [--registry-name <name>] [--json]\n",
            "  aster-use registry cache prune [--registry-name <name>] [cache options] --yes [--json]\n",
            "  aster-use state backup <path> [--json]\n",
            "  aster-use state verify-backup <path> [--json]\n",
            "  aster-use state backup-retention <directory> [--max-backups <n>] [--max-bytes <n>] [--plan-digest <sha256> --yes] [--json]\n",
            "  aster-use state plan-restore <backup> [--json]\n",
            "  aster-use state restore <backup> --rollback-backup <external-path> --plan-digest <sha256> --yes [--json]\n",
            "  aster-use state restore-status [--json]\n",
            "  aster-use extension list|inspect|doctor [args] [--json]\n",
            "  aster-use extension diagnose <publisher/name> [--history] --scope-id <id> [--scope-kind <user|workspace>] [--json]\n",
            "  aster-use extension planning-evidence <publisher/name> [--json]\n",
            "  aster-use extension snapshot|watch [--after-generation <n>] [--timeout-ms <ms>] [--json]\n",
        ),
        serde_json::json!({
            "commands": [
                "capabilities",
                "capability",
                "doctor",
                "install",
                "upgrade",
                "uninstall",
                "component",
                "plugin",
                "knowledge",
                "registry",
                "state",
                "extension"
            ]
        }),
    )
}

async fn package_command_alias(command: &str, args: &[String]) -> UseResult<CommandOutput> {
    let mut delegated = Vec::with_capacity(args.len() + 1);
    delegated.push(command.to_string());
    delegated.extend_from_slice(args);
    component::run(&delegated).await
}

async fn capabilities() -> UseResult<CommandOutput> {
    let (extension_generation, extensions) = extension_capabilities().await?;
    Ok(CommandOutput::success(
        "Package surfaces: skill, okf, ui, mcp, tool",
        serde_json::json!({
            "domains": [],
            "externalSurfaces": ["tool", "mcp", "okf", "skill", "ui"],
            "extensionRegistry": {
                "schemaVersion": 1,
                "generation": extension_generation,
                "hotPlug": true
            },
            "extensions": extensions
        }),
    ))
}

async fn capability(args: &[String]) -> UseResult<CommandOutput> {
    match args.first().map(String::as_str) {
        Some("snapshot") => {
            validate_capability_options(args, false)?;
            let snapshot = capability_registry_snapshot().await?;
            Ok(CommandOutput::success(
                format!(
                    "Capability registry generation {} ({}).",
                    snapshot.generation, snapshot.revision
                ),
                serde_json::json!({ "registry": snapshot }),
            ))
        }
        Some("watch") => {
            validate_capability_options(args, true)?;
            let after_generation = integer_option(args, "--after-generation", 0)?;
            let after_revision = option_argument(args, "--after-revision")?;
            let timeout = duration_option(args, "--timeout-ms", 30_000)?;
            match wait_for_capability_change(after_generation, after_revision, timeout).await? {
                Some(snapshot) => Ok(CommandOutput::success(
                    "The capability registry changed.",
                    serde_json::json!({ "changed": true, "registry": snapshot }),
                )),
                None => Ok(CommandOutput::success(
                    "The capability registry did not change.",
                    serde_json::json!({
                        "changed": false,
                        "afterGeneration": after_generation,
                        "afterRevision": after_revision,
                        "timeoutMs": timeout.as_millis().min(u64::MAX as u128) as u64
                    }),
                )),
            }
        }
        Some(value) => Err(usage_error(format!("unknown capability command '{value}'"))),
        None => Err(usage_error("capability requires snapshot or watch")),
    }
}

async fn doctor(domain: Option<&str>) -> UseResult<CommandOutput> {
    let diagnostics = match domain {
        None | Some("--json") => installed_extensions()
            .await?
            .iter()
            .map(extension_diagnostic)
            .collect(),
        Some(value) if is_removed_builtin(value) => return Err(removed_builtin(value)),
        Some(value) => match installed_extension_for_id(value).await? {
            Some(extension) => vec![extension_diagnostic(&extension)],
            None => {
                return Err(UseError::new(
                    "use.domain_unknown",
                    format!("Unknown domain '{value}'."),
                )
                .with_suggestion(
                    "Install the external capability or run 'aster use capabilities --json'.",
                ))
            }
        },
    };
    let ready = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.readiness == Readiness::Ready)
        .count();
    Ok(CommandOutput::success(
        format!("{ready}/{} domains ready", diagnostics.len()),
        serde_json::json!({ "diagnostics": diagnostics }),
    ))
}

async fn extension(args: &[String]) -> UseResult<CommandOutput> {
    match args.first().map(String::as_str) {
        None | Some("list") => extension_list().await,
        Some("inspect" | "doctor") => {
            let package_id = value_argument(args, 1, "extension inspect requires an ID")?;
            extension_inspect(package_id).await
        }
        Some("diagnose") => {
            validate_extension_diagnostic_options(args)?;
            let package_id = value_argument(args, 1, "extension diagnose requires an ID")?;
            extension_operation_diagnostic(
                package_id,
                managed_scope_argument(args)?,
                flag_argument(args, "--history")?,
            )
            .await
        }
        Some("planning-evidence") => {
            validate_extension_options(args, 2, false)?;
            let package_id = value_argument(args, 1, "extension planning-evidence requires an ID")?;
            extension_planning_evidence(package_id).await
        }
        Some("snapshot") => {
            validate_extension_options(args, 1, false)?;
            extension_snapshot().await
        }
        Some("watch") => {
            validate_extension_watch_options(args)?;
            let after_generation = integer_option(args, "--after-generation", 0)?;
            let timeout = duration_option(args, "--timeout-ms", 30_000)?;
            extension_watch(after_generation, timeout).await
        }
        Some(command) => Err(UseError::new(
            "use.extension.command_unknown",
            format!("Unknown extension command '{command}'."),
        )),
    }
}

async fn mcp(args: &[String]) -> UseResult<CommandOutput> {
    match args.first().map(String::as_str) {
        Some("serve") => {
            let target = value_argument(args, 1, "mcp serve requires a domain or package ID")?;
            if is_removed_builtin(target) || matches!(target, "ocr-native" | "use/ocr-native") {
                Err(removed_builtin(target))
            } else {
                Err(UseError::new(
                    "use.mcp.target_unknown",
                    format!("Unknown MCP target '{target}'."),
                ))
            }
        }
        Some("start" | "status" | "stop") => Err(removed_builtin("browser")),
        _ => Err(usage_error("mcp requires start, status, stop, or serve")),
    }
}

fn component_value(id: &str, diagnostic: &DomainDiagnostic) -> serde_json::Value {
    let (presence, health) = match diagnostic.readiness {
        Readiness::Ready => (builtin_presence(id), "ready"),
        Readiness::Missing => ("missing", "unknown"),
        Readiness::Broken => ("external", "broken"),
        Readiness::Unknown => ("missing", "unknown"),
    };
    serde_json::json!({
        "id": id,
        "description": diagnostic.message,
        "presence": presence,
        "health": health,
        "version": diagnostic.version,
        "path": diagnostic.path
    })
}

fn extension_diagnostic(extension: &crate::extension_cli::ExtensionView) -> DomainDiagnostic {
    let (readiness, message, suggestions) = if !extension.compatible {
        (
            Readiness::Broken,
            format!(
                "Extension '{}' {} is incompatible with Aster Use {}.",
                extension.package_id,
                extension.version,
                env!("CARGO_PKG_VERSION")
            ),
            vec!["Install a compatible extension version or update Aster Use.".to_string()],
        )
    } else if extension.enabled {
        (
            Readiness::Ready,
            format!(
                "Extension '{}' is ready on route '{}'.",
                extension.package_id, extension.route
            ),
            Vec::new(),
        )
    } else {
        (
            Readiness::Unknown,
            format!(
                "Extension '{}' is installed but disabled.",
                extension.package_id
            ),
            vec![
                "Create and apply a reviewed enablement plan through the package manager."
                    .to_string(),
            ],
        )
    };
    DomainDiagnostic {
        domain: extension.route.clone(),
        readiness,
        provider: Some(extension.package_id.clone()),
        version: Some(extension.version.clone()),
        path: Some(extension.package_root.clone()),
        message,
        suggestions,
    }
}

fn builtin_presence(_id: &str) -> &'static str {
    "external"
}

fn removed_builtin(id: &str) -> UseError {
    UseError::new(
        "use.builtin_removed",
        format!("Built-in route '{id}' has been removed from aster-use."),
    )
}

fn is_removed_builtin(id: &str) -> bool {
    matches!(
        id,
        "browser" | "use/browser" | "ocr" | "use/ocr" | "box" | "use/box"
    )
}

fn builtin_diagnostic(id: &str) -> Option<DomainDiagnostic> {
    if is_removed_builtin(id) {
        return None;
    }
    None
}

fn option_argument<'a>(args: &'a [String], name: &str) -> UseResult<Option<&'a str>> {
    let mut value = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == name {
            if value.is_some() {
                return Err(usage_error(format!("{name} may be provided only once")));
            }
            value = Some(
                args.get(index + 1)
                    .map(String::as_str)
                    .filter(|candidate| !candidate.starts_with('-'))
                    .ok_or_else(|| usage_error(format!("{name} requires a value")))?,
            );
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(value)
}

fn flag_argument(args: &[String], name: &str) -> UseResult<bool> {
    let count = args
        .iter()
        .filter(|argument| argument.as_str() == name)
        .count();
    if count > 1 {
        Err(usage_error(format!("{name} may be provided only once")))
    } else {
        Ok(count == 1)
    }
}

fn validate_component_install_options(args: &[String]) -> UseResult<()> {
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--json" | "--force" | "--offline" => index += 1,
            "--registry-name"
            | "--version"
            | "--channel"
            | "--package-lock-digest"
            | "--scope-kind"
            | "--scope-id" => {
                if args.get(index + 1).is_none() {
                    return Err(usage_error(format!("{} requires a value", args[index])));
                }
                index += 2;
            }
            value => {
                return Err(usage_error(format!(
                    "unknown component install option '{value}'"
                )))
            }
        }
    }
    Ok(())
}

fn validate_component_upgrade_options(args: &[String]) -> UseResult<()> {
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--json" | "--offline" => index += 1,
            "--registry-name"
            | "--version"
            | "--channel"
            | "--package-lock-digest"
            | "--scope-kind"
            | "--scope-id" => {
                if args.get(index + 1).is_none() {
                    return Err(usage_error(format!("{} requires a value", args[index])));
                }
                index += 2;
            }
            value => {
                return Err(usage_error(format!(
                    "unknown component upgrade option '{value}'"
                )))
            }
        }
    }
    Ok(())
}

fn validate_extension_options(
    args: &[String],
    first_option: usize,
    allow_timeout: bool,
) -> UseResult<()> {
    let mut index = first_option;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => index += 1,
            "--timeout-ms" if allow_timeout => {
                if args.get(index + 1).is_none() {
                    return Err(usage_error("--timeout-ms requires a value"));
                }
                index += 2;
            }
            value => return Err(usage_error(format!("unknown extension option '{value}'"))),
        }
    }
    Ok(())
}

fn validate_extension_watch_options(args: &[String]) -> UseResult<()> {
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => index += 1,
            "--after-generation" | "--timeout-ms" => {
                if args.get(index + 1).is_none() {
                    return Err(usage_error(format!("{} requires a value", args[index])));
                }
                index += 2;
            }
            value => {
                return Err(usage_error(format!(
                    "unknown extension watch option '{value}'"
                )))
            }
        }
    }
    Ok(())
}

fn validate_extension_diagnostic_options(args: &[String]) -> UseResult<()> {
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--json" | "--history" => index += 1,
            "--scope-kind" | "--scope-id" => {
                if args
                    .get(index + 1)
                    .is_none_or(|value| value.starts_with('-'))
                {
                    return Err(usage_error(format!("{} requires a value", args[index])));
                }
                index += 2;
            }
            value => {
                return Err(usage_error(format!(
                    "unknown extension diagnose option '{value}'"
                )))
            }
        }
    }
    Ok(())
}

fn managed_scope_argument(args: &[String]) -> UseResult<aster_use_core::PlanScope> {
    let kind = match option_argument(args, "--scope-kind")?.unwrap_or("user") {
        "user" => aster_use_core::PlanScopeKind::User,
        "workspace" => aster_use_core::PlanScopeKind::Workspace,
        value => {
            return Err(usage_error(format!(
                "--scope-kind must be 'user' or 'workspace', received '{value}'"
            )))
        }
    };
    let scope_id = option_argument(args, "--scope-id")?
        .ok_or_else(|| usage_error("--scope-id is required"))?;
    Ok(aster_use_core::PlanScope {
        kind,
        id: scope_id.to_owned(),
    })
}

fn validate_capability_options(args: &[String], watch: bool) -> UseResult<()> {
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => index += 1,
            "--after-generation" | "--after-revision" | "--timeout-ms" if watch => {
                if args.get(index + 1).is_none() {
                    return Err(usage_error(format!("{} requires a value", args[index])));
                }
                index += 2;
            }
            value => return Err(usage_error(format!("unknown capability option '{value}'"))),
        }
    }
    Ok(())
}

fn integer_option(args: &[String], name: &str, default: u64) -> UseResult<u64> {
    let Some(value) = option_argument(args, name)? else {
        return Ok(default);
    };
    value.parse::<u64>().map_err(|_| {
        usage_error(format!(
            "{name} must be a non-negative integer, received '{value}'"
        ))
    })
}

fn duration_option(args: &[String], name: &str, default_ms: u64) -> UseResult<Duration> {
    Ok(Duration::from_millis(integer_option(
        args, name, default_ms,
    )?))
}

fn value_argument<'a>(args: &'a [String], index: usize, message: &str) -> UseResult<&'a str> {
    args.get(index)
        .map(String::as_str)
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| usage_error(message))
}

fn usage_error(message: impl Into<String>) -> UseError {
    UseError::new("use.cli.invalid_usage", message)
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
