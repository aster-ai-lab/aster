use super::*;
use aster_power::inference::DevicePreference;
use greedy::{run, StepChoice};

fn plan() -> XqPipelinePlan {
    XqPipelinePlan {
        placement: XqPlacement::five_layers_per_device(),
        context_tokens: 32,
        max_step_tokens: 2,
        max_peer_bytes: 1_000_000,
        max_peer_copies: 64,
        max_staged_bytes: 1_000_000,
        driver_reserve_bytes: 64 * 1024 * 1024,
    }
}
fn choice(next: u32, position: usize) -> StepChoice {
    StepChoice {
        next,
        position,
        layers: [5; 8],
        copies: 9,
        bytes: 1024,
    }
}

#[test]
fn smaller_replica_groups_account_for_all_layers_without_claiming_inactive_devices() {
    use crate::parallel::XqReplicaWidth;
    for width in [XqReplicaWidth::Two, XqReplicaWidth::Four] {
        let mut plan = plan();
        plan.placement = XqPlacement::balanced(width);
        let mut expected = [0; 8];
        for &ordinal in plan.placement.layer_ordinals() {
            expected[ordinal] += 1;
        }
        let mut position = 0;
        let output = run(
            &[7, 8, 9],
            2,
            &plan,
            &InferenceLimits::default(),
            &CancellationToken::new(),
            |ids| {
                position += ids.len();
                Ok(StepChoice {
                    next: 20,
                    position,
                    layers: expected,
                    copies: 1,
                    bytes: 128,
                })
            },
        )
        .unwrap();
        assert_eq!(output.stats.model_steps, 3);
        assert_eq!(
            output.stats.executed_layers_by_ordinal,
            expected.map(|count| count as u64 * 3)
        );
        assert!(
            output.stats.executed_layers_by_ordinal[width.device_count()..]
                .iter()
                .all(|&calls| calls == 0)
        );
        let cpu = RuntimeDevice::resolve(DevicePreference::Cpu).unwrap();
        assert!(plan::admit_devices(
            &vec![cpu.clone(); width.device_count()],
            &expected.map(|count| count as u64),
            &plan,
            &InferenceLimits::default()
        )
        .is_err());
        assert!(
            plan::admit_devices(&vec![cpu; 8], &[1; 8], &plan, &InferenceLimits::default())
                .is_err()
        );
    }
}

#[test]
fn greedy_prefill_chunks_and_decode_track_the_uncached_tail_and_every_device_step() {
    let prompt = [7, 8, 9, 10, 11];
    let mut calls = Vec::new();
    let mut position = 0;
    let output = run(
        &prompt,
        3,
        &plan(),
        &InferenceLimits::default(),
        &CancellationToken::new(),
        |ids| {
            calls.push(ids.to_vec());
            position += ids.len();
            // Only the final prefill choice is emitted; earlier chunk choices
            // must never leak into the completion or consume an output slot.
            Ok(choice(
                match calls.len() {
                    1 => 129264,
                    2 => 71,
                    3 => 20,
                    4 => 21,
                    _ => 22,
                },
                position,
            ))
        },
    )
    .unwrap();
    assert_eq!(
        calls,
        vec![vec![7, 8], vec![9, 10], vec![11], vec![20], vec![21]]
    );
    assert_eq!(output.tokens, vec![20, 21, 22]);
    assert_eq!(output.processed_tokens, 7);
    assert_eq!(output.pending_tail, 22);
    assert_eq!(output.stop_reason, XqStopReason::Length);
    assert_eq!(output.stats.model_steps, 5);
    assert_eq!(output.stats.executed_layers_by_ordinal, [25; 8]);
    assert_eq!(output.stats.completed_peer_copies, 45);
    assert_eq!(output.stats.completed_peer_bytes, 5120);
}

#[test]
fn eos_is_counted_once_and_never_fed_back_or_generated_past() {
    let mut calls = 0;
    let output = run(
        &[17, 18],
        7,
        &plan(),
        &InferenceLimits::default(),
        &CancellationToken::new(),
        |ids| {
            calls += 1;
            assert_eq!(ids, [17, 18]);
            Ok(choice(1, 2))
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(output.tokens, [1]);
    assert_eq!(output.stop_reason, XqStopReason::Eos);
    assert_eq!(output.processed_tokens, 2);
    assert_eq!(output.pending_tail, 1);
    for stop in [XqStopReason::Eos, XqStopReason::Length] {
        assert_eq!(XqStopReason::parse(stop.as_str()).unwrap(), stop);
    }
    assert!(XqStopReason::parse("stop").is_err());
}

#[test]
fn native_clock_times_selected_ids_after_all_prefill_and_counts_decode_intervals() {
    use std::cell::Cell;
    let clock = Cell::new(0_u64);
    let mut position = 0;
    let mut calls = 0;
    let output = greedy::run_streaming_clocked(
        &[7, 8, 9, 10, 11],
        3,
        &plan(),
        &InferenceLimits::default(),
        &CancellationToken::new(),
        (
            |ids: &[u32]| {
                position += ids.len();
                calls += 1;
                // Three prefill chunks cost 100ns each. Each real decode costs
                // 20ns; each output callback adds another 7ns before the next step.
                clock.set(clock.get() + if calls <= 3 { 100 } else { 20 });
                Ok(choice(20 + calls as u32, position))
            },
            |_| {
                clock.set(clock.get() + 7);
                Ok(())
            },
        ),
        || Ok(clock.get()),
    )
    .unwrap();
    assert_eq!(output.tokens, [23, 24, 25]);
    assert_eq!(output.stats.model_steps, 5);
    assert_eq!(output.timings.first_token_ns, 300);
    assert_eq!(output.timings.last_token_ns, 354);
    assert_eq!(output.timings.finished_ns, 361);
    assert_eq!(output.tokens.len() - 1, 2);
    // The two intervals include exactly two GPU steps and two callback costs.
    assert_eq!(
        output.timings.last_token_ns - output.timings.first_token_ns,
        54
    );
}

#[test]
fn single_eos_has_no_decode_interval_and_a_backwards_clock_never_returns_a_receipt() {
    let clock = std::cell::Cell::new(10_u64);
    let output = greedy::run_streaming_clocked(
        &[7],
        3,
        &plan(),
        &InferenceLimits::default(),
        &CancellationToken::new(),
        (
            |_| Ok(choice(1, 1)),
            |_| {
                clock.set(11);
                Ok(())
            },
        ),
        || Ok(clock.get()),
    )
    .unwrap();
    assert_eq!(output.tokens, [1]);
    assert_eq!(output.timings.first_token_ns, output.timings.last_token_ns);
    assert_eq!(output.timings.finished_ns, 11);
    for times in [[10, 9, 11], [10, 11, 9]] {
        let mut ticks = times.into_iter();
        let mut position = 0;
        let output = greedy::run_streaming_clocked(
            &[7],
            2,
            &plan(),
            &InferenceLimits::default(),
            &CancellationToken::new(),
            (
                |ids: &[u32]| {
                    position += ids.len();
                    Ok(choice(20, position))
                },
                |_| Ok(()),
            ),
            || Ok(ticks.next().unwrap()),
        );
        assert!(output.is_err());
    }
}

#[test]
fn incremental_token_delivery_observes_cancellation_or_consumer_failure_before_more_decode() {
    for cancel_in_consumer in [true, false] {
        let cancel = CancellationToken::new();
        let mut calls = 0;
        let mut emitted = Vec::new();
        let output = greedy::run_streaming(
            &[17, 18],
            3,
            &plan(),
            &InferenceLimits::default(),
            &cancel,
            |ids| {
                calls += 1;
                assert_eq!(ids, [17, 18]);
                Ok(choice(20, 2))
            },
            |token| {
                emitted.push(token);
                if cancel_in_consumer {
                    cancel.cancel();
                    Ok(())
                } else {
                    Err(PowerError::InferenceFailed(
                        "bounded stream consumer closed".into(),
                    ))
                }
            },
        );
        assert!(output.is_err());
        assert_eq!(calls, 1);
        assert_eq!(emitted, [20]);
    }
}

#[test]
fn invalid_request_limits_ids_and_image_path_refuse_before_any_model_call() {
    let limits = InferenceLimits::default();
    for (prompt, count) in [
        (vec![], 1),
        (vec![7], 0),
        (vec![7; 32], 1),
        (vec![129280], 1),
        (vec![129264], 1),
        (vec![7], usize::MAX),
    ] {
        let mut called = false;
        assert!(run(
            &prompt,
            count,
            &plan(),
            &limits,
            &CancellationToken::new(),
            |_| {
                called = true;
                Ok(choice(1, 1))
            }
        )
        .is_err());
        assert!(!called);
    }
    let limited = InferenceLimits {
        max_input_bytes: 7,
        ..limits.clone()
    };
    assert!(greedy::validate_request(&[7], 1, &plan(), &limited).is_err());
    let parallel = InferenceLimits {
        max_concurrent_requests: 2,
        ..limits
    };
    assert!(plan().validate(&parallel).is_err());
    let mut zero = plan();
    zero.max_peer_bytes = 0;
    assert!(zero.validate(&InferenceLimits::default()).is_err());
}

#[test]
fn step_refuses_wrong_acceptance_bounds_context_ids_or_unbounded_positions() {
    let plan = plan();
    let limits = InferenceLimits::default();
    assert!(plan::validate_token_step(&[7, 8], 0, 1, &plan, &limits).is_ok());
    for (ids, position, accepted) in [
        (vec![], 0, 0),
        (vec![7; 3], 0, 3),
        (vec![7], 32, 1),
        (vec![7], usize::MAX, 1),
        (vec![7], 0, 2),
        (vec![129264], 0, 1),
        (vec![129280], 0, 1),
    ] {
        assert!(plan::validate_token_step(&ids, position, accepted, &plan, &limits).is_err());
    }
}

#[test]
fn cancellation_and_bad_or_incomplete_engine_results_never_return_a_completion() {
    let limits = InferenceLimits::default();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(run(&[7], 1, &plan(), &limits, &cancelled, |_| panic!(
        "cancel must precede computation"
    ))
    .is_err());
    for variant in 0..4 {
        assert!(
            run(&[7], 1, &plan(), &limits, &CancellationToken::new(), |_| {
                let mut out = choice(1, 1);
                match variant {
                    0 => out.position = 0,
                    1 => out.layers[7] = 0,
                    2 => out.next = 129264,
                    _ => out.next = 129280,
                };
                Ok(out)
            })
            .is_err()
        );
    }
    let cancel = CancellationToken::new();
    assert!(run(&[7], 1, &plan(), &limits, &cancel, |_| {
        cancel.cancel();
        Ok(choice(1, 1))
    })
    .is_err());
}

#[test]
fn a_live_request_permit_cannot_be_duplicated_and_returns_on_every_early_drop() {
    let live = Arc::new(AtomicBool::new(false));
    let first = RequestLease::acquire(&live).unwrap();
    assert!(RequestLease::acquire(&live).is_err());
    drop(first);
    let second = RequestLease::acquire(&live).unwrap();
    assert!(live.load(Ordering::Acquire));
    drop(second);
    assert!(!live.load(Ordering::Acquire));
}

#[test]
fn pipeline_device_admission_never_turns_eight_cpu_declarations_into_eight_cuda_devices() {
    let runtimes = vec![RuntimeDevice::resolve(DevicePreference::Cpu).unwrap(); 8];
    assert!(plan::admit_devices(&runtimes, &[1; 8], &plan(), &InferenceLimits::default()).is_err());
    assert!(plan::admit_devices(
        &runtimes[..7],
        &[1; 8],
        &plan(),
        &InferenceLimits::default()
    )
    .is_err());
}

#[test]
fn placement_resident_admission_aggregates_exact_layer_names_rotary_snapshots_and_endpoints() {
    use crate::weights::xq::{storage::tests::fixture_from_tensors, XqTensorType};
    let names: Vec<_> = (0..40)
        .map(|id| format!("blk.{id}.fixture"))
        .chain([
            "token_embd.weight".into(),
            "output.weight".into(),
            "output_norm.weight".into(),
            "blk.40.mtp_only".into(),
        ])
        .collect();
    let tensors: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), vec![1], XqTensorType::F32, vec![0; 4]))
        .collect();
    let (_dir, weights) = fixture_from_tensors(&tensors, 8192);
    let architecture = crate::model::DeepSeekModel::V41Flash1017Xq
        .verify_config(include_bytes!("../../fixtures/xq-model-config.json"))
        .unwrap()
        .text_architecture()
        .unwrap();
    let plan = plan();
    let limits = InferenceLimits::default();
    let resident = plan
        .resident_by_device(&weights, &architecture, &limits)
        .unwrap();
    let mut independent = [20_u64; 8]; // five 4-byte layer fixtures per owner
    for id in 0..40 {
        let compressors = usize::from([2, 8, 14, 20].contains(&id));
        let index_queries = usize::from([2, 8, 14, 20, 24, 28, 32, 36].contains(&id));
        independent[id / 5] += 32 * 64 * 4 * (2 + 2 * compressors + index_queries) as u64;
    }
    independent[0] += 4;
    independent[7] += 8;
    independent[7] += (crate::tokenizer::RAW_VOCABULARY * 8) as u64;
    assert_eq!(resident, independent); // MTP layer40 is not a base decoder layer
    let limited = InferenceLimits {
        max_resident_weight_bytes: *resident.iter().max().unwrap() - 1,
        ..limits
    };
    assert!(plan
        .resident_by_device(&weights, &architecture, &limited)
        .is_err());
}
