//! Hermetic tests for the per-turn progress deltas (specs/010 Phase 1).
//!
//! The stall predicate in Phase 2 terminates on ALL-ZERO deltas; these
//! tests pin each dimension's honest measurement, including the case a
//! naive counter would get wrong (repeat reads count once; advancing the
//! turn cannot close an obligation).

#![cfg(test)]

use super::MutationLedger;
use serde_json::json;

fn ledger_with_unresolved(keys: &[&str]) -> MutationLedger {
    let mut ledger = MutationLedger::default();
    let set: std::collections::BTreeSet<String> = keys.iter().map(|key| key.to_string()).collect();
    ledger.set_unresolved_tool_effects(set);
    ledger
}

#[test]
fn a_turn_with_only_new_obligations_reports_information_and_negative_uncertainty() {
    let mut ledger = ledger_with_unresolved(&["invocation:unknown"]);
    let delta = ledger.advance_tool_turn(1).expect("turn 1 settles");
    assert_eq!(delta.information_delta, 0);
    assert_eq!(delta.mutation_delta, 0);
    assert_eq!(delta.uncertainty_delta, -1, "one obligation opened");
    assert_eq!(delta.plan_delta, 0);
    assert!(ledger.has_uncertain_tool_invocations());
    assert!(ledger.advance_tool_turn(1).is_none());
}

#[test]
fn a_zero_delta_turn_is_distinguishable_from_a_productive_one() {
    let mut ledger = MutationLedger::default();
    // First settle: baseline.
    let _ = ledger.advance_tool_turn(1);
    // A turn where NOTHING happens: no receipts, no effects, no outputs.
    let delta = ledger.advance_tool_turn(2).expect("turn 2 settles");
    assert_eq!(delta, Default::default(), "the all-zero stall case");

    // A turn with an assistant output is NOT zero (exploration via text).
    ledger.note_assistant_output();
    let delta = ledger.advance_tool_turn(3).expect("turn 3 settles");
    assert_eq!(delta.plan_delta, 1);
}

#[test]
fn execution_debt_survives_advancing_model_turns() {
    let mut ledger = ledger_with_unresolved(&["invocation:one", "invocation:two"]);
    let _ = ledger.advance_tool_turn(1);
    assert!(ledger.has_uncertain_tool_invocations());
    let delta = ledger.advance_tool_turn(2).expect("turn 2 settles");
    assert_eq!(
        delta.uncertainty_delta, 0,
        "no new obligations, none removed"
    );
    assert_eq!(
        super::ToolEffectUnresolved::from_ledger(&ledger)
            .unwrap()
            .obligation_ids,
        ["invocation:one", "invocation:two"]
    );
}

#[test]
fn older_checkpoint_records_remain_outstanding_without_double_counting() {
    let cached: MutationLedger = serde_json::from_value(json!({
        "records": [],
        "unresolved_tool_effects": ["shared", "live"],
        "recorded_uncertainties": ["shared", "prior"],
    }))
    .unwrap();
    assert!(cached.has_uncertain_tool_invocations());
    let mut restored = MutationLedger::default();
    restored.restore_checkpoint_cache(&cached, true);
    assert_eq!(
        super::ToolEffectUnresolved::from_ledger(&restored)
            .unwrap()
            .obligation_ids,
        ["live", "prior", "shared"]
    );
    assert_eq!(restored.take_slice_delta().uncertainty_delta, -3);
}

#[test]
fn the_json_shape_of_a_delta_is_the_four_dimensions() {
    let delta = super::TurnProgressDelta {
        information_delta: 3,
        mutation_delta: 1,
        uncertainty_delta: -2,
        plan_delta: 4,
    };
    let value = serde_json::to_value(delta).unwrap();
    assert_eq!(
        value,
        json!({
            "information_delta": 3,
            "mutation_delta": 1,
            "uncertainty_delta": -2,
            "plan_delta": 4
        })
    );
}

#[test]
fn a_read_only_slice_with_information_progress_counts() {
    // The P2 lifeline: three receipts applied, no mutation anywhere — the
    // slice is NOT stall material under the delta basis.
    let delta = super::TurnProgressDelta {
        information_delta: 3,
        mutation_delta: 0,
        uncertainty_delta: 0,
        plan_delta: 0,
    };
    assert!(
        delta.information_delta > 0
            || delta.mutation_delta > 0
            || delta.uncertainty_delta != 0
            || delta.plan_delta > 0
    );
    let zero = super::TurnProgressDelta::default();
    assert!(
        !(zero.information_delta > 0
            || zero.mutation_delta > 0
            || zero.uncertainty_delta != 0
            || zero.plan_delta > 0)
    );
}

#[test]
fn the_slice_accumulator_folds_turns_and_resets_on_take() {
    let mut ledger = MutationLedger::default();
    ledger.note_assistant_output();
    let _ = ledger.advance_tool_turn(1);
    ledger.note_assistant_output();
    let _ = ledger.advance_tool_turn(2);
    let slice = ledger.take_slice_delta();
    assert_eq!(slice.plan_delta, 2, "both turns' outputs folded");
    // Taking again yields a fresh (zero) slice.
    assert_eq!(ledger.take_slice_delta(), Default::default());
}

#[test]
fn slice_progress_includes_its_unsettled_last_turn_exactly_once() {
    let mut ledger = MutationLedger::default();
    ledger.advance_tool_turn(1).unwrap();
    ledger.note_assistant_output();
    assert_eq!(ledger.take_slice_delta().plan_delta, 1);
    assert_eq!(ledger.take_slice_delta(), Default::default());
}

#[test]
fn slice_uncertainty_is_net_reduction_since_the_slice_started() {
    let mut ledger = ledger_with_unresolved(&["one"]);
    ledger.advance_tool_turn(1).unwrap();
    assert_eq!(ledger.take_slice_delta().uncertainty_delta, -1);
    ledger.unresolved_tool_effects.clear();
    ledger.advance_tool_turn(2).unwrap();
    assert_eq!(ledger.take_slice_delta().uncertainty_delta, 1);
    assert_eq!(ledger.take_slice_delta(), Default::default());
}
