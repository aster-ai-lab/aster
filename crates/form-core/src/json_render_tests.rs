#![cfg(test)]
use super::*;

const FORM: &str = include_str!("../../../packages/intelligent-ui/fixtures/form.json");

#[test]
fn shared_fixture_keeps_typed_defaults_order_and_schema() {
    let document = parse_json_render(FORM).unwrap();
    assert_eq!(
        document
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["name", "amount", "ready", "choice", "notes"]
    );
    assert_eq!(document.fields[1].initial.as_f64(), Some(3.0));
    assert!(document.fields[0].required);
    assert_eq!(document.fields[3].schema["enum"], json!(["alpha", "beta"]));
    assert_eq!(document.text, ["Details"]);
}

#[test]
fn refuses_unknown_properties_bindings_cycles_and_invalid_values() {
    let original: Value = serde_json::from_str(FORM).unwrap();
    for (pointer, value) in [
        ("/elements/name/props/onChange", json!("fetch()")),
        (
            "/elements/name/props/value/$bindState",
            json!("/secret/token"),
        ),
        (
            "/elements/name/props/value/$bindState",
            json!("/form/constructor"),
        ),
        (
            "/elements/name/props/value/$bindState",
            json!("/form/amount"),
        ),
        ("/elements/root/children", json!(["root"])),
        ("/elements/root/children", json!(["missing"])),
        ("/elements/submit/props/action", json!("shell")),
        ("/elements/choice/props/options", json!(["beta", "beta"])),
        ("/elements/amount/props/min", json!(9)),
        ("/state/form/amount", json!("wrong")),
        ("/state/form/name", json!(null)),
        ("/state/form", json!(null)),
        ("/state", json!(null)),
        ("/elements/root/children", json!(null)),
    ] {
        let mut candidate = original.clone();
        if let Some(target) = candidate.pointer_mut(pointer) {
            *target = value;
        } else {
            candidate["elements"]["name"]["props"]["onChange"] = value;
        }
        assert!(
            parse_json_render(&candidate.to_string()).is_err(),
            "{pointer}"
        );
    }
    for source in [
        "root = Input(\"name\", \"Name\")",
        "<Card />",
        "{\"root\":\"a\",\"root\":\"b\"}",
    ] {
        assert!(parse_json_render(source).is_err());
    }
    let duplicate = FORM.replacen(
        "\"root\": \"root\"",
        "\"root\":\"root\",\"root\":\"root\"",
        1,
    );
    assert!(parse_json_render(&duplicate).is_err());
    assert!(parse_json_render(&"x".repeat(MAX_JSON_RENDER_BYTES + 1)).is_err());
}

#[test]
fn generated_renderer_catalog_matches_the_native_contract() {
    let catalog: Value = serde_json::from_str(include_str!(
        "../../../packages/intelligent-ui/src/catalog.generated.json"
    ))
    .unwrap();
    assert_eq!(catalog, json_render_component_catalog());
}

#[test]
fn shared_display_fixture_validates_layout_slider_metric_and_chart() {
    let document = parse_json_render(include_str!(
        "../../../packages/intelligent-ui/fixtures/display.json"
    ))
    .unwrap();
    assert_eq!(document.fields.len(), 1);
    assert_eq!(document.fields[0].name, "days");
    assert_eq!(document.fields[0].schema["minimum"].as_f64(), Some(1.0));
    assert_eq!(document.fields[0].initial.as_f64(), Some(5.0));
    assert!(document.text.contains(&"Count: 2.0items".to_owned()));
    assert!(document.text.contains(&"A: 1.0".to_owned()));
}
