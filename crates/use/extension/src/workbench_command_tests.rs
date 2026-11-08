#![cfg(test)]

use super::{ExtensionManifest, PluginUiMenuLocation};

const BASE: &str = include_str!("../fixtures/manifests/plugin-v3.acl");
const COMMANDS: &str = r#"
  command "refresh" { title = "Reload" icon = "refresh" enablement { theme = "light" } }
  menu "view.title" { command = "refresh" order = 10 when { locale = "en-US" } }
  menu "view.context" { command = "refresh" order = 20 }
"#;

fn manifest(commands: &str) -> String {
    let closing = BASE.rfind('}').unwrap();
    format!(
        "{} ui \"commands\" {{ entry = \"ui/commands/index.html\" point = \"studio.editor\" {commands} }} }}",
        &BASE[..closing]
    )
}

// Catch command metadata or distinct placement conditions being lost through ACL parsing and receipts.
#[test]
fn aix_view_commands_and_menu_positions_are_typed_and_preserved() {
    let parsed = ExtensionManifest::parse_acl(&manifest(COMMANDS)).unwrap();
    let view = parsed.ui.iter().find(|view| view.id == "commands").unwrap();
    assert_eq!(view.commands[0].id, "refresh");
    assert_eq!(view.commands[0].title, "Reload");
    assert_eq!(
        view.commands[0]
            .enablement
            .as_ref()
            .unwrap()
            .theme
            .unwrap()
            .as_str(),
        "light"
    );
    assert_eq!(view.menus[0].location, PluginUiMenuLocation::ViewTitle);
    assert_eq!(
        view.menus[0].when.as_ref().unwrap().locale.as_deref(),
        Some("en-US")
    );
    assert!(view.menus[1].when.is_none());
    assert_eq!(
        PluginUiMenuLocation::parse("view.context")
            .unwrap()
            .as_str(),
        "view.context"
    );
    let facts = serde_json::to_value(&parsed).unwrap();
    assert_eq!(
        serde_json::from_value::<ExtensionManifest>(facts).unwrap(),
        parsed
    );
    assert!(ExtensionManifest::parse_acl(
        &manifest(COMMANDS).replace("studio.editor", "studio.composer.actions")
    )
    .is_err());
}

// Catch undeclared actions, privileged builtin claims, invalid placement and unbounded declarations being admitted.
#[test]
fn aix_view_commands_refuse_unknown_references_and_unsupported_contracts() {
    for invalid in [
        "command {}",
        "command \"refresh\" {}",
        "command \"refresh\" { title = \"\" }",
        "command \"app.settings.open\" { title = \"Open settings\" }",
        "command \"refresh\" { title = \"Reload\" args = [\"workspace\"] }",
        "command \"refresh\" { title = \"Reload\" when { theme = \"light\" } }",
        "command \"refresh\" { title = \"Reload\" enablement {} }",
        "command \"refresh\" { title = \"Reload\" enablement { all = [\"unknown\"] } }",
        "command \"refresh\" { title = \"Reload\" enablement { theme = \"light\" } enablement { theme = \"dark\" } }",
        "command \"refresh\" { title = \"Reload\" } command \"refresh\" { title = \"Duplicate\" }",
        "menu \"view.title\" { command = \"missing\" }",
        "command \"refresh\" { title = \"Reload\" } menu \"app.menu\" { command = \"refresh\" }",
        "command \"refresh\" { title = \"Reload\" } menu \"view.title\" { command = \"refresh\" unknown = true }",
        "command \"refresh\" { title = \"Reload\" } menu \"view.title\" { command = \"refresh\" order = 2147483648 }",
        "command \"refresh\" { title = \"Reload\" } menu \"view.title\" { command = \"refresh\" } menu \"view.title\" { command = \"refresh\" }",
    ] {
        assert!(
            ExtensionManifest::parse_acl(&manifest(invalid)).is_err(),
            "{invalid}"
        );
    }
    let commands: String = (0..32)
        .map(|i| format!("command \"action-{i}\" {{ title = \"Action\" }}\n"))
        .collect();
    let menus: String = (0..32)
        .flat_map(|i| {
            ["view.title", "view.context"]
                .map(|location| format!("menu \"{location}\" {{ command = \"action-{i}\" }}\n"))
        })
        .collect();
    assert!(ExtensionManifest::parse_acl(&manifest(&format!("{commands}{menus}"))).is_ok());
    assert!(ExtensionManifest::parse_acl(&manifest(&format!(
        "{commands} command \"extra\" {{ title = \"Extra\" }}"
    )))
    .is_err());
    assert!(ExtensionManifest::parse_acl(&manifest(&format!(
        "{commands}{menus} menu \"view.title\" {{ command = \"action-0\" }}"
    )))
    .is_err());
}

fn root_manifest(body: &str) -> String {
    let closing = BASE.rfind('}').unwrap();
    format!("{}\n{body}\n}}", &BASE[..closing])
}

#[test]
fn package_commands_menus_and_keybindings_are_workbench_points() {
    use super::{PluginEditorPriority, PluginUiPoint, PluginWorkbenchMenuLocation};
    let parsed = ExtensionManifest::parse_acl(&root_manifest(
        r#"
        command "refresh" { title = "Reload" category = "Review" icon = "refresh" }
        menu "commandPalette" { command = "refresh" order = 10 }
        menu "editor/context" { command = "refresh" }
        keybinding { command = "refresh" key = "mod+shift+r" mac = "cmd+shift+r" }
        ui "markdown" { entry = "ui/editor/index.html" point = "studio.editor.custom" selector = "*.md" priority = "default" }
        "#,
    ))
    .unwrap();
    assert_eq!(parsed.commands[0].id, "refresh");
    assert_eq!(parsed.commands[0].category.as_deref(), Some("Review"));
    assert_eq!(
        parsed.menus[0].location,
        PluginWorkbenchMenuLocation::CommandPalette
    );
    assert_eq!(parsed.keybindings[0].key, "mod+shift+r");
    let editor = parsed.ui.iter().find(|view| view.id == "markdown").unwrap();
    assert_eq!(editor.point, Some(PluginUiPoint::CustomEditor));
    assert_eq!(editor.selector.as_deref(), Some("*.md"));
    assert_eq!(editor.priority, Some(PluginEditorPriority::Default));
    for invalid in [
        r#"command "open" { title = "Open" } menu "commandPalette" { command = "missing" }"#,
        r#"command "open" { title = "Open" } keybinding { command = "open" key = "shift" }"#,
        r#"ui "bad" { entry = "ui/editor/index.html" point = "studio.editor.custom" }"#,
        r#"ui "bad" { entry = "ui/editor/index.html" point = "studio.editor" selector = "*.md" }"#,
    ] {
        assert!(
            ExtensionManifest::parse_acl(&root_manifest(invalid)).is_err(),
            "{invalid}"
        );
    }
    assert!(ExtensionManifest::parse_acl(&manifest("point = \"studio.commands\"")).is_err());
}
