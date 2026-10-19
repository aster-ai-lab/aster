#![cfg(test)]

use std::collections::BTreeMap;

use a3s_code_core::harness_evidence::ToolRequestOriginV1;
use a3s_code_core::run::RunEventRecord;
use a3s_code_core::AgentEvent;
use serde_json::{json, Value};

pub(super) fn summarize(
    events: &[RunEventRecord],
    parent_tool_ids: &[String],
    observed_vm_count: Option<usize>,
) -> Value {
    let mut origins = BTreeMap::new();
    let mut started = BTreeMap::new();
    let mut intervals = Vec::new();
    let mut parent_intervals = Vec::new();
    let mut child_intervals = Vec::new();
    let (mut parents, mut children, mut unknown) = (0usize, 0usize, 0usize);
    let (mut parent_bytes, mut child_bytes) = (0usize, 0usize);
    let (mut input_tokens, mut output_tokens, mut reported_calls) = (0usize, 0usize, 0usize);
    let mut model_calls = 0usize;
    let mut program_children = Vec::new();
    let (mut receipt_calls, mut receipt_bytes) = (0usize, 0usize);
    let (mut program_parents, mut missing_child_receipts, mut missing_receipt_bytes) =
        (0usize, 0usize, 0usize);
    for record in events {
        match &record.event {
            AgentEvent::ToolRequestBound {
                tool_id, snapshot, ..
            } => {
                origins.insert(tool_id.clone(), snapshot.origin);
            }
            AgentEvent::ToolStart { id, .. } | AgentEvent::ToolExecutionStart { id, .. } => {
                started.insert(id.clone(), record.timestamp_ms);
            }
            AgentEvent::ToolEnd {
                id,
                name,
                output,
                metadata,
                ..
            } => {
                let origin = origins.get(id);
                let parent = origin.is_some() || parent_tool_ids.contains(id);
                let child = matches!(
                    origin,
                    Some(
                        ToolRequestOriginV1::Nested
                            | ToolRequestOriginV1::HostDirectNestedTrusted
                            | ToolRequestOriginV1::HostDirectNestedGoverned
                    )
                );
                if child {
                    children += 1;
                    child_bytes += output.len();
                } else if parent {
                    parents += 1;
                    parent_bytes += output.len();
                } else {
                    unknown += 1;
                }
                if let Some(start) = started.remove(id) {
                    if start <= record.timestamp_ms {
                        let interval = (start, record.timestamp_ms);
                        intervals.push(interval);
                        if child {
                            child_intervals.push(interval);
                        } else if parent {
                            parent_intervals.push(interval);
                        }
                    }
                }
                let calls = metadata
                    .as_ref()
                    .and_then(|metadata| metadata.pointer("/program/tool_calls"))
                    .and_then(Value::as_array);
                if name == "program" {
                    program_parents += 1;
                    if calls.is_none()
                        || metadata
                            .as_ref()
                            .and_then(|metadata| metadata.pointer("/program/tool_calls_complete"))
                            .and_then(Value::as_bool)
                            == Some(false)
                    {
                        missing_child_receipts += 1;
                    }
                }
                if let Some(calls) = calls {
                    let child_intervals = calls
                        .iter()
                        .filter_map(|call| {
                            let start = call["start_offset_ms"].as_u64()?;
                            Some((start, start.checked_add(call["duration_ms"].as_u64()?)?))
                        })
                        .collect::<Vec<_>>();
                    receipt_calls += calls.len();
                    missing_receipt_bytes += calls
                        .iter()
                        .filter(|call| call["output_bytes"].as_u64().is_none())
                        .count();
                    receipt_bytes += calls
                        .iter()
                        .filter_map(|call| call["output_bytes"].as_u64())
                        .sum::<u64>() as usize;
                    program_children.push(json!({"parent_tool_id":id,"calls":calls.len(),
                        "timed_calls":child_intervals.len(),"interval_union_ms":(child_intervals.len()==calls.len()).then(|| interval_union(&child_intervals)),
                        "clock":"monotonic-ms-since-program-start"}));
                }
            }
            AgentEvent::ModelInputBound { .. } => model_calls += 1,
            AgentEvent::ModelUsageBound { snapshot } if snapshot.reported_total_tokens > 0 => {
                reported_calls += 1;
                input_tokens += snapshot.reported_prompt_tokens;
                output_tokens += snapshot.reported_completion_tokens;
            }
            _ => {}
        }
    }
    let finished = parents + children + unknown;
    // Public child events and program receipts lack a shared child identity in
    // this stream. Do not add both representations or silently guess overlap.
    let child_count = if unknown > 0 {
        None
    } else if program_parents == 0 {
        Some(children)
    } else if missing_child_receipts == 0 && children == 0 {
        Some(receipt_calls)
    } else {
        None
    };
    let child_output_bytes = child_count.and_then(|_| {
        if program_parents == 0 {
            Some(child_bytes)
        } else {
            (missing_receipt_bytes == 0).then_some(receipt_bytes)
        }
    });
    json!({
        "schema":"aster.ptc-run-metrics.v1", "model_calls":model_calls,
        "parent_tool_calls":parents, "child_tool_calls":child_count,
        "child_public_event_calls":children, "unknown_origin_tool_calls":unknown,"origin_complete":unknown==0,
        "program_receipt_child_calls":receipt_calls,"programs_missing_child_receipts":missing_child_receipts,
        "vm_count":observed_vm_count,
        "vm_count_source":if observed_vm_count.is_some() {"independent-runtime-observation"} else {"unavailable"},
        "tool_interval_union_ms":(intervals.len()==finished && started.is_empty()).then(|| interval_union(&intervals)),
        "parent_interval_union_ms":(unknown==0 && parent_intervals.len()==parents).then(|| interval_union(&parent_intervals)),
        "child_interval_union_ms":(unknown==0 && program_parents==0 && child_intervals.len()==children).then(|| interval_union(&child_intervals)),
        "observed_tool_interval_union_ms":interval_union(&intervals),
        "timed_tool_calls":intervals.len(),
        "program_child_intervals":program_children,
        "unsettled_execution_intervals":started.len(),
        "parent_output_bytes":parent_bytes, "child_output_bytes":child_output_bytes,
        "reported_usage_calls":reported_calls,
        "reported_tokens":(reported_calls > 0).then(|| json!({"input":input_tokens,"output":output_tokens})),
        "usage_complete":model_calls > 0 && model_calls == reported_calls,
        "limits":"Parent IDs come from the actual model transcript. ToolStart-to-ToolEnd intervals include preparation unless an execution-start event narrows them. Parent intervals may contain children; do not add their durations. Program child intervals use per-program monotonic clocks, so separate programs' unions are not a global child union. Child receipt bytes reflect retained bounded output. Zero-only normalized token usage cannot establish provider reporting. VM counts require independent runtime observations; Bash calls do not establish VM counts."
    })
}

fn interval_union(intervals: &[(u64, u64)]) -> u64 {
    let mut sorted = intervals.to_vec();
    sorted.sort_unstable();
    let mut current: Option<(u64, u64)> = None;
    let mut total = 0;
    for (start, end) in sorted {
        match current {
            Some((left, right)) if start <= right => current = Some((left, right.max(end))),
            Some((left, right)) => {
                total += right - left;
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    total + current.map_or(0, |(start, end)| end - start)
}

#[test]
fn intervals_merge_parallel_and_nested_work_without_double_counting() {
    assert_eq!(
        interval_union(&[(10, 100), (20, 70), (40, 90), (100, 120), (150, 160)]),
        120
    );
    assert_eq!(interval_union(&[]), 0);
}

#[test]
fn unavailable_tokens_and_vm_counts_are_null_instead_of_zero() {
    let metrics = summarize(&[], &[], None);
    assert!(metrics["reported_tokens"].is_null());
    assert!(metrics["vm_count"].is_null());
    assert_eq!(metrics["usage_complete"], false);
}

#[test]
fn available_provider_tokens_are_preserved_without_substituting_estimates() {
    use a3s_code_core::harness_evidence::{ModelUsageSnapshotV1, ToolResultContextUsageV1};
    let snapshot = ModelUsageSnapshotV1 {
        schema: "a3s.code.model-usage-snapshot.v1".into(),
        call_sequence: 1,
        input_snapshot_digest: "a".repeat(64),
        estimated_prompt_tokens: 999,
        reported_prompt_tokens: 17,
        reported_completion_tokens: 13,
        reported_total_tokens: 30,
        reported_cache_read_tokens: None,
        reported_cache_write_tokens: None,
        tool_results: ToolResultContextUsageV1 {
            total_count: 0,
            unique_count: 0,
            repeated_count: 0,
            content_bytes: 0,
            repeated_content_bytes: 0,
            estimated_tokens: 0,
            repeated_estimated_tokens: 0,
            contents_digest: None,
            repeated_contents_digest: None,
        },
        snapshot_digest: "b".repeat(64),
    };
    let events = [RunEventRecord {
        sequence: 0,
        timestamp_ms: 0,
        event: AgentEvent::ModelUsageBound { snapshot },
    }];
    let metrics = summarize(&events, &[], None);
    assert_eq!(metrics["reported_tokens"], json!({"input":17,"output":13}));
    assert_eq!(metrics["reported_usage_calls"], 1);
    assert_eq!(
        metrics["usage_complete"], false,
        "an absent input record cannot establish complete usage"
    );
}

#[test]
fn program_receipts_preserve_child_measurements_when_public_child_events_are_absent() {
    let event = RunEventRecord {
        sequence: 0,
        timestamp_ms: 100,
        event: AgentEvent::ToolEnd {
            id: "program-parent".into(),
            name: "program".into(),
            args: None,
            output: "summary".into(),
            exit_code: 0,
            error_kind: None,
            metadata: Some(json!({"program":{"tool_calls":[
                {"tool_name":"read","start_offset_ms":10,"duration_ms":50,"output_bytes":3},
                {"tool_name":"read","start_offset_ms":20,"duration_ms":50,"output_bytes":5}
            ]}})),
        },
    };
    let metrics = summarize(&[event], &["program-parent".into()], None);
    assert_eq!(metrics["parent_tool_calls"], 1);
    assert_eq!(metrics["child_tool_calls"], 2);
    assert_eq!(metrics["child_output_bytes"], 8);
    assert_eq!(
        metrics["program_child_intervals"][0]["interval_union_ms"],
        60
    );
    assert!(metrics["child_interval_union_ms"].is_null());
    assert!(metrics["parent_interval_union_ms"].is_null());
    assert!(metrics["tool_interval_union_ms"].is_null());
}

#[test]
fn missing_program_receipts_do_not_report_zero_children_or_complete_timing() {
    let events = [RunEventRecord {
        sequence: 0,
        timestamp_ms: 100,
        event: AgentEvent::ToolEnd {
            id: "cancelled-program".into(),
            name: "program".into(),
            args: None,
            output: "cancelled".into(),
            exit_code: 1,
            error_kind: None,
            metadata: None,
        },
    }];
    let metrics = summarize(&events, &["cancelled-program".into()], None);
    assert_eq!(metrics["programs_missing_child_receipts"], 1);
    assert!(metrics["child_tool_calls"].is_null());
    assert!(metrics["child_output_bytes"].is_null());
    assert!(metrics["tool_interval_union_ms"].is_null());
}

#[test]
fn explicitly_incomplete_program_receipts_keep_observed_calls_but_not_exact_totals() {
    for calls in [
        json!([]),
        json!([{
            "tool_name":"read","start_offset_ms":10,"duration_ms":50,"output_bytes":3
        }]),
    ] {
        let retained = calls.as_array().unwrap().len();
        let event = RunEventRecord {
            sequence: 0,
            timestamp_ms: 100,
            event: AgentEvent::ToolEnd {
                id: "incomplete-program".into(),
                name: "program".into(),
                args: None,
                output: "failed".into(),
                exit_code: 1,
                error_kind: None,
                metadata: Some(json!({"program":{
                    "tool_calls_complete":false,"tool_calls":calls
                }})),
            },
        };
        let metrics = summarize(&[event], &["incomplete-program".into()], None);
        assert_eq!(metrics["program_receipt_child_calls"], retained);
        assert_eq!(metrics["programs_missing_child_receipts"], 1);
        assert!(metrics["child_tool_calls"].is_null());
        assert!(metrics["child_output_bytes"].is_null());
    }
}

#[test]
fn unidentified_tool_origin_cannot_establish_a_zero_child_count() {
    let events = [RunEventRecord {
        sequence: 0,
        timestamp_ms: 100,
        event: AgentEvent::ToolEnd {
            id: "unidentified".into(),
            name: "read".into(),
            args: None,
            output: "fixture".into(),
            exit_code: 0,
            error_kind: None,
            metadata: None,
        },
    }];
    let metrics = summarize(&events, &[], None);
    assert_eq!(metrics["unknown_origin_tool_calls"], 1);
    assert!(metrics["child_tool_calls"].is_null());
    assert!(metrics["child_output_bytes"].is_null());
    assert!(metrics["child_interval_union_ms"].is_null());
    assert!(!metrics["origin_complete"].as_bool().unwrap());
}

#[test]
fn overlapping_parent_child_receipts_keep_distinct_counts_and_bytes() {
    use a3s_code_core::harness_evidence::ToolRequestSnapshotV1;
    let mut events = Vec::new();
    for (id, name, origin, start, end, output) in [
        (
            "parent",
            "batch",
            ToolRequestOriginV1::Agent,
            10,
            100,
            "summary",
        ),
        ("left", "read", ToolRequestOriginV1::Nested, 20, 70, "左"),
        (
            "right",
            "bash",
            ToolRequestOriginV1::Nested,
            40,
            90,
            "right",
        ),
    ] {
        for (timestamp_ms, event) in [
            (
                start,
                AgentEvent::ToolRequestBound {
                    tool_id: id.into(),
                    tool_name: name.into(),
                    snapshot: ToolRequestSnapshotV1 {
                        schema: "a3s.code.tool-request-snapshot.v1".into(),
                        origin,
                        tool_id_digest: "a".repeat(64),
                        tool_name_digest: "b".repeat(64),
                        arguments_bytes: 2,
                        arguments_digest: "c".repeat(64),
                        snapshot_digest: "d".repeat(64),
                    },
                },
            ),
            (
                start,
                AgentEvent::ToolExecutionStart {
                    id: id.into(),
                    name: name.into(),
                    args: json!({}),
                },
            ),
            (
                end,
                AgentEvent::ToolEnd {
                    id: id.into(),
                    name: name.into(),
                    args: None,
                    output: output.into(),
                    exit_code: 0,
                    metadata: None,
                    error_kind: None,
                },
            ),
        ] {
            events.push(RunEventRecord {
                sequence: events.len(),
                timestamp_ms,
                event,
            });
        }
    }
    events.sort_by_key(|event| event.timestamp_ms);
    let metrics = summarize(&events, &[], Some(1));
    assert_eq!(metrics["parent_tool_calls"], 1);
    assert_eq!(metrics["child_tool_calls"], 2);
    assert_eq!(metrics["tool_interval_union_ms"], 90);
    assert_eq!(metrics["child_interval_union_ms"], 70);
    assert_eq!(metrics["parent_output_bytes"], 7);
    assert_eq!(metrics["child_output_bytes"], 8);
    assert_eq!(metrics["vm_count"], 1);
    assert!(
        summarize(&events, &[], None)["vm_count"].is_null(),
        "a Bash call is not a VM observation"
    );
}
