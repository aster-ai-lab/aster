//! `FEATURES` is the source of truth and the operator tables are hand-maintained mirrors with no compile-time check of their own.
//! This test is that check.

use aster_shell::agent::config::FEATURES;

const CONFIG_REFERENCE: &str = include_str!("../docs/user-guide/26-config-reference.md");

#[test]
fn every_registered_feature_reaches_the_operator() {
    for spec in FEATURES {
        assert!(
            CONFIG_REFERENCE.contains(&format!("`{}`", spec.path)),
            "{} has no row in the configuration reference",
            spec.path,
        );
        assert!(
            CONFIG_REFERENCE.contains(&format!("`{}`", spec.env)),
            "{} is undocumented in the configuration reference",
            spec.env,
        );
    }
}
