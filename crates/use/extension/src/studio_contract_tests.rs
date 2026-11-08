#![cfg(test)]

use super::*;
use crate::{ExtensionManifest, PluginUiPoint};

#[test]
fn declared_points_require_their_native_declaration_and_available_scope() {
    let contracts = studio_extension_point_contracts();
    for point in [
        PluginUiPoint::Editor,
        PluginUiPoint::Workspace,
        PluginUiPoint::Applets,
        PluginUiPoint::Composer,
        PluginUiPoint::CustomEditor,
    ] {
        let canonical = StudioExtensionPoint::parse(point.as_str()).unwrap();
        assert_eq!(
            canonical.contract().declaration,
            Some(StudioDeclaration::Ui)
        );
    }
    assert_eq!(
        contracts["sidebar"].declaration,
        Some(StudioDeclaration::ViewContainer)
    );
    assert!(PluginUiPoint::parse(contracts["sidebar"].id.as_str()).is_err());
    assert_eq!(
        contracts["composerMode"].declaration,
        Some(StudioDeclaration::ComposerMode)
    );
    assert_eq!(contracts["composerMode"].composition.as_str(), "collection");
    assert_eq!(
        contracts["composerMode"].scopes,
        [PluginUiScope::Workspace, PluginUiScope::Session]
    );
    assert!(contracts["harness"].declaration.is_none());
    for slot in ["command", "menu", "keybinding"] {
        assert!(contracts[slot].declaration.is_none());
        assert_eq!(contracts[slot].composition.as_str(), "collection");
        assert!(PluginUiPoint::parse(contracts[slot].id.as_str()).is_err());
    }
    assert_eq!(
        contracts["customEditor"].id.as_str(),
        "studio.editor.custom"
    );
    assert_eq!(contracts["customEditor"].composition.as_str(), "collection");
    assert!(!contracts["composer"]
        .scopes
        .iter()
        .any(|scope| scope.as_str() == "subtask"));
    assert!(StudioExtensionPoint::parse("studio.unknown").is_err());
    let base = include_str!("../fixtures/manifests/plugin-v3.acl");
    let closing = base.rfind('}').unwrap();
    let source = |attributes: &str| {
        format!(
            "{} ui \"contract-proof\" {{ entry = \"ui/main.html\" {attributes} }} }}",
            &base[..closing]
        )
    };
    let admitted =
        ExtensionManifest::parse_acl(&source("contract_version = 1 mode = \"append\"")).unwrap();
    let view = admitted
        .ui
        .iter()
        .find(|view| view.id == "contract-proof")
        .unwrap();
    assert_eq!(view.contract_version, Some(1));
    assert_eq!(view.mode, Some(StudioCompositionMode::Append));
    for invalid in [
        "point = \"studio.harness.components\"",
        "contract_version = 0",
        "contract_version = 2",
        "contract_version = -1",
        "contract_version = 1.5",
        "mode = \"replace\"",
        "mode = \"decorate\"",
        "target = \"foreign.workspace\"",
    ] {
        assert!(
            ExtensionManifest::parse_acl(&source(invalid)).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn studio_contract_enums_round_trip_and_reject_unknown_values() {
    for point in StudioExtensionPoint::ALL {
        assert_eq!(StudioExtensionPoint::parse(point.as_str()).unwrap(), point);
    }
    for (value, expected) in [
        ("ui", StudioDeclaration::Ui),
        ("view_container", StudioDeclaration::ViewContainer),
    ] {
        assert_eq!(StudioDeclaration::parse(value).unwrap(), expected);
        assert_eq!(expected.as_str(), value);
    }
    for (value, expected) in [
        ("keyed", StudioComposition::Keyed),
        ("collection", StudioComposition::Collection),
    ] {
        assert_eq!(StudioComposition::parse(value).unwrap(), expected);
        assert_eq!(expected.as_str(), value);
    }
    assert_eq!(
        StudioCompositionMode::parse("append").unwrap().as_str(),
        "append"
    );
    assert_eq!(StudioFallback::parse("host").unwrap().as_str(), "host");
    assert_eq!(StudioStatePolicy::parse("owner").unwrap().as_str(), "owner");
    assert!(StudioExtensionPoint::parse("studio.future").is_err());
    assert!(StudioDeclaration::parse("native").is_err());
    assert!(StudioComposition::parse("replace").is_err());
    assert!(StudioCompositionMode::parse("decorate").is_err());
    assert!(StudioFallback::parse("none").is_err());
    assert!(StudioStatePolicy::parse("shared").is_err());
}
