//! The three admitted Meta Harness assemble lists.
//! A turn may name one of these lists. Any other list is refused.

use serde_json::Value;

use super::error::PlaneError;

const DEFAULT: &[&str] = &["system", "tools", "budget", "compact", "infer"];
const LEAN: &[&str] = &["system", "tools", "budget", "infer"];
const HOST: &[&str] = &[
    "system",
    "tools",
    "host:intent_stamp",
    "budget",
    "compact",
    "infer",
];

pub fn default_components() -> Vec<String> {
    owned(DEFAULT)
}

/// `None` means the caller omitted the field, so the next run uses the default list.
pub fn admitted_components(value: Option<&Value>) -> Result<Vec<String>, PlaneError> {
    let Some(value) = value else {
        return Ok(default_components());
    };
    let Some(items) = value.as_array() else {
        return Err(PlaneError::bad("harnessComponents must be a list"));
    };
    let names: Vec<String> = items
        .iter()
        .map(|item| item.as_str().unwrap_or("").to_owned())
        .collect();
    if is_admitted(&names) {
        return Ok(names);
    }
    Err(PlaneError::bad(
        "harnessComponents is not an admitted assemble",
    ))
}

fn is_admitted(names: &[String]) -> bool {
    same(names, DEFAULT) || same(names, LEAN) || same(names, HOST)
}

fn same(names: &[String], recipe: &[&str]) -> bool {
    names.len() == recipe.len()
        && names
            .iter()
            .zip(recipe)
            .all(|(name, expected)| name == expected)
}

fn owned(recipe: &[&str]) -> Vec<String> {
    recipe.iter().map(|name| (*name).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn omitted_list_is_the_default_assemble() {
        assert_eq!(
            admitted_components(None).expect("default"),
            default_components()
        );
    }

    #[test]
    fn the_three_recipes_pass_and_a_fourth_does_not() {
        assert!(admitted_components(Some(&json!(LEAN))).is_ok());
        assert!(admitted_components(Some(&json!(HOST))).is_ok());
        assert!(admitted_components(Some(&json!(["system", "infer"]))).is_err());
        assert!(admitted_components(Some(&json!("lean"))).is_err());
    }
}
