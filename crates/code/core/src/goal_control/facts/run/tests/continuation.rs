#![cfg(test)]

use super::*;
use crate::goal_control::GoalContinuationPolicyV1;

fn source(max: u64, stalled: u64, clock: u64) -> ContinuationSource {
    serde_json::from_value(serde_json::json!({
        "policy": {
            "automatic_enabled": true,
            "max_slices": max,
            "max_no_progress_slices": stalled,
            "deadline_unix_ms": 10000,
        },
        "observed_unix_ms": clock,
    }))
    .unwrap()
}

#[test]
fn progress_basis_preserves_legacy_bytes_and_refuses_unknown_semantics() {
    let legacy = source(8, 1, 100);
    let encoded = serde_json::to_value(&legacy).unwrap();
    assert!(encoded.get("progress_basis").is_none());
    assert_eq!(
        serde_json::from_value::<ContinuationSource>(encoded.clone()).unwrap(),
        legacy
    );
    let current = ContinuationSource::observed_mutations(legacy.policy.clone(), 100);
    let mut current_json = serde_json::to_value(current).unwrap();
    assert_eq!(
        current_json["progress_basis"],
        "observed_workspace_mutation_v1"
    );
    current_json["progress_basis"] = serde_json::json!("unknown_progress_semantics");
    assert!(serde_json::from_value::<ContinuationSource>(current_json).is_err());
}

fn usage(log: &dyn LogStore, workspace: &WorkspaceRef) -> ContinuationUsage {
    fold_definitions(
        &log.read("session").unwrap(),
        "session",
        workspace,
        None,
        None,
    )
    .unwrap()
    .continuation
}

fn append_run(
    log: &dyn LogStore,
    workspace: &WorkspaceRef,
    id: &str,
    source: Option<&ContinuationSource>,
) {
    let prepared = prepare_run(
        &log.read("session").unwrap(),
        "session",
        workspace,
        None,
        Some(id),
        source,
    )
    .unwrap()
    .unwrap();
    log.append("session", &[prepared.pending.unwrap()], None)
        .unwrap();
}

fn replan(log: &dyn LogStore, workspace: &WorkspaceRef, plan: ExecutionPlan, id: &str) {
    let view = definition_from_facts(&log.read("session").unwrap(), "session", workspace, None)
        .unwrap()
        .unwrap();
    let mut command = GoalPlanCommandV1 {
        kind: GoalCommandKindV1::ProposePlan,
        operation_id: id.into(),
        goal_id: view.binding.goal_id,
        session_id: "session".into(),
        workspace: workspace.clone(),
        work_policy_digest: None,
        expected_goal_revision: view.binding.goal_revision,
        expected_plan_revision: view.plan_revision,
        expected_goal_sequence: view.goal_sequence,
        plan: Some(plan),
        proposal_sequence: None,
    };
    let prepared = prepare_plan(
        &log.read("session").unwrap(),
        "session",
        workspace,
        None,
        &command,
    )
    .unwrap();
    log.append("session", &[prepared.pending.unwrap()], None)
        .unwrap();
    let view = definition_from_facts(&log.read("session").unwrap(), "session", workspace, None)
        .unwrap()
        .unwrap();
    command.kind = GoalCommandKindV1::AcceptPlan;
    command.operation_id = format!("accept-{id}");
    command.expected_plan_revision = view.plan_revision;
    command.expected_goal_sequence = view.goal_sequence;
    command.proposal_sequence = view.plan_fact_sequence;
    command.plan = None;
    let prepared = prepare_plan(
        &log.read("session").unwrap(),
        "session",
        workspace,
        None,
        &command,
    )
    .unwrap();
    log.append("session", &[prepared.pending.unwrap()], None)
        .unwrap();
}

fn prove_history(log: &dyn LogStore) -> WorkspaceRef {
    let (workspace, _) = seed(log);
    append_run(log, &workspace, "ordinary", None);
    let ordinary = log.read("session").unwrap();
    assert!(ordinary.last().unwrap().payload["body"]
        .get("continuation")
        .is_none());
    let ordinary = usage(log, &workspace);
    assert_eq!(
        (
            ordinary.admitted_slices,
            ordinary.slices_since_progress,
            ordinary.last_observed_unix_ms
        ),
        (0, 0, 0)
    );
    let first = source(2, 2, 100);
    append_run(log, &workspace, "auto-1", Some(&first));
    let before = log.read("session").unwrap();
    for _ in 0..4 {
        assert!(prepare_run(
            &before,
            "session",
            &workspace,
            None,
            Some("auto-1"),
            Some(&first)
        )
        .unwrap()
        .unwrap()
        .pending
        .is_none());
    }
    assert!(prepare_run(&before, "session", &workspace, None, Some("auto-1"), None).is_err());
    assert!(prepare_run(
        &before,
        "session",
        &workspace,
        None,
        Some("auto-1"),
        Some(&source(2, 2, 101))
    )
    .is_err());
    assert_eq!(log.read("session").unwrap(), before);
    assert_eq!(usage(log, &workspace).admitted_slices, 1);
    let view = definition_from_facts(&before, "session", &workspace, None)
        .unwrap()
        .unwrap();
    replan(log, &workspace, view.plan.unwrap(), "replan");
    assert_eq!(usage(log, &workspace).slices_since_progress, 1);
    let second = source(2, 2, 200);
    append_run(log, &workspace, "auto-2", Some(&second));
    let before = log.read("session").unwrap();
    assert!(
        prepare_run(
            &before,
            "session",
            &workspace,
            None,
            Some("auto-2"),
            Some(&second)
        )
        .unwrap()
        .unwrap()
        .pending
        .is_none(),
        "exact replay consumes no extra opportunity at the cap"
    );
    assert!(prepare_run(
        &before,
        "session",
        &workspace,
        None,
        Some("auto-3"),
        Some(&source(2, 9, 300))
    )
    .is_err());
    assert!(prepare_run(
        &before,
        "session",
        &workspace,
        None,
        Some("auto-3"),
        Some(&source(9, 2, 300))
    )
    .is_err());
    append_run(log, &workspace, "explicit-independent", None);
    let before = log.read("session").unwrap();
    let view = definition_from_facts(&before, "session", &workspace, None)
        .unwrap()
        .unwrap();
    let plan = view.plan.clone().unwrap();
    let mut revision = crate::goal_control::facts::tests::command();
    revision.kind = GoalCommandKindV1::Revise;
    revision.operation_id = "revise".into();
    revision.expected_goal_revision = Some(view.binding.goal_revision);
    revision.expected_plan_revision = view.plan_revision;
    revision.expected_goal_sequence = Some(view.goal_sequence);
    revision.goal.description = "Revised requirements, same retained history".into();
    let revised = prepare_definition(&before, "session", &workspace, None, &revision).unwrap();
    log.append("session", &[revised.pending.unwrap()], None)
        .unwrap();
    replan(log, &workspace, plan, "replan-revision");
    assert_eq!(usage(log, &workspace).admitted_slices, 2);
    assert_eq!(usage(log, &workspace).slices_since_progress, 2);
    let before = log.read("session").unwrap();
    assert!(prepare_run(
        &before,
        "session",
        &workspace,
        None,
        Some("auto-3"),
        Some(&source(2, 9, 300))
    )
    .is_err());
    append_run(log, &workspace, "auto-3", Some(&source(3, 3, 300)));
    assert_eq!(usage(log, &workspace).admitted_slices, 3);
    assert_eq!(usage(log, &workspace).slices_since_progress, 3);
    workspace
}

/// At the stall bound only a changed accepted plan earns one more slice, once.
#[test]
fn stall_bound_admits_exactly_one_slice_after_a_changed_plan() {
    prove_stall(&MemoryLog::new());
    let directory = tempfile::tempdir().unwrap();
    let workspace = prove_stall(&FileLog::open(directory.path()).unwrap());
    let reopened = FileLog::open(directory.path()).unwrap();
    assert_eq!(usage(&reopened, &workspace).admitted_slices, 2);
    assert_eq!(usage(&reopened, &workspace).slices_since_progress, 2);
}

fn prove_stall(log: &dyn LogStore) -> WorkspaceRef {
    let (workspace, _) = seed(log);
    let check = |clock| {
        check_continuation(
            &log.read("session").unwrap(),
            "session",
            &workspace,
            None,
            &source(9, 1, clock),
        )
    };
    let stalled = |result: Result<(), GoalControlError>| {
        matches!(
            result,
            Err(GoalControlError::Refused {
                refusal: GoalRefusal::Waiting,
                ..
            })
        )
    };
    append_run(log, &workspace, "auto-1", Some(&source(9, 1, 100)));
    assert!(stalled(check(110)));
    let plan = || {
        definition_from_facts(&log.read("session").unwrap(), "session", &workspace, None)
            .unwrap()
            .unwrap()
            .plan
            .unwrap()
    };
    replan(log, &workspace, plan(), "same");
    assert!(stalled(check(120)), "an identical plan is no change");
    let mut changed = plan();
    changed.steps[1].content = "Verify the original source with a second reader".into();
    replan(log, &workspace, changed, "changed");
    assert!(check(130).is_ok());
    append_run(log, &workspace, "auto-replan", Some(&source(9, 1, 130)));
    assert_eq!(usage(log, &workspace).slices_since_progress, 2);
    let mut again = plan();
    again.steps[1].content = "Verify the original source a third way".into();
    replan(log, &workspace, again, "again");
    assert!(stalled(check(140)), "at most one budgeted replan per stall");
    let lifetime = check_continuation(
        &log.read("session").unwrap(),
        "session",
        &workspace,
        None,
        &source(2, 9, 150),
    );
    assert!(matches!(
        lifetime,
        Err(GoalControlError::Refused {
            refusal: GoalRefusal::BudgetExhausted,
            ..
        })
    ));
    assert_eq!(usage(log, &workspace).admitted_slices, 2);
    workspace
}

#[test]
fn continuation_history_preserves_caps_across_replay_revision_replan_and_both_logs() {
    prove_history(&MemoryLog::new());
    let directory = tempfile::tempdir().unwrap();
    let (workspace, before) = {
        let log = FileLog::open(directory.path()).unwrap();
        let workspace = prove_history(&log);
        let before = log.read("session").unwrap();
        let binding = crate::store::SessionFactSourceV1::from_facts("session", &before).unwrap();
        let writer = log.try_writer_fence().unwrap();
        crate::fact_control::append_compaction_done(
            &log,
            &writer,
            "session",
            "All budgets reset to zero",
            binding.as_ref(),
        )
        .unwrap();
        assert_eq!(usage(&log, &workspace).admitted_slices, 3);
        assert_eq!(usage(&log, &workspace).slices_since_progress, 3);
        drop(writer);
        (workspace, log.read("session").unwrap())
    };
    let log = FileLog::open(directory.path()).unwrap();
    assert_eq!(log.read("session").unwrap(), before);
    assert_eq!(usage(&log, &workspace).admitted_slices, 3);
    assert!(prepare_run(
        &before,
        "session",
        &workspace,
        None,
        Some("after-restart"),
        Some(&source(3, 3, 400))
    )
    .is_err());
    assert_eq!(log.read("session").unwrap(), before);
}

#[test]
fn continuation_sources_refuse_recomputed_invalid_limits_clock_and_unbound_goal() {
    for file in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let log: Box<dyn LogStore> = if file {
            Box::new(FileLog::open(directory.path()).unwrap())
        } else {
            Box::new(MemoryLog::new())
        };
        let (workspace, facts) = seed(log.as_ref());
        assert!(prepare_run(
            &[],
            "session",
            &workspace,
            None,
            Some("no-goal"),
            Some(&source(2, 2, 100))
        )
        .is_err());
        let prepared = prepare_run(
            &facts,
            "session",
            &workspace,
            None,
            Some("auto-1"),
            Some(&source(2, 2, 100)),
        )
        .unwrap()
        .unwrap();
        log.append("session", &[prepared.pending.unwrap()], None)
            .unwrap();
        let before = log.read("session").unwrap();
        for case in 0..5 {
            let mut corrupt = before.clone();
            let fact = corrupt.last_mut().unwrap();
            let mut stored = RunFact::deserialize(&fact.payload).unwrap();
            let changed_source = stored.body.continuation.as_mut().unwrap();
            match case {
                0 => changed_source.policy.automatic_enabled = false,
                1 => changed_source.observed_unix_ms = 0,
                2 => changed_source.observed_unix_ms = changed_source.policy.deadline_unix_ms.get(),
                3 => changed_source.policy.max_slices = std::num::NonZeroU64::new(1).unwrap(),
                _ => {
                    changed_source.policy.max_no_progress_slices =
                        std::num::NonZeroU64::new(1).unwrap()
                }
            }
            stored.command_digest = digest(&prepared.definition, &stored.body).unwrap();
            fact.payload = serde_json::to_value(stored).unwrap();
            if case < 3 {
                assert!(fold_definitions(&corrupt, "session", &workspace, None, None).is_err());
            } else {
                let capped = source(1, 1, 200);
                assert!(prepare_run(
                    &corrupt,
                    "session",
                    &workspace,
                    None,
                    Some("next"),
                    Some(&capped)
                )
                .is_err());
            }
        }
        for clock in [0, 99, 10000] {
            assert!(prepare_run(
                &before,
                "session",
                &workspace,
                None,
                Some("next"),
                Some(&source(3, 3, clock))
            )
            .is_err());
        }
        for field in ["policy", "observed_unix_ms"] {
            let mut corrupt = before.clone();
            corrupt.last_mut().unwrap().payload["body"]["continuation"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(fold_definitions(&corrupt, "session", &workspace, None, None).is_err());
        }
        assert_eq!(log.read("session").unwrap(), before);
    }
}

#[test]
fn continuation_source_cannot_disappear_by_substituting_its_fact_kind() {
    let log = MemoryLog::new();
    let (workspace, _) = seed(&log);
    append_run(&log, &workspace, "auto-1", Some(&source(1, 1, 100)));
    let mut facts = log.read("session").unwrap();
    facts.last_mut().unwrap().kind = "host.audit".into();
    assert!(fold_definitions(&facts, "session", &workspace, None, None).is_err());
}

#[test]
fn continuation_source_preserves_ordinary_wire_and_is_refused_by_the_original_body_reader() {
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OriginalRunBody {
        run_id: String,
        step_id: String,
        expected_goal_sequence: u64,
        plan_fact_sequence: u64,
    }

    let log = MemoryLog::new();
    let (workspace, _) = seed(&log);
    append_run(&log, &workspace, "ordinary", None);
    let body = log.read("session").unwrap().last().unwrap().payload["body"].clone();
    let old = OriginalRunBody::deserialize(&body).unwrap();
    assert_eq!(serde_json::to_value(old).unwrap(), body);
    append_run(&log, &workspace, "auto-1", Some(&source(1, 1, 100)));
    let body = log.read("session").unwrap().last().unwrap().payload["body"].clone();
    assert!(OriginalRunBody::deserialize(&body).is_err());
}

#[test]
fn the_progress_basis_closed_set_parses_all_members() {
    use super::super::continuation::ProgressBasis;
    assert!(ProgressBasis::parse("observed_workspace_mutation_v1").is_ok());
    assert!(ProgressBasis::parse("turn_progress_delta_v1").is_ok());
    assert!(ProgressBasis::parse("turn_progress_delta_v2").is_ok());
    assert!(ProgressBasis::parse("turn_progress_delta_v3").is_ok());
    assert!(ProgressBasis::parse("mutation").is_err());
    assert_eq!(
        ProgressBasis::TurnProgressDeltaV1.as_str(),
        "turn_progress_delta_v1"
    );
    assert_eq!(
        ProgressBasis::TurnProgressDeltaV2.as_str(),
        "turn_progress_delta_v2"
    );
    assert_eq!(
        ProgressBasis::TurnProgressDeltaV3.as_str(),
        "turn_progress_delta_v3"
    );
}

/// The satisfiability exit (specs/010 P4 slice 1): a stalled goal under
/// the delta basis reaches a TERMINAL refusal — never an infinite admit
/// loop. This is the frozen-server scenario pinned as a named refusal.
#[test]
fn a_zero_delta_stall_terminates_instead_of_admitting_forever() {
    let policy = serde_json::from_value::<GoalContinuationPolicyV1>(serde_json::json!({
        "automatic_enabled": true,
        "max_slices": 1_000,
        "max_no_progress_slices": 3,
        "deadline_unix_ms": 100_000,
    }))
    .unwrap();
    let mut usage = ContinuationUsage::default();
    // Slices 1..3 admit (no progress yet, under the bound).
    for clock in [100u64, 200, 300] {
        let ok = usage.admit(&ContinuationSource::turn_progress(policy.clone(), clock));
        assert!(ok.is_ok(), "slice under the stall bound admits");
        // Zero progress: nothing resets slices_since_progress.
    }
    // The 4th admit (past the bound, unchanged plan) is the terminal exit.
    let refused = usage
        .admit(&ContinuationSource::turn_progress(policy.clone(), 400))
        .expect_err("past the stall bound with an unchanged plan refuses");
    match refused {
        GoalControlError::Refused { refusal, .. } => {
            assert_eq!(refusal, GoalRefusal::Waiting);
        }
        other => panic!("expected a Waiting refusal, got {other:?}"),
    }
    // A changed plan earns exactly one more slice, then terminates again.
    usage.accepted_plan = Some(test_plan_identity(7));
    let grace = usage.admit(&ContinuationSource::turn_progress(policy.clone(), 500));
    assert!(grace.is_ok(), "a changed plan earns one more slice");
    let final_refusal = usage
        .admit(&ContinuationSource::turn_progress(policy.clone(), 600))
        .expect_err("a second replan earns nothing");
    assert!(matches!(
        final_refusal,
        GoalControlError::Refused {
            refusal: GoalRefusal::Waiting,
            ..
        }
    ));
}

/// A nonzero delta resets the stall count — the read-only lifeline: 20
/// slices of pure exploration never terminate under the delta basis.
#[test]
fn information_progress_keeps_admitting_indefinitely() {
    let policy = serde_json::from_value::<GoalContinuationPolicyV1>(serde_json::json!({
        "automatic_enabled": true,
        "max_slices": 1_000,
        "max_no_progress_slices": 3,
        "deadline_unix_ms": 100_000,
    }))
    .unwrap();
    let mut usage = ContinuationUsage::default();
    for i in 0..20 {
        let ok = usage.admit(&ContinuationSource::turn_progress(policy.clone(), 100 + i));
        assert!(ok.is_ok(), "slice {i} with progress information admits");
        // The fact fold counts positive information progress; the admission
        // path sees slices_since_progress back to zero each time.
        usage.slices_since_progress = 0;
    }
}

fn append_settled_progress(
    log: &dyn LogStore,
    workspace: &WorkspaceRef,
    id: &str,
    source: &ContinuationSource,
    progress: crate::harness_loop::TurnProgressDelta,
) {
    append_run(log, workspace, id, Some(source));
    let pending = crate::goal_control::facts::settle::prepare_settlement(
        &log.read("session").unwrap(),
        "session",
        workspace,
        None,
        id,
        None,
        Some(progress),
        None,
    )
    .unwrap()
    .unwrap();
    log.append("session", &[pending], None).unwrap();
}

#[test]
fn growing_uncertainty_does_not_reset_stall_across_replay_and_reopen() {
    for (basis, uncertainty_delta) in [
        (ProgressBasis::TurnProgressDeltaV2, -1),
        (ProgressBasis::TurnProgressDeltaV2, i64::MIN),
        (ProgressBasis::TurnProgressDeltaV3, -1),
        (ProgressBasis::TurnProgressDeltaV3, i64::MIN),
    ] {
        for file in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let log: Box<dyn LogStore> = if file {
                Box::new(FileLog::open(directory.path()).unwrap())
            } else {
                Box::new(MemoryLog::new())
            };
            let (workspace, _) = seed(log.as_ref());
            let mut admission = ContinuationSource::turn_progress(source(8, 1, 100).policy, 100);
            admission.progress_basis = Some(basis);
            append_settled_progress(
                log.as_ref(),
                &workspace,
                "auto-1",
                &admission,
                crate::harness_loop::TurnProgressDelta {
                    uncertainty_delta,
                    ..Default::default()
                },
            );
            let before = log.read("session").unwrap();
            assert_eq!(
                before.last().unwrap().payload["body"]["progress"]["uncertainty_delta"],
                uncertainty_delta
            );
            let check = |log: &dyn LogStore| {
                assert_eq!(usage(log, &workspace).slices_since_progress, 1);
                assert!(matches!(
                    check_continuation(
                        &log.read("session").unwrap(),
                        "session",
                        &workspace,
                        None,
                        &ContinuationSource {
                            observed_unix_ms: 200,
                            ..admission.clone()
                        }
                    ),
                    Err(GoalControlError::Refused {
                        refusal: GoalRefusal::Waiting,
                        ..
                    })
                ));
                assert_eq!(log.read("session").unwrap(), before);
            };
            check(log.as_ref());
            drop(log);
            if file {
                check(&FileLog::open(directory.path()).unwrap());
            }
        }
    }
}

#[test]
fn current_raw_mutation_and_plan_counters_need_original_workspace_or_step_proof() {
    for progress in [
        crate::harness_loop::TurnProgressDelta {
            mutation_delta: 1,
            ..Default::default()
        },
        crate::harness_loop::TurnProgressDelta {
            plan_delta: 1,
            ..Default::default()
        },
    ] {
        let log = MemoryLog::new();
        let (workspace, _) = seed(&log);
        let admission = ContinuationSource::turn_progress(source(8, 1, 100).policy, 100);
        append_settled_progress(&log, &workspace, "unproved-count", &admission, progress);
        assert_eq!(usage(&log, &workspace).slices_since_progress, 1);
        assert!(matches!(
            check_continuation(
                &log.read("session").unwrap(),
                "session",
                &workspace,
                None,
                &ContinuationSource {
                    observed_unix_ms: 200,
                    ..admission
                }
            ),
            Err(GoalControlError::Refused {
                refusal: GoalRefusal::Waiting,
                ..
            })
        ));
    }
}

#[test]
fn reduced_uncertainty_and_read_only_information_keep_their_continuation_opportunity() {
    use crate::harness_loop::TurnProgressDelta;
    for progress in [
        TurnProgressDelta {
            uncertainty_delta: 1,
            ..Default::default()
        },
        TurnProgressDelta {
            information_delta: 1,
            uncertainty_delta: -1,
            ..Default::default()
        },
        TurnProgressDelta {
            mutation_delta: 1,
            ..Default::default()
        },
        TurnProgressDelta {
            plan_delta: 1,
            ..Default::default()
        },
    ] {
        let log = MemoryLog::new();
        let (workspace, _) = seed(&log);
        let mut admission = ContinuationSource::turn_progress(source(8, 1, 100).policy, 100);
        // V2's recorded counters remain valid even without V3's original-state proof.
        admission.progress_basis = Some(ProgressBasis::TurnProgressDeltaV2);
        append_settled_progress(&log, &workspace, "auto-1", &admission, progress);
        assert_eq!(usage(&log, &workspace).slices_since_progress, 0);
        assert!(check_continuation(
            &log.read("session").unwrap(),
            "session",
            &workspace,
            None,
            &ContinuationSource::turn_progress(admission.policy, 200)
        )
        .is_ok());
    }
}

#[test]
fn historical_negative_progress_preserves_already_admitted_slices() {
    for file in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let log: Box<dyn LogStore> = if file {
            Box::new(FileLog::open(directory.path()).unwrap())
        } else {
            Box::new(MemoryLog::new())
        };
        let (workspace, _) = seed(log.as_ref());
        let mut old = serde_json::to_value(source(8, 1, 100)).unwrap();
        old["progress_basis"] = serde_json::json!("turn_progress_delta_v1");
        let old: ContinuationSource = serde_json::from_value(old).unwrap();
        append_settled_progress(
            log.as_ref(),
            &workspace,
            "old-1",
            &old,
            crate::harness_loop::TurnProgressDelta {
                uncertainty_delta: -1,
                ..Default::default()
            },
        );
        let mut second = old.clone();
        second.observed_unix_ms = 200;
        append_run(log.as_ref(), &workspace, "old-2", Some(&second));
        let before = log.read("session").unwrap();
        assert_eq!(usage(log.as_ref(), &workspace).admitted_slices, 2);
        drop(log);
        if file {
            let reopened = FileLog::open(directory.path()).unwrap();
            assert_eq!(reopened.read("session").unwrap(), before);
            assert_eq!(usage(&reopened, &workspace).admitted_slices, 2);
        }
    }
}

fn test_plan_identity(seed: u64) -> ExecutionIdentityV1 {
    serde_json::from_value(serde_json::json!({
        "schema": "plan",
        "domain": format!("satisfiability-{seed}"),
        "digest": format!("cap-{seed}"),
    }))
    .unwrap()
}
