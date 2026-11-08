use super::*;

pub(super) async fn run(args: &[String]) -> UseResult<CommandOutput> {
    let command = args.first().map(String::as_str).ok_or_else(|| {
        usage_error("component requires list, status, install, upgrade, or uninstall")
    })?;
    match command {
        "list" => list().await,
        "status" => {
            let id = value_argument(args, 1, "component status requires an ID")?;
            status(id).await
        }
        "install" => install(args).await,
        "upgrade" => upgrade(args).await,
        "uninstall" => uninstall(args).await,
        value => Err(usage_error(format!("unknown component command '{value}'"))),
    }
}

async fn upgrade(args: &[String]) -> UseResult<CommandOutput> {
    let id = value_argument(args, 1, "component upgrade requires an ID")?;
    if super::is_removed_builtin(id) {
        return Err(super::removed_builtin(id));
    }
    validate_component_upgrade_options(args)?;
    let offline = flag_argument(args, "--offline")?;
    if builtin_diagnostic(id).is_some() {
        return Err(UseError::new(
            "use.plugin.package_upgrade_unsupported",
            format!("Built-in component '{id}' is not a package graph."),
        ));
    }
    let scope = managed_scope_argument(args)?;
    let resolved = installed_extension_for_id(id).await?;
    let package_id = external_package_id(id).or_else(|| {
        resolved
            .as_ref()
            .map(|extension| extension.package_id.as_str())
    });
    let package_id = package_id.ok_or_else(|| {
        UseError::new("use.component_unknown", format!("Unknown package '{id}'."))
    })?;
    let registry_name = option_argument(args, "--registry-name")?;
    let version = option_argument(args, "--version")?;
    let channel = option_argument(args, "--channel")?;
    let expected_lock = option_argument(args, "--package-lock-digest")?;
    let result = upgrade_remote_extension(
        package_id,
        registry_name,
        version,
        channel,
        expected_lock,
        offline,
        &scope,
    )
    .await?;
    Ok(CommandOutput::success(
        if result.changed {
            format!("Upgraded package '{}'.", result.extension.package_id)
        } else {
            format!(
                "package '{}' already matches the resolved graph.",
                result.extension.package_id
            )
        },
        serde_json::json!({
            "component": external_component_value(&result.extension, id.starts_with("use/")),
            "changed": result.changed,
            "registryAccess": result.registry_access,
            "registrySourceRevision": result.registry_source_revision,
            "packageGraph": result.package_graph,
            "pluginManager": result.plugin_manager
        }),
    ))
}

async fn list() -> UseResult<CommandOutput> {
    let extensions = installed_extensions().await?;
    let components = extensions
        .iter()
        .map(|extension| external_component_value(extension, false))
        .collect::<Vec<_>>();
    let human = extensions
        .iter()
        .map(|extension| format!("use/{}", extension.package_id))
        .collect::<Vec<_>>();
    Ok(CommandOutput::success(
        human.join("\n"),
        serde_json::json!({ "components": components }),
    ))
}

async fn status(id: &str) -> UseResult<CommandOutput> {
    if super::is_removed_builtin(id) {
        return Err(super::removed_builtin(id));
    }
    if let Some(diagnostic) = builtin_diagnostic(id) {
        return Ok(CommandOutput {
            human: diagnostic.message.clone(),
            json: serde_json::json!({
                "schemaVersion": 1,
                "ok": true,
                "component": component_value(id, &diagnostic),
            }),
            exit_code: 0,
            should_print: true,
        });
    }
    if let Some(extension) = installed_extension_for_id(id).await? {
        return Ok(CommandOutput {
            human: format!(
                "Extension '{}' is {} on route '{}'.",
                extension.package_id,
                if !extension.compatible {
                    "incompatible"
                } else if extension.enabled {
                    "enabled"
                } else {
                    "disabled"
                },
                extension.route
            ),
            json: serde_json::json!({
                "schemaVersion": 1,
                "ok": true,
                "component": external_component_value(&extension, id.starts_with("use/")),
            }),
            exit_code: 0,
            should_print: true,
        });
    }
    Err(UseError::new(
        "use.component_unknown",
        format!("Unknown delegated component '{id}'."),
    ))
}

async fn install(args: &[String]) -> UseResult<CommandOutput> {
    let id = value_argument(args, 1, "component install requires an ID")?;
    if super::is_removed_builtin(id) {
        return Err(super::removed_builtin(id));
    }
    validate_component_install_options(args)?;
    let offline = flag_argument(args, "--offline")?;
    if offline && builtin_diagnostic(id).is_some() {
        return Err(usage_error(
            "--offline is available only for Registry-backed packages",
        ));
    }
    if let Some(diagnostic) = builtin_diagnostic(id) {
        if diagnostic.readiness != Readiness::Ready {
            return Err(UseError::new(
                "use.runtime.install_unavailable",
                format!(
                    "Managed installation for '{}' is not available in this initial release.",
                    id
                ),
            )
            .with_suggestion(
                diagnostic
                    .suggestions
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Install a compatible system provider.".to_string()),
            ));
        }
        return Ok(CommandOutput::success(
            format!("Component '{id}' is already ready."),
            serde_json::json!({
                "component": component_value(id, &diagnostic),
                "changed": false
            }),
        ));
    }

    let resolved = installed_extension_for_id(id).await?;
    let package_id = external_package_id(id).or_else(|| {
        resolved
            .as_ref()
            .map(|extension| extension.package_id.as_str())
    });
    let Some(package_id) = package_id else {
        return Err(UseError::new(
            "use.component_unknown",
            format!("Unknown delegated component '{id}'."),
        )
        .with_suggestion(
            "Install external capabilities by their '<publisher>/<name>' package ID.",
        ));
    };
    if args.iter().any(|argument| argument == "--force") {
        return Err(usage_error(
            "--force is not valid for packages; apply a newly resolved package-lock plan",
        ));
    }
    let scope = managed_scope_argument(args)?;
    let registry_name = option_argument(args, "--registry-name")?;
    let version = option_argument(args, "--version")?;
    let channel = option_argument(args, "--channel")?;
    let expected_package_lock = option_argument(args, "--package-lock-digest")?;
    let result = install_remote_extension(
        package_id,
        registry_name,
        version,
        channel,
        expected_package_lock,
        offline,
        &scope,
    )
    .await?;
    Ok(CommandOutput::success(
        if result.changed {
            format!("Installed extension '{}'.", result.extension.package_id)
        } else {
            format!(
                "Extension '{}' is already installed.",
                result.extension.package_id
            )
        },
        serde_json::json!({
            "component": external_component_value(&result.extension, id.starts_with("use/")),
            "changed": result.changed,
            "registryAccess": result.registry_access,
            "registrySourceRevision": result.registry_source_revision,
            "packageGraph": result.package_graph,
            "pluginManager": result.plugin_manager
        }),
    ))
}

async fn uninstall(args: &[String]) -> UseResult<CommandOutput> {
    let id = value_argument(args, 1, "component uninstall requires an ID")?;
    if super::is_removed_builtin(id) {
        return Err(super::removed_builtin(id));
    }
    let scope = managed_scope_argument(args)?;
    if let Some(extension) = installed_extension_for_id(id).await? {
        let result = uninstall_extension(&extension.package_id, &scope).await?;
        return Ok(CommandOutput::success(
            if result.changed {
                format!("Uninstalled extension '{}'.", result.package_id)
            } else {
                format!("Extension '{}' is not installed.", result.package_id)
            },
            serde_json::json!({
                "component": format!("use/{}", result.package_id),
                "route": extension.route,
                "changed": result.changed,
                "packageGraph": result.package_graph,
                "pluginManager": result.plugin_manager
            }),
        ));
    }
    if let Some(package_id) = external_package_id(id) {
        let result = uninstall_extension(package_id, &scope).await?;
        return Ok(CommandOutput::success(
            if result.changed {
                format!("Uninstalled extension '{}'.", result.package_id)
            } else {
                format!("Extension '{}' is not installed.", result.package_id)
            },
            serde_json::json!({
                "component": format!("use/{}", result.package_id),
                "changed": result.changed,
                "packageGraph": result.package_graph,
                "pluginManager": result.plugin_manager
            }),
        ));
    }
    Err(UseError::new(
        "use.component_unknown",
        format!("Unknown delegated component '{id}'."),
    ))
}
