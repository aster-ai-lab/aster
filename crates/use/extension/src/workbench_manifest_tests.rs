#![cfg(test)]

use super::{ExtensionManifest, PluginViewLocation};

#[test]
fn ui_contribution_identity_is_typed_package_scoped_and_round_trips() {
    use super::{PluginUiContributionIdentity as Identity, PluginUiContributionKind as Kind};

    assert_eq!(Kind::parse("view-container").unwrap(), Kind::ViewContainer);
    assert_eq!(Kind::View.as_str(), "view");
    assert_eq!(
        Kind::parse("widget").unwrap_err().code,
        "use.extension.ui_contribution_kind_invalid"
    );

    let container = Identity::new("acme/research", Kind::ViewContainer, "connections").unwrap();
    let view = Identity::new("acme/research", Kind::View, "connections").unwrap();
    let other_package = Identity::new("acme/other", Kind::View, "connections").unwrap();
    assert_ne!(container, view);
    assert_ne!(view, other_package);
    assert_eq!(
        serde_json::from_value::<Identity>(serde_json::to_value(&view).unwrap()).unwrap(),
        view
    );
    assert!(Identity::new("invalid", Kind::View, "connections").is_err());
    assert!(Identity::new("acme/research", Kind::View, "Bad-ID").is_err());
}

#[test]
fn duplicate_view_ids_remain_rejected_by_contribution_identity_validation() {
    let duplicate = VIEWS.replace(
        "ui \"preview\" {",
        "ui \"connections\" { title = \"Duplicate\" entry = \"ui/duplicate/index.html\" container = \"resources\" }\n  ui \"preview\" {",
    );
    assert!(ExtensionManifest::parse_acl(&manifest(&duplicate)).is_err());
}

const BASE: &str = include_str!("../fixtures/manifests/plugin-v3.acl");

// Catch dropped conditions and admission of unknown, contradictory or unbounded context requirements.
#[test]
fn aix_visibility_conditions_are_typed_bounded_and_preserved() {
    let blocks = VIEWS
        .replace("location = \"activitybar\"", "location = \"activitybar\" when { theme = \"dark\" }")
        .replace("title = \"Connections\"", "title = \"Connections\" when { all = [\"workspace\", \"conversation\"] none = [\"run\"] locale = \"en-US\" }");
    let parsed = ExtensionManifest::parse_acl(&manifest(&blocks)).unwrap();
    let facts = serde_json::to_value(&parsed).unwrap();
    assert_eq!(facts["viewContainers"][0]["when"]["theme"], "dark");
    assert_eq!(
        facts["ui"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "connections")
            .unwrap()["when"]["all"],
        serde_json::json!(["workspace", "conversation"])
    );
    assert_eq!(
        serde_json::from_value::<ExtensionManifest>(facts).unwrap(),
        parsed
    );
    for invalid in [
        "when {}",
        "when { all = [\"unknown\"] }",
        "when { all = [\"run\", \"run\"] }",
        "when { all = [\"run\"] none = [\"run\"] }",
        "when { all = \"run\" }",
        "when { theme = \"system\" }",
        "when { locale = \"../en\" }",
        "when { locale = \"\" }",
        "when { extra = true }",
        "when \"foreign\" { theme = \"dark\" }",
        "when { nested {} }",
        "when { theme = \"dark\" } when { theme = \"light\" }",
    ] {
        let source = VIEWS.replace(
            "title = \"Connections\"",
            &format!("title = \"Connections\" {invalid}"),
        );
        assert!(
            ExtensionManifest::parse_acl(&manifest(&source)).is_err(),
            "{invalid}"
        );
    }
    assert!(ExtensionManifest::parse_acl(&manifest(&VIEWS.replace(
        "location = \"panel\"",
        "location = \"panel\" when { all = [\"unknown\"] }"
    )))
    .is_err());
    assert!(ExtensionManifest::parse_acl(&manifest(&format!(
        "ui \"long-locale\" {{ entry = \"ui/index.html\" when {{ locale = \"{}\" }} }}",
        "a".repeat(65)
    )))
    .is_err());
}

fn manifest(blocks: &str) -> String {
    let closing = BASE.rfind('}').unwrap();
    format!("{}\n{blocks}\n}}", &BASE[..closing])
}

const VIEWS: &str = r#"
  view_container "resources" {
    title = "Resources"
    icon = "database"
    order = 20
    location = "activitybar"
  }
  ui "connections" {
    title = "Connections"
    entry = "ui/connections/index.html"
    container = "resources"
  }
  ui "preview" {
    title = "Preview"
    entry = "ui/preview/index.html"
    container = "resources"
  }
  view_container "results" {
    title = "Results"
    icon = "table"
    location = "panel"
  }
  ui "history" {
    entry = "ui/history/index.html"
    container = "results"
  }
"#;

// Catch an unknown renderer or an unowned tree provider being admitted from ACL.
#[test]
fn aix_tree_presentation_is_closed_and_requires_an_owned_container() {
    use super::PluginUiPresentation;
    for (name, kind) in [
        ("tree", PluginUiPresentation::Tree),
        ("webview", PluginUiPresentation::Webview),
    ] {
        let blocks = VIEWS.replace(
            "title = \"Connections\"",
            &format!("title = \"Connections\" presentation = \"{name}\""),
        );
        let parsed = ExtensionManifest::parse_acl(&manifest(&blocks)).unwrap();
        let view = parsed
            .ui
            .iter()
            .find(|view| view.id == "connections")
            .unwrap();
        assert_eq!(view.presentation, Some(kind));
        assert_eq!(PluginUiPresentation::parse(kind.as_str()).unwrap(), kind);
        assert_eq!(serde_json::to_value(view).unwrap()["presentation"], name);
        assert_eq!(
            serde_json::from_value::<super::PluginUiSurface>(serde_json::to_value(view).unwrap())
                .unwrap(),
            *view
        );
    }
    for block in [
        r#"ui "unowned-tree" { entry = "ui/tree/index.html" presentation = "tree" }"#,
        r#"ui "point-tree" { entry = "ui/tree/index.html" point = "studio.applets" presentation = "tree" }"#,
        r#"ui "composer-tree" { entry = "ui/tree/index.html" point = "studio.composer.actions" presentation = "tree" }"#,
    ] {
        assert!(ExtensionManifest::parse_acl(&manifest(block)).is_err());
    }
    assert_eq!(
        PluginUiPresentation::parse("foreign").unwrap_err().code,
        "use.extension.ui_presentation_invalid"
    );
    assert!(ExtensionManifest::parse_acl(&manifest(&VIEWS.replace(
        "title = \"Connections\"",
        "title = \"Connections\" presentation = \"foreign\""
    )))
    .is_err());
}

#[test]
fn aix_acl_groups_owned_views_in_activitybar_and_panel_containers() {
    let parsed = ExtensionManifest::parse_acl(&manifest(VIEWS)).unwrap();
    assert_eq!(parsed.view_containers.len(), 2);
    assert_eq!(
        parsed.view_containers[0].location,
        PluginViewLocation::ActivityBar
    );
    assert_eq!(parsed.view_containers[0].title, "Resources");
    assert_eq!(parsed.view_containers[0].order, 20);
    assert_eq!(parsed.view_containers[1].location.as_str(), "panel");
    let views: Vec<_> = parsed
        .ui
        .iter()
        .filter(|view| view.container.as_deref() == Some("resources"))
        .map(|view| view.id.as_str())
        .collect();
    assert_eq!(views, ["connections", "preview"]);
    let facts = serde_json::to_value(&parsed).unwrap();
    assert_eq!(facts["viewContainers"][0]["location"], "activitybar");
    let restored: ExtensionManifest = serde_json::from_value(facts).unwrap();
    assert_eq!(restored, parsed);
}

#[test]
fn aix_view_declarations_fail_closed_on_missing_placement_and_foreign_container_binding() {
    for blocks in [
        VIEWS.replace("location = \"activitybar\"", ""),
        VIEWS.replace("location = \"activitybar\"", "location = \"floating\""),
        VIEWS.replace(
            "container = \"resources\"",
            "container = \"other-package/resources\"",
        ),
        VIEWS.replace("container = \"resources\"", "container = \"undeclared\""),
        VIEWS.replace("view_container \"results\"", "view_container \"resources\""),
        VIEWS.replace("container = \"results\"", ""),
        VIEWS.replace(
            "location = \"panel\"",
            "location = \"panel\"\n unexpected = true",
        ),
    ] {
        assert!(ExtensionManifest::parse_acl(&manifest(&blocks)).is_err());
    }
}

#[test]
fn aix_limits_container_count_and_preserves_existing_unplaced_ui_metadata() {
    let declarations = (0..33).map(|id| format!(r#"
        view_container "container-{id}" {{ title = "Container" icon = "package" location = "activitybar" }}
        ui "view-{id}" {{ entry = "ui/view-{id}/index.html" container = "container-{id}" }}
    "#)).collect::<String>();
    assert!(ExtensionManifest::parse_acl(&manifest(&declarations))
        .unwrap_err()
        .message
        .contains("32 view containers"));
    let legacy = ExtensionManifest::parse_acl(BASE).unwrap();
    assert!(legacy.view_containers.is_empty());
    assert!(legacy.ui.iter().all(|view| view.container.is_none()));
    let facts = serde_json::to_value(legacy).unwrap();
    assert!(facts.get("viewContainers").is_none());
    assert!(facts["ui"][0].get("container").is_none());
}

#[test]
fn aix_ui_points_are_explicit_versioned_and_do_not_mix_with_container_placement() {
    use super::{PluginUiPoint, STUDIO_CONTRIBUTION_PROTOCOL_VERSION};
    assert_eq!(STUDIO_CONTRIBUTION_PROTOCOL_VERSION, 1);
    for (value, point) in [
        ("studio.editor", PluginUiPoint::Editor),
        ("studio.workspace", PluginUiPoint::Workspace),
        ("studio.applets", PluginUiPoint::Applets),
        ("studio.composer.actions", PluginUiPoint::Composer),
    ] {
        let parsed = ExtensionManifest::parse_acl(&manifest(&format!(
            r#"ui "point-view" {{ entry = "ui/point/index.html" point = "{value}" }}"#
        )))
        .unwrap();
        let view = parsed
            .ui
            .iter()
            .find(|view| view.id == "point-view")
            .unwrap();
        assert_eq!(view.point, Some(point));
        assert_eq!(PluginUiPoint::parse(point.as_str()).unwrap(), point);
        let facts = serde_json::to_value(view).unwrap();
        assert_eq!(facts["point"], value);
    }
    assert_eq!(
        PluginUiPoint::parse("studio.unknown").unwrap_err().code,
        "use.extension.ui_point_invalid"
    );
    assert!(ExtensionManifest::parse_acl(&manifest(&VIEWS.replace(
        "container = \"resources\"",
        "container = \"resources\" point = \"studio.applets\""
    )))
    .is_err());
}

// Catch silently widened ownership or a temporal declaration with no supported consumer.
#[test]
fn aix_composer_scope_is_closed_and_does_not_grant_other_points_temporal_ownership() {
    use super::PluginUiScope;

    for (name, scope) in [
        ("window", PluginUiScope::Window),
        ("workspace", PluginUiScope::Workspace),
        ("session", PluginUiScope::Session),
        ("run", PluginUiScope::Run),
        ("turn", PluginUiScope::Turn),
    ] {
        let parsed = ExtensionManifest::parse_acl(&manifest(&format!(
            r#"ui "action" {{ entry = "ui/action/index.html" point = "studio.composer.actions" scope = "{name}" }}"#
        )))
        .unwrap();
        let view = parsed.ui.iter().find(|view| view.id == "action").unwrap();
        assert_eq!(view.scope, Some(scope));
        assert_eq!(PluginUiScope::parse(scope.as_str()).unwrap(), scope);
        let facts = serde_json::to_value(view).unwrap();
        assert_eq!(facts["scope"], name);
        assert_eq!(
            serde_json::from_value::<super::PluginUiSurface>(facts).unwrap(),
            *view
        );
    }
    for declaration in [
        r#"point = "studio.composer.actions" scope = "subtask""#,
        r#"point = "studio.composer.actions" scope = """#,
        r#"point = "studio.editor" scope = "session""#,
        r#"point = "studio.workspace" scope = "workspace""#,
        r#"scope = "session""#,
    ] {
        assert!(ExtensionManifest::parse_acl(&manifest(&format!(
            r#"ui "action" {{ entry = "ui/action/index.html" {declaration} }}"#
        )))
        .is_err());
    }
    assert!(ExtensionManifest::parse_acl(&manifest(&VIEWS.replace(
        "container = \"resources\"",
        "container = \"resources\" scope = \"session\""
    )))
    .is_err());
    assert_eq!(
        PluginUiScope::parse("subtask").unwrap_err().code,
        "use.extension.ui_scope_invalid"
    );
    assert!(ExtensionManifest::parse_acl(BASE)
        .unwrap()
        .ui
        .iter()
        .all(|view| view.scope.is_none()));
}
