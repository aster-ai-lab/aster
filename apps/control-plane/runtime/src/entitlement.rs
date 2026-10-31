//! Entitlement ∩ Grants projection — local allowlist until Cloud compile.
//!
//! The allowlists are read once from the environment so a deployment can name what it sells
//! without a rebuild:
//!
//! * `ASTER_ENTITLED_MODELS`, `ASTER_ENTITLED_APIS`, `ASTER_ENTITLED_TOOLS` — comma-separated SKUs;
//! * `*` allows everything, which is what a single-tenant install wants;
//! * unset stays closed. A gateway that sells access must not open because a variable was
//!   forgotten, so the default keeps the deny-by-default behaviour.

use std::collections::HashSet;
use std::sync::OnceLock;

/// `None` means "every SKU is entitled"; `Some(set)` is an explicit allowlist.
type Allowlist = Option<HashSet<String>>;

struct Entitlement {
    models: Allowlist,
    apis: Allowlist,
    tools: Allowlist,
}

static ENTITLEMENT: OnceLock<Entitlement> = OnceLock::new();

fn read(name: &str) -> Allowlist {
    match std::env::var(name) {
        Ok(value) if value.trim() == "*" => None,
        Ok(value) => Some(
            value
                .split(',')
                .map(str::trim)
                .filter(|sku| !sku.is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        Err(_) => Some(HashSet::new()),
    }
}

fn entitlement() -> &'static Entitlement {
    ENTITLEMENT.get_or_init(|| Entitlement {
        models: read("ASTER_ENTITLED_MODELS"),
        apis: read("ASTER_ENTITLED_APIS"),
        tools: read("ASTER_ENTITLED_TOOLS"),
    })
}

fn allows(list: &Allowlist, sku: &str) -> bool {
    match list {
        None => true,
        Some(set) => set.contains(sku),
    }
}

pub fn is_entitled_api(sku: &str) -> bool {
    allows(&entitlement().apis, sku)
}

pub fn is_entitled_tool(sku: &str) -> bool {
    allows(&entitlement().tools, sku)
}

pub fn is_entitled_model(id: &str) -> bool {
    allows(&entitlement().models, id)
}

fn names<'a>(list: &'a Allowlist) -> Vec<&'a str> {
    match list {
        None => vec!["*"],
        Some(set) => set.iter().map(String::as_str).collect(),
    }
}

/// The SKUs this process sells. A wildcard list reports `["*"]` rather than pretending to
/// enumerate every model id it would accept.
pub fn entitled_models() -> Vec<&'static str> {
    names(&entitlement().models)
}

pub fn entitled_apis() -> Vec<&'static str> {
    names(&entitlement().apis)
}

pub fn entitled_tools() -> Vec<&'static str> {
    names(&entitlement().tools)
}

/// True when every list is an empty allowlist, i.e. the gateway refuses all traffic.
pub fn denies_everything() -> bool {
    let entitlement = entitlement();
    [&entitlement.models, &entitlement.apis, &entitlement.tools]
        .iter()
        .all(|list| list.as_ref().is_some_and(HashSet::is_empty))
}

pub fn spend_cap_usd() -> f64 {
    std::env::var("ASTER_FUNCTION_SPEND_CAP_USD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_list_stays_closed() {
        assert_eq!(read("ASTER_TEST_ENTITLEMENT_UNSET"), Some(HashSet::new()));
    }

    #[test]
    fn a_list_names_exactly_its_skus() {
        std::env::set_var("ASTER_TEST_ENTITLEMENT_LIST", "alpha, beta ,,");
        let list = read("ASTER_TEST_ENTITLEMENT_LIST").expect("explicit list");
        assert!(list.contains("alpha") && list.contains("beta"));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn a_star_opens_the_list() {
        std::env::set_var("ASTER_TEST_ENTITLEMENT_STAR", "*");
        assert_eq!(read("ASTER_TEST_ENTITLEMENT_STAR"), None);
    }
}
