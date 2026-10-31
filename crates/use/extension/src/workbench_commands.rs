use std::collections::BTreeSet;

use aster_acl::Block;
use aster_use_core::{UseError, UseResult};
use serde::{Deserialize, Serialize};

use super::{
    bounded_text, manifest_error, optional_i32_attribute, optional_string_attribute,
    require_known_attributes, string_attribute, valid_segment, PluginUiCondition,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginUiCommand {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enablement: Option<PluginUiCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PluginUiMenuLocation {
    #[serde(rename = "view.title")]
    ViewTitle,
    #[serde(rename = "view.context")]
    ViewContext,
}

impl PluginUiMenuLocation {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "view.title" => Ok(Self::ViewTitle),
            "view.context" => Ok(Self::ViewContext),
            _ => Err(UseError::new(
                "use.extension.ui_menu_location_invalid",
                "AIX UI menu location must be view.title or view.context.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ViewTitle => "view.title",
            Self::ViewContext => "view.context",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginUiMenu {
    pub location: PluginUiMenuLocation,
    pub command: String,
    pub order: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<PluginUiCondition>,
}

/// A command the package contributes to the command palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginPackageCommand {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enablement: Option<PluginUiCondition>,
}

/// Workbench menu locations that are not owned by one view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PluginWorkbenchMenuLocation {
    #[serde(rename = "commandPalette")]
    CommandPalette,
    #[serde(rename = "editor/context")]
    EditorContext,
    #[serde(rename = "editor/title")]
    EditorTitle,
}

impl PluginWorkbenchMenuLocation {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "commandPalette" => Ok(Self::CommandPalette),
            "editor/context" => Ok(Self::EditorContext),
            "editor/title" => Ok(Self::EditorTitle),
            _ => Err(UseError::new(
                "use.extension.menu_location_invalid",
                "AIX workbench menu location must be commandPalette, editor/context or editor/title.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandPalette => "commandPalette",
            Self::EditorContext => "editor/context",
            Self::EditorTitle => "editor/title",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginWorkbenchMenu {
    pub location: PluginWorkbenchMenuLocation,
    pub command: String,
    pub order: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<PluginUiCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginHarnessComponent {
    pub id: String,
    pub component: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginKeybinding {
    pub command: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<PluginUiCondition>,
}

pub(super) fn parse_commands(block: &Block) -> UseResult<Vec<PluginUiCommand>> {
    let children: Vec<_> = block
        .blocks
        .iter()
        .filter(|child| child.name == "command")
        .collect();
    if children.len() > 32 {
        return Err(manifest_error("AIX UI may declare at most 32 commands."));
    }
    let mut ids = BTreeSet::new();
    children
        .into_iter()
        .map(|command| {
            let [id] = command.labels.as_slice() else {
                return Err(manifest_error("AIX command requires one local ID."));
            };
            if !valid_segment(id) || !ids.insert(id) {
                return Err(manifest_error(
                    "AIX command IDs must be valid and unique inside the UI surface.",
                ));
            }
            require_known_attributes(command, &["title", "icon"])?;
            if command.blocks.len() > 1
                || command
                    .blocks
                    .iter()
                    .any(|child| child.name != "enablement")
            {
                return Err(manifest_error(
                    "AIX command permits at most one enablement block.",
                ));
            }
            let enablement = command
                .blocks
                .first()
                .map(super::workbench_manifest::parse_condition)
                .transpose()?;
            let icon = optional_string_attribute(command, "icon")?;
            if icon.as_ref().is_some_and(|icon| !valid_segment(icon)) {
                return Err(manifest_error(
                    "AIX command icon must be a lowercase icon identifier.",
                ));
            }
            Ok(PluginUiCommand {
                id: id.clone(),
                title: bounded_text(string_attribute(command, "title")?, "command title", 64)?,
                icon,
                enablement,
            })
        })
        .collect()
}

pub(super) fn parse_menus(
    block: &Block,
    commands: &[PluginUiCommand],
) -> UseResult<Vec<PluginUiMenu>> {
    let children: Vec<_> = block
        .blocks
        .iter()
        .filter(|child| child.name == "menu")
        .collect();
    if children.len() > 64 {
        return Err(manifest_error(
            "AIX UI may declare at most 64 menu entries.",
        ));
    }
    let mut identities = BTreeSet::new();
    children
        .into_iter()
        .map(|menu| {
            let [location] = menu.labels.as_slice() else {
                return Err(manifest_error("AIX menu requires one location label."));
            };
            let location = PluginUiMenuLocation::parse(location)?;
            require_known_attributes(menu, &["command", "order"])?;
            let command = string_attribute(menu, "command")?;
            if !commands.iter().any(|item| item.id == command)
                || !identities.insert((location, command.clone()))
            {
                return Err(manifest_error(
                    "AIX menu requires a unique reference to a command owned by its UI surface.",
                ));
            }
            Ok(PluginUiMenu {
                location,
                command,
                order: optional_i32_attribute(menu, "order")?.unwrap_or(100),
                when: super::workbench_manifest::parse_when(menu)?,
            })
        })
        .collect()
}

pub(super) fn parse_package_command(block: &Block) -> UseResult<PluginPackageCommand> {
    let [id] = block.labels.as_slice() else {
        return Err(manifest_error("AIX command requires one local ID."));
    };
    if !valid_segment(id) {
        return Err(manifest_error(
            "AIX command IDs must be valid and unique inside the package.",
        ));
    }
    require_known_attributes(block, &["title", "category", "icon"])?;
    if block.blocks.len() > 1 || block.blocks.iter().any(|child| child.name != "enablement") {
        return Err(manifest_error(
            "AIX command permits at most one enablement block.",
        ));
    }
    let icon = optional_string_attribute(block, "icon")?;
    if icon.as_ref().is_some_and(|icon| !valid_segment(icon)) {
        return Err(manifest_error(
            "AIX command icon must be a lowercase icon identifier.",
        ));
    }
    let category = optional_string_attribute(block, "category")?
        .map(|value| bounded_text(value, "command category", 64))
        .transpose()?;
    Ok(PluginPackageCommand {
        id: id.clone(),
        title: bounded_text(string_attribute(block, "title")?, "command title", 64)?,
        category,
        icon,
        enablement: block
            .blocks
            .first()
            .map(super::workbench_manifest::parse_condition)
            .transpose()?,
    })
}

pub(super) fn parse_workbench_menu(block: &Block) -> UseResult<PluginWorkbenchMenu> {
    let [location] = block.labels.as_slice() else {
        return Err(manifest_error("AIX menu requires one location label."));
    };
    require_known_attributes(block, &["command", "order"])?;
    Ok(PluginWorkbenchMenu {
        location: PluginWorkbenchMenuLocation::parse(location)?,
        command: string_attribute(block, "command")?,
        order: optional_i32_attribute(block, "order")?.unwrap_or(100),
        when: super::workbench_manifest::parse_when(block)?,
    })
}

pub(super) fn parse_harness(block: &Block) -> UseResult<PluginHarnessComponent> {
    let [id] = block.labels.as_slice() else {
        return Err(manifest_error("AIX harness requires one local ID."));
    };
    if !valid_segment(id) || !block.blocks.is_empty() {
        return Err(manifest_error(
            "AIX harness IDs must be valid and unique inside the package.",
        ));
    }
    require_known_attributes(block, &["component"])?;
    let component = string_attribute(block, "component")?;
    if !valid_segment(&component) {
        return Err(manifest_error(
            "AIX harness component must be a lowercase identifier.",
        ));
    }
    Ok(PluginHarnessComponent {
        id: id.clone(),
        component,
    })
}

pub(super) fn parse_keybinding(block: &Block) -> UseResult<PluginKeybinding> {
    if !block.labels.is_empty() {
        return Err(manifest_error("AIX keybinding does not take a label."));
    }
    require_known_attributes(block, &["command", "key", "mac"])?;
    let key = string_attribute(block, "key")?;
    let mac = optional_string_attribute(block, "mac")?;
    if !valid_chord(&key) || mac.as_ref().is_some_and(|value| !valid_chord(value)) {
        return Err(manifest_error(
            "AIX keybinding must be one chord of modifiers plus a key.",
        ));
    }
    Ok(PluginKeybinding {
        command: string_attribute(block, "command")?,
        key,
        mac,
        when: super::workbench_manifest::parse_when(block)?,
    })
}

fn valid_chord(value: &str) -> bool {
    let mut parts = value.split('+');
    let Some(first) = parts.next() else {
        return false;
    };
    let parts: Vec<&str> = std::iter::once(first).chain(parts).collect();
    if parts.len() < 2 || parts.len() > 4 {
        return false;
    }
    let (key, modifiers) = parts.split_last().unwrap();
    let mut seen = BTreeSet::new();
    modifiers.iter().all(|modifier| {
        matches!(
            *modifier,
            "ctrl" | "shift" | "alt" | "cmd" | "meta" | "win" | "mod"
        ) && seen.insert(*modifier)
    }) && valid_key(key)
}

fn valid_key(value: &str) -> bool {
    matches!(
        value,
        "enter"
            | "escape"
            | "tab"
            | "space"
            | "backspace"
            | "delete"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    ) || (value.len() == 1
        && value
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric()))
}

pub(super) fn validate_palette(
    commands: &[PluginPackageCommand],
    menus: &[PluginWorkbenchMenu],
    keybindings: &[PluginKeybinding],
) -> UseResult<()> {
    if commands.len() > 32 {
        return Err(manifest_error(
            "AIX may declare at most 32 package commands.",
        ));
    }
    if menus.len() > 64 {
        return Err(manifest_error(
            "AIX may declare at most 64 workbench menu entries.",
        ));
    }
    if keybindings.len() > 64 {
        return Err(manifest_error("AIX may declare at most 64 keybindings."));
    }
    let mut ids = BTreeSet::new();
    if commands
        .iter()
        .any(|command| !ids.insert(command.id.as_str()))
    {
        return Err(manifest_error(
            "AIX command IDs must be valid and unique inside the package.",
        ));
    }
    let mut menu_ids = BTreeSet::new();
    if menus.iter().any(|menu| {
        !ids.contains(menu.command.as_str())
            || !menu_ids.insert((menu.location, menu.command.as_str()))
    }) {
        return Err(manifest_error(
            "AIX workbench menu requires a unique reference to a package command.",
        ));
    }
    let mut chords = BTreeSet::new();
    if keybindings.iter().any(|binding| {
        !ids.contains(binding.command.as_str())
            || !chords.insert((binding.command.as_str(), binding.key.as_str()))
    }) {
        return Err(manifest_error(
            "AIX keybinding requires a unique reference to a package command.",
        ));
    }
    Ok(())
}
