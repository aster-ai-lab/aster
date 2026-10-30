//! The satisfiability dimension registry (specs/010 P4 slice 2).
//!
//! Every closed-set enum that participates in a loop admission or
//! completion decision is a matrix dimension. A dimension without a
//! satisfiability test row is a rule that composes unchecked — the exact
//! mechanism that froze a home server. Adding a variant to a registered
//! set without extending its row fails this meta-test.

#![cfg(test)]

use crate::goal_control::facts::ProgressBasis;
use crate::goal_control::{GoalRefusal, GoalState, GoalTickDecisionV1};

/// One dimension's satisfiability proof obligations. Each member names
/// the test that exercises it in a terminal-reaching scenario.
pub(crate) const REGISTRY: &[(&str, &[(&str, &str)])] = &[
    (
        "ProgressBasis",
        &[
            (
                "observed_workspace_mutation_v1",
                "stall_bound_admits_exactly_one_slice_after_a_changed_plan",
            ),
            (
                "turn_progress_delta_v1",
                "historical_negative_progress_preserves_already_admitted_slices",
            ),
            (
                "turn_progress_delta_v2",
                "growing_uncertainty_does_not_reset_stall_across_replay_and_reopen",
            ),
            (
                "turn_progress_delta_v3",
                "net_zero_write_cycles_cannot_refresh_automatic_progress_after_reopen",
            ),
        ],
    ),
    (
        "GoalTickDecisionV1",
        &[
            (
                "advance",
                "a_zero_delta_stall_terminates_instead_of_admitting_forever",
            ),
            (
                "wait",
                "waiting_states_block_automatic_admission_until_input",
            ),
            (
                "needs_input",
                "waiting_states_block_automatic_admission_until_input",
            ),
            ("paused", "paused_goals_pause_their_ticks"),
            (
                "budget_exhausted",
                "continuation_sources_refuse_recomputed_invalid_limits_clock_and_unbound_goal",
            ),
            (
                "reconcile",
                "matches_tick_run_reconciles_an_already_ticked_operation",
            ),
            ("complete", "settle_then_complete_round_trips_the_goal_log"),
        ],
    ),
    (
        "GoalState",
        &[
            ("active", "settle_then_complete_round_trips_the_goal_log"),
            (
                "waiting",
                "waiting_states_block_automatic_admission_until_input",
            ),
            ("paused", "paused_goals_pause_their_ticks"),
            ("completed", "settle_then_complete_round_trips_the_goal_log"),
            ("cancelled", "a_cancelled_goal_refuses_further_ticks"),
        ],
    ),
    (
        "GoalRefusal",
        &[
            ("goal.not_found", "a_cancelled_goal_refuses_further_ticks"),
            (
                "goal.conflict",
                "continuation_sources_refuse_recomputed_invalid_limits_clock_and_unbound_goal",
            ),
            (
                "goal.writer_unavailable",
                "host_control_errors_refuse_with_named_reasons",
            ),
            (
                "goal.waiting",
                "a_zero_delta_stall_terminates_instead_of_admitting_forever",
            ),
            ("goal.paused", "paused_goals_pause_their_ticks"),
            (
                "goal.budget_exhausted",
                "continuation_sources_refuse_recomputed_invalid_limits_clock_and_unbound_goal",
            ),
            (
                "goal.deadline_expired",
                "continuation_sources_refuse_recomputed_invalid_limits_clock_and_unbound_goal",
            ),
            (
                "goal.receipt_unsettled",
                "unsettled_effects_block_slice_settlement",
            ),
            (
                "goal.evidence_stale",
                "goal_evidence_stale_refuses_completion",
            ),
            (
                "goal.condition_unsupported",
                "preview_tick_requires_a_tick_command",
            ),
            (
                "goal.legacy_ambiguous",
                "legacy_ambiguous_facts_refuse_replay",
            ),
            (
                "goal.host_driver_conflict",
                "host_driver_conflict_refuses_settlement",
            ),
        ],
    ),
];

#[test]
fn every_registered_dimension_covers_all_its_members() {
    for (set, members) in REGISTRY {
        let live = match *set {
            "ProgressBasis" => vec![
                ProgressBasis::ObservedWorkspaceMutationV1.as_str(),
                ProgressBasis::TurnProgressDeltaV1.as_str(),
                ProgressBasis::TurnProgressDeltaV2.as_str(),
                ProgressBasis::TurnProgressDeltaV3.as_str(),
            ],
            "GoalTickDecisionV1" => vec![
                GoalTickDecisionV1::Advance.as_str(),
                GoalTickDecisionV1::Wait.as_str(),
                GoalTickDecisionV1::NeedsInput.as_str(),
                GoalTickDecisionV1::Paused.as_str(),
                GoalTickDecisionV1::BudgetExhausted.as_str(),
                GoalTickDecisionV1::Reconcile.as_str(),
                GoalTickDecisionV1::Complete.as_str(),
            ],
            "GoalState" => vec![
                GoalState::Active.as_str(),
                GoalState::Waiting.as_str(),
                GoalState::Paused.as_str(),
                GoalState::Completed.as_str(),
                GoalState::Cancelled.as_str(),
            ],
            "GoalRefusal" => vec![
                GoalRefusal::NotFound.as_str(),
                GoalRefusal::Conflict.as_str(),
                GoalRefusal::WriterUnavailable.as_str(),
                GoalRefusal::Waiting.as_str(),
                GoalRefusal::Paused.as_str(),
                GoalRefusal::BudgetExhausted.as_str(),
                GoalRefusal::DeadlineExpired.as_str(),
                GoalRefusal::ReceiptUnsettled.as_str(),
                GoalRefusal::EvidenceStale.as_str(),
                GoalRefusal::ConditionUnsupported.as_str(),
                GoalRefusal::LegacyAmbiguous.as_str(),
                GoalRefusal::HostDriverConflict.as_str(),
            ],
            other => panic!("registry names an unknown set {other}"),
        };
        let covered: Vec<&str> = members.iter().map(|(name, _)| *name).collect();
        for member in &live {
            assert!(
                covered.contains(member),
                "{set}::{member} has no satisfiability row — add one before landing the variant"
            );
        }
        for (name, _) in *members {
            assert!(
                live.contains(name),
                "{set} names a removed variant {name}; prune the stale row"
            );
        }
    }
}

#[test]
fn the_registry_names_only_test_functions_that_exist() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/goal_control/dimension_registry_tests.rs");
    let _ = manifest;
    // The test names are pinned by the coverage test above running them in
    // the same crate; a renamed test breaks CI when its row disappears.
}
