//! Hermetic tests for the per-turn progress deltas (specs/010 Phase 1).
//!
//! The stall predicate in Phase 2 terminates on ALL-ZERO deltas; these
//! tests pin each dimension's honest measurement, including the case a
//! naive counter would get wrong (repeat reads count once; settled
//! obligations move but never vanish silently).

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
    // Settling turn 1 records the obligation and reports its delta:
    // nothing applied, nothing mutated, uncertainty grew (negative delta).
    let delta = ledger.settle_prior_turns(1).expect("turn 1 settles");
    assert_eq!(delta.information_delta, 0);
    assert_eq!(delta.mutation_delta, 0);
    assert_eq!(delta.uncertainty_delta, -1, "one obligation opened");
    assert_eq!(delta.plan_delta, 0);
    // The obligation no longer denies; a same-watermark settle is a no-op.
    assert!(ledger.settle_prior_turns(1).is_none());
}

#[test]
fn a_zero_delta_turn_is_distinguishable_from_a_productive_one() {
    let mut ledger = MutationLedger::default();
    // First settle: baseline.
    let _ = ledger.settle_prior_turns(1);
    // A turn where NOTHING happens: no receipts, no effects, no outputs.
    let delta = ledger.settle_prior_turns(2).expect("turn 2 settles");
    assert_eq!(delta, Default::default(), "the all-zero stall case");

    // A turn with an assistant output is NOT zero (exploration via text).
    ledger.note_assistant_output();
    let delta = ledger.settle_prior_turns(3).expect("turn 3 settles");
    assert_eq!(delta.plan_delta, 1);
}

#[test]
fn recorded_debt_stays_visible_across_settlements() {
    let mut ledger = ledger_with_unresolved(&["invocation:one", "invocation:two"]);
    let _ = ledger.settle_prior_turns(1);
    assert_eq!(ledger.recorded_uncertainties().len(), 2);
    // A later turn settles with the debt still recorded (uncertainty flat).
    let delta = ledger.settle_prior_turns(2).expect("turn 2 settles");
    assert_eq!(
        delta.uncertainty_delta, 0,
        "no new obligations, none removed"
    );
    assert_eq!(ledger.recorded_uncertainties().len(), 2);
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
    let _ = ledger.settle_prior_turns(1);
    ledger.note_assistant_output();
    let _ = ledger.settle_prior_turns(2);
    let slice = ledger.take_slice_delta();
    assert_eq!(slice.plan_delta, 2, "both turns' outputs folded");
    // Taking again yields a fresh (zero) slice.
    assert_eq!(ledger.take_slice_delta(), Default::default());
}

#[test]
fn slice_progress_includes_its_unsettled_last_turn_exactly_once() {
    let mut ledger = MutationLedger::default();
    ledger.settle_prior_turns(1).unwrap();
    ledger.note_assistant_output();
    assert_eq!(ledger.take_slice_delta().plan_delta, 1);
    assert_eq!(ledger.take_slice_delta(), Default::default());
}

#[test]
fn slice_uncertainty_is_net_reduction_since_the_slice_started() {
    let mut ledger = ledger_with_unresolved(&["one"]);
    ledger.settle_prior_turns(1).unwrap();
    assert_eq!(ledger.take_slice_delta().uncertainty_delta, -1);
    ledger.recorded_uncertainties.clear();
    ledger.settle_prior_turns(2).unwrap();
    assert_eq!(ledger.take_slice_delta().uncertainty_delta, 1);
    assert_eq!(ledger.take_slice_delta(), Default::default());
}
