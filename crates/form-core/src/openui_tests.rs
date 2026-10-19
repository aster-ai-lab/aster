#![cfg(test)]

use super::*;

#[test]
fn compiles_forward_references_and_rejects_the_reported_select_argument_order() {
    // Catches the reported form parking a turn despite an unrenderable Select.
    let prefix = "root = Card(\"Confirm\", [project, go])\ngo = Button(\"submit\", \"Continue\")\n";
    assert!(validate_openui(&format!(
        "{prefix}project = Select(\"project\", \"Project\", \"\", [\"Power\", \"VideoJev\"])"
    ))
    .is_ok());
    for field in [
        "project = Select(\"project\", \"Project\", [\"Power\"])",
        "project = Select(\"project\", \"Project\", \"\", [\"Power\", \"Power\"])",
        "project = Input(\"project\", \"Project\", false)",
        "project = Widget(\"Project\")",
        "project = Stack([root])",
    ] {
        assert!(
            validate_openui(&format!("{prefix}{field}")).is_err(),
            "{field}"
        );
    }
    assert!(validate_openui("root = Card(\"Missing\", [unknown])").is_err());
    assert!(validate_openui(&"x".repeat(65_537)).is_err());
}

#[test]
fn component_table_round_trips_the_closed_set_and_contains_parameter_order() {
    // Catches drift between accepted components, documentation and positional arguments.
    let catalog = openui_component_catalog();
    for name in catalog.as_object().unwrap().keys() {
        assert_eq!(Component::parse(name).unwrap().as_str(), name);
    }
    assert!(Component::parse("Unknown").is_err());
    assert!(openui_parameter_table()
        .contains("Select(name, label, initial, options, required, placeholder)"));
    assert!(openui_parameter_table().contains("Grid(columns, children)"));
}

#[test]
fn generated_renderer_catalogs_and_skill_parameters_match_the_native_source() {
    // Catches the host, renderer, skill and tool description drifting to different argument orders.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for path in [
        "packages/intelligent-ui/src/catalog.generated.json",
        "apps/work/packages/intelligent-ui/src/catalog.generated.json",
    ] {
        let generated: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(path)).unwrap()).unwrap();
        assert_eq!(
            generated,
            openui_component_catalog(),
            "refresh {path} with a3s-form-core openui-catalog"
        );
    }
    let skill =
        std::fs::read_to_string(root.join("apps/work/resources/skills/ask-form/SKILL.md")).unwrap();
    assert!(skill.contains(&openui_parameter_table().replace("; ", "\n")));
}
