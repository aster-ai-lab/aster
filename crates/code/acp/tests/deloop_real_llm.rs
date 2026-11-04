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
#[path = "deloop_real_llm/self_checks.rs"]
mod self_checks;
#[path = "long_horizon/upstream.rs"]
mod upstream;

use agent_client_protocol as acp;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

const VERSIONS: u64 = 7;
const NOTES: &str = "Preserve this unrelated user draft exactly.\n";
const REQUEST: &str = "Read README.md. Produce report.json for all seven distinct versions of feed.jsonl. This external feed advances after each completed read, so read it sequentially in separate tool turns with the same read arguments. Do not write intermediate files or run commands. Preserve each batch's calculation in your working context, including across compaction. Only after collecting all seven versions write the final report.json, then finish. Use native file tools. Do not modify feed.jsonl, README.md or notes.txt.";
const CONTRACT: &str = "feed.jsonl has one JSON header line followed by item JSON lines. Header: version, versions, currency, discount_bps, tax_bps. Each item has sku, quantity and unit_cents; description is provenance. Calculate each batch independently: units=sum quantities; subtotal_cents=sum(quantity*unit_cents); discount_cents=floor((subtotal_cents*discount_bps+5000)/10000); tax_cents=floor(((subtotal_cents-discount_cents)*tax_bps+5000)/10000); total_cents=subtotal_cents-discount_cents+tax_cents. All arithmetic uses integer cents. report.json must contain exactly batches and total_cents. batches is an array ordered by version 1..7, each with exactly version, units, subtotal_cents, discount_cents, tax_cents, total_cents. The outer total_cents sums all seven batch totals. Read all item lines. Source versions advance externally after each completed read; do not manufacture versions or edit sources. No intermediate writes; final report.json is the only writable file.\n";

fn feed(version: u64) -> String {
    let mut lines = vec![json!({"version":version,"versions":VERSIONS,"currency":"USD","discount_bps":version*50,"tax_bps":750}).to_string()];
    lines.extend((0..48).map(|index| json!({"sku":format!("batch-{version}-item-{index:02}"),"quantity":version+index%5+1,"unit_cents":97+index*13+version,
        "description":format!("External invoice batch {version}, source record {index}. Retain this record's quantity and price for the ordered batch audit; do not substitute records from another batch.")}).to_string()));
    lines.join("\n") + "\n"
}

fn expected_report() -> Value {
    let batches: Vec<_> = (1..=VERSIONS).map(|version| {
        let rows: Vec<Value> = feed(version).lines().map(|line|serde_json::from_str(line).unwrap()).collect();
        let units: u64 = rows[1..].iter().map(|item|item["quantity"].as_u64().unwrap()).sum();
        let subtotal: u64 = rows[1..].iter().map(|item|item["quantity"].as_u64().unwrap()*item["unit_cents"].as_u64().unwrap()).sum();
        let discount = (subtotal*rows[0]["discount_bps"].as_u64().unwrap()+5000)/10000;
        let tax = ((subtotal-discount)*rows[0]["tax_bps"].as_u64().unwrap()+5000)/10000;
        json!({"version":version,"units":units,"subtotal_cents":subtotal,"discount_cents":discount,"tax_cents":tax,"total_cents":subtotal-discount+tax})
    }).collect();
    let total: u64 = batches
        .iter()
        .map(|batch| batch["total_cents"].as_u64().unwrap())
        .sum();
    json!({"batches":batches,"total_cents":total})
}

fn correct(workspace: &Path) -> bool {
    let _timing = artifact::timing::span(artifact::timing::Phase::Oracle);
    artifact::read(workspace, "report.json")
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .as_ref()
        == Some(&expected_report())
        && artifact::read(workspace, "README.md").ok().as_deref() == Some(CONTRACT.as_bytes())
        && artifact::read(workspace, "notes.txt").ok().as_deref() == Some(NOTES.as_bytes())
}

fn path_matches(input: &str, workspace: &Path, name: &str) -> bool {
    let input = Path::new(input);
    if input.is_absolute() {
        input == workspace.join(name)
    } else {
        workspace.join(input) == workspace.join(name)
    }
}

fn is_feed_request(args: &Value, workspace: &Path) -> bool {
    args["file_path"]
        .as_str()
        .is_some_and(|path| path_matches(path, workspace, "feed.jsonl"))
        || args["files"].as_array().is_some_and(|files| {
            files.iter().any(|entry| {
                entry["path"]
                    .as_str()
                    .is_some_and(|path| path_matches(path, workspace, "feed.jsonl"))
            })
        })
}

fn observe_feed_reads(
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
        if update["kind"] == "read" && is_feed_request(&update["rawInput"], workspace) {
            pending.insert(id.to_owned());
        }
        if update["status"] == "completed" && pending.contains(id) {
            completed.insert(id.to_owned());
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "real GLM single-Run changing feed and forced compaction; explicit ACL/model/evidence required"]
async fn changing_external_batches_survive_compaction_in_one_real_acp_run() {
    tokio::task::LocalSet::new()
        .run_until(run_changing_batches(
            "deloop-changing-feed",
            live::qualification_output_tokens(),
            16_384,
        ))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "independent 8K capacity diagnostic; original 4K qualification remains separate"]
async fn changing_external_batches_capacity_diagnostic_with_8192_output() {
    tokio::task::LocalSet::new()
        .run_until(run_changing_batches(
            "deloop-changing-feed-8k",
            8192,
            16_384,
        ))
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "independent 32K context/8K output diagnostic; original conditions remain separate"]
async fn changing_external_batches_context_diagnostic_with_32768_context() {
    tokio::task::LocalSet::new()
        .run_until(run_changing_batches(
            "deloop-changing-feed-32k",
            8192,
            32_768,
        ))
        .await;
}

async fn run_changing_batches(prefix: &str, output_tokens: u32, context_tokens: u32) {
    let mut campaign =
        live::Campaign::prepare_with_output(prefix, Some(context_tokens), output_tokens).await;
    let fixture_timing = artifact::timing::span(artifact::timing::Phase::FixtureSetup);
    let workspace = campaign.root.path().join("workspace 观察");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::write(workspace.join("README.md"), CONTRACT).unwrap();
    std::fs::write(workspace.join("notes.txt"), NOTES).unwrap();
    std::fs::write(workspace.join("feed.jsonl"), feed(1)).unwrap();
    assert!(!correct(&workspace));
    drop(fixture_timing);
    let driver = campaign.spawn(&workspace, None, false);
    let mut session = None;
    let mut pending = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut cursor = 0;
    let mut published = 1;
    let started = Instant::now();
    let work = tokio::time::timeout(Duration::from_secs(1800),async {
            let opened = driver.open(&workspace,Some(json!({"asterHost":{"completionAttestation":"disk_content"}}).as_object().unwrap().clone())).await?;
            session = Some(opened.clone());
            let prompt = driver.prompt(&opened,REQUEST);
            tokio::pin!(prompt);
            let mut tick = tokio::time::interval(Duration::from_millis(20));
            loop { tokio::select! {
                result = &mut prompt => break result,
                _ = tick.tick() => {
                    let events = { let capture = driver.capture.lock().unwrap(); let events = capture.events[cursor..].to_vec(); cursor = capture.events.len(); events };
                    observe_feed_reads(&events,&workspace,&mut pending,&mut completed);
                    let next_version=(completed.len() as u64+1).min(VERSIONS);
                    if next_version != published {
                        let _timing = artifact::timing::span(artifact::timing::Phase::FeedPublish);
                        published=next_version;
                        tokio::fs::write(workspace.join("feed.jsonl"),feed(published)).await.map_err(|_|"external_feed_write")?;
                    }
                }
            }}
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
    let cleanup = driver.close().await;
    let relay = campaign.relay.take().unwrap().close().await;
    let fact_timing = artifact::timing::span(artifact::timing::Phase::FactRead);
    let fact_read = session.as_ref().map(|session| {
        aster_core::fact_control::read_workspace_facts(&workspace, session.0.as_ref())
    });
    let facts_available = fact_read.as_ref().is_some_and(Result::is_ok);
    let facts = fact_read.and_then(Result::ok).unwrap_or_default();
    drop(fact_timing);
    let oracle_timing = artifact::timing::span(artifact::timing::Phase::Oracle);
    let preparations: BTreeMap<_, _> = facts
        .iter()
        .filter(|fact| fact.kind == "tool.prepared")
        .filter_map(|fact| {
            serde_json::from_value::<aster_effect::ToolPreparedV1>(fact.payload["prepared"].clone())
                .ok()
        })
        .map(|prepared| (prepared.identity.key().unwrap(), prepared))
        .collect();
    let feed_arguments: BTreeSet<_> = facts
        .iter()
        .filter(|fact| fact.kind == "model.turn")
        .filter_map(|fact| {
            serde_json::from_value::<aster_effect::ModelDecision>(fact.payload.clone()).ok()
        })
        .flat_map(|decision| match decision {
            aster_effect::ModelDecision::Tool { call, also } => {
                std::iter::once(call).chain(also).collect()
            }
            _ => Vec::new(),
        })
        .filter(|call| call.name == "read" && is_feed_request(&call.args, &workspace))
        .map(|call| aster_effect::tool_args_digest(&call.args).unwrap())
        .collect();
    let mut observations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut changed_paths = BTreeSet::new();
    let mut source_mutation = false;
    let mut early_mutation = false;
    let mut results = 0;
    for fact in facts.iter().filter(|fact| fact.kind == "tool.result") {
        let Ok(result) = serde_json::from_value::<aster_effect::ToolExecutionResultV1>(
            fact.payload["receipt"].clone(),
        ) else {
            continue;
        };
        let Some(prepared) = preparations.get(&result.identity.key().unwrap()) else {
            continue;
        };
        changed_paths.extend(
            result
                .effects
                .changes
                .iter()
                .map(|change| change.path.clone()),
        );
        source_mutation |= result
            .effects
            .changes
            .iter()
            .any(|change| path_matches(&change.path, &workspace, "feed.jsonl"));
        if prepared.tool_name == "read"
            && feed_arguments.contains(&prepared.original_args_digest)
            && result.outcome == aster_effect::ToolExecutionOutcome::Succeeded
        {
            if let Some(identity) = fact.payload["informationIdentity"].as_str() {
                observations
                    .entry(prepared.effective_args_digest.clone())
                    .or_default()
                    .insert(identity.to_owned());
                results += 1;
            }
        }
        early_mutation |= results < VERSIONS && !result.effects.changes.is_empty();
    }
    let compactions = facts
        .iter()
        .filter(|fact| fact.kind == "compaction.done")
        .count();
    let attested = events.iter().any(|event| {
        event["method"] == "aster/turn/outcome"
            && event["params"]["outcome"]["terminal"] == "verified"
            && event["params"]["outcome"]["status"] == "passed"
    });
    let workflow_pass = work.is_ok()
        && correct(&workspace)
        && facts_available
        && attested
        && published == VERSIONS
        && completed.len() >= VERSIONS as usize
        && !source_mutation
        && !early_mutation
        && cleanup.is_ok()
        && tools_terminal
        && !overflow;
    let same_argument_observations = observations.values().map(BTreeSet::len).max().unwrap_or(0);
    let mechanism_exercised = published == VERSIONS
        && completed.len() >= VERSIONS as usize
        && same_argument_observations >= VERSIONS as usize
        && !early_mutation
        && !source_mutation
        && compactions >= 2;
    let identity = campaign.identity_unchanged();
    let pass = workflow_pass
        && mechanism_exercised
        && identity
        && relay.as_ref().is_ok_and(|stats| {
            stats.upstream_calls > 0 && stats.statuses.get(&200).copied().unwrap_or(0) > 0
        });
    drop(oracle_timing);
    let _result_timing = artifact::timing::span(artifact::timing::Phase::ResultWrite);
    let record = json!({"pass":pass,"workflow_pass":workflow_pass,"mechanism_exercised":mechanism_exercised,"model":campaign.model,"context_budget":context_tokens,"pair_build_id":campaign.build_id,"acp_sha256":campaign.acp_hash,"aster_sha256":campaign.aster_hash,"frozen_pair_directory":campaign.binary.parent(),"identity_unchanged":identity,"elapsed_ms":elapsed,"prompt_result":work,"oracle_correct":correct(&workspace),"expected":expected_report(),"agent_changed_paths":facts_available.then_some(&changed_paths),"source_agent_mutation":facts_available.then_some(source_mutation),"early_agent_mutation":facts_available.then_some(early_mutation),"published_versions":published,"completed_feed_reads":completed.len(),"original_facts_available":facts_available,"read_results":facts_available.then_some(results),"same_argument_information_identities":facts_available.then_some(same_argument_observations),"read_argument_identities":facts_available.then_some(observations.len()),"compactions":facts_available.then_some(compactions),"bound_verified_outcome":attested,"cleanup":cleanup,"tools_terminal":tools_terminal,"overflow":overflow,"relay":relay,"limits":{"requests":40,"output_tokens":output_tokens,"workflow_ms":1800000,"prompt_ms":600000},"usage_note":"Native usage can be unknown; never convert missing counters to zero cost."});
    std::fs::write(
        campaign.output.join("result.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    artifact::write_public_compaction_evidence(&campaign.output, &facts).unwrap();
    std::fs::write(
        campaign.output.join("wire-notifications.json"),
        serde_json::to_vec_pretty(&events).unwrap(),
    )
    .unwrap();
    for name in ["report.json", "README.md", "notes.txt", "feed.jsonl"] {
        if let Ok(bytes) = artifact::read(&workspace, name) {
            std::fs::write(campaign.output.join(name), bytes).unwrap();
        }
    }
    println!(
        "DELOOP_GLM_RESULT {}",
        json!({"pass":pass,"evidence":campaign.output,"compactions":compactions,"published":published})
    );
    assert!(pass, "retained workflow and mechanism verdicts separately");
}
