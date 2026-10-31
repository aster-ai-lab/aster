#![cfg(test)]

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use aster_use_core::OkfFormatVersion;

use super::{
    ExtensionManifest, PluginFlowEngine, PluginFlowRuntime, PluginMcpLaunch, SurfaceActivation,
    ToolTaskSource, ToolWorkload,
};

const NAMED_SURFACE_MANIFEST_BYTES: &[u8] = include_bytes!("../fixtures/manifests/plugin-v3.acl");
const NAMED_SURFACE_MANIFEST: &str = include_str!("../fixtures/manifests/plugin-v3.acl");
const NAMED_SURFACE_MANIFEST_DIGEST: &str =
    include_str!("../fixtures/manifests/plugin-v3.sha256").trim_ascii_end();
const OKF_MANIFEST_BYTES: &[u8] = include_bytes!("../fixtures/manifests/plugin-v3-okf.acl");
const OKF_MANIFEST: &str = include_str!("../fixtures/manifests/plugin-v3-okf.acl");
const OKF_MANIFEST_DIGEST: &str =
    include_str!("../fixtures/manifests/plugin-v3-okf.sha256").trim_ascii_end();
const PACKAGE_MANIFEST: &str =
    include_str!("../fixtures/packages/plugin-v3-package/package/aster-use-extension.acl");
const MHS_BRIDGE_MANIFEST: &str =
    include_str!("../fixtures/packages/plugin-v3-mhs-bridge/package/aster-use-extension.acl");

#[test]
fn schema_v3_acl_fixture_has_a_stable_digest() {
    let digest = format!("sha256:{:x}", Sha256::digest(NAMED_SURFACE_MANIFEST_BYTES));

    assert_eq!(digest, NAMED_SURFACE_MANIFEST_DIGEST);
}

#[test]
fn schema_v3_okf_acl_fixture_has_a_stable_digest() {
    let digest = format!("sha256:{:x}", Sha256::digest(OKF_MANIFEST_BYTES));

    assert_eq!(digest, OKF_MANIFEST_DIGEST);
}

#[test]
fn registry_management_route_is_reserved_from_packages() {
    let manifest = NAMED_SURFACE_MANIFEST.replace(
        "route          = \"research\"",
        "route          = \"registry\"",
    );
    let error = ExtensionManifest::parse_acl(&manifest).unwrap_err();
    assert_eq!(error.code, "use.extension.manifest_invalid");
    assert!(error.message.contains("invalid or reserved"));
}

#[test]
fn parses_schema_v3_named_multi_surfaces() {
    let manifest = ExtensionManifest::parse_acl(NAMED_SURFACE_MANIFEST).unwrap();

    assert_eq!(manifest.schema_version, 3);
    assert_eq!(manifest.tools.len(), 2);
    assert_eq!(manifest.mcp_servers.len(), 2);
    assert!(manifest.flows.is_empty());
    assert_eq!(manifest.skills.len(), 2);
    assert_eq!(manifest.ui.len(), 2);
    assert_eq!(manifest.surface_kinds(), ["tool", "mcp", "skill", "ui"]);

    let task = &manifest.tools[0];
    assert_eq!(task.id, "convert");
    assert_eq!(task.activation, SurfaceActivation::Lazy);
    assert!(!task.optional);
    let ToolWorkload::Task(task) = &task.workload else {
        panic!("convert must be a Tool Task");
    };
    assert_eq!(task.command, "acme-research-convert");
    assert_eq!(task.timeout_ms, 120_000);
    assert!(task.json_output);
    assert!(!task.interactive);
    assert_eq!(
        task.source,
        ToolTaskSource::Executable {
            executable: PathBuf::from("tools/convert/bin/convert")
        }
    );

    let service = &manifest.tools[1];
    assert_eq!(service.id, "index");
    assert_eq!(service.activation, SurfaceActivation::Eager);
    let ToolWorkload::Service(service) = &service.workload else {
        panic!("index must be a Tool Service");
    };
    assert_eq!(
        service.release,
        PathBuf::from("releases/index-tool-v1.json")
    );
    assert_eq!(service.base_path, "/api");
    assert_eq!(
        service.contract,
        Some(PathBuf::from("tools/index/openapi.json"))
    );

    assert!(matches!(
        manifest.mcp_servers[0].launch,
        PluginMcpLaunch::Stdio { .. }
    ));
    assert!(matches!(
        manifest.mcp_servers[1].launch,
        PluginMcpLaunch::StreamableHttp { .. }
    ));
    assert_eq!(manifest.skills[0].requires_tools, ["convert", "index"]);
    assert_eq!(manifest.skills[0].requires_mcp, ["library"]);
    assert_eq!(manifest.ui[0].skill.as_deref(), Some("review"));
    assert_eq!(manifest.ui[0].title, "review");
    assert_eq!(manifest.ui[0].description, "");
    assert_eq!(manifest.ui[0].icon, "package");
    assert_eq!(manifest.ui[0].order, 100);
    assert_eq!(manifest.ui[0].bind_tools, ["index"]);
    assert_eq!(manifest.ui[0].bind_mcp, ["library"]);
}

#[test]
fn schema_v3_parses_aster_flow_surfaces_and_typed_consumers() {
    let manifest = ExtensionManifest::parse_acl(PACKAGE_MANIFEST).unwrap();
    assert_eq!(
        manifest.surface_kinds(),
        ["tool", "mcp", "okf", "flow", "skill", "ui"]
    );
    let flow = &manifest.flows[0];
    assert_eq!(flow.id, "reason");
    assert_eq!(flow.engine, PluginFlowEngine::AsterFlow);
    assert_eq!(flow.runtime, PluginFlowRuntime::NativeTs);
    assert_eq!(flow.source, PathBuf::from("flows/reason.ts"));
    assert_eq!(flow.export_name, "run");
    assert_eq!(flow.requires_tools, ["echo"]);
    assert_eq!(flow.requires_mcp, ["context"]);
    assert_eq!(flow.requires_okf, ["domain"]);
    assert_eq!(manifest.skills[0].requires_flows, ["reason"]);
    assert_eq!(manifest.ui[0].bind_flows, ["reason"]);

    let graph = manifest.plugin_surfaces().unwrap();
    let flow_ref = aster_use_core::PluginSurfaceRef {
        kind: aster_use_core::PackageSurfaceKind::Flow,
        id: "reason".to_string(),
    };
    assert!(graph.iter().any(|surface| surface.surface == flow_ref));
    assert!(graph
        .iter()
        .find(|surface| surface.surface.kind == aster_use_core::PackageSurfaceKind::Skill)
        .unwrap()
        .dependencies
        .contains(&flow_ref));
}

#[test]
fn mhs_bridge_reuses_standard_mcp_flow_skill_and_ui_surfaces() {
    let manifest = ExtensionManifest::parse_acl(MHS_BRIDGE_MANIFEST).unwrap();
    assert_eq!(manifest.package_id, "acme/mhs-bridge");
    assert!(manifest.tools.is_empty());
    assert!(manifest.okf.is_empty());
    assert_eq!(manifest.mcp_servers.len(), 1);
    assert_eq!(manifest.flows.len(), 1);
    assert_eq!(manifest.skills.len(), 1);
    assert_eq!(manifest.ui.len(), 1);
    assert_eq!(manifest.surface_kinds(), ["mcp", "flow", "skill", "ui"]);

    assert!(matches!(
        manifest.mcp_servers[0].launch,
        PluginMcpLaunch::StreamableHttp { .. }
    ));
    assert_eq!(manifest.flows[0].requires_mcp, ["fleet"]);
    assert_eq!(manifest.skills[0].requires_mcp, ["fleet"]);
    assert_eq!(manifest.skills[0].requires_flows, ["monitor"]);
    assert_eq!(manifest.ui[0].bind_mcp, ["fleet"]);
    assert_eq!(manifest.ui[0].bind_flows, ["monitor"]);

    let graph = manifest.plugin_surfaces().unwrap();
    assert_eq!(graph.len(), 4);
    let fleet = aster_use_core::PluginSurfaceRef {
        kind: aster_use_core::PackageSurfaceKind::Mcp,
        id: "fleet".to_string(),
    };
    let monitor = aster_use_core::PluginSurfaceRef {
        kind: aster_use_core::PackageSurfaceKind::Flow,
        id: "monitor".to_string(),
    };
    let operator = aster_use_core::PluginSurfaceRef {
        kind: aster_use_core::PackageSurfaceKind::Skill,
        id: "operator".to_string(),
    };
    let dependencies = |kind, id: &str| {
        &graph
            .iter()
            .find(|surface| surface.surface.kind == kind && surface.surface.id == id)
            .unwrap()
            .dependencies
    };
    assert!(dependencies(aster_use_core::PackageSurfaceKind::Mcp, "fleet").is_empty());
    assert_eq!(
        dependencies(aster_use_core::PackageSurfaceKind::Flow, "monitor"),
        std::slice::from_ref(&fleet)
    );
    assert_eq!(
        dependencies(aster_use_core::PackageSurfaceKind::Skill, "operator"),
        &[monitor.clone(), fleet.clone()]
    );
    assert_eq!(
        dependencies(aster_use_core::PackageSurfaceKind::Ui, "devices"),
        &[monitor, fleet, operator]
    );
}

#[test]
fn schema_v3_rejects_invalid_flow_runtime_source_export_and_dependencies() {
    let unsupported_engine = PACKAGE_MANIFEST.replace("aster-flow", "other-flow");
    assert!(ExtensionManifest::parse_acl(&unsupported_engine)
        .unwrap_err()
        .message
        .contains("engine 'other-flow' is unsupported"));

    let unsupported = PACKAGE_MANIFEST.replace("native-ts", "javascript");
    assert!(ExtensionManifest::parse_acl(&unsupported)
        .unwrap_err()
        .message
        .contains("runtime 'javascript' is unsupported"));

    let wrong_source = PACKAGE_MANIFEST.replace("flows/reason.ts", "flows/reason.js");
    assert!(ExtensionManifest::parse_acl(&wrong_source)
        .unwrap_err()
        .message
        .contains("TypeScript .ts file"));

    let invalid_export =
        PACKAGE_MANIFEST.replace("export        = \"run\"", "export = \"run-flow\"");
    assert!(ExtensionManifest::parse_acl(&invalid_export)
        .unwrap_err()
        .message
        .contains("portable TypeScript identifier"));

    let missing = PACKAGE_MANIFEST.replace(
        "requires_tool = [\"echo\"]",
        "requires_tool = [\"missing\"]",
    );
    assert!(ExtensionManifest::parse_acl(&missing)
        .unwrap_err()
        .message
        .contains("Flow 'reason' requires unknown Tool 'missing'"));
}

#[test]
fn schema_v3_ui_parses_bounded_workbench_metadata() {
    let manifest = NAMED_SURFACE_MANIFEST.replace(
        "ui \"review\" {",
        "ui \"review\" {\n    title       = \"Research Review\"\n    description = \"Inspect evidence from the installed package.\"\n    icon        = \"flask-conical\"\n    order       = 80",
    );
    let manifest = ExtensionManifest::parse_acl(&manifest).unwrap();
    let ui = &manifest.ui[0];
    assert_eq!(ui.title, "Research Review");
    assert_eq!(
        ui.description,
        "Inspect evidence from the installed package."
    );
    assert_eq!(ui.icon, "flask-conical");
    assert_eq!(ui.order, 80);

    let invalid = NAMED_SURFACE_MANIFEST.replace(
        "ui \"review\" {",
        "ui \"review\" {\n    icon = \"Flask Icon\"",
    );
    assert!(ExtensionManifest::parse_acl(&invalid).is_err());
}

#[test]
fn rejects_duplicate_or_missing_named_surface_dependencies() {
    let duplicate = NAMED_SURFACE_MANIFEST.replace("tool \"index\" {", "tool \"convert\" {");
    let error = ExtensionManifest::parse_acl(&duplicate).unwrap_err();
    assert!(error
        .message
        .contains("Duplicate Tool surface ID 'convert'"));

    let missing_tool =
        NAMED_SURFACE_MANIFEST.replace("[\"convert\", \"index\"]", "[\"convert\", \"missing\"]");
    let error = ExtensionManifest::parse_acl(&missing_tool).unwrap_err();
    assert!(error
        .message
        .contains("Skill 'review' requires unknown Tool 'missing'"));

    let missing_skill =
        NAMED_SURFACE_MANIFEST.replace("skill     = \"review\"", "skill = \"missing\"");
    let error = ExtensionManifest::parse_acl(&missing_skill).unwrap_err();
    assert!(error
        .message
        .contains("UI 'review' requires unknown Skill 'missing'"));
}

#[test]
fn rejects_obsolete_cli_surfaces_and_unsafe_schema_v3_tool_contracts() {
    let obsolete_cli = r#"

  cli {
    executable = "bin/obsolete"
  }
"#;
    let mixed = NAMED_SURFACE_MANIFEST.replace(
        "\n  tool \"convert\" {",
        &format!("{obsolete_cli}\n  tool \"convert\" {{"),
    );
    let error = ExtensionManifest::parse_acl(&mixed).unwrap_err();
    assert!(error.message.contains("Unknown extension surface 'cli'"));

    let interactive = NAMED_SURFACE_MANIFEST.replace("interactive = false", "interactive = true");
    let error = ExtensionManifest::parse_acl(&interactive).unwrap_err();
    assert!(error.message.contains("Tool Tasks must be non-interactive"));

    let escaping = NAMED_SURFACE_MANIFEST.replace("tools/convert/bin/convert", "../bin/convert");
    assert!(ExtensionManifest::parse_acl(&escaping).is_err());
}

#[test]
fn rejects_pre_release_manifest_schemas_with_a_rebuild_instruction() {
    for schema_version in [1, 2] {
        let manifest = NAMED_SURFACE_MANIFEST.replace(
            "schema_version = 3",
            &format!("schema_version = {schema_version}"),
        );
        let error = ExtensionManifest::parse_acl(&manifest).unwrap_err();
        assert!(error.message.contains("schema version 3"));
        assert!(error.message.contains("rebuild"));
    }
}

#[test]
fn schema_v3_requires_an_explicit_v3_host_compatibility_gate() {
    let current_host = NAMED_SURFACE_MANIFEST.replace(">=0.3.0, <0.4.0", ">=0.2.0, <0.4.0");
    let error = ExtensionManifest::parse_acl(&current_host).unwrap_err();
    assert!(error
        .message
        .contains("Schema version 3 must require Aster Use 0.3"));
}

#[test]
fn parsed_v3_manifest_still_requires_the_current_compiled_host() {
    let incompatible =
        ExtensionManifest::parse_acl(&NAMED_SURFACE_MANIFEST.replace(">=0.3.0, <0.4.0", "=0.3.0"))
            .unwrap();
    assert!(incompatible.supports_use_version("0.3.0").unwrap());
    assert_eq!(
        incompatible.require_current_use().unwrap_err().code,
        "use.extension.host_incompatible"
    );
    ExtensionManifest::parse_acl(NAMED_SURFACE_MANIFEST)
        .unwrap()
        .require_current_use()
        .unwrap();
}

#[test]
fn schema_v3_parses_an_exact_named_okf_surface_and_skill_dependency() {
    let manifest = ExtensionManifest::parse_acl(OKF_MANIFEST).unwrap();
    assert_eq!(manifest.surface_kinds(), ["okf", "skill"]);
    assert_eq!(manifest.okf.len(), 1);
    assert_eq!(manifest.okf[0].id, "domain-knowledge");
    assert_eq!(
        manifest.okf[0].bundle.format_version,
        OkfFormatVersion::V0_2
    );
    assert_eq!(manifest.okf[0].bundle.root, "okf/domain-knowledge");
    assert_eq!(
        manifest.skills[0].requires_okf,
        ["domain-knowledge".to_owned()]
    );
}

#[test]
fn schema_v3_okf_rejects_drift_unknown_authority_and_missing_dependencies() {
    let okf = r#"

  okf "domain-knowledge" {
    format_version         = "0.2"
    root                   = "okf/domain-knowledge"
    content_digest         = "sha256:bd85b0b63adb32bdf616384a619286af4c32401542655dd09e00450902ab478d"
    concept_count          = 4
    file_count             = 7
    expanded_bytes         = 2053
    max_files              = 256
    max_concepts           = 64
    max_expanded_bytes     = 67108864
    max_document_bytes     = 1048576
    max_links_per_document = 2048
    optional               = false
  }
"#;
    let manifest = NAMED_SURFACE_MANIFEST.replace(
        "\n  tool \"convert\" {",
        &format!("{okf}\n  tool \"convert\" {{"),
    );

    let unknown_dependency = manifest.replace(
        "requires_mcp  = [\"library\"]",
        "requires_mcp  = [\"library\"]\n    requires_okf  = [\"missing\"]",
    );
    let error = ExtensionManifest::parse_acl(&unknown_dependency).unwrap_err();
    assert!(error
        .message
        .contains("requires unknown OKF surface 'missing'"));

    let executable_metadata = manifest.replace(
        "optional               = false",
        "optional               = false\n    executor               = \"tool:index\"",
    );
    let error = ExtensionManifest::parse_acl(&executable_metadata).unwrap_err();
    assert!(error.message.contains("Unknown 'okf' attribute 'executor'"));

    let escaping = manifest.replace("okf/domain-knowledge", "../personal-vault");
    assert!(ExtensionManifest::parse_acl(&escaping).is_err());
}

#[test]
fn schema_v3_parses_sorted_versioned_package_dependencies() {
    let dependencies = r#"

  dependency "acme/vector-store" {
    version = "^2.1.0"
  }

  dependency "acme/base" {
    version = ">=1.0.0, <2.0.0"
  }
"#;
    let manifest = NAMED_SURFACE_MANIFEST.replace(
        "\n  repository {",
        &format!("{dependencies}\n  repository {{"),
    );
    let parsed = ExtensionManifest::parse_acl(&manifest).unwrap();
    assert_eq!(parsed.dependencies.len(), 2);
    assert_eq!(parsed.dependencies[0].package_id, "acme/base");
    assert_eq!(
        parsed.dependencies[0].version_requirement,
        ">=1.0.0, <2.0.0"
    );
    assert_eq!(parsed.dependencies[1].package_id, "acme/vector-store");
    assert_eq!(parsed.dependencies[1].version_requirement, "^2.1.0");
}

#[test]
fn schema_v3_rejects_unsafe_package_dependency_declarations() {
    let dependency = r#"

  dependency "acme/base" {
    version = "^1.0.0"
  }
"#;
    let manifest = NAMED_SURFACE_MANIFEST.replace(
        "\n  repository {",
        &format!("{dependency}\n  repository {{"),
    );

    let duplicate = manifest.replace(dependency, &format!("{dependency}{dependency}"));
    assert!(ExtensionManifest::parse_acl(&duplicate)
        .unwrap_err()
        .message
        .contains("sorted and unique"));

    let self_dependency = manifest.replace("acme/base", "acme/research");
    assert!(ExtensionManifest::parse_acl(&self_dependency)
        .unwrap_err()
        .message
        .contains("cannot depend on itself"));

    let noncanonical = manifest.replace("^1.0.0", "1.0.0");
    assert!(ExtensionManifest::parse_acl(&noncanonical)
        .unwrap_err()
        .message
        .contains("canonical semantic-version"));

    let endpoint = manifest.replace(
        "version = \"^1.0.0\"",
        "version = \"^1.0.0\"\n    registry = \"https://untrusted.example\"",
    );
    assert!(ExtensionManifest::parse_acl(&endpoint)
        .unwrap_err()
        .message
        .contains("Unknown 'dependency' attribute 'registry'"));
}

#[test]
fn an_agent_package_declares_one_required_agent_surface() {
    let manifest = r#"
extension "aster/legal-expert" {
  schema_version = 3
  version        = "1.0.0"
  route          = "legal-expert"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/aster/legal-expert"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  agent "legal-expert" {
    harness      = "builtin"
    instructions = "agents/legal-expert/INSTRUCTIONS.md"
    runtime      = "edge"
    optional     = false

    resources {
      cpu_millis   = 500
      memory_bytes = 268435456
    }
  }

  ui "review" {
    entry     = "ui/review/index.html"
    styles    = []
    scripts   = []
    bind_tool = []
    bind_mcp  = []
    optional  = true
  }
}
"#;
    let parsed = ExtensionManifest::parse_acl(manifest).unwrap();
    assert_eq!(parsed.surface_kinds(), ["agent", "ui"]);
    assert_eq!(parsed.agents.len(), 1);
    assert_eq!(parsed.agents[0].id, "legal-expert");
    assert_eq!(parsed.agents[0].runtime, crate::AgentRuntimePlace::Edge);
    assert_eq!(parsed.agents[0].runtime.as_str(), "edge");
    assert_eq!(parsed.agents[0].harness, Some(crate::AgentHarness::Builtin));
    assert_eq!(parsed.agents[0].card, None);
    assert!(parsed.agents[0]
        .instructions
        .as_ref()
        .is_some_and(|path| path.ends_with("INSTRUCTIONS.md")));
    assert!(parsed.agents[0].entry.is_none());
    let missing_harness = manifest.replace("    harness      = \"builtin\"\n", "");
    assert!(ExtensionManifest::parse_acl(&missing_harness)
        .unwrap_err()
        .message
        .contains("requires string attribute 'harness'"));
    let unknown_harness =
        manifest.replace("harness      = \"builtin\"", "harness      = \"langchain\"");
    assert!(ExtensionManifest::parse_acl(&unknown_harness)
        .unwrap_err()
        .message
        .contains("closed set: builtin, package"));
    let no_instructions = manifest.replace(
        "    instructions = \"agents/legal-expert/INSTRUCTIONS.md\"\n",
        "",
    );
    assert!(ExtensionManifest::parse_acl(&no_instructions)
        .unwrap_err()
        .message
        .contains("requires instructions"));
    let with_entry = manifest.replace(
        "    optional     = false\n",
        "    entry        = \"mcp/rate/server.mjs\"\n    optional     = false\n",
    );
    assert!(ExtensionManifest::parse_acl(&with_entry)
        .unwrap_err()
        .message
        .contains("cannot set entry"));
    let package = manifest
        .replace("harness      = \"builtin\"", "harness      = \"package\"")
        .replace(
            "    instructions = \"agents/legal-expert/INSTRUCTIONS.md\"\n",
            "    entry        = \"mcp/rate/server.mjs\"\n",
        );
    let parsed_package = ExtensionManifest::parse_acl(&package).unwrap();
    assert_eq!(
        parsed_package.agents[0].harness,
        Some(crate::AgentHarness::Package)
    );
    assert!(parsed_package.agents[0].instructions.is_none());
    assert!(parsed_package.agents[0]
        .entry
        .as_ref()
        .is_some_and(|path| path.ends_with("server.mjs")));
    let package_without_entry = package.replace("    entry        = \"mcp/rate/server.mjs\"\n", "");
    assert!(ExtensionManifest::parse_acl(&package_without_entry)
        .unwrap_err()
        .message
        .contains("requires entry"));
    assert!(parsed.agents[0].parameters.is_none());
    assert_eq!(parsed.agents[0].runtime, crate::AgentRuntimePlace::Edge);
    let resources = parsed.agents[0].resources.as_ref().unwrap();
    assert_eq!(resources.cpu_millis, 500);
    assert_eq!(resources.memory_bytes, 268_435_456);
    assert!(parsed.agents[0].requires_skills.is_empty());
    let missing = manifest.replace("    runtime      = \"edge\"\n", "");
    assert!(ExtensionManifest::parse_acl(&missing)
        .unwrap_err()
        .message
        .contains("requires string attribute 'runtime'"));
    let unknown = manifest.replace("runtime      = \"edge\"", "runtime      = \"local\"");
    assert!(ExtensionManifest::parse_acl(&unknown)
        .unwrap_err()
        .message
        .contains("closed set: edge, cloud, remote"));
    let empty = manifest.replace("cpu_millis   = 500", "cpu_millis   = 0");
    assert!(ExtensionManifest::parse_acl(&empty)
        .unwrap_err()
        .message
        .contains("positive integer"));
    let declared = manifest.replace(
        "    optional     = false\n",
        "    parameters   = \"agents/legal-expert/parameters.schema.json\"\n    optional     = false\n",
    );
    let parsed = ExtensionManifest::parse_acl(&declared).unwrap();
    assert!(parsed.agents[0]
        .parameters
        .as_ref()
        .is_some_and(|path| path.ends_with("parameters.schema.json")));
    let notes = declared.replace("parameters.schema.json", "notes.txt");
    assert!(ExtensionManifest::parse_acl(&notes)
        .unwrap_err()
        .message
        .contains("parameters must be a JSON file"));
    let with_card = manifest.replace(
        "    runtime      = \"edge\"\n",
        "    runtime      = \"edge\"\n    card         = \"https://agents.example.com/.well-known/agent-card.json\"\n",
    );
    assert!(ExtensionManifest::parse_acl(&with_card)
        .unwrap_err()
        .message
        .contains("cannot set card"));
    let external = r#"
extension "aster/counsel" {
  schema_version = 3
  version        = "1.0.0"
  route          = "counsel"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/aster/counsel"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  agent "counsel" {
    runtime    = "remote"
    card       = "https://agents.example.com/.well-known/agent-card.json"
    parameters = "agents/counsel/parameters.schema.json"
    optional   = false
  }
}
"#;
    let parsed = ExtensionManifest::parse_acl(external).unwrap();
    assert_eq!(parsed.agents[0].runtime, crate::AgentRuntimePlace::Remote);
    assert_eq!(parsed.agents[0].runtime.as_str(), "remote");
    assert_eq!(
        parsed.agents[0].card.as_deref(),
        Some("https://agents.example.com/.well-known/agent-card.json")
    );
    assert!(parsed.agents[0].harness.is_none());
    assert!(parsed.agents[0].resources.is_none());
    assert!(parsed.agents[0].instructions.is_none());
    assert!(parsed.agents[0].entry.is_none());
    let hosted = external.replace("runtime    = \"remote\"", "runtime    = \"edge\"");
    assert!(ExtensionManifest::parse_acl(&hosted)
        .unwrap_err()
        .message
        .contains("cannot set card"));
    let harnessed = external.replace(
        "    card       = \"https://agents.example.com/.well-known/agent-card.json\"\n",
        "    harness    = \"builtin\"\n    card       = \"https://agents.example.com/.well-known/agent-card.json\"\n",
    );
    assert!(ExtensionManifest::parse_acl(&harnessed)
        .unwrap_err()
        .message
        .contains("cannot set harness"));
    let insecure = external.replace("https://agents.example.com", "http://agents.example.com");
    assert!(ExtensionManifest::parse_acl(&insecure)
        .unwrap_err()
        .message
        .contains("credential-free HTTPS"));
}

#[test]
fn a_skill_host_is_the_closed_set_docx_review_and_paperloom() {
    let base = r#"
extension "aster/review" {
  schema_version = 3
  version        = "1.0.0"
  route          = "review"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/aster/review"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  skill "review" {
    path = "skills/review/SKILL.md"
    host = "docx-review"
  }

  agent "legal-expert" {
    harness        = "builtin"
    instructions   = "agents/legal-expert/INSTRUCTIONS.md"
    runtime        = "edge"
    requires_skill = ["review"]
    optional       = false

    resources {
      cpu_millis   = 500
      memory_bytes = 268435456
    }
  }
}
"#;
    let parsed = ExtensionManifest::parse_acl(base).unwrap();
    assert_eq!(
        parsed.skills[0].host,
        Some(crate::SkillHostMode::DocxReview)
    );
    assert_eq!(parsed.agents[0].requires_skills, ["review"]);
    let paperloom =
        ExtensionManifest::parse_acl(&base.replace("docx-review", "paperloom")).unwrap();
    assert_eq!(
        paperloom.skills[0].host,
        Some(crate::SkillHostMode::Paperloom)
    );
    assert_eq!(crate::SkillHostMode::Paperloom.as_str(), "paperloom");
    let rejected = base.replace("docx-review", "segmented-document");
    let error = ExtensionManifest::parse_acl(&rejected).unwrap_err();
    assert!(error.message.contains("closed set: docx-review"));
    let agent_host = base.replace(
        "    requires_skill = [\"review\"]\n",
        "    host           = \"docx-review\"\n    requires_skill = [\"review\"]\n",
    );
    let error = ExtensionManifest::parse_acl(&agent_host).unwrap_err();
    assert!(error.message.contains("Unknown 'agent' attribute 'host'"));
}

#[test]
fn a_plugin_surface_is_a_package_type() {
    let manifest = r#"
extension "aster/notifier" {
  schema_version = 3
  version        = "1.0.0"
  route          = "notifier"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/aster/notifier"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  plugin "notifier" {
    entry = "plugin/notifier.txt"
  }
}
"#;
    let parsed = ExtensionManifest::parse_acl(manifest).unwrap();
    assert_eq!(parsed.surface_kinds(), ["plugin"]);
    assert_eq!(parsed.plugins[0].id, "notifier");
    assert!(parsed.plugins[0].entry.ends_with("notifier.txt"));
    let graph = parsed.plugin_surfaces().unwrap();
    assert_eq!(
        graph[0].surface.kind,
        aster_use_core::PackageSurfaceKind::Plugin
    );
}

#[test]
fn an_agent_or_plugin_package_depends_on_many_surfaces_and_pages() {
    let okf = r#"
  okf "domain-knowledge" {
    format_version         = "0.2"
    root                   = "okf/domain-knowledge"
    content_digest         = "sha256:bd85b0b63adb32bdf616384a619286af4c32401542655dd09e00450902ab478d"
    concept_count          = 4
    file_count             = 7
    expanded_bytes         = 2053
    max_files              = 256
    max_concepts           = 64
    max_expanded_bytes     = 67108864
    max_document_bytes     = 1048576
    max_links_per_document = 2048
    optional               = false
  }
"#;
    let flow = r#"
  flow "draft" {
    engine        = "aster-flow"
    runtime       = "native-ts"
    source        = "flows/draft.ts"
    export        = "draft"
    requires_tool = ["convert"]
    requires_mcp  = ["library"]
    requires_okf  = ["domain-knowledge"]
    optional      = true
  }
"#;
    let owned = r#"
    requires_tool  = ["convert", "index"]
    requires_mcp   = ["library", "local-library"]
    requires_okf   = ["domain-knowledge"]
    requires_skill = ["review"]
    requires_flow  = ["draft"]
    requires_ui    = ["review", "status"]
"#;
    let base = NAMED_SURFACE_MANIFEST
        .replace(
            "  ui \"review\" {\n    entry",
            "  ui \"review\" {\n    point     = \"studio.applets\"\n    entry",
        )
        .replace(
            "  ui \"status\" {\n    entry",
            "  ui \"status\" {\n    point     = \"studio.composer.actions\"\n    entry",
        );
    let body = base.trim_end().trim_end_matches('}').to_owned();
    let agent = format!(
        "{body}{okf}{flow}\n  agent \"counsel\" {{\n    harness = \"builtin\"\n    instructions = \"agents/counsel/INSTRUCTIONS.md\"\n    runtime = \"cloud\"\n    resources {{\n      cpu_millis = 1000\n      memory_bytes = 536870912\n    }}\n{owned}  }}\n}}\n"
    );
    let plugin = format!(
        "{body}{okf}{flow}\n  plugin \"notifier\" {{\n    entry = \"plugin/notifier.txt\"\n{owned}  }}\n}}\n"
    );

    for manifest in [agent, plugin] {
        let parsed = ExtensionManifest::parse_acl(&manifest).unwrap();
        let owner = parsed
            .plugin_surfaces()
            .unwrap()
            .into_iter()
            .find(|surface| {
                matches!(
                    surface.surface.kind,
                    aster_use_core::PackageSurfaceKind::Agent
                        | aster_use_core::PackageSurfaceKind::Plugin
                )
            })
            .expect("owner");
        let deps: Vec<_> = owner
            .dependencies
            .iter()
            .map(|dependency| format!("{:?}/{}", dependency.kind, dependency.id))
            .collect();
        assert!(deps.iter().any(|dep| dep.ends_with("/convert")));
        assert!(deps.iter().any(|dep| dep.ends_with("/index")));
        assert!(deps.iter().any(|dep| dep.ends_with("/library")));
        assert!(deps.iter().any(|dep| dep.ends_with("/local-library")));
        assert!(deps.iter().any(|dep| dep.ends_with("/domain-knowledge")));
        assert!(deps.iter().any(|dep| dep.ends_with("/review")));
        assert!(deps.iter().any(|dep| dep.ends_with("/draft")));
        assert!(deps.iter().any(|dep| dep.ends_with("/status")));
        let pages = parsed.studio_pages();
        assert!(pages.contains(&"studio.applets"));
        assert!(pages.contains(&"studio.composer.actions"));
        assert!(pages.contains(&"studio.connectors"));
        assert!(pages.contains(&"studio.flows"));
        assert!(pages.contains(&"studio.knowledge"));
        assert!(pages.contains(&"studio.skills"));
        assert!(pages.contains(&"studio.tools"));
        assert_eq!(pages.len(), 8);
    }
}

#[test]
fn the_vercel_weather_sample_declares_a_city_parameter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/vercel-weather");
    let manifest = std::fs::read_to_string(root.join("aster-use-extension.acl")).unwrap();
    let parsed = ExtensionManifest::parse_acl(&manifest).unwrap();
    assert_eq!(parsed.agents.len(), 1);
    assert_eq!(parsed.agents[0].id, "aster-vercel-weather");
    assert_eq!(parsed.agents[0].runtime, crate::AgentRuntimePlace::Cloud);
    assert!(parsed.mcp_servers.is_empty());
    assert!(parsed.agents[0]
        .entry
        .as_ref()
        .is_some_and(|path| path.ends_with("agent/run.mjs")));
    let schema_path = parsed.agents[0].parameters.clone().expect("parameters");
    let schema = std::fs::read(root.join(schema_path)).unwrap();
    let fields = crate::parse_agent_parameter_schema(&schema).unwrap();
    assert_eq!(fields.fields.len(), 1);
    assert_eq!(fields.fields[0].name, "city");
    assert!(fields.fields[0].required);
}
