use std::collections::BTreeSet;

use aster_acl::Block;
use aster_use_core::{UseError, UseResult};
use serde::{Deserialize, Serialize};

use super::{
    bounded_text, manifest_error, optional_i32_attribute, optional_list_attribute,
    optional_string_attribute, require_known_attributes, string_attribute, valid_segment,
    PluginUiSurface,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginUiContextKey {
    Workspace,
    Document,
    Conversation,
    Run,
    Turn,
    Harness,
}

impl PluginUiContextKey {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "workspace" => Ok(Self::Workspace),
            "document" => Ok(Self::Document),
            "conversation" => Ok(Self::Conversation),
            "run" => Ok(Self::Run),
            "turn" => Ok(Self::Turn),
            "harness" => Ok(Self::Harness),
            _ => Err(UseError::new(
                "use.extension.ui_context_invalid",
                "AIX UI context key must be workspace, document, conversation, run, turn or harness.",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Document => "document",
            Self::Conversation => "conversation",
            Self::Run => "run",
            Self::Turn => "turn",
            Self::Harness => "harness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginUiTheme {
    Light,
    Dark,
}

impl PluginUiTheme {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(UseError::new(
                "use.extension.ui_theme_invalid",
                "AIX UI theme must be light or dark.",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginUiCondition {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all: Vec<PluginUiContextKey>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub none: Vec<PluginUiContextKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<PluginUiTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

pub(super) fn parse_when(block: &Block) -> UseResult<Option<PluginUiCondition>> {
    let permitted = if block.name == "ui" {
        &["when", "command", "menu"][..]
    } else {
        &["when"][..]
    };
    let conditions: Vec<_> = block
        .blocks
        .iter()
        .filter(|child| child.name == "when")
        .collect();
    if conditions.len() > 1
        || block
            .blocks
            .iter()
            .any(|child| !permitted.contains(&child.name.as_str()))
    {
        return Err(manifest_error("AIX UI permits at most one when block."));
    }
    let Some(condition) = conditions.first() else {
        return Ok(None);
    };
    parse_condition(condition).map(Some)
}

pub(super) fn parse_condition(condition: &Block) -> UseResult<PluginUiCondition> {
    if !condition.labels.is_empty() || !condition.blocks.is_empty() {
        return Err(manifest_error(
            "AIX when requires no labels or nested blocks.",
        ));
    }
    require_known_attributes(condition, &["all", "none", "theme", "locale"])?;
    let keys = |name| -> UseResult<Vec<PluginUiContextKey>> {
        let values = optional_list_attribute(condition, name)?;
        if values.len() > 6 {
            return Err(manifest_error(
                "AIX when permits at most six context keys per list.",
            ));
        }
        let parsed = values
            .iter()
            .map(|value| PluginUiContextKey::parse(value))
            .collect::<UseResult<Vec<_>>>()?;
        if parsed.iter().copied().collect::<BTreeSet<_>>().len() != parsed.len() {
            return Err(manifest_error("AIX when context keys must be unique."));
        }
        Ok(parsed)
    };
    let all = keys("all")?;
    let none = keys("none")?;
    if all.iter().any(|key| none.contains(key)) {
        return Err(manifest_error(
            "AIX when context requirements cannot contradict.",
        ));
    }
    let theme = optional_string_attribute(condition, "theme")?
        .map(|value| PluginUiTheme::parse(&value))
        .transpose()?;
    let locale = optional_string_attribute(condition, "locale")?;
    if locale.as_ref().is_some_and(|locale| {
        locale.len() > 64
            || !locale.split('-').all(|part| {
                !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
    }) {
        return Err(manifest_error(
            "AIX when locale must be a bounded language tag.",
        ));
    }
    if all.is_empty() && none.is_empty() && theme.is_none() && locale.is_none() {
        return Err(manifest_error("AIX when requires a condition."));
    }
    Ok(PluginUiCondition {
        all,
        none,
        theme,
        locale,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginViewLocation {
    #[serde(rename = "activitybar")]
    ActivityBar,
    Panel,
}

impl PluginViewLocation {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "activitybar" => Ok(Self::ActivityBar),
            "panel" => Ok(Self::Panel),
            _ => Err(UseError::new(
                "use.extension.view_location_invalid",
                "AIX view location must be activitybar or panel.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActivityBar => "activitybar",
            Self::Panel => "panel",
        }
    }
}

/// Stable native UI contribution contract, independent of the package schema.
pub const STUDIO_CONTRIBUTION_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginUiScope {
    Window,
    Workspace,
    Session,
    Run,
    Turn,
}

impl PluginUiScope {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "window" => Ok(Self::Window),
            "workspace" => Ok(Self::Workspace),
            "session" => Ok(Self::Session),
            "run" => Ok(Self::Run),
            "turn" => Ok(Self::Turn),
            _ => Err(UseError::new(
                "use.extension.ui_scope_invalid",
                "AIX UI scope must be window, workspace, session, run or turn.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Workspace => "workspace",
            Self::Session => "session",
            Self::Run => "run",
            Self::Turn => "turn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginUiPresentation {
    Webview,
    Tree,
}

impl PluginUiPresentation {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "webview" => Ok(Self::Webview),
            "tree" => Ok(Self::Tree),
            _ => Err(UseError::new(
                "use.extension.ui_presentation_invalid",
                "AIX UI presentation must be webview or tree.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webview => "webview",
            Self::Tree => "tree",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginUiPoint {
    #[serde(rename = "studio.editor")]
    Editor,
    #[serde(rename = "studio.workspace")]
    Workspace,
    #[serde(rename = "studio.applets")]
    Applets,
    #[serde(rename = "studio.composer.actions")]
    Composer,
    #[serde(rename = "studio.editor.custom")]
    CustomEditor,
}

impl PluginUiPoint {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "studio.editor" => Ok(Self::Editor),
            "studio.workspace" => Ok(Self::Workspace),
            "studio.applets" => Ok(Self::Applets),
            "studio.composer.actions" => Ok(Self::Composer),
            "studio.editor.custom" => Ok(Self::CustomEditor),
            _ => Err(UseError::new(
                "use.extension.ui_point_invalid",
                "AIX UI point must be studio.editor, studio.workspace, studio.applets, studio.composer.actions or studio.editor.custom.",
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Editor => "studio.editor",
            Self::Workspace => "studio.workspace",
            Self::Applets => "studio.applets",
            Self::Composer => "studio.composer.actions",
            Self::CustomEditor => "studio.editor.custom",
        }
    }
}

/// How a custom editor competes when several packages match one filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginEditorPriority {
    Default,
    Option,
}

impl PluginEditorPriority {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "default" => Ok(Self::Default),
            "option" => Ok(Self::Option),
            _ => Err(UseError::new(
                "use.extension.editor_priority_invalid",
                "AIX custom editor priority must be default or option.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Option => "option",
        }
    }
}

pub(super) fn valid_filename_pattern(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() || chars.len() > 128 || value.starts_with('/') {
        return false;
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return false;
    }
    chars
        .iter()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '*' | '?' | '.' | '-' | '_' | '/'))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginViewContainer {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub order: i32,
    pub location: PluginViewLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<PluginUiCondition>,
}

/// Closed namespace for package-local Studio contribution identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginUiContributionKind {
    ViewContainer,
    View,
}

impl PluginUiContributionKind {
    pub fn parse(value: &str) -> UseResult<Self> {
        match value {
            "view-container" => Ok(Self::ViewContainer),
            "view" => Ok(Self::View),
            _ => Err(UseError::new(
                "use.extension.ui_contribution_kind_invalid",
                "AIX UI contribution kind must be view-container or view.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ViewContainer => "view-container",
            Self::View => "view",
        }
    }
}

/// Fully scoped identity for a Studio UI contribution declared by an AIX package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginUiContributionIdentity {
    pub package_id: String,
    pub kind: PluginUiContributionKind,
    pub local_id: String,
}

impl PluginUiContributionIdentity {
    pub fn new(
        package_id: impl Into<String>,
        kind: PluginUiContributionKind,
        local_id: impl Into<String>,
    ) -> UseResult<Self> {
        let package_id = package_id.into();
        let local_id = local_id.into();
        if !super::valid_package_id(&package_id) || !valid_segment(&local_id) {
            return Err(manifest_error(
                "AIX UI contribution identity requires a valid package ID and local ID.",
            ));
        }
        Ok(Self {
            package_id,
            kind,
            local_id,
        })
    }
}

pub(super) fn parse_container(block: &Block) -> UseResult<PluginViewContainer> {
    let [id] = block.labels.as_slice() else {
        return Err(manifest_error("AIX view_container requires one local ID."));
    };
    if !valid_segment(id) {
        return Err(manifest_error(
            "AIX view_container requires a valid local ID.",
        ));
    }
    let when = parse_when(block)?;
    require_known_attributes(block, &["title", "icon", "order", "location"])?;
    let title = bounded_text(
        string_attribute(block, "title")?,
        "view container title",
        64,
    )?;
    let icon = string_attribute(block, "icon")?;
    if !valid_segment(&icon) {
        return Err(manifest_error(
            "AIX view container icon must be a lowercase icon identifier.",
        ));
    }
    Ok(PluginViewContainer {
        id: id.clone(),
        title,
        icon,
        order: optional_i32_attribute(block, "order")?.unwrap_or(100),
        location: PluginViewLocation::parse(&string_attribute(block, "location")?)?,
        when,
    })
}

pub(super) fn validate(
    package_id: &str,
    containers: &[PluginViewContainer],
    ui: &[PluginUiSurface],
) -> UseResult<()> {
    if containers.len() > 32 {
        return Err(manifest_error(
            "AIX may declare at most 32 view containers.",
        ));
    }
    let mut identities = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for container in containers {
        let identity = PluginUiContributionIdentity::new(
            package_id,
            PluginUiContributionKind::ViewContainer,
            container.id.clone(),
        )?;
        if !identities.insert(identity) || !ids.insert(container.id.as_str()) {
            return Err(manifest_error(
                "AIX view container IDs must be unique inside the package.",
            ));
        }
        if !ui
            .iter()
            .any(|view| view.container.as_deref() == Some(container.id.as_str()))
        {
            return Err(manifest_error(
                "AIX view containers require an owned UI view.",
            ));
        }
    }
    for view in ui {
        if !identities.insert(PluginUiContributionIdentity::new(
            package_id,
            PluginUiContributionKind::View,
            view.id.clone(),
        )?) {
            return Err(manifest_error(
                "AIX UI view IDs must be unique inside the package.",
            ));
        }
    }
    for view in ui {
        let point = if let Some(container) = &view.container {
            match containers
                .iter()
                .find(|item| &item.id == container)
                .map(|item| item.location)
            {
                Some(PluginViewLocation::ActivityBar) => crate::StudioExtensionPoint::Sidebar,
                Some(PluginViewLocation::Panel) => crate::StudioExtensionPoint::Panel,
                None => return Err(manifest_error("AIX UI requires an owned view container.")),
            }
        } else {
            crate::StudioExtensionPoint::parse(
                view.point.unwrap_or(PluginUiPoint::Editor).as_str(),
            )?
        };
        let contract = point.contract();
        if view
            .contract_version
            .is_some_and(|version| version != contract.contract_version)
            || view
                .mode
                .is_some_and(|mode| !contract.modes.contains(&mode))
        {
            return Err(manifest_error(
                "Unsupported Studio extension point contract or composition mode.",
            ));
        }
        if view.scope.is_some() && view.point != Some(PluginUiPoint::Composer) {
            return Err(manifest_error(
                "AIX UI scope currently requires studio.composer.actions.",
            ));
        }
        if view.presentation == Some(PluginUiPresentation::Tree) && view.container.is_none() {
            return Err(manifest_error(
                "AIX tree UI requires an owned view container.",
            ));
        }
        if view.point == Some(PluginUiPoint::CustomEditor) {
            if view.container.is_some()
                || view
                    .selector
                    .as_ref()
                    .is_none_or(|pattern| !valid_filename_pattern(pattern))
                || view.presentation == Some(PluginUiPresentation::Tree)
            {
                return Err(manifest_error(
                    "AIX custom editor requires a filename pattern and cannot use a view container or tree.",
                ));
            }
        } else if view.selector.is_some() || view.priority.is_some() {
            return Err(manifest_error(
                "AIX filename pattern and editor priority require studio.editor.custom.",
            ));
        }
        if view.point.is_some() && view.container.is_some() {
            return Err(manifest_error(
                "AIX UI must select a point or a container, never both.",
            ));
        }
        if let Some(container) = &view.container {
            if !valid_segment(container) || !ids.contains(container.as_str()) {
                return Err(manifest_error(
                    "AIX UI container must reference this package's declared view container.",
                ));
            }
        }
    }
    Ok(())
}
