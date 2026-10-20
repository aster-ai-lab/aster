#![cfg(test)]

use super::*;

#[test]
fn no_count_ceiling_removes_only_derived_tool_and_transition_limits() {
    let config = session_harness_config(0, 1024, Vec::new()).unwrap();
    assert_eq!(config.tool_budget_limit(), None);
    assert_eq!(config.tool_round_cap(), None);
    assert_eq!(config.step_limit_option(), None);
    assert_eq!(config.compact_after_chars(), 1024);
    assert_eq!(config.model_attempts(), 2);

    let explicit = session_harness_config_with(0, 1024, Vec::new(), Some(7), Some(3)).unwrap();
    assert_eq!(explicit.tool_budget_limit(), None);
    assert_eq!(explicit.step_limit_option(), Some(7));
    assert_eq!(explicit.model_attempts(), 3);
    assert!(session_harness_config_with(0, 1024, Vec::new(), Some(0), None).is_err());
    assert!(session_harness_config_with(0, 1024, Vec::new(), None, Some(0)).is_err());

    let bounded = session_harness_config(3, 1024, Vec::new()).unwrap();
    assert_eq!(bounded.tool_budget_limit(), Some(3));
    assert_eq!(bounded.tool_round_cap(), Some(3));
    assert_eq!(bounded.step_limit_option(), Some(32));
}
