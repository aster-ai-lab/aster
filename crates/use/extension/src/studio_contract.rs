use aster_use_core::{UseError, UseResult};
use serde::{Deserialize, Serialize};

use crate::{PluginUiPresentation, PluginUiScope};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StudioExtensionPoint {
    #[serde(rename = "studio.activitybar")]
    ActivityBar,
    #[serde(rename = "studio.editor")]
    Editor,
    #[serde(rename = "studio.sidebar")]
    Sidebar,
    #[serde(rename = "studio.panel")]
    Panel,
    #[serde(rename = "studio.workspace")]
    Workspace,
    #[serde(rename = "studio.applets")]
    Applets,
    #[serde(rename = "studio.composer.actions")]
    Composer,
    #[serde(rename = "studio.composer.modes")]
    ComposerModes,
    #[serde(rename = "studio.harness.components")]
    Harness,
    /// Command palette entries. Menus and keybindings reference these ids.
    #[serde(rename = "studio.commands")]
    Commands,
    /// Workbench menus outside a single view: command palette and editor menus.
    #[serde(rename = "studio.menus")]
    Menus,
    /// Keybindings for commands this package owns.
    #[serde(rename = "studio.keybindings")]
    Keybindings,
    /// Webview editor selected by a filename pattern.
    #[serde(rename = "studio.editor.custom")]
    CustomEditor,
    /// Home shelves: writing, creation, and document tools.
    #[serde(rename = "studio.home")]
    Home,
}

impl StudioExtensionPoint {
    pub const ALL: [Self; 14] = [
        Self::ActivityBar,
        Self::Editor,
        Self::Sidebar,
        Self::Panel,
        Self::Workspace,
        Self::Applets,
        Self::Composer,
        Self::ComposerModes,
        Self::Harness,
        Self::Commands,
        Self::Menus,
        Self::Keybindings,
        Self::CustomEditor,
        Self::Home,
    ];

    pub fn parse(value: &str) -> UseResult<Self> {
        Self::ALL
            .into_iter()
            .find(|point| point.as_str() == value)
            .ok_or_else(|| {
                UseError::new(
                    "use.extension.studio_point_invalid",
                    "Unknown Studio extension point.",
                )
            })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActivityBar => "studio.activitybar",
            Self::Editor => "studio.editor",
            Self::Sidebar => "studio.sidebar",
            Self::Panel => "studio.panel",
            Self::Workspace => "studio.workspace",
            Self::Applets => "studio.applets",
            Self::Composer => "studio.composer.actions",
            Self::ComposerModes => "studio.composer.modes",
            Self::Harness => "studio.harness.components",
            Self::Commands => "studio.commands",
            Self::Menus => "studio.menus",
            Self::Keybindings => "studio.keybindings",
            Self::CustomEditor => "studio.editor.custom",
            Self::Home => "studio.home",
        }
    }

    pub fn slot(self) -> &'static str {
        match self {
            Self::ActivityBar => "activity",
            Self::Editor => "page",
            Self::Sidebar => "sidebar",
            Self::Panel => "panel",
            Self::Workspace => "workspace",
            Self::Applets => "applet",
            Self::Composer => "composer",
            Self::ComposerModes => "composerMode",
            Self::Harness => "harness",
            Self::Commands => "command",
            Self::Menus => "menu",
            Self::Keybindings => "keybinding",
            Self::CustomEditor => "customEditor",
            Self::Home => "home",
        }
    }

    pub fn contract(self) -> StudioExtensionPointContract {
        use StudioExtensionPoint::*;
        let declaration = match self {
            ActivityBar | Sidebar | Panel => Some(StudioDeclaration::ViewContainer),
            ComposerModes => Some(StudioDeclaration::ComposerMode),
            Harness | Commands | Menus | Keybindings | Home => None,
            _ => Some(StudioDeclaration::Ui),
        };
        let presentations = match self {
            Sidebar | Panel => vec![PluginUiPresentation::Webview, PluginUiPresentation::Tree],
            Editor | Workspace | Applets | Composer | CustomEditor => {
                vec![PluginUiPresentation::Webview]
            }
            _ => vec![],
        };
        let scopes = match self {
            Composer => vec![
                PluginUiScope::Window,
                PluginUiScope::Workspace,
                PluginUiScope::Session,
                PluginUiScope::Run,
                PluginUiScope::Turn,
            ],
            Workspace => vec![PluginUiScope::Workspace],
            ComposerModes => vec![PluginUiScope::Workspace, PluginUiScope::Session],
            Harness => vec![],
            _ => vec![PluginUiScope::Window],
        };
        StudioExtensionPointContract {
            id: self,
            contract_version: 1,
            declaration,
            presentations,
            scopes,
            composition: if matches!(
                self,
                Composer
                    | ComposerModes
                    | Harness
                    | Commands
                    | Menus
                    | Keybindings
                    | CustomEditor
                    | Home
            ) {
                StudioComposition::Collection
            } else {
                StudioComposition::Keyed
            },
            modes: vec![StudioCompositionMode::Append],
            targets: vec![],
            fallback: StudioFallback::Host,
            state_policy: StudioStatePolicy::Owner,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StudioDeclaration {
    Ui,
    ViewContainer,
    ComposerMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StudioComposition {
    Keyed,
    Collection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StudioCompositionMode {
    Append,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StudioFallback {
    Host,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StudioStatePolicy {
    Owner,
}

macro_rules! closed_contract_set {
    ($kind:ident, $( $variant:ident => $value:literal ),+ $(,)?) => {
        impl $kind {
            pub fn parse(value: &str) -> UseResult<Self> {
                match value {
                    $( $value => Ok(Self::$variant), )+
                    _ => Err(UseError::new("use.extension.studio_contract_invalid",
                        concat!("Unknown ", stringify!($kind), " value."))),
                }
            }
            pub fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $value, )+ }
            }
        }
    };
}

closed_contract_set!(StudioDeclaration, Ui => "ui", ViewContainer => "view_container", ComposerMode => "composer_mode");
closed_contract_set!(StudioComposition, Keyed => "keyed", Collection => "collection");
closed_contract_set!(StudioCompositionMode, Append => "append");
closed_contract_set!(StudioFallback, Host => "host");
closed_contract_set!(StudioStatePolicy, Owner => "owner");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioExtensionPointContract {
    pub id: StudioExtensionPoint,
    pub contract_version: u32,
    pub declaration: Option<StudioDeclaration>,
    pub presentations: Vec<PluginUiPresentation>,
    pub scopes: Vec<PluginUiScope>,
    pub composition: StudioComposition,
    pub modes: Vec<StudioCompositionMode>,
    pub targets: Vec<&'static str>,
    pub fallback: StudioFallback,
    pub state_policy: StudioStatePolicy,
}

pub fn studio_extension_point_contracts(
) -> std::collections::BTreeMap<&'static str, StudioExtensionPointContract> {
    StudioExtensionPoint::ALL
        .into_iter()
        .map(|point| (point.slot(), point.contract()))
        .collect()
}

#[cfg(test)]
#[path = "studio_contract_tests.rs"]
mod tests;
