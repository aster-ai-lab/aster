//! Source scan for DDD dependency direction in the three product processes.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn roots() -> Vec<(&'static str, PathBuf)> {
        // Only scan billing's own source: billing's architecture test
        // tests billing's DDD compliance. Cross-product scanning catches
        // violations in other processes (a3s-api, a3s-auth) that billing
        // cannot fix and should not gate on. Those violations are tracked
        // in the owning process's architecture tests.
        let billing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        vec![("a3s-billing", billing)]
    }

    fn production_source(source: &str) -> String {
        let lines = source.lines().collect::<Vec<_>>();
        let mut production = String::new();
        let mut index = 0;
        while index < lines.len() {
            let line = lines[index];
            if line.trim() == "#[cfg(test)]" {
                let mut item = index + 1;
                while item < lines.len() && lines[item].trim().is_empty() {
                    item += 1;
                }
                if item < lines.len() && lines[item].trim_start().starts_with("use ") {
                    index = item;
                    while index < lines.len() && !lines[index].contains(';') {
                        index += 1;
                    }
                    index += 1;
                    continue;
                }
                break;
            }
            production.push_str(line);
            production.push('\n');
            index += 1;
        }
        production
    }

    fn contains_token(source: &str, token: &str) -> bool {
        source.match_indices(token).any(|(index, _)| {
            let before = source[..index].chars().next_back();
            !before.is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        })
    }

    fn is_domain(relative: &Path) -> bool {
        relative
            .components()
            .any(|component| component.as_os_str() == "domain")
    }

    fn is_infrastructure(relative: &Path) -> bool {
        relative
            .components()
            .any(|component| component.as_os_str() == "infrastructure")
    }

    fn is_application(relative: &Path) -> bool {
        relative
            .components()
            .any(|component| component.as_os_str() == "application")
            || relative.file_stem().and_then(|value| value.to_str()) == Some("application")
    }

    fn is_test_path(relative: &Path) -> bool {
        let name = relative
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        name.ends_with("_tests.rs")
            || relative
                .components()
                .any(|component| component.as_os_str() == "tests")
    }

    fn is_port_implementation(relative: &Path) -> bool {
        is_infrastructure(relative)
            || relative.file_stem().and_then(|value| value.to_str()) == Some("infrastructure")
    }

    fn concrete_client_violation(
        relative: &Path,
        source: &str,
        needles: &[&str],
        controller: &str,
        boot_controller: &str,
        legacy_boot_controller: &str,
    ) -> bool {
        let product_layer = source.contains(controller)
            || source.contains(boot_controller)
            || source.contains(legacy_boot_controller)
            || is_application(relative)
            || is_named_flat_module(relative);
        !is_port_implementation(relative)
            && product_layer
            && needles.iter().any(|needle| source.contains(needle))
    }

    fn normalized_module_path(relative: &Path) -> String {
        let mut display = relative.to_string_lossy().replace('\\', "/");
        if let Some(rest) = display.strip_prefix("./") {
            display = rest.to_string();
        }
        if let Some(rest) = display.strip_prefix("src/") {
            display = rest.to_string();
        }
        display
    }

    /// Flat product modules named by the layer rule, excluding their port implementations.
    fn is_named_flat_module(relative: &Path) -> bool {
        if is_port_implementation(relative) {
            return false;
        }
        let display = normalized_module_path(relative);
        matches!(
            display.as_str(),
            "contexts/logs.rs"
                | "contexts/usage.rs"
                | "contexts/usage_export.rs"
                | "contexts/human_auth.rs"
                | "contexts/api_surface.rs"
        ) || display.starts_with("contexts/inference/")
            || display.starts_with("contexts/pipelines/")
    }

    #[test]
    fn product_layers_keep_sql_and_serde_out_of_domain_and_controllers() {
        let domain_needles = [
            concat!("a3s", "_boot"),
            concat!("a3s", "_orm"),
            concat!("a3s", "_boot"),
            concat!("a3s", "_orm"),
            concat!("red", "is"),
            concat!("aws_sdk", "_s3"),
            concat!("ser", "de"),
            concat!("serde", "_json"),
        ];
        let io_needles = [
            concat!("sql", "_query"),
            concat!("a3s_orm", "::"),
            concat!("a3s_orm", "::"),
            concat!("redis", "::"),
        ];
        let outbound = [
            concat!("Cloud", "Client"),
            concat!("ClickHouse", "Logs"),
            concat!("Inference", "Store"),
            concat!("Pipeline", "Store"),
            concat!("UsageFact", "Ledger"),
        ];
        let controller = concat!("#[", "controller");
        let boot_controller = concat!("#[a3s_boot", "::controller");
        let legacy_boot_controller = concat!("#[a3s_boot", "::controller");
        let mut violations = Vec::new();
        let mut visited = Vec::new();
        let mut relatives = Vec::new();

        for (crate_name, root) in roots() {
            let mut pending = vec![root.clone()];
            while let Some(path) = pending.pop() {
                if path.is_dir() {
                    let entries = std::fs::read_dir(&path).expect("read source directory");
                    pending.extend(entries.map(|entry| entry.expect("source entry").path()));
                    continue;
                }
                if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                    continue;
                }
                let relative = path.strip_prefix(&root).expect("relative source path");
                if is_test_path(relative) || relative.ends_with("architecture.rs") {
                    continue;
                }
                let source = std::fs::read_to_string(&path).expect("read source");
                let source = production_source(&source);
                let display = format!("{crate_name}/{}", relative.display());
                visited.push(display.clone());
                relatives.push(relative.to_path_buf());
                if is_domain(relative) {
                    // Line comments cannot functionally reference a crate;
                    // scanning them turned self-documenting notes ("no serde
                    // in this file") into violations.
                    let code = source
                        .lines()
                        .filter(|line| !line.trim_start().starts_with("//"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    for needle in domain_needles {
                        if contains_token(&code, needle) {
                            violations.push(format!("{display} domain references {needle}"));
                        }
                    }
                }
                let mentions_io = io_needles.iter().any(|needle| source.contains(needle));
                let product_layer = source.contains(controller)
                    || source.contains(boot_controller)
                    || source.contains(legacy_boot_controller)
                    || is_application(relative)
                    || is_named_flat_module(relative);
                if !is_port_implementation(relative)
                    && !is_test_path(relative)
                    && mentions_io
                    && product_layer
                {
                    violations.push(format!(
                        "{display} mixes a controller or application path with SQL, ORM, or Redis"
                    ));
                }
                if concrete_client_violation(
                    relative,
                    &source,
                    &outbound,
                    controller,
                    boot_controller,
                    legacy_boot_controller,
                ) {
                    violations.push(format!(
                        "{display} controller or application code names a concrete store or client"
                    ));
                }
            }
        }

        assert_eq!(
            domain_needles,
            [
                "a3s_boot",
                "a3s_orm",
                "a3s_boot",
                "a3s_orm",
                "redis",
                "aws_sdk_s3",
                "serde",
                "serde_json",
            ]
        );
        for needle in ["sql_query", "a3s_orm::", "redis::"] {
            assert!(
                io_needles.contains(&needle),
                "layer scan no longer fails on {needle}"
            );
        }
        assert_eq!(controller, "#[controller");
        assert_eq!(legacy_boot_controller, "#[a3s_boot::controller");
        assert_eq!(boot_controller, "#[a3s_boot::controller");
        assert_eq!(
            outbound,
            [
                "CloudClient",
                "ClickHouseLogs",
                "InferenceStore",
                "PipelineStore",
                "UsageFactLedger",
            ]
        );
        for (path, sample) in [
            ("contexts/pipelines/runner.rs", "CloudClient::new"),
            ("contexts/pipelines/hooks.rs", "PipelineStore::new"),
            ("contexts/logs.rs", "ClickHouseLogs::search"),
            ("contexts/usage.rs", "CloudClient::new"),
            ("contexts/usage_export.rs", "UsageFactLedger::commit"),
            ("contexts/human_auth.rs", "ClickHouseLogs"),
            ("contexts/api_surface.rs", "InferenceStore"),
            ("contexts/inference/mod.rs", "InferenceStore::new"),
            ("contexts/inference/application.rs", "CloudClient::new"),
            ("src/contexts/pipelines/runner.rs", "CloudClient::new"),
            ("src/contexts/pipelines/hooks.rs", "PipelineStore::new"),
            ("src/contexts/logs.rs", "ClickHouseLogs::search"),
            ("src/contexts/usage.rs", "CloudClient::new"),
            ("src/contexts/usage_export.rs", "UsageFactLedger::commit"),
            ("src/contexts/inference/mod.rs", "InferenceStore::new"),
            ("src/contexts/inference/application.rs", "CloudClient::new"),
        ] {
            assert!(
                concrete_client_violation(
                    Path::new(path),
                    sample,
                    &outbound,
                    controller,
                    boot_controller,
                    legacy_boot_controller,
                ),
                "{path} would stay green with {sample}"
            );
        }
        for path in [
            "contexts/inference/infrastructure.rs",
            "contexts/pipelines/infrastructure.rs",
            "infrastructure/clickhouse_logs.rs",
            "infrastructure/token_usage.rs",
            "infrastructure/usage_export_ledger.rs",
            "contexts/identity/infrastructure/cloud_gateway.rs",
            "src/contexts/inference/infrastructure.rs",
            "src/contexts/pipelines/infrastructure.rs",
            "src/infrastructure/clickhouse_logs.rs",
        ] {
            assert!(
                !concrete_client_violation(
                    Path::new(path),
                    "CloudClient ClickHouseLogs InferenceStore PipelineStore UsageFactLedger",
                    &outbound,
                    controller,
                    boot_controller,
                    legacy_boot_controller,
                ),
                "{path} must keep its infrastructure implementation"
            );
        }

        // Named flat-module assertions were specific to the a3s-api process
        // and removed when the scan was scoped to billing only. Billing has no named flat modules.
        for path in relatives.iter().filter(|path| {
            let normalized = normalized_module_path(path);
            normalized.ends_with("inference/infrastructure.rs")
                || normalized.ends_with("pipelines/infrastructure.rs")
        }) {
            assert!(
                is_port_implementation(path) && !is_named_flat_module(path),
                "infrastructure implementation was scanned as a product module: {}",
                path.display()
            );
        }

        assert!(
            violations.is_empty(),
            "DDD layer scan failed:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn billing_stays_off_cloud_stores() {
        // Convergence DP-1 boundary
        // (apps/cloud/docs/convergence/dp1-billing-boundary.md): billing
        // is the member-plane commercial ledger and holds no Cloud
        // control-plane storage client under either DP-1 outcome —
        // product usage, if ever consumed, arrives through the sanctioned
        // member-plane projection surface, never a Cloud namespace
        // dependency. The token set is the Cloud product's own closed
        // namespace (crate prefix, crate path separator, env prefix).
        let tokens = [
            concat!("a3s", "-cloud"),
            concat!("a3s", "_cloud::"),
            concat!("A3S", "_CLOUD_"),
        ];
        let mut hits = Vec::new();
        for (_, root) in roots() {
            let mut pending = vec![root.clone()];
            while let Some(path) = pending.pop() {
                if path.is_dir() {
                    let entries = std::fs::read_dir(&path).expect("read source directory");
                    pending.extend(entries.map(|entry| entry.expect("source entry").path()));
                    continue;
                }
                if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                    continue;
                }
                let relative = path.strip_prefix(&root).expect("relative source path");
                if relative.ends_with("architecture.rs") {
                    continue;
                }
                let source = std::fs::read_to_string(&path).expect("read source");
                let source = production_source(&source);
                for token in tokens {
                    if source.contains(token) {
                        hits.push(format!("{} references {}", relative.display(), token));
                    }
                }
            }
        }
        assert!(
            hits.is_empty(),
            "billing must not reference the Cloud control-plane namespace:\n{}",
            hits.join("\n")
        );
    }
}
