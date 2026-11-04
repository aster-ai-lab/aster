#![cfg(all(test, unix))]

#[path = "long_horizon/artifact.rs"]
mod artifact;
#[path = "long_horizon/candidate.rs"]
mod candidate;
#[path = "long_horizon/config.rs"]
mod config;
#[path = "long_horizon/driver.rs"]
mod driver;
#[path = "long_horizon/live.rs"]
mod live;
#[path = "long_horizon/relay.rs"]
mod relay;
#[path = "long_horizon/upstream.rs"]
mod upstream;

use agent_client_protocol as acp;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

fn correct(workspace: &Path, expected: &Value, protected: &BTreeMap<String, Vec<u8>>) -> bool {
    let _timing = artifact::timing::span(artifact::timing::Phase::Oracle);
    artifact::read(workspace, "selection.json")
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .as_ref()
        == Some(expected)
        && protected.iter().all(|(name, bytes)| {
            artifact::read(workspace, name).ok().as_deref() == Some(bytes.as_slice())
        })
}

fn path_matches(value: &str, workspace: &Path, name: &str) -> bool {
    let value = Path::new(value);
    if value.is_absolute() {
        value == workspace.join(name)
    } else {
        workspace.join(value) == workspace.join(name)
    }
}

fn reads_events(args: &Value, workspace: &Path) -> bool {
    args["file_path"]
        .as_str()
        .is_some_and(|path| path_matches(path, workspace, "events.jsonl"))
        || args["files"].as_array().is_some_and(|files| {
            files.iter().any(|file| {
                file["path"]
                    .as_str()
                    .is_some_and(|path| path_matches(path, workspace, "events.jsonl"))
            })
        })
}

fn observe_reads(
    events: &[Value],
    workspace: &Path,
    pending: &mut BTreeSet<String>,
    completed: &mut BTreeSet<String>,
) {
    for event in events {
        let update = &event["update"];
        let Some(id) = update["toolCallId"].as_str() else {
            continue;
        };
        if update["kind"] == "read" && reads_events(&update["rawInput"], workspace) {
            pending.insert(id.to_owned());
        }
        if update["status"] == "completed" && pending.contains(id) {
            completed.insert(id.to_owned());
        }
    }
}

#[test]
fn independent_oracle_checks_values_order_missing_versions_and_protected_bytes() {
    let root = tempfile::tempdir().unwrap();
    let expected = json!({"observations":[
        {"revision":1,"selected":[{"token":"a7","label":"星🙂"},{"token":"c9","label":"鳶"}]},
        {"revision":2,"selected":[{"token":"b8","label":"新規"}]}
    ]});
    let protected = BTreeMap::from([("notes.txt".into(), b"original note\n".to_vec())]);
    std::fs::write(root.path().join("notes.txt"), &protected["notes.txt"]).unwrap();
    assert!(!correct(root.path(), &expected, &protected));
    let write = |value: &Value| {
        std::fs::write(
            root.path().join("selection.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
    };
    write(&expected);
    assert!(correct(root.path(), &expected, &protected));
    let mut wrong = expected.clone();
    wrong["observations"][0]["selected"][0]["label"] = json!("different");
    write(&wrong);
    assert!(!correct(root.path(), &expected, &protected));
    let mut missing = expected.clone();
    missing["observations"].as_array_mut().unwrap().pop();
    write(&missing);
    assert!(!correct(root.path(), &expected, &protected));
    let mut reordered = expected.clone();
    reordered["observations"][0]["selected"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    write(&reordered);
    assert!(!correct(root.path(), &expected, &protected));
    write(&expected);
    std::fs::write(root.path().join("notes.txt"), b"changed").unwrap();
    assert!(!correct(root.path(), &expected, &protected));
}

#[test]
fn feed_watcher_deduplicates_completions_and_matches_only_the_declared_path() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace 独立観察");
    let events = vec![
        json!({"update":{"kind":"read","toolCallId":"first","rawInput":{"file_path":"events.jsonl"}}}),
        json!({"update":{"toolCallId":"first","status":"completed"}}),
        json!({"update":{"kind":"read","toolCallId":"unrelated","rawInput":{"file_path":"selection.json"},"status":"completed"}}),
        json!({"update":{"kind":"read","toolCallId":"second","rawInput":{"files":[{"path":workspace.join("events.jsonl")} ]},"status":"completed"}}),
        json!({"update":{"kind":"read","toolCallId":"failed","rawInput":{"file_path":"events.jsonl"},"status":"failed"}}),
    ];
    let mut pending = BTreeSet::new();
    let mut completed = BTreeSet::new();
    for _ in 0..2 {
        observe_reads(&events, &workspace, &mut pending, &mut completed);
    }
    assert_eq!(completed, BTreeSet::from(["first".into(), "second".into()]));
    assert_eq!(pending.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "independent pre-frozen non-arithmetic holdout; explicit ACL/model/input/evidence required"]
async fn opaque_selected_records_survive_a_real_long_run() {
    tokio::task::LocalSet::new().run_until(run_holdout()).await;
}

async fn run_holdout() {
    let inputs = std::path::PathBuf::from(
        std::env::var_os("ASTER_DELOOP_HOLDOUT_INPUTS").expect("explicit frozen input directory"),
    );
    assert!(inputs.is_absolute());
    let registration: Value =
        serde_json::from_slice(&artifact::read(&inputs, "preregistration.json").unwrap()).unwrap();
    let mut source = BTreeMap::new();
    for name in [
        "CONTRACT.md",
        "notes.txt",
        "request.txt",
        "expected.json",
        "revision-1.jsonl",
        "revision-2.jsonl",
        "revision-3.jsonl",
        "revision-4.jsonl",
        "revision-5.jsonl",
    ] {
        assert_eq!(
            artifact::digest(&inputs.join(name)).unwrap(),
            registration["inputs"][name]
        );
        source.insert(name.to_string(), artifact::read(&inputs, name).unwrap());
    }
    let expected: Value = serde_json::from_slice(&source["expected.json"]).unwrap();
    let mut expected_from_inputs = Vec::new();
    for revision in 1..=5 {
        let rows =
            String::from_utf8(source[&format!("revision-{revision}.jsonl")].clone()).unwrap();
        let mut rows = rows
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap());
        let header = rows.next().unwrap();
        assert_eq!(header, json!({"revision":revision,"revisions":5}));
        let rows = rows.collect::<Vec<_>>();
        assert_eq!(rows.len(), 37);
        let selected = rows
            .iter()
            .filter(|row| row["include"] == true)
            .map(|row| json!({"token":row["token"],"label":row["label"]}))
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 4);
        expected_from_inputs.push(json!({"revision":revision,"selected":selected}));
    }
    assert_eq!(expected, json!({"observations":expected_from_inputs}));
    let mut campaign =
        live::Campaign::prepare_with_context("deloop-independent-records", Some(24_576)).await;
    let fixture_timing = artifact::timing::span(artifact::timing::Phase::FixtureSetup);
    let workspace = campaign.root.path().join("workspace 独立観察");
    std::fs::create_dir(&workspace).unwrap();
    for name in ["CONTRACT.md", "notes.txt"] {
        std::fs::write(workspace.join(name), &source[name]).unwrap();
    }
    std::fs::write(workspace.join("events.jsonl"), &source["revision-1.jsonl"]).unwrap();
    let request = String::from_utf8(source["request.txt"].clone()).unwrap();
    drop(fixture_timing);
    let driver = campaign.spawn(&workspace, None, false);
    let mut session = None;
    let mut pending = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut cursor = 0;
    let mut published = 1;
    let started = Instant::now();
    let work = tokio::time::timeout(Duration::from_secs(1800), async {
        let opened = driver.open(&workspace, Some(json!({"asterHost":{"completionAttestation":"disk_content"}}).as_object().unwrap().clone())).await?;
        session = Some(opened.clone());
        let prompt = driver.prompt(&opened, &request);
        tokio::pin!(prompt);
        let mut tick = tokio::time::interval(Duration::from_millis(20));
        loop {
            tokio::select! {
                result = &mut prompt => break result,
                _ = tick.tick() => {
                    let events = {
                        let capture = driver.capture.lock().unwrap();
                        let events = capture.events[cursor..].to_vec();
                        cursor = capture.events.len();
                        events
                    };
                    observe_reads(&events, &workspace, &mut pending, &mut completed);
                    let revision = (completed.len() + 1).min(5);
                    if revision != published {
                        let _timing = artifact::timing::span(artifact::timing::Phase::FeedPublish);
                        published = revision;
                        tokio::fs::write(workspace.join("events.jsonl"), &source[&format!("revision-{published}.jsonl")]).await.map_err(|_| "external_feed_write")?;
                    }
                }
            }
        }
    }).await.unwrap_or(Err("workflow_deadline"));
    let elapsed = started.elapsed().as_millis();
    let (events, tools_terminal, overflow) = {
        let capture = driver.capture.lock().unwrap();
        (
            capture.events.clone(),
            capture.tools.values().all(|status| {
                matches!(
                    status,
                    acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed
                )
            }),
            capture.overflow,
        )
    };
    observe_reads(&events, &workspace, &mut pending, &mut completed);
    let cleanup = driver.close().await;
    let relay = campaign.relay.take().unwrap().close().await;
    let fact_timing = artifact::timing::span(artifact::timing::Phase::FactRead);
    let facts = session
        .as_ref()
        .map(|id| aster_core::fact_control::read_workspace_facts(&workspace, id.0.as_ref()));
    let facts_available = facts.as_ref().is_some_and(Result::is_ok);
    let facts = facts.and_then(Result::ok).unwrap_or_default();
    drop(fact_timing);
    let oracle_timing = artifact::timing::span(artifact::timing::Phase::Oracle);
    let compactions = facts
        .iter()
        .filter(|fact| fact.kind == "compaction.done")
        .count();
    let mut changed_paths = BTreeSet::new();
    let mut source_mutation = false;
    let mut early_mutation = false;
    let mut read_receipts = 0;
    let mut receipts_complete = true;
    for fact in facts.iter().filter(|fact| fact.kind == "tool.result") {
        if let Ok(receipt) = serde_json::from_value::<aster_effect::ToolExecutionResultV1>(
            fact.payload["receipt"].clone(),
        ) {
            receipts_complete &= receipt.validate().is_ok();
            if pending.contains(&receipt.identity.tool_call_id)
                && receipt.outcome == aster_effect::ToolExecutionOutcome::Succeeded
            {
                read_receipts += 1;
            }
            early_mutation |= read_receipts < 5 && !receipt.effects.changes.is_empty();
            for change in receipt.effects.changes {
                source_mutation |= !path_matches(&change.path, &workspace, "selection.json");
                changed_paths.insert(change.path);
            }
        } else {
            receipts_complete = false;
        }
    }
    let protected = BTreeMap::from([
        ("CONTRACT.md".into(), source["CONTRACT.md"].clone()),
        ("notes.txt".into(), source["notes.txt"].clone()),
        ("events.jsonl".into(), source["revision-5.jsonl"].clone()),
    ]);
    let artifact_correct = correct(&workspace, &expected, &protected);
    let attested = events.iter().any(|event| {
        event["method"] == "aster/turn/outcome"
            && event["params"]["outcome"]["terminal"] == "verified"
            && event["params"]["outcome"]["status"] == "passed"
    });
    let pass = work.is_ok()
        && artifact_correct
        && facts_available
        && attested
        && completed.len() >= 5
        && published == 5
        && !source_mutation
        && !early_mutation
        && receipts_complete
        && cleanup.is_ok()
        && tools_terminal
        && !overflow
        && campaign.identity_unchanged()
        && relay
            .as_ref()
            .is_ok_and(|r| r.upstream_calls > 0 && r.statuses.get(&200).copied().unwrap_or(0) > 0);
    drop(oracle_timing);
    let _result_timing = artifact::timing::span(artifact::timing::Phase::ResultWrite);
    let record = json!({"pass":pass,"artifact_correct":artifact_correct,"prompt_result":work,
        "model":campaign.model,"pair_build_id":campaign.build_id,"elapsed_ms":elapsed,
        "published_versions":published,"completed_reads":completed.len(),"compactions":compactions,
        "bound_verified_outcome":attested,"original_facts_available":facts_available,
        "source_mutation":source_mutation,"early_mutation":early_mutation,"receipts_complete":receipts_complete,"read_receipts":read_receipts,"changed_paths":changed_paths,"cleanup":cleanup,
        "tools_terminal":tools_terminal,"overflow":overflow,"relay":relay,
        "input_registration":registration,"limits":{"requests":40,"output_tokens":campaign.output_tokens,"context_tokens":24576,"prompt_ms":600000,"workflow_ms":1800000},
        "coverage_note":"compaction facts and actual summary-model calls are separate observations; no forced count makes task correctness pass"});
    std::fs::write(
        campaign.output.join("result.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    artifact::write_public_compaction_evidence(&campaign.output, &facts).unwrap();
    std::fs::write(
        campaign.output.join("wire-notifications.json"),
        serde_json::to_vec_pretty(
            &events
                .iter()
                .filter(|event| event["update"]["sessionUpdate"] != "agent_thought_chunk")
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    )
    .unwrap();
    for name in ["selection.json", "CONTRACT.md", "notes.txt", "events.jsonl"] {
        if let Ok(bytes) = artifact::read(&workspace, name) {
            std::fs::write(campaign.output.join(name), bytes).unwrap();
        }
    }
    println!(
        "DELOOP_HOLDOUT_RESULT {}",
        json!({"pass":pass,"artifact_correct":artifact_correct,"evidence":campaign.output})
    );
    assert!(
        pass,
        "independent holdout failed; original evidence retained"
    );
}
