use std::collections::BTreeSet;

use aster_acl::Block;
use aster_use_core::{UseError, UseResult};
use serde::{Deserialize, Serialize};

use crate::{
    bounded_text, list_attribute, number_attribute, optional_bool_attribute,
    optional_i32_attribute, optional_string_attribute, require_known_attributes, string_attribute,
    valid_segment, PluginAgentSurface, PluginUiContextKey, PluginUiPoint, PluginUiPresentation,
    PluginUiSurface,
};

pub const COMPOSER_MODE_CONTRACT_VERSION: u32 = 1;
pub const MAX_COMPOSER_MODES: usize = 32;
const MAX_COMPOSER_MODE_CONTROLS: usize = 8;
const MAX_CONTROL_OPTIONS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PluginComposerModeKeptNotice {
    #[default]
    Editor,
    Draft,
}

impl PluginComposerModeKeptNotice {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "draft" => Ok(Self::Draft),
            "editor" => Ok(Self::Editor),
            _ => Err(UseError::new(
                "use.extension.composer_mode_kept_notice_invalid",
                "AIX composer mode kept_notice must be draft or editor.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Editor => "editor",
        }
    }
}

fn is_editor_notice(notice: &PluginComposerModeKeptNotice) -> bool {
    *notice == PluginComposerModeKeptNotice::Editor
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginComposerModeScope {
    Workspace,
    Session,
}

impl PluginComposerModeScope {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "workspace" => Ok(Self::Workspace),
            "session" => Ok(Self::Session),
            _ => Err(UseError::new(
                "use.extension.composer_mode_scope_invalid",
                "AIX composer mode scope must be workspace or session.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Session => "session",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginComposerMode {
    pub id: String,
    pub contract_version: u32,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    pub scope: PluginComposerModeScope,
    pub context: Vec<PluginUiContextKey>,
    pub order: i32,
    /// Host-rendered outline fields. Absent on execution-only modes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<PluginComposerModeControl>,
    /// Placeholder shown while this mode is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Which sentence the host shows when a later homepage send finds existing work.
    #[serde(default, skip_serializing_if = "is_editor_notice")]
    pub kept_notice: PluginComposerModeKeptNotice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginComposerModeControlKind {
    Number,
    Select,
    Toggle,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PluginComposerModeInitial {
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginComposerModeControl {
    pub id: String,
    pub kind: PluginComposerModeControlKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub option_labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<PluginComposerModeInitial>,
}

fn refused(message: impl Into<String>) -> UseError {
    UseError::new("use.extension.composer_mode_invalid", message)
}

pub(super) fn parse(block: &Block) -> UseResult<PluginComposerMode> {
    let [id] = block.labels.as_slice() else {
        return Err(refused("AIX composer mode requires one local ID."));
    };
    if !valid_segment(id) || id.len() > 128 {
        return Err(refused(
            "AIX composer mode requires a bounded local ID and no nested blocks.",
        ));
    }
    let controls = parse_controls(&block.blocks)?;
    require_known_attributes(
        block,
        &[
            "contract_version",
            "title",
            "description",
            "icon",
            "agent",
            "editor",
            "scope",
            "context",
            "order",
            "prompt",
            "kept_notice",
        ],
    )?;
    if number_attribute(block, "contract_version")? != f64::from(COMPOSER_MODE_CONTRACT_VERSION) {
        return Err(UseError::new(
            "use.extension.composer_mode_contract_invalid",
            "AIX composer mode contract_version must be 1.",
        ));
    }
    let icon = string_attribute(block, "icon")?;
    let agent = string_attribute(block, "agent")?;
    let editor = optional_string_attribute(block, "editor")?;
    if [&icon, &agent]
        .into_iter()
        .chain(editor.as_ref())
        .any(|value| !valid_segment(value) || value.len() > 128)
    {
        return Err(refused(
            "AIX composer mode icon, agent and editor must be bounded local identifiers.",
        ));
    }
    let keys = list_attribute(block, "context")?;
    if keys.len() > 6 || keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
        return Err(refused(
            "AIX composer mode context permits at most six unique keys.",
        ));
    }
    let context = keys
        .iter()
        .map(|key| PluginUiContextKey::parse(key))
        .collect::<UseResult<Vec<_>>>()?;
    let description = optional_string_attribute(block, "description")?.unwrap_or_default();
    if description.chars().count() > 240 {
        return Err(refused(
            "AIX composer mode description permits at most 240 characters.",
        ));
    }
    let prompt = match optional_string_attribute(block, "prompt")? {
        Some(value) => Some(bounded_text(value, "composer mode prompt", 80)?),
        None => None,
    };
    let kept_notice = match optional_string_attribute(block, "kept_notice")? {
        Some(value) => PluginComposerModeKeptNotice::parse(&value)?,
        None => PluginComposerModeKeptNotice::Editor,
    };
    Ok(PluginComposerMode {
        id: id.clone(),
        contract_version: COMPOSER_MODE_CONTRACT_VERSION,
        title: bounded_text(string_attribute(block, "title")?, "composer mode title", 64)?,
        description,
        icon,
        agent,
        editor,
        scope: PluginComposerModeScope::parse(&string_attribute(block, "scope")?)?,
        context,
        order: optional_i32_attribute(block, "order")?.unwrap_or(100),
        controls,
        prompt,
        kept_notice,
    })
}

fn control_id_ok(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(first) if first.is_ascii_lowercase())
        && value.len() <= 64
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn parse_controls(blocks: &[Block]) -> UseResult<Vec<PluginComposerModeControl>> {
    if blocks.len() > MAX_COMPOSER_MODE_CONTROLS {
        return Err(refused("AIX composer mode permits at most 8 controls."));
    }
    let mut controls = Vec::with_capacity(blocks.len());
    for block in blocks {
        if block.name != "control" || !block.blocks.is_empty() {
            return Err(refused(
                "AIX composer mode requires a bounded local ID and no nested blocks.",
            ));
        }
        controls.push(parse_control(block)?);
    }
    if controls
        .iter()
        .map(|control| control.id.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        != controls.len()
    {
        return Err(refused("AIX composer mode control ids must be unique."));
    }
    Ok(controls)
}

fn forbid_attributes(block: &Block, names: &[&str]) -> UseResult<()> {
    if let Some(name) = names
        .iter()
        .find(|name| block.attributes.contains_key(**name))
    {
        return Err(refused(format!(
            "AIX composer mode control cannot declare '{name}'."
        )));
    }
    Ok(())
}

fn parse_control(block: &Block) -> UseResult<PluginComposerModeControl> {
    let [id] = block.labels.as_slice() else {
        return Err(refused("AIX composer mode control requires one local ID."));
    };
    if !control_id_ok(id) {
        return Err(refused(
            "AIX composer mode control requires a bounded local ID.",
        ));
    }
    require_known_attributes(
        block,
        &[
            "kind",
            "label",
            "min",
            "max",
            "options",
            "option_labels",
            "initial",
        ],
    )?;
    let kind = match string_attribute(block, "kind")?.as_str() {
        "number" => PluginComposerModeControlKind::Number,
        "select" => PluginComposerModeControlKind::Select,
        "toggle" => PluginComposerModeControlKind::Toggle,
        "text" => PluginComposerModeControlKind::Text,
        _ => {
            return Err(refused(
                "AIX composer mode control kind must be number, select, toggle, or text.",
            ))
        }
    };
    let label = bounded_text(
        string_attribute(block, "label")?,
        "composer mode control label",
        32,
    )?;
    let (min, max, options, option_labels, initial) = match kind {
        PluginComposerModeControlKind::Number => {
            forbid_attributes(block, &["options", "option_labels", "initial"])?;
            let min = optional_i32_attribute(block, "min")?
                .ok_or_else(|| refused("AIX composer mode number control requires min."))?;
            let max = optional_i32_attribute(block, "max")?
                .ok_or_else(|| refused("AIX composer mode number control requires max."))?;
            if !(1..=100).contains(&min) || !(1..=100).contains(&max) || min > max {
                return Err(refused(
                    "AIX composer mode number control bounds must sit between 1 and 100.",
                ));
            }
            (Some(min), Some(max), Vec::new(), Vec::new(), None)
        }
        PluginComposerModeControlKind::Select => {
            forbid_attributes(block, &["min", "max"])?;
            let options = bounded_options(&list_attribute(block, "options")?)?;
            let option_labels = match optional_string_list(block, "option_labels")? {
                values if values.is_empty() => Vec::new(),
                values if values.len() == options.len() => values
                    .into_iter()
                    .map(|value| bounded_text(value, "composer mode option label", 32))
                    .collect::<UseResult<Vec<_>>>()?,
                _ => {
                    return Err(refused(
                        "AIX composer mode option labels must match the options.",
                    ))
                }
            };
            let initial = string_attribute(block, "initial")?;
            if !options.iter().any(|option| option == &initial) {
                return Err(refused(
                    "AIX composer mode select initial must be one of its options.",
                ));
            }
            (
                None,
                None,
                options,
                option_labels,
                Some(PluginComposerModeInitial::Text(initial)),
            )
        }
        PluginComposerModeControlKind::Toggle => {
            forbid_attributes(block, &["min", "max", "options", "option_labels"])?;
            let initial = optional_bool_attribute(block, "initial")?.unwrap_or(true);
            (
                None,
                None,
                Vec::new(),
                Vec::new(),
                Some(PluginComposerModeInitial::Bool(initial)),
            )
        }
        PluginComposerModeControlKind::Text => {
            forbid_attributes(block, &["min", "options", "option_labels"])?;
            let max = optional_i32_attribute(block, "max")?
                .ok_or_else(|| refused("AIX composer mode text control requires max."))?;
            if !(1..=8000).contains(&max) {
                return Err(refused(
                    "AIX composer mode text control max must sit between 1 and 8000.",
                ));
            }
            let initial = optional_string_attribute(block, "initial")?;
            if let Some(value) = &initial {
                if value.chars().count() > max as usize {
                    return Err(refused("AIX composer mode text initial exceeds its max."));
                }
            }
            (
                None,
                Some(max),
                Vec::new(),
                Vec::new(),
                initial.map(PluginComposerModeInitial::Text),
            )
        }
    };
    Ok(PluginComposerModeControl {
        id: id.clone(),
        kind,
        label,
        min,
        max,
        options,
        option_labels,
        initial,
    })
}

fn optional_string_list(block: &Block, name: &str) -> UseResult<Vec<String>> {
    if !block.attributes.contains_key(name) {
        return Ok(Vec::new());
    }
    list_attribute(block, name)
}

fn bounded_options(options: &[String]) -> UseResult<Vec<String>> {
    if options.is_empty()
        || options.len() > MAX_CONTROL_OPTIONS
        || options.iter().collect::<BTreeSet<_>>().len() != options.len()
    {
        return Err(refused(
            "AIX composer mode select options must be unique and at most 128.",
        ));
    }
    options
        .iter()
        .map(|option| bounded_text(option.clone(), "composer mode option", 200))
        .collect()
}

pub(super) fn validate(
    modes: &[PluginComposerMode],
    agents: &[PluginAgentSurface],
    ui: &[PluginUiSurface],
) -> UseResult<()> {
    if modes.len() > MAX_COMPOSER_MODES
        || modes
            .iter()
            .map(|mode| &mode.id)
            .collect::<BTreeSet<_>>()
            .len()
            != modes.len()
    {
        return Err(refused("AIX composer modes must be unique and at most 32."));
    }
    for mode in modes {
        if !agents
            .iter()
            .any(|agent| agent.id == mode.agent && !agent.optional)
        {
            return Err(refused(format!(
                "Composer mode '{}' requires a non-optional local agent '{}'.",
                mode.id, mode.agent
            )));
        }
        if let Some(editor) = &mode.editor {
            if !ui.iter().any(|view| {
                view.id == *editor
                    && !view.optional
                    && view.container.is_none()
                    && view.presentation != Some(PluginUiPresentation::Tree)
                    && view.point != Some(PluginUiPoint::Composer)
            }) {
                return Err(refused(format!(
                    "Composer mode '{}' requires a non-optional local editor '{}'.",
                    mode.id, editor
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ExtensionManifest;

    #[test]
    fn slides_mode_names_its_agent_and_editor() {
        let acl = r#"extension "aster/slides" {
  schema_version = 3
  version = "1.0.0"
  route = "slides"
  requires_use = ">=0.3.0, <0.4.0"
  actions = ["read"]
  repository {
    url = "https://github.com/aster-ai-lab/Use-Registry"
    revision = "0000000000000000000000000000000000000000"
  }
  agent "slides" {
    harness = "builtin"
    instructions = "agents/slides/INSTRUCTIONS.md"
    runtime = "edge"
    optional = false
    resources {
      cpu_millis = 500
      memory_bytes = 268435456
    }
  }
  ui "workbench" {
    title = "演示文稿"
    description = "deck"
    icon = "panels"
    entry = "ui/index.html"
    presentation = "webview"
    point = "studio.applets"
    optional = false
  }
  composer_mode "slides" {
    contract_version = 1
    title = "演示文稿"
    description = "theme"
    icon = "panels"
    agent = "slides"
    editor = "workbench"
    scope = "workspace"
    context = ["workspace"]
    order = 20
  }
}
"#;
        let parsed = ExtensionManifest::parse_acl(acl).expect("slides mode");
        assert_eq!(parsed.composer_modes[0].id, "slides");
        assert_eq!(parsed.composer_modes[0].agent, "slides");
        assert_eq!(
            parsed.composer_modes[0].editor.as_deref(),
            Some("workbench")
        );
        assert_eq!(parsed.composer_modes[0].title, "演示文稿");
    }
}
