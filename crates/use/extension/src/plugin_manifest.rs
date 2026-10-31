use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use aster_acl::Block;
use aster_use_core::{
    OkfBundleContract, OkfBundleLimits, OkfFormatVersion, UseResult, OKF_BUNDLE_CONTRACT_SCHEMA,
};
use serde::{Deserialize, Serialize};

use super::{
    bounded_text, manifest_error, optional_bool_attribute, optional_i32_attribute,
    optional_list_attribute, optional_string_attribute, require_known_attributes, string_attribute,
    valid_segment, validate_relative_path,
};

const MAX_SURFACES_PER_KIND: usize = 64;
const MAX_DEPENDENCIES_PER_SURFACE: usize = 64;
const MAX_TASK_TIMEOUT_MS: u64 = 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurfaceActivation {
    Eager,
    Lazy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSurface {
    pub id: String,
    pub activation: SurfaceActivation,
    pub optional: bool,
    pub workload: ToolWorkload,
}

impl ToolSurface {
    pub(crate) fn package_paths(&self) -> Vec<&Path> {
        match &self.workload {
            ToolWorkload::Task(task) => match &task.source {
                ToolTaskSource::Executable { executable } => vec![executable.as_path()],
                ToolTaskSource::Release { release } => vec![release.as_path()],
            },
            ToolWorkload::Service(service) => {
                let mut paths = vec![service.release.as_path()];
                if let Some(contract) = &service.contract {
                    paths.push(contract.as_path());
                }
                paths
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolWorkload {
    Task(ToolTaskSurface),
    Service(ToolServiceSurface),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTaskSurface {
    pub source: ToolTaskSource,
    pub command: String,
    pub json_output: bool,
    pub interactive: bool,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolTaskSource {
    Executable { executable: PathBuf },
    Release { release: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolServiceSurface {
    pub release: PathBuf,
    pub base_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMcpSurface {
    pub id: String,
    pub activation: SurfaceActivation,
    pub optional: bool,
    pub launch: PluginMcpLaunch,
}

impl PluginMcpSurface {
    pub(crate) fn package_paths(&self) -> Vec<&Path> {
        match &self.launch {
            PluginMcpLaunch::Stdio { executable, .. } => vec![executable.as_path()],
            PluginMcpLaunch::StreamableHttp { release } => vec![release.as_path()],
            PluginMcpLaunch::Catalog { server } => vec![server.as_path()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginMcpLaunch {
    Stdio {
        executable: PathBuf,
        args: Vec<String>,
    },
    StreamableHttp {
        release: PathBuf,
    },
    /// Connection facts live in `server.json` (`stdio`, `http`, or `sse`).
    /// The member host reads that file; this surface does not launch a binary.
    Catalog {
        server: PathBuf,
    },
}

/// Who runs the agent loop. The closed set is `builtin` and `package`.
///
/// `builtin` is the host Aster Code harness. `package` is a process shipped in
/// the package. LangChain, the Vercel AI SDK, and Codex stay inside that process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentHarness {
    Builtin,
    Package,
}

impl AgentHarness {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "builtin" => Ok(Self::Builtin),
            "package" => Ok(Self::Package),
            _ => Err("builtin, package"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Package => "package",
        }
    }
}

/// Where a submitted agent form runs. The closed set is `edge`, `cloud`, and `remote`.
///
/// `edge` and `cloud` start an Aster OS Agent Runtime in that place. `remote`
/// sends the same parameter values to an external A2A agent card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentRuntimePlace {
    Edge,
    Cloud,
    Remote,
}

impl AgentRuntimePlace {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "edge" => Ok(Self::Edge),
            "cloud" => Ok(Self::Cloud),
            "remote" => Ok(Self::Remote),
            _ => Err("edge, cloud, remote"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Edge => "edge",
            Self::Cloud => "cloud",
            Self::Remote => "remote",
        }
    }
}

/// Compute the host must admit before the agent run starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResources {
    pub cpu_millis: u64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAgentSurface {
    pub id: String,
    pub runtime: AgentRuntimePlace,
    /// Set when `runtime` is `edge` or `cloud`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<AgentHarness>,
    /// Set when `runtime` is `edge` or `cloud`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<AgentResources>,
    /// HTTPS agent-card URL. Set when `runtime` is `remote`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
    /// Standing instructions for a builtin harness. A package harness may omit this file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<PathBuf>,
    /// Package-relative process that implements the agent-run protocol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<PathBuf>,
    /// JSON Schema file for the call form. Absent when the agent takes no parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<PathBuf>,
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_mcp: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_okf: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flows: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_ui: Vec<String>,
}

impl PluginAgentSurface {
    pub fn package_paths(&self) -> Vec<&Path> {
        let mut paths = Vec::new();
        if let Some(path) = &self.instructions {
            paths.push(path.as_path());
        }
        if let Some(path) = &self.entry {
            paths.push(path.as_path());
        }
        if let Some(path) = &self.parameters {
            paths.push(path.as_path());
        }
        paths
    }
}

/// A `plugin` contribution inside an aster package. `entry` is a file in the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePluginSurface {
    pub id: String,
    pub entry: PathBuf,
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_mcp: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_okf: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flows: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_ui: Vec<String>,
}

/// Host work a published skill asks the shell to do before the model turn.
/// The closed set is `docx-review`, `paperloom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillHostMode {
    DocxReview,
    Paperloom,
}

impl SkillHostMode {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "docx-review" => Ok(Self::DocxReview),
            "paperloom" => Ok(Self::Paperloom),
            _ => Err(manifest_error(format!(
                "Skill host '{value}' is not in the closed set: docx-review, paperloom."
            ))),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DocxReview => "docx-review",
            Self::Paperloom => "paperloom",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSkillSurface {
    pub id: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<SkillHostMode>,
    pub requires_tools: Vec<String>,
    pub requires_mcp: Vec<String>,
    pub requires_okf: Vec<String>,
    pub requires_flows: Vec<String>,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginOkfSurface {
    pub id: String,
    pub bundle: OkfBundleContract,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginFlowEngine {
    AsterFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginFlowRuntime {
    NativeTs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginFlowSurface {
    pub id: String,
    pub engine: PluginFlowEngine,
    pub runtime: PluginFlowRuntime,
    pub source: PathBuf,
    pub export_name: String,
    pub requires_tools: Vec<String>,
    pub requires_mcp: Vec<String>,
    pub requires_okf: Vec<String>,
    pub optional: bool,
}

impl PluginFlowSurface {
    pub(crate) fn package_paths(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.source.as_path())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginUiSurface {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<super::StudioCompositionMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<super::PluginUiPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<super::PluginUiScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<super::PluginUiCondition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<super::PluginUiCommand>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menus: Vec<super::PluginUiMenu>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<super::PluginUiPresentation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<super::PluginEditorPriority>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_ui_icon")]
    pub icon: String,
    #[serde(default = "default_ui_order")]
    pub order: i32,
    pub entry: PathBuf,
    /// Directory of a built static application. Every regular file under it
    /// belongs to this surface. Absent for a page that lists `styles` and
    /// `scripts` explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    pub styles: Vec<PathBuf>,
    pub scripts: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    pub bind_tools: Vec<String>,
    pub bind_mcp: Vec<String>,
    pub bind_flows: Vec<String>,
    pub optional: bool,
}

impl PluginUiSurface {
    pub(crate) fn package_paths(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.entry.as_path())
            .chain(self.styles.iter().map(PathBuf::as_path))
            .chain(self.scripts.iter().map(PathBuf::as_path))
    }
}

pub(crate) fn parse_tool(block: &Block) -> UseResult<ToolSurface> {
    let id = named_surface_id(block, "Tool")?;
    require_known_attributes(
        block,
        &[
            "workload",
            "interface",
            "executable",
            "release",
            "command",
            "json_output",
            "interactive",
            "timeout_ms",
            "base_path",
            "contract",
            "activation",
            "optional",
        ],
    )?;
    let workload = string_attribute(block, "workload")?;
    let interface = string_attribute(block, "interface")?;
    let optional = optional_bool_attribute(block, "optional")?.unwrap_or(false);

    let (activation, workload) = match (workload.as_str(), interface.as_str()) {
        ("task", "cli") => {
            reject_present_attributes(block, &["base_path", "contract"])?;
            let executable = optional_path_attribute(block, "executable")?;
            let release = optional_path_attribute(block, "release")?;
            let source = match (executable, release) {
                (Some(executable), None) => ToolTaskSource::Executable { executable },
                (None, Some(release)) => ToolTaskSource::Release { release },
                _ => {
                    return Err(manifest_error(format!(
                        "Tool Task '{id}' must declare exactly one of 'executable' or 'release'."
                    )));
                }
            };
            let command = string_attribute(block, "command")?;
            if !valid_command(&command) {
                return Err(manifest_error(format!(
                    "Tool Task '{id}' command '{command}' is invalid."
                )));
            }
            let interactive = optional_bool_attribute(block, "interactive")?.unwrap_or(false);
            if interactive {
                return Err(manifest_error(
                    "Tool Tasks must be non-interactive in extension schema version 3.",
                ));
            }
            let timeout_ms = optional_u64_attribute(block, "timeout_ms")?.unwrap_or(120_000);
            if timeout_ms == 0 || timeout_ms > MAX_TASK_TIMEOUT_MS {
                return Err(manifest_error(format!(
                    "Tool Task '{id}' timeout_ms must be between 1 and {MAX_TASK_TIMEOUT_MS}."
                )));
            }
            (
                parse_activation(block, SurfaceActivation::Lazy)?,
                ToolWorkload::Task(ToolTaskSurface {
                    source,
                    command,
                    json_output: optional_bool_attribute(block, "json_output")?.unwrap_or(false),
                    interactive,
                    timeout_ms,
                }),
            )
        }
        ("service", "http") => {
            reject_present_attributes(
                block,
                &[
                    "executable",
                    "command",
                    "json_output",
                    "interactive",
                    "timeout_ms",
                ],
            )?;
            let release = path_attribute(block, "release")?;
            let base_path = string_attribute(block, "base_path")?;
            if !valid_http_path(&base_path) {
                return Err(manifest_error(format!(
                    "Tool Service '{id}' base_path '{base_path}' is invalid."
                )));
            }
            let contract = optional_path_attribute(block, "contract")?;
            if contract.as_ref().is_some_and(|path| {
                !matches!(
                    path.extension().and_then(|value| value.to_str()),
                    Some("json" | "yaml" | "yml")
                )
            }) {
                return Err(manifest_error(format!(
                    "Tool Service '{id}' contract must be JSON or YAML."
                )));
            }
            (
                parse_activation(block, SurfaceActivation::Eager)?,
                ToolWorkload::Service(ToolServiceSurface {
                    release,
                    base_path,
                    contract,
                }),
            )
        }
        _ => {
            return Err(manifest_error(format!(
                "Tool '{id}' must use workload/interface 'task/cli' or 'service/http'."
            )));
        }
    };

    Ok(ToolSurface {
        id,
        activation,
        optional,
        workload,
    })
}

pub(crate) fn parse_mcp(block: &Block) -> UseResult<PluginMcpSurface> {
    let id = named_surface_id(block, "MCP")?;
    require_known_attributes(
        block,
        &[
            "transport",
            "executable",
            "args",
            "release",
            "server",
            "activation",
            "optional",
        ],
    )?;
    let transport = string_attribute(block, "transport")?;
    let optional = optional_bool_attribute(block, "optional")?.unwrap_or(false);
    let (activation, launch) = match transport.as_str() {
        "stdio" => {
            reject_present_attributes(block, &["release", "server"])?;
            (
                parse_activation(block, SurfaceActivation::Lazy)?,
                PluginMcpLaunch::Stdio {
                    executable: path_attribute(block, "executable")?,
                    args: optional_list_attribute(block, "args")?,
                },
            )
        }
        "streamable-http" => {
            reject_present_attributes(block, &["executable", "args", "server"])?;
            (
                parse_activation(block, SurfaceActivation::Eager)?,
                PluginMcpLaunch::StreamableHttp {
                    release: path_attribute(block, "release")?,
                },
            )
        }
        "catalog" => {
            reject_present_attributes(block, &["executable", "args", "release"])?;
            let server = path_attribute(block, "server")?;
            if server.file_name().and_then(|value| value.to_str()) != Some("server.json") {
                return Err(manifest_error(format!(
                    "MCP surface '{id}' catalog server must be server.json."
                )));
            }
            (
                parse_activation(block, SurfaceActivation::Lazy)?,
                PluginMcpLaunch::Catalog { server },
            )
        }
        value => {
            return Err(manifest_error(format!(
                "MCP surface '{id}' transport '{value}' is unsupported."
            )));
        }
    };
    Ok(PluginMcpSurface {
        id,
        activation,
        optional,
        launch,
    })
}

pub(crate) fn parse_plugin(block: &Block) -> UseResult<PackagePluginSurface> {
    let id = named_surface_id(block, "Plugin")?;
    require_known_attributes(
        block,
        &[
            "entry",
            "optional",
            "requires_tool",
            "requires_mcp",
            "requires_okf",
            "requires_skill",
            "requires_flow",
            "requires_ui",
        ],
    )?;
    let entry = path_attribute(block, "entry")?;
    let optional = optional_bool_attribute(block, "optional")?.unwrap_or(false);
    let dependencies = parse_owned_dependencies(block)?;
    Ok(PackagePluginSurface {
        id,
        entry,
        optional,
        requires_tools: dependencies.tools,
        requires_mcp: dependencies.mcp,
        requires_okf: dependencies.okf,
        requires_skills: dependencies.skills,
        requires_flows: dependencies.flows,
        requires_ui: dependencies.ui,
    })
}

pub(crate) fn parse_agent(block: &Block) -> UseResult<PluginAgentSurface> {
    let id = named_surface_id(block, "Agent")?;
    require_known_attributes(
        block,
        &[
            "harness",
            "runtime",
            "instructions",
            "entry",
            "card",
            "parameters",
            "optional",
            "requires_tool",
            "requires_mcp",
            "requires_okf",
            "requires_skill",
            "requires_flow",
            "requires_ui",
        ],
    )?;
    let parameters = optional_path_attribute(block, "parameters")?;
    if let Some(path) = &parameters {
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            return Err(manifest_error(format!(
                "Agent surface '{id}' parameters must be a JSON file."
            )));
        }
    }
    let runtime_name = string_attribute(block, "runtime")?;
    let runtime = match AgentRuntimePlace::parse(&runtime_name) {
        Ok(place) => place,
        Err(closed) => {
            return Err(manifest_error(format!(
                "Agent surface '{id}' runtime '{runtime_name}' is not in the closed set: {closed}."
            )));
        }
    };
    let (harness, resources, card, instructions, entry) = match runtime {
        AgentRuntimePlace::Edge | AgentRuntimePlace::Cloud => {
            if block.attributes.contains_key("card") {
                return Err(manifest_error(format!(
                    "Agent surface '{id}' with runtime {} cannot set card.",
                    runtime.as_str()
                )));
            }
            let harness_name = string_attribute(block, "harness")?;
            let harness = match AgentHarness::parse(&harness_name) {
                Ok(harness) => harness,
                Err(closed) => {
                    return Err(manifest_error(format!(
                        "Agent surface '{id}' harness '{harness_name}' is not in the closed set: {closed}."
                    )));
                }
            };
            let instructions = optional_path_attribute(block, "instructions")?;
            if let Some(path) = &instructions {
                if path.file_name().and_then(|value| value.to_str()) != Some("INSTRUCTIONS.md") {
                    return Err(manifest_error(format!(
                        "Agent surface '{id}' instructions must be named INSTRUCTIONS.md."
                    )));
                }
            }
            let entry = optional_path_attribute(block, "entry")?;
            match harness {
                AgentHarness::Builtin => {
                    if instructions.is_none() {
                        return Err(manifest_error(format!(
                            "Agent surface '{id}' with harness builtin requires instructions."
                        )));
                    }
                    if entry.is_some() {
                        return Err(manifest_error(format!(
                            "Agent surface '{id}' with harness builtin cannot set entry."
                        )));
                    }
                }
                AgentHarness::Package => {
                    if entry.is_none() {
                        return Err(manifest_error(format!(
                            "Agent surface '{id}' with harness package requires entry."
                        )));
                    }
                }
            }
            (
                Some(harness),
                Some(parse_agent_resources(block, &id)?),
                None,
                instructions,
                entry,
            )
        }
        AgentRuntimePlace::Remote => {
            for name in ["harness", "instructions", "entry"] {
                if block.attributes.contains_key(name) {
                    return Err(manifest_error(format!(
                        "Agent surface '{id}' with runtime remote cannot set {name}."
                    )));
                }
            }
            if block.blocks.iter().any(|child| child.name == "resources") {
                return Err(manifest_error(format!(
                    "Agent surface '{id}' with runtime remote cannot set resources."
                )));
            }
            let card = string_attribute(block, "card")?;
            validate_agent_card_url(&card)?;
            (None, None, Some(card), None, None)
        }
    };
    let optional = optional_bool_attribute(block, "optional")?.unwrap_or(false);
    if optional {
        return Err(manifest_error(format!(
            "Agent surface '{id}' is the package core and cannot be optional."
        )));
    }
    let dependencies = parse_owned_dependencies(block)?;
    Ok(PluginAgentSurface {
        id,
        runtime,
        harness,
        resources,
        card,
        instructions,
        entry,
        parameters,
        optional,
        requires_tools: dependencies.tools,
        requires_mcp: dependencies.mcp,
        requires_okf: dependencies.okf,
        requires_skills: dependencies.skills,
        requires_flows: dependencies.flows,
        requires_ui: dependencies.ui,
    })
}

fn validate_agent_card_url(url: &str) -> UseResult<()> {
    let parsed = url::Url::parse(url)
        .map_err(|error| manifest_error(format!("Invalid agent card URL '{url}': {error}")))?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.host_str().is_none()
    {
        return Err(manifest_error(
            "Agent card URLs must be credential-free HTTPS URLs without a query or fragment."
                .to_owned(),
        ));
    }
    Ok(())
}

struct OwnedDependencies {
    tools: Vec<String>,
    mcp: Vec<String>,
    okf: Vec<String>,
    skills: Vec<String>,
    flows: Vec<String>,
    ui: Vec<String>,
}

fn parse_agent_resources(block: &Block, id: &str) -> UseResult<AgentResources> {
    let Some(resources) = block.blocks.iter().find(|child| child.name == "resources") else {
        return Err(manifest_error(format!(
            "Agent surface '{id}' requires a resources block."
        )));
    };
    require_known_attributes(resources, &["cpu_millis", "memory_bytes"])?;
    if !resources.blocks.is_empty() || !resources.labels.is_empty() {
        return Err(manifest_error(format!(
            "Agent surface '{id}' resources block accepts only cpu_millis and memory_bytes."
        )));
    }
    Ok(AgentResources {
        cpu_millis: positive_u64_attribute(resources, "cpu_millis")?,
        memory_bytes: positive_u64_attribute(resources, "memory_bytes")?,
    })
}

fn parse_owned_dependencies(block: &Block) -> UseResult<OwnedDependencies> {
    Ok(OwnedDependencies {
        tools: dependency_ids(block, "requires_tool", "Tool")?,
        mcp: dependency_ids(block, "requires_mcp", "MCP")?,
        okf: dependency_ids(block, "requires_okf", "OKF")?,
        skills: dependency_ids(block, "requires_skill", "Skill")?,
        flows: dependency_ids(block, "requires_flow", "Flow")?,
        ui: dependency_ids(block, "requires_ui", "UI")?,
    })
}

pub(crate) fn parse_skill(block: &Block) -> UseResult<PluginSkillSurface> {
    let id = named_surface_id(block, "Skill")?;
    require_known_attributes(
        block,
        &[
            "path",
            "requires_tool",
            "requires_mcp",
            "requires_okf",
            "requires_flow",
            "optional",
            "host",
        ],
    )?;
    let path = path_attribute(block, "path")?;
    if path.file_name().and_then(|value| value.to_str()) != Some("SKILL.md") {
        return Err(manifest_error(format!(
            "Skill surface '{id}' must point to SKILL.md."
        )));
    }
    let host = optional_string_attribute(block, "host")?
        .map(|value| SkillHostMode::parse(&value))
        .transpose()?;
    let requires_tools = dependency_ids(block, "requires_tool", "Tool")?;
    let requires_mcp = dependency_ids(block, "requires_mcp", "MCP")?;
    Ok(PluginSkillSurface {
        id,
        path,
        host,
        requires_tools,
        requires_mcp,
        requires_okf: dependency_ids(block, "requires_okf", "OKF")?,
        requires_flows: dependency_ids(block, "requires_flow", "Flow")?,
        optional: optional_bool_attribute(block, "optional")?.unwrap_or(false),
    })
}

pub(crate) fn parse_okf(block: &Block) -> UseResult<PluginOkfSurface> {
    let id = named_surface_id(block, "OKF")?;
    require_known_attributes(
        block,
        &[
            "format_version",
            "root",
            "content_digest",
            "concept_count",
            "file_count",
            "expanded_bytes",
            "max_files",
            "max_concepts",
            "max_expanded_bytes",
            "max_document_bytes",
            "max_links_per_document",
            "optional",
        ],
    )?;
    let format_version = match string_attribute(block, "format_version")?.as_str() {
        "0.2" => OkfFormatVersion::V0_2,
        value => {
            return Err(manifest_error(format!(
                "OKF surface '{id}' format_version '{value}' is unsupported."
            )));
        }
    };
    let bundle = OkfBundleContract {
        schema: OKF_BUNDLE_CONTRACT_SCHEMA.to_owned(),
        format_version,
        root: string_attribute(block, "root")?,
        content_digest: string_attribute(block, "content_digest")?,
        concept_count: u64_attribute(block, "concept_count")?,
        file_count: u64_attribute(block, "file_count")?,
        expanded_bytes: u64_attribute(block, "expanded_bytes")?,
        limits: OkfBundleLimits {
            max_files: u64_attribute(block, "max_files")?,
            max_concepts: u64_attribute(block, "max_concepts")?,
            max_expanded_bytes: u64_attribute(block, "max_expanded_bytes")?,
            max_document_bytes: u64_attribute(block, "max_document_bytes")?,
            max_links_per_document: u64_attribute(block, "max_links_per_document")?,
        },
    };
    bundle.validate().map_err(|error| {
        manifest_error(format!(
            "OKF surface '{id}' has an invalid bundle contract: {}",
            error.message
        ))
    })?;
    Ok(PluginOkfSurface {
        id,
        bundle,
        optional: optional_bool_attribute(block, "optional")?.unwrap_or(false),
    })
}

pub(crate) fn parse_flow(block: &Block) -> UseResult<PluginFlowSurface> {
    let id = named_surface_id(block, "Flow")?;
    require_known_attributes(
        block,
        &[
            "engine",
            "runtime",
            "source",
            "export",
            "requires_tool",
            "requires_mcp",
            "requires_okf",
            "optional",
        ],
    )?;
    let engine = match string_attribute(block, "engine")?.as_str() {
        "aster-flow" => PluginFlowEngine::AsterFlow,
        value => {
            return Err(manifest_error(format!(
                "Flow surface '{id}' engine '{value}' is unsupported."
            )));
        }
    };
    let runtime = match string_attribute(block, "runtime")?.as_str() {
        "native-ts" => PluginFlowRuntime::NativeTs,
        value => {
            return Err(manifest_error(format!(
                "Flow surface '{id}' runtime '{value}' is unsupported."
            )));
        }
    };
    let source = path_attribute(block, "source")?;
    if source.extension().and_then(|value| value.to_str()) != Some("ts") {
        return Err(manifest_error(format!(
            "Flow surface '{id}' source must be a TypeScript .ts file."
        )));
    }
    let export_name = string_attribute(block, "export")?;
    if !valid_flow_export(&export_name) {
        return Err(manifest_error(format!(
            "Flow surface '{id}' export must be a portable TypeScript identifier."
        )));
    }
    Ok(PluginFlowSurface {
        id,
        engine,
        runtime,
        source,
        export_name,
        requires_tools: dependency_ids(block, "requires_tool", "Tool")?,
        requires_mcp: dependency_ids(block, "requires_mcp", "MCP")?,
        requires_okf: dependency_ids(block, "requires_okf", "OKF")?,
        optional: optional_bool_attribute(block, "optional")?.unwrap_or(false),
    })
}

pub(crate) fn parse_ui(block: &Block) -> UseResult<PluginUiSurface> {
    let id = named_surface_id(block, "UI")?;
    require_known_attributes(
        block,
        &[
            "title",
            "contract_version",
            "mode",
            "container",
            "point",
            "scope",
            "presentation",
            "selector",
            "priority",
            "description",
            "icon",
            "order",
            "entry",
            "root",
            "styles",
            "scripts",
            "skill",
            "bind_tool",
            "bind_mcp",
            "bind_flow",
            "optional",
        ],
    )?;
    let title = optional_string_attribute(block, "title")?
        .map(|value| bounded_text(value, "UI title", 64))
        .transpose()?
        .unwrap_or_else(|| id.clone());
    let description = optional_string_attribute(block, "description")?
        .map(|value| bounded_text(value, "UI description", 240))
        .transpose()?
        .unwrap_or_default();
    let icon = optional_string_attribute(block, "icon")?.unwrap_or_else(default_ui_icon);
    if !valid_segment(&icon) {
        return Err(manifest_error(format!(
            "UI icon '{icon}' must be a lowercase icon identifier."
        )));
    }
    let order = optional_i32_attribute(block, "order")?.unwrap_or_else(default_ui_order);
    let entry = path_attribute(block, "entry")?;
    if entry.extension().and_then(|value| value.to_str()) != Some("html") {
        return Err(manifest_error(format!(
            "UI surface '{id}' entry must be an HTML file."
        )));
    }
    let root = optional_path_attribute(block, "root")?;
    let styles = resource_paths(block, "styles", "css")?;
    let scripts = resource_paths(block, "scripts", "js")?;
    if let Some(root) = &root {
        if !entry.starts_with(root) {
            return Err(manifest_error(format!(
                "UI surface '{id}' entry must stay inside its root."
            )));
        }
        for resource in styles.iter().chain(&scripts) {
            if !resource.starts_with(root) {
                return Err(manifest_error(format!(
                    "UI surface '{id}' asset '{}' must stay inside its root.",
                    resource.display()
                )));
            }
        }
    }
    let mut resources = BTreeSet::from([entry.clone()]);
    for resource in styles.iter().chain(&scripts) {
        if !resources.insert(resource.clone()) {
            return Err(manifest_error(format!(
                "UI surface '{id}' asset '{}' is declared more than once.",
                resource.display()
            )));
        }
    }
    let skill = optional_string_attribute(block, "skill")?;
    if skill.as_ref().is_some_and(|value| !valid_segment(value)) {
        return Err(manifest_error(format!(
            "UI surface '{id}' Skill reference is invalid."
        )));
    }
    let commands = super::workbench_commands::parse_commands(block)?;
    let menus = super::workbench_commands::parse_menus(block, &commands)?;
    let point = optional_string_attribute(block, "point")?
        .map(|value| super::PluginUiPoint::parse(&value))
        .transpose()?;
    if point == Some(super::PluginUiPoint::Composer) && (!commands.is_empty() || !menus.is_empty())
    {
        return Err(manifest_error(
            "AIX Composer uses its draft protocol and cannot declare view commands.",
        ));
    }
    Ok(PluginUiSurface {
        id,
        contract_version: optional_i32_attribute(block, "contract_version")?
            .map(|version| {
                u32::try_from(version)
                    .map_err(|_| manifest_error("Invalid Studio extension point contract version."))
            })
            .transpose()?,
        mode: optional_string_attribute(block, "mode")?
            .map(|value| super::StudioCompositionMode::parse(&value))
            .transpose()?,
        container: optional_string_attribute(block, "container")?,
        point,
        scope: optional_string_attribute(block, "scope")?
            .map(|value| super::PluginUiScope::parse(&value))
            .transpose()?,
        when: super::workbench_manifest::parse_when(block)?,
        commands,
        menus,
        presentation: optional_string_attribute(block, "presentation")?
            .map(|value| super::PluginUiPresentation::parse(&value))
            .transpose()?,
        selector: optional_string_attribute(block, "selector")?,
        priority: optional_string_attribute(block, "priority")?
            .map(|value| super::PluginEditorPriority::parse(&value))
            .transpose()?,
        title,
        description,
        icon,
        order,
        entry,
        root,
        styles,
        scripts,
        skill,
        bind_tools: dependency_ids(block, "bind_tool", "Tool")?,
        bind_mcp: dependency_ids(block, "bind_mcp", "MCP")?,
        bind_flows: dependency_ids(block, "bind_flow", "Flow")?,
        optional: optional_bool_attribute(block, "optional")?.unwrap_or(false),
    })
}

fn default_ui_icon() -> String {
    "package".to_string()
}

fn default_ui_order() -> i32 {
    100
}

pub(crate) fn validate_dependencies(
    tools: &[ToolSurface],
    mcp: &[PluginMcpSurface],
    okf: &[PluginOkfSurface],
    flows: &[PluginFlowSurface],
    skills: &[PluginSkillSurface],
    agents: &[PluginAgentSurface],
    ui: &[PluginUiSurface],
    plugins: &[PackagePluginSurface],
) -> UseResult<()> {
    validate_surface_count("Tool", tools.len())?;
    validate_surface_count("MCP", mcp.len())?;
    validate_surface_count("OKF", okf.len())?;
    validate_surface_count("Flow", flows.len())?;
    validate_surface_count("Skill", skills.len())?;
    validate_surface_count("UI", ui.len())?;
    validate_surface_count("Plugin", plugins.len())?;
    validate_surface_count("Agent", agents.len())?;

    let tool_ids = unique_ids("Tool", tools.iter().map(|surface| surface.id.as_str()))?;
    let mcp_ids = unique_ids("MCP", mcp.iter().map(|surface| surface.id.as_str()))?;
    let okf_ids = unique_ids("OKF", okf.iter().map(|surface| surface.id.as_str()))?;
    let flow_ids = unique_ids("Flow", flows.iter().map(|surface| surface.id.as_str()))?;
    let skill_ids = unique_ids("Skill", skills.iter().map(|surface| surface.id.as_str()))?;
    let ui_ids = unique_ids("UI", ui.iter().map(|surface| surface.id.as_str()))?;
    unique_ids("Plugin", plugins.iter().map(|surface| surface.id.as_str()))?;
    unique_ids("Agent", agents.iter().map(|surface| surface.id.as_str()))?;

    for flow in flows {
        for dependency in &flow.requires_tools {
            if !tool_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "Flow '{}' requires unknown Tool '{dependency}'.",
                    flow.id
                )));
            }
        }
        for dependency in &flow.requires_mcp {
            if !mcp_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "Flow '{}' requires unknown MCP surface '{dependency}'.",
                    flow.id
                )));
            }
        }
        for dependency in &flow.requires_okf {
            if !okf_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "Flow '{}' requires unknown OKF surface '{dependency}'.",
                    flow.id
                )));
            }
        }
    }
    for skill in skills {
        for dependency in &skill.requires_tools {
            if !tool_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "Skill '{}' requires unknown Tool '{dependency}'.",
                    skill.id
                )));
            }
        }
        for dependency in &skill.requires_mcp {
            if !mcp_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "Skill '{}' requires unknown MCP surface '{dependency}'.",
                    skill.id
                )));
            }
        }
        for dependency in &skill.requires_okf {
            if !okf_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "Skill '{}' requires unknown OKF surface '{dependency}'.",
                    skill.id
                )));
            }
        }
        for dependency in &skill.requires_flows {
            if !flow_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "Skill '{}' requires unknown Flow surface '{dependency}'.",
                    skill.id
                )));
            }
        }
    }
    for surface in ui {
        if let Some(skill) = &surface.skill {
            if !skill_ids.contains(skill.as_str()) {
                return Err(manifest_error(format!(
                    "UI '{}' requires unknown Skill '{skill}'.",
                    surface.id
                )));
            }
        }
        for dependency in &surface.bind_tools {
            if !tool_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "UI '{}' binds unknown Tool '{dependency}'.",
                    surface.id
                )));
            }
        }
        for dependency in &surface.bind_mcp {
            if !mcp_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "UI '{}' binds unknown MCP surface '{dependency}'.",
                    surface.id
                )));
            }
        }
        for dependency in &surface.bind_flows {
            if !flow_ids.contains(dependency.as_str()) {
                return Err(manifest_error(format!(
                    "UI '{}' binds unknown Flow surface '{dependency}'.",
                    surface.id
                )));
            }
        }
    }
    for agent in agents {
        require_known_dependencies("Agent", &agent.id, &agent.requires_tools, &tool_ids, "Tool")?;
        require_known_dependencies("Agent", &agent.id, &agent.requires_mcp, &mcp_ids, "MCP")?;
        require_known_dependencies("Agent", &agent.id, &agent.requires_okf, &okf_ids, "OKF")?;
        require_known_dependencies(
            "Agent",
            &agent.id,
            &agent.requires_skills,
            &skill_ids,
            "Skill",
        )?;
        require_known_dependencies("Agent", &agent.id, &agent.requires_flows, &flow_ids, "Flow")?;
        require_known_dependencies("Agent", &agent.id, &agent.requires_ui, &ui_ids, "UI")?;
    }
    for plugin in plugins {
        require_known_dependencies(
            "Plugin",
            &plugin.id,
            &plugin.requires_tools,
            &tool_ids,
            "Tool",
        )?;
        require_known_dependencies("Plugin", &plugin.id, &plugin.requires_mcp, &mcp_ids, "MCP")?;
        require_known_dependencies("Plugin", &plugin.id, &plugin.requires_okf, &okf_ids, "OKF")?;
        require_known_dependencies(
            "Plugin",
            &plugin.id,
            &plugin.requires_skills,
            &skill_ids,
            "Skill",
        )?;
        require_known_dependencies(
            "Plugin",
            &plugin.id,
            &plugin.requires_flows,
            &flow_ids,
            "Flow",
        )?;
        require_known_dependencies("Plugin", &plugin.id, &plugin.requires_ui, &ui_ids, "UI")?;
    }
    Ok(())
}

fn require_known_dependencies(
    owner: &str,
    id: &str,
    dependencies: &[String],
    known: &BTreeSet<&str>,
    kind: &str,
) -> UseResult<()> {
    for dependency in dependencies {
        if !known.contains(dependency.as_str()) {
            return Err(manifest_error(format!(
                "{owner} '{id}' requires unknown {kind} surface '{dependency}'."
            )));
        }
    }
    Ok(())
}

fn named_surface_id(block: &Block, label: &str) -> UseResult<String> {
    if block.labels.len() != 1 || !nested_blocks_allowed(label, block) {
        return Err(manifest_error(format!(
            "A schema version 3 {label} surface requires one ID label and only supported nested blocks."
        )));
    }
    let id = block.labels[0].clone();
    if !valid_segment(&id) {
        return Err(manifest_error(format!(
            "{label} surface ID '{id}' is invalid."
        )));
    }
    Ok(id)
}

fn nested_blocks_allowed(label: &str, block: &Block) -> bool {
    match label {
        "UI" => block
            .blocks
            .iter()
            .all(|child| matches!(child.name.as_str(), "when" | "command" | "menu")),
        "Agent" => {
            let resources = block
                .blocks
                .iter()
                .filter(|child| child.name == "resources")
                .count();
            resources == block.blocks.len() && resources <= 1
        }
        _ => block.blocks.is_empty(),
    }
}

fn validate_surface_count(label: &str, count: usize) -> UseResult<()> {
    if count == 0 {
        return Ok(());
    }
    if count > MAX_SURFACES_PER_KIND {
        return Err(manifest_error(format!(
            "A package may declare at most {MAX_SURFACES_PER_KIND} {label} surfaces."
        )));
    }
    Ok(())
}

fn unique_ids<'a>(
    label: &str,
    values: impl Iterator<Item = &'a str>,
) -> UseResult<BTreeSet<&'a str>> {
    let mut ids = BTreeSet::new();
    for value in values {
        if !ids.insert(value) {
            return Err(manifest_error(format!(
                "Duplicate {label} surface ID '{value}'."
            )));
        }
    }
    Ok(ids)
}

fn dependency_ids(block: &Block, name: &str, label: &str) -> UseResult<Vec<String>> {
    let values = optional_list_attribute(block, name)?;
    if values.len() > MAX_DEPENDENCIES_PER_SURFACE {
        return Err(manifest_error(format!(
            "'{name}' accepts at most {MAX_DEPENDENCIES_PER_SURFACE} {label} references."
        )));
    }
    let mut seen = BTreeSet::new();
    for value in &values {
        if !valid_segment(value) || !seen.insert(value) {
            return Err(manifest_error(format!(
                "'{name}' contains an invalid or duplicate {label} surface ID."
            )));
        }
    }
    Ok(values)
}

fn resource_paths(block: &Block, name: &str, extension: &str) -> UseResult<Vec<PathBuf>> {
    let values = optional_list_attribute(block, name)?;
    if values.len() > 16 {
        return Err(manifest_error(format!(
            "UI '{name}' accepts at most 16 assets."
        )));
    }
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .map(|value| {
            let path = PathBuf::from(value);
            validate_relative_path(&path)?;
            if path.extension().and_then(|value| value.to_str()) != Some(extension) {
                return Err(manifest_error(format!(
                    "UI '{name}' assets must use the .{extension} extension."
                )));
            }
            if !seen.insert(path.clone()) {
                return Err(manifest_error(format!(
                    "UI asset '{}' is declared more than once.",
                    path.display()
                )));
            }
            Ok(path)
        })
        .collect()
}

fn parse_activation(block: &Block, default: SurfaceActivation) -> UseResult<SurfaceActivation> {
    match optional_string_attribute(block, "activation")?.as_deref() {
        None => Ok(default),
        Some("eager") => Ok(SurfaceActivation::Eager),
        Some("lazy") => Ok(SurfaceActivation::Lazy),
        Some(value) => Err(manifest_error(format!(
            "'{}' activation '{value}' is invalid.",
            block.name
        ))),
    }
}

fn path_attribute(block: &Block, name: &str) -> UseResult<PathBuf> {
    let path = PathBuf::from(string_attribute(block, name)?);
    validate_relative_path(&path)?;
    Ok(path)
}

fn optional_path_attribute(block: &Block, name: &str) -> UseResult<Option<PathBuf>> {
    optional_string_attribute(block, name)?
        .map(|value| {
            let path = PathBuf::from(value);
            validate_relative_path(&path)?;
            Ok(path)
        })
        .transpose()
}

fn optional_u64_attribute(block: &Block, name: &str) -> UseResult<Option<u64>> {
    let Some(value) = block.attributes.get(name) else {
        return Ok(None);
    };
    let Some(value) = value.as_number() else {
        return Err(manifest_error(format!(
            "'{}' requires numeric attribute '{name}'.",
            block.name
        )));
    };
    if !value.is_finite() || value.fract() != 0.0 || !(0.0..=u64::MAX as f64).contains(&value) {
        return Err(manifest_error(format!(
            "'{}' attribute '{name}' must be a non-negative integer.",
            block.name
        )));
    }
    Ok(Some(value as u64))
}

fn u64_attribute(block: &Block, name: &str) -> UseResult<u64> {
    optional_u64_attribute(block, name)?.ok_or_else(|| {
        manifest_error(format!(
            "'{}' requires numeric attribute '{name}'.",
            block.name
        ))
    })
}

fn positive_u64_attribute(block: &Block, name: &str) -> UseResult<u64> {
    let value = u64_attribute(block, name)?;
    if value == 0 {
        return Err(manifest_error(format!(
            "'{}' attribute '{name}' must be a positive integer.",
            block.name
        )));
    }
    Ok(value)
}

fn reject_present_attributes(block: &Block, names: &[&str]) -> UseResult<()> {
    if let Some(name) = names
        .iter()
        .find(|name| block.attributes.contains_key(**name))
    {
        return Err(manifest_error(format!(
            "'{}' attribute '{name}' is not valid for the selected surface contract.",
            block.name
        )));
    }
    Ok(())
}

fn valid_command(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && matches!(value.as_bytes().first(), Some(b'a'..=b'z'))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_flow_export(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && value.len() <= 128
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_http_path(value: &str) -> bool {
    value.starts_with('/')
        && value.len() <= 1024
        && !value.contains("//")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'?' | b'#' | b'\\'))
        && !value
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
}
