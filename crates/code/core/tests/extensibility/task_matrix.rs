#![cfg(test)]

//! One explicitly frozen T023 Run per process. No credentials, reasoning or
//! provider response text is copied into its metadata report.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use a3s_code_core::{
    permissions::{PermissionDecision, PermissionPolicy},
    Agent, AgentEvent, CodeConfig, PlanningMode, RunStatus, SessionOptions, ToolPresentationModeV1,
    ToolPresentationProfileV1,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::{
    field::{Field, Visit},
    span::{Attributes, Id},
    Event, Subscriber,
};
use tracing_subscriber::{
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
    Layer,
};

const MAX_MANIFEST: usize = 4 * 1024 * 1024;
const MAX_REPORT: usize = 16 * 1024 * 1024;
const MAX_LOG: u64 = 64 * 1024 * 1024;
const MAX_LINE: usize = 2 * 1024 * 1024;
const MAX_RECEIPTS: usize = 1024;
const ROUTE: &str = "deepseek/deepseek-flash";
const ENDPOINT: &str = "https://api.deepseek.com";
type Check<T> = Result<T, &'static str>;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenTree {
    root: PathBuf,
    #[serde(deserialize_with = "unique_hashes")]
    files: BTreeMap<String, String>,
    #[serde(deserialize_with = "unique_hashes")]
    symlinks: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Budget {
    max_execution_ms: u64,
    llm_api_timeout_ms: u64,
    tool_timeout_ms: u64,
    max_tool_rounds: usize,
    max_continuation_turns: u32,
    max_parallel_tasks: usize,
    thinking_budget: usize,
    reasoning_effort: String,
    reported_token_limit: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    run_id: String,
    trusted_acl: PathBuf,
    route: String,
    endpoint: String,
    profile: ToolPresentationModeV1,
    workspace: PathBuf,
    #[serde(deserialize_with = "unique_hashes")]
    inputs: BTreeMap<String, String>,
    prompt: String,
    source: FrozenTree,
    assets: FrozenTree,
    runner_sha256: String,
    build_features: Vec<String>,
    permissions: Vec<String>,
    budget: Budget,
    submissions: Vec<String>,
    evidence_dir: PathBuf,
}

fn unique_hashes<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("unique frozen path hashes")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut access: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<String, String>()? {
                if values.len() >= 8192 || values.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom(
                        "duplicate or excessive frozen paths",
                    ));
                }
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(Unique)
}

#[derive(Default, Serialize)]
struct CommandPhases {
    closed: bool,
    phases_ms: BTreeMap<String, Vec<f64>>,
}

#[derive(Default)]
struct VmObservations {
    next: AtomicU64,
    commands: Mutex<BTreeMap<u64, CommandPhases>>,
    incomplete: AtomicBool,
}

struct CommandKey(u64);
struct VmLayer(Arc<VmObservations>);

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for VmLayer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().target() != "a3s_code_core::sandbox::oci"
            || attrs.metadata().name() != "oci_command"
        {
            return;
        }
        let key = self.0.next.fetch_add(1, Ordering::SeqCst);
        let mut commands = self.0.commands.lock().unwrap();
        if commands.len() >= 4096 {
            self.0.incomplete.store(true, Ordering::SeqCst);
            return;
        }
        commands.insert(key, CommandPhases::default());
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(CommandKey(key));
        }
    }
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if event.metadata().target() != "a3s_code_core::sandbox::oci" {
            return;
        }
        #[derive(Default)]
        struct Fields {
            phase: Option<String>,
            elapsed: Option<f64>,
        }
        impl Visit for Fields {
            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() == "phase" {
                    self.phase = Some(value.into());
                }
            }
            fn record_f64(&mut self, field: &Field, value: f64) {
                if field.name() == "elapsed_ms" {
                    self.elapsed = Some(value);
                }
            }
            fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let (Some(phase), Some(elapsed)) = (fields.phase, fields.elapsed) else {
            return;
        };
        let Some(scope) = ctx.event_scope(event) else {
            self.0.incomplete.store(true, Ordering::SeqCst);
            return;
        };
        for span in scope {
            if let Some(key) = span.extensions().get::<CommandKey>() {
                if let Some(command) = self.0.commands.lock().unwrap().get_mut(&key.0) {
                    command.phases_ms.entry(phase).or_default().push(elapsed);
                }
                return;
            }
        }
        self.0.incomplete.store(true, Ordering::SeqCst);
    }
    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(&id) {
            if let Some(key) = span.extensions().get::<CommandKey>() {
                if let Some(command) = self.0.commands.lock().unwrap().get_mut(&key.0) {
                    command.closed = true;
                }
            }
        }
    }
}

impl VmObservations {
    fn summary(&self) -> Value {
        let commands = self.commands.lock().unwrap();
        let expected = [
            "host_connect",
            "bundle_prepare",
            "vm_create",
            "process_start",
            "collect_wait",
            "vm_delete",
            "host_shutdown",
            "scratch_remove",
        ];
        let complete = !self.incomplete.load(Ordering::SeqCst)
            && commands.values().all(|command| {
                command.closed
                    && command.phases_ms.len() == expected.len()
                    && expected.iter().all(|phase| {
                        command.phases_ms.get(*phase).is_some_and(|values| {
                            values.len() == 1 && values[0].is_finite() && values[0] >= 0.0
                        })
                    })
            });
        json!({"complete":complete,"vm_count":complete.then_some(commands.len()),"commands":*commands,
            "cause":if complete {Value::Null}else{json!("incomplete_command_phase_observation")},
            "source":"OCI execute span and successful vm_create event; complete phases and closed spans required; never inferred from Bash calls"})
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn bounded(path: &Path, maximum: usize) -> Check<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "file_unavailable")?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err("file_type_or_size_refused");
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "file_unavailable")?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "file_read_failed")?;
    if bytes.len() > maximum {
        return Err("file_grew_past_bound");
    }
    Ok(bytes)
}

fn relative(raw: &str) -> Check<&Path> {
    let path = Path::new(raw);
    if raw.is_empty()
        || raw.contains('\\')
        || raw
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.is_absolute()
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("relative_path_refused");
    }
    Ok(path)
}

fn canonical_directory(path: &Path) -> Check<PathBuf> {
    if !path.is_absolute() || !fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err("directory_unavailable");
    }
    let canonical = path.canonicalize().map_err(|_| "directory_unavailable")?;
    if canonical != path {
        return Err("directory_not_canonical");
    }
    Ok(canonical)
}

fn file_hash(path: &Path, maximum: u64) -> Check<String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "frozen_file_missing")?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err("frozen_file_type_or_size_refused");
    }
    let mut reader = File::open(path)
        .map_err(|_| "frozen_file_missing")?
        .take(maximum + 1);
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut total = 0;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| "frozen_file_read_failed")?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > maximum {
            return Err("frozen_file_grew_past_bound");
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn frozen_files(root: &Path, files: &BTreeMap<String, String>, maximum: u64) -> Check<()> {
    if files.is_empty() || files.len() > 8192 {
        return Err("frozen_file_count_refused");
    }
    for (name, expected) in files {
        let path = root.join(relative(name)?);
        let mut parent = path.parent();
        while let Some(directory) = parent.filter(|path| *path != root) {
            if !fs::symlink_metadata(directory).is_ok_and(|m| m.is_dir()) {
                return Err("frozen_parent_symlink_or_missing");
            }
            parent = directory.parent();
        }
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || file_hash(&path, maximum)? != *expected
        {
            return Err("frozen_file_digest_mismatch");
        }
    }
    Ok(())
}

fn tree_entries(root: &Path) -> Check<(BTreeSet<String>, BTreeMap<String, String>)> {
    let mut pending = vec![(root.to_path_buf(), 0)];
    let mut files = BTreeSet::new();
    let mut links = BTreeMap::new();
    let mut total = 0u64;
    let mut entries = 0usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 64 {
            return Err("tree_depth_refused");
        }
        for entry in fs::read_dir(directory).map_err(|_| "tree_read_failed")? {
            entries += 1;
            if entries > 8192 {
                return Err("tree_size_refused");
            }
            let entry = entry.map_err(|_| "tree_read_failed")?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|_| "tree_read_failed")?;
            let name = path
                .strip_prefix(root)
                .map_err(|_| "tree_escape")?
                .to_str()
                .ok_or("non_utf8_path")?
                .replace('\\', "/");
            if kind.is_symlink() {
                // Guest absolute links are identities, never host read targets.
                let target = fs::read_link(path).map_err(|_| "symlink_read_failed")?;
                links.insert(
                    name,
                    digest(target.to_str().ok_or("non_utf8_symlink")?.as_bytes()),
                );
            } else if kind.is_dir() {
                pending.push((path, depth + 1));
            } else if kind.is_file() {
                total = total
                    .checked_add(entry.metadata().map_err(|_| "tree_read_failed")?.len())
                    .ok_or("tree_size_refused")?;
                files.insert(name);
            } else {
                return Err("special_file_refused");
            }
            if files.len() + links.len() + pending.len() > 8192 || total > 2 * 1024 * 1024 * 1024 {
                return Err("tree_size_refused");
            }
        }
    }
    Ok((files, links))
}

fn validate(manifest: &Manifest) -> Check<(CodeConfig, String)> {
    if manifest.schema != "aster.task-matrix-run.v1"
        || manifest.run_id.is_empty()
        || manifest.run_id.len() > 64
        || !manifest
            .run_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        || manifest.route != ROUTE
        || manifest.endpoint != ENDPOINT
        || !matches!(
            manifest.profile,
            ToolPresentationModeV1::Adaptive | ToolPresentationModeV1::Code
        )
    {
        return Err("run_identity_refused");
    }
    let budget = &manifest.budget;
    if !(1..=1_800_000).contains(&budget.max_execution_ms)
        || !(1..=600_000).contains(&budget.llm_api_timeout_ms)
        || !(1..=300_000).contains(&budget.tool_timeout_ms)
        || !(1..=256).contains(&budget.max_tool_rounds)
        || budget.max_continuation_turns > 32
        || !(1..=8).contains(&budget.max_parallel_tasks)
        || budget.thinking_budget == 0
        || budget.thinking_budget > 100_000
        || !matches!(
            budget.reasoning_effort.as_str(),
            "low" | "medium" | "high" | "xhigh" | "max"
        )
        || budget.reported_token_limit == 0
        || budget.reported_token_limit > 2_000_000
    {
        return Err("run_budget_refused");
    }
    let workspace = canonical_directory(&manifest.workspace)?;
    let source = canonical_directory(&manifest.source.root)?;
    let assets = canonical_directory(&manifest.assets.root)?;
    if source
        != Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .map_err(|_| "source_root_unavailable")?
        || workspace.starts_with(&assets)
        || assets.starts_with(&workspace)
        || manifest.evidence_dir.starts_with(&workspace)
        || workspace.starts_with(&manifest.evidence_dir)
        || manifest.permissions.is_empty()
        || manifest.permissions.len() > 128
        || manifest.permissions.iter().any(|rule| rule.len() > 1024)
        || manifest.submissions.is_empty()
        || manifest.submissions.len() > 16
        || !manifest.trusted_acl.is_absolute()
        || manifest.trusted_acl.starts_with(&workspace)
        || manifest.build_features.is_empty()
        || manifest.build_features.len() > 256
    {
        return Err("run_boundary_refused");
    }
    for required in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "crates/code/core/tests/test_extensibility_real_llm.rs",
        "crates/code/core/tests/extensibility/metrics.rs",
        "crates/code/core/tests/extensibility/task_matrix.rs",
    ] {
        if !manifest.source.files.contains_key(required) {
            return Err("source_manifest_incomplete");
        }
    }
    if !manifest.source.symlinks.is_empty() {
        return Err("source_symlink_refused");
    }
    frozen_files(&source, &manifest.source.files, 64 * 1024 * 1024)?;
    frozen_files(&assets, &manifest.assets.files, 512 * 1024 * 1024)?;
    let (asset_files, asset_links) = tree_entries(&assets)?;
    if asset_files != manifest.assets.files.keys().cloned().collect()
        || asset_links != manifest.assets.symlinks
    {
        return Err("asset_manifest_incomplete_or_changed");
    }
    frozen_files(&workspace, &manifest.inputs, 32 * 1024 * 1024)?;
    let (input_files, input_links) = tree_entries(&workspace)?;
    if input_files != manifest.inputs.keys().cloned().collect() || !input_links.is_empty() {
        return Err("workspace_not_fresh_or_inputs_unfrozen");
    }
    if manifest.prompt.is_empty() || manifest.prompt.len() > 256 * 1024 {
        return Err("prompt_bound_refused");
    }
    for path in &manifest.submissions {
        relative(path)?;
    }
    if manifest.submissions.iter().collect::<BTreeSet<_>>().len() != manifest.submissions.len() {
        return Err("duplicate_submission_refused");
    }
    if file_hash(
        &std::env::current_exe().map_err(|_| "test_executable_unavailable")?,
        1024 * 1024 * 1024,
    )? != manifest.runner_sha256
    {
        return Err("test_executable_digest_mismatch");
    }
    let raw = bounded(&manifest.trusted_acl, 1024 * 1024)?;
    let acl = std::str::from_utf8(&raw).map_err(|_| "trusted_acl_not_utf8")?;
    let declared = CodeConfig::from_acl(acl).map_err(|_| "trusted_acl_invalid")?;
    let config = select_model(declared)?;
    Ok((config, manifest.prompt.clone()))
}

fn select_model(declared: CodeConfig) -> Check<CodeConfig> {
    if declared
        .providers
        .iter()
        .filter(|provider| provider.name == "deepseek")
        .count()
        != 1
    {
        return Err("model_provider_not_uniquely_declared");
    }
    let mut provider = declared
        .find_provider("deepseek")
        .cloned()
        .ok_or("model_provider_not_declared")?;
    if provider.base_url.as_deref() != Some(ENDPOINT) {
        return Err("model_endpoint_mismatch");
    }
    provider.models.retain(|model| model.id == "deepseek-flash");
    if provider.models.len() != 1 {
        return Err("model_route_not_uniquely_declared");
    }
    let model = &provider.models[0];
    if provider.get_base_url(model) != Some(ENDPOINT) {
        return Err("model_endpoint_mismatch");
    }
    if !provider
        .get_api_key(model)
        .is_some_and(|key| !key.trim().is_empty())
    {
        return Err("model_credentials_missing");
    }
    Ok(CodeConfig {
        default_model: Some(ROUTE.into()),
        providers: vec![provider],
        project_doc_max_bytes: Some(0),
        ..Default::default()
    })
}

fn write_report(directory: &Path, value: &Value) -> Check<()> {
    write_record(directory, "run.json", value)
}

fn write_record(directory: &Path, name: &str, value: &Value) -> Check<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "report_encode_failed")?;
    if bytes.len() > MAX_REPORT {
        return Err("report_bound_exceeded");
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(directory.join(name))
        .map_err(|_| "report_write_failed")?;
    file.write_all(&bytes).map_err(|_| "report_write_failed")?;
    file.sync_all().map_err(|_| "report_write_failed")
}

fn reserve_evidence(manifest: &Manifest) -> Check<()> {
    let path = &manifest.evidence_dir;
    let workspace = canonical_directory(&manifest.workspace)?;
    let assets = canonical_directory(&manifest.assets.root)?;
    if !path.is_absolute()
        || path.exists()
        || path.starts_with(&workspace)
        || workspace.starts_with(path)
        || path.starts_with(&assets)
        || assets.starts_with(path)
    {
        return Err("evidence_boundary_refused");
    }
    let parent = canonical_directory(path.parent().ok_or("evidence_parent_missing")?)?;
    if parent.join(path.file_name().ok_or("evidence_name_missing")?) != *path {
        return Err("evidence_path_not_canonical");
    }
    fs::create_dir(path).map_err(|_| "evidence_directory_refused")
}

fn receipt_evidence(workspace: &Path, session: &str) -> Check<Value> {
    let path = a3s_code_core::session_folder::effect_log_dir(workspace, session)
        .join(format!("{session}.jsonl"));
    if !path.exists() {
        return Ok(json!({"available":false,"reason":"original_log_missing"}));
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| "receipt_log_unavailable")?;
    if !metadata.is_file() || metadata.len() > MAX_LOG {
        return Err("receipt_log_bound_refused");
    }
    let mut reader =
        BufReader::new(File::open(&path).map_err(|_| "receipt_log_unavailable")?).take(MAX_LOG + 1);
    let mut receipts = Vec::new();
    let mut line = Vec::new();
    let mut total = 0;
    loop {
        line.clear();
        let count = (&mut reader)
            .take(MAX_LINE as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|_| "receipt_read_failed")?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if count > MAX_LINE || total > MAX_LOG {
            return Err("receipt_log_bound_refused");
        }
        let fact: Value = serde_json::from_slice(&line).map_err(|_| "receipt_log_invalid")?;
        if fact["type"] != "tool.result" {
            continue;
        }
        let Some(receipt) = fact["payload"].get("receipt") else {
            continue;
        };
        if receipts.len() >= MAX_RECEIPTS {
            return Err("receipt_count_refused");
        }
        let typed: a3s_effect::coding::ToolExecutionResultV1 =
            serde_json::from_value(receipt.clone()).map_err(|_| "receipt_schema_invalid")?;
        typed.validate().map_err(|_| "receipt_schema_invalid")?;
        let bytes = serde_json::to_vec(receipt).map_err(|_| "receipt_encode_failed")?;
        let output = serde_json::to_vec(&typed.output).map_err(|_| "receipt_encode_failed")?;
        let effects = serde_json::to_vec(&typed.effects).map_err(|_| "receipt_encode_failed")?;
        receipts.push(json!({"sequence":fact["seq"],"sha256":digest(&bytes),"bytes":bytes.len(),
            "schema":typed.schema,"identity":typed.identity,"prepared_digest":typed.prepared_digest,
            "outcome":typed.outcome,"exit_code":typed.exit_code,
            "output":{"sha256":digest(&output),"bytes":output.len()},
            "effects":{"sha256":digest(&effects),"bytes":effects.len(),"completeness":typed.effects.completeness,"change_count":typed.effects.changes.len()},
            "plaintext_copied":false}));
    }
    Ok(
        json!({"available":true,"original_log":path,"original_log_sha256":file_hash(&path,MAX_LOG)?,"receipts":receipts}),
    )
}

fn receipt_parent_ids(evidence: &Value, run: &str) -> Check<Vec<String>> {
    if evidence["available"] != true {
        return Err("parent_receipts_unavailable");
    }
    let receipts = evidence["receipts"]
        .as_array()
        .ok_or("parent_receipts_unavailable")?;
    let mut ids = BTreeSet::new();
    for receipt in receipts {
        if receipt["identity"]["admission"]["run_id"] != run {
            return Err("parent_receipt_run_identity_mismatch");
        }
        let id = receipt["identity"]["tool_call_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("parent_receipt_identity_missing")?;
        if !ids.insert(id.to_owned()) {
            return Err("duplicate_parent_tool_id");
        }
    }
    Ok(ids.into_iter().collect())
}

fn measured_tool_metrics(metrics: &Value, event_window_complete: bool) -> bool {
    event_window_complete
        && metrics["origin_complete"] == true
        && [
            "parent_tool_calls",
            "child_tool_calls",
            "tool_interval_union_ms",
            "parent_interval_union_ms",
            "parent_output_bytes",
            "child_output_bytes",
        ]
        .iter()
        .all(|name| metrics[*name].as_u64().is_some())
        && metrics["program_child_intervals"]
            .as_array()
            .is_some_and(|programs| {
                programs.iter().all(|program| {
                    program["calls"].as_u64().is_some()
                        && program["calls"] == program["timed_calls"]
                        && program["interval_union_ms"].as_u64().is_some()
                })
            })
}

fn event_metadata(events: &[a3s_code_core::run::RunEventRecord]) -> Value {
    let mut presentation = Vec::new();
    let mut inputs = Vec::new();
    let mut terminal = Value::Null;
    let mut error_events = 0;
    for record in events {
        match &record.event {
            AgentEvent::ModelPresentationBound { snapshot } => presentation.push(json!(snapshot)),
            AgentEvent::ModelInputBound { snapshot } => inputs.push(json!(snapshot)),
            AgentEvent::End {
                text,
                completion_terminal,
                forced_stop,
                ..
            } => {
                terminal = json!({
                "completion":completion_terminal,"forced_stop":forced_stop,
                "final_text_bytes":text.len(),"final_text_sha256":digest(text.as_bytes())})
            }
            AgentEvent::Error { .. } => error_events += 1,
            _ => {}
        }
    }
    json!({"presentation":presentation,"model_inputs":inputs,"terminal":terminal,"error_event_count":error_events})
}

fn freeze_submissions(manifest: &Manifest) -> Check<Value> {
    let directory = manifest.evidence_dir.join("submission");
    fs::create_dir(&directory).map_err(|_| "submission_snapshot_directory_failed")?;
    let mut files = Vec::new();
    for name in &manifest.submissions {
        let path = manifest.workspace.join(relative(name)?);
        if !path.exists() {
            files.push(json!({"path":name,"present":false}));
            continue;
        }
        let canonical = path.canonicalize().map_err(|_| "submission_unavailable")?;
        if !canonical.starts_with(&manifest.workspace) {
            return Err("submission_escape_refused");
        }
        let bytes = bounded(&path, 16 * 1024 * 1024)?;
        let target = directory.join(name);
        fs::create_dir_all(target.parent().ok_or("submission_parent_missing")?)
            .map_err(|_| "submission_snapshot_failed")?;
        fs::write(target, &bytes).map_err(|_| "submission_snapshot_failed")?;
        files.push(json!({"path":name,"present":true,"bytes":bytes.len(),"sha256":digest(&bytes)}));
    }
    Ok(json!(files))
}

fn immutable_inputs(
    workspace: &Path,
    inputs: &BTreeMap<String, String>,
    submissions: &[String],
) -> Check<()> {
    let frozen = inputs
        .iter()
        .filter(|(name, _)| !submissions.contains(name))
        .map(|(name, hash)| (name.clone(), hash.clone()))
        .collect::<BTreeMap<_, _>>();
    if frozen.is_empty() {
        Ok(())
    } else {
        frozen_files(workspace, &frozen, 32 * 1024 * 1024)
    }
}

async fn observed_run(manifest: &Manifest, config: CodeConfig, prompt: String) -> Check<Value> {
    if std::env::consts::OS != "macos" || std::env::consts::ARCH != "aarch64" {
        return Err("platform_not_qualified_by_this_fixture");
    }
    if std::env::var_os("A3S_CODE_ALLOW_PROCESS_HOST_SANDBOX").is_some() {
        return Err("process_host_override_refused");
    }
    let selected = std::env::var_os("ASTER_SANDBOX_DIR").ok_or("sandbox_override_required")?;
    if PathBuf::from(selected)
        .canonicalize()
        .map_err(|_| "sandbox_override_unavailable")?
        != manifest.assets.root
    {
        return Err("sandbox_override_digest_group_mismatch");
    }
    let observations = Arc::new(VmObservations::default());
    tracing_subscriber::registry()
        .with(VmLayer(Arc::clone(&observations)))
        .try_init()
        .map_err(|_| "runtime_observer_already_installed")?;
    use a3s_code_core::sandbox::BashSandbox;
    let sandbox = a3s_code_core::sandbox::oci::OciBashSandbox::discover(&manifest.workspace)
        .map_err(|_| "sandbox_preflight_refused")?;
    sandbox
        .validate_execution_available()
        .map_err(|_| "sandbox_preflight_refused")?;
    let preflight = Instant::now();
    let output = sandbox
        .exec(a3s_code_core::sandbox::SandboxCommandRequest {
            command: "printf t023-preflight".into(),
            guest_workspace: "/workspace".into(),
            timeout_ms: 5000,
            output_observer: None,
            env: None,
        })
        .await;
    sandbox.shutdown().await;
    let preflight_record = json!({"elapsed_ms":preflight.elapsed().as_millis(),"observations":observations.summary(),
        "settled_receipt":output.as_ref().ok().map(|out|json!({"exit_code":out.exit_code,"timed_out":out.timed_out,"stdout":out.stdout,"stderr":out.stderr})),
        "error":output.is_err()});
    write_record(
        &manifest.evidence_dir,
        "backend-preflight.json",
        &preflight_record,
    )?;
    let output = output.map_err(|_| "backend_probe_failed")?;
    if output.exit_code != 0
        || output.timed_out
        || output.stdout != "t023-preflight"
        || !output.stderr.is_empty()
        || preflight_record["observations"]["complete"] != true
        || preflight_record["observations"]["vm_count"] != 1
    {
        return Err("backend_probe_receipt_or_settlement_failed");
    }
    observations.commands.lock().unwrap().clear();
    observations.incomplete.store(false, Ordering::SeqCst);
    let token_guard = Arc::new(a3s_code_core::orchestration::WorkflowBudget::new(Some(
        manifest.budget.reported_token_limit,
    )));
    let mut policy = PermissionPolicy::new().allow_all(
        &manifest
            .permissions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    policy.default_decision = PermissionDecision::Deny;
    let budget = &manifest.budget;
    let mut options = SessionOptions::new()
        .with_session_id(&manifest.run_id)
        .with_model(ROUTE)
        .with_tool_presentation_profile(ToolPresentationProfileV1::for_mode(manifest.profile))
        .with_memory(Arc::new(a3s_memory::InMemoryStore::new()))
        .with_permission_policy(policy)
        .with_default_security()
        .with_planning_mode(PlanningMode::Disabled)
        .with_auto_delegation_enabled(false)
        .with_manual_delegation_enabled(false)
        .with_continuation(false)
        .with_max_continuation_turns(budget.max_continuation_turns)
        .with_max_tool_rounds(budget.max_tool_rounds)
        .with_max_parallel_tasks(budget.max_parallel_tasks)
        .with_tool_timeout(budget.tool_timeout_ms)
        .with_llm_api_timeout(budget.llm_api_timeout_ms)
        .with_thinking_budget(budget.thinking_budget)
        .with_reasoning_effort(&budget.reasoning_effort)
        .with_temperature(0.0)
        .with_auto_compact(true)
        .with_budget_guard(token_guard.clone());
    options.max_execution_time_ms = Some(budget.max_execution_ms);
    options.retention_limits = Some(a3s_code_core::retention::SessionRetentionLimits {
        max_runs_retained: Some(4),
        max_events_per_run: Some(262144),
        max_event_bytes_per_run: Some(64 * 1024 * 1024),
        max_trace_events: Some(4096),
        max_terminal_subagent_tasks: Some(32),
    });
    let preparation = Instant::now();
    let agent = Agent::from_config(config)
        .await
        .map_err(|error| error.runtime_code())?;
    let session = match agent
        .session_async(manifest.workspace.display().to_string(), Some(options))
        .await
    {
        Ok(session) => session,
        Err(error) => {
            let _ = tokio::time::timeout(Duration::from_secs(120), agent.close()).await;
            return Err(error.runtime_code());
        }
    };
    let preparation_ms = preparation.elapsed().as_millis();
    let initial_skill_count = session.skill_names().len();
    if initial_skill_count != 0 {
        let _ = tokio::time::timeout(Duration::from_secs(120), session.close()).await;
        let _ = tokio::time::timeout(Duration::from_secs(120), agent.close()).await;
        return Err("ambient_skills_present");
    }
    let began = Instant::now();
    let opened = session.stream_with_run(&prompt, None).await;
    let mut error_code = None;
    let mut timed_out = false;
    let mut stream_settled = true;
    if let Ok(mut stream) = opened {
        let drain = async { while stream.events.recv().await.is_some() {} };
        timed_out = tokio::time::timeout(Duration::from_millis(budget.max_execution_ms), drain)
            .await
            .is_err();
        if timed_out {
            stream_settled = session
                .cancel_and_settle(Duration::from_secs(2), Duration::from_secs(2))
                .await;
        }
        match tokio::time::timeout(Duration::from_secs(5), &mut stream.worker).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => stream_settled = false,
            Err(_) => {
                stream.worker.abort();
                let _ = stream.worker.await;
                stream_settled = false;
            }
        }
    } else if let Err(error) = opened {
        error_code = Some(error.runtime_code());
    }
    let task_ms = began.elapsed().as_millis();
    let close_settled = tokio::time::timeout(Duration::from_secs(120), session.close())
        .await
        .is_ok();
    let runs = session.runs().await;
    let parent_receipts = receipt_evidence(&manifest.workspace, &manifest.run_id);
    let parent_ids = parent_receipts
        .as_ref()
        .map_err(|code| *code)
        .and_then(|evidence| {
            if runs.len() != 1 {
                return Err("single_run_identity_missing");
            }
            receipt_parent_ids(evidence, &runs[0].id)
        });
    let mut run_reports = Vec::new();
    let vm = observations.summary();
    let vm_count = vm["vm_count"]
        .as_u64()
        .and_then(|count| usize::try_from(count).ok());
    let mut presentation_matches = true;
    let mut profiled_calls = 0;
    let mut ordinary_terminal = false;
    let mut event_window_complete = true;
    let mut metrics_complete = parent_ids.is_ok();
    for run in &runs {
        let events = session.run_events(&run.id).await;
        event_window_complete &= run.event_count == events.len();
        for event in &events {
            match &event.event {
                AgentEvent::ModelPresentationBound{snapshot} if snapshot.application==a3s_code_core::harness_evidence::ModelPresentationApplicationV1::Profiled => {
                    profiled_calls+=1;presentation_matches&=snapshot.profile.mode()==manifest.profile;
                }
                AgentEvent::End{forced_stop,..}=>ordinary_terminal=forced_stop.is_none(),
                _=>{}
            }
        }
        let metrics =
            super::metrics::summarize(&events, parent_ids.as_deref().unwrap_or(&[]), vm_count);
        metrics_complete &= measured_tool_metrics(&metrics, run.event_count == events.len());
        run_reports.push(json!({"run_id":run.id,"status":run.status,"event_count":run.event_count,
            "error":run.error.as_ref().map(|error|json!({"sha256":digest(error.as_bytes()),"bytes":error.len()})),
            "retained_events":events.len(),"event_window_complete":run.event_count==events.len(),
            "metrics":metrics,"evidence":event_metadata(&events)}));
    }
    let mut metadata = json!({"preparation_ms":preparation_ms,"task_ms":task_ms,
        "clock":"Core stream admission through event drain and bounded worker/cancel settlement; excludes backend preflight, preparation, close and independent oracle",
        "timed_out":timed_out,"error_code":error_code,"stream_settled":stream_settled,"close_settled":close_settled,
        "session_closed":session.is_closed(),"runs":run_reports,
        "reported_token_ledger":token_guard.consumed_tokens(),"token_limit_semantics":"reported-token soft cap; missing provider usage is not estimated",
        "presentation_matches":profiled_calls>0&&presentation_matches,"event_window_complete":event_window_complete,
        "metrics_complete":metrics_complete&&event_window_complete,"parent_identity_source":"typed original top-level tool.result receipts bound to the exact admitted Run",
        "parent_identity_error":parent_ids.as_ref().err(),"initial_skill_count":initial_skill_count,"final_skill_count":session.skill_names().len(),
        "execution_completed":!timed_out&&error_code.is_none()&&stream_settled&&close_settled&&ordinary_terminal&&runs.len()==1&&runs[0].status==RunStatus::Completed,
        "oracle_passed":null,"runtime_observations":vm});
    // Keep partial runtime evidence even if subsequent receipt/output reads fail.
    let partial = write_record(
        &manifest.evidence_dir,
        "runtime-observation.json",
        &json!({"schema":"aster.task-matrix-result.v1","status":"observed","runtime":metadata}),
    );
    // Evidence I/O failure must not bypass explicit resource settlement.
    let agent_close_settled = tokio::time::timeout(Duration::from_secs(120), agent.close())
        .await
        .is_ok();
    metadata["agent_close_settled"] = json!(agent_close_settled);
    if !agent_close_settled {
        metadata["execution_completed"] = json!(false);
    }
    write_record(
        &manifest.evidence_dir,
        "runtime-observation.json",
        &json!({"schema":"aster.task-matrix-result.v1","status":"observed","runtime":metadata}),
    )?;
    partial?;
    Ok(metadata)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "one frozen T023 Run; requires ASTER_T023_MANIFEST, explicit DeepSeek ACL and signed OCI assets; --exact --test-threads=1"]
async fn frozen_task_matrix_single_run() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    assert!(!STARTED.swap(true, Ordering::SeqCst), "one Run per process");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let exact = [
        "task_matrix::frozen_task_matrix_single_run",
        "--exact",
        "--ignored",
        "--test-threads=1",
        "--nocapture",
    ];
    assert!(
        args.len() == exact.len()
            && args.iter().map(String::as_str).collect::<BTreeSet<_>>()
                == exact.into_iter().collect(),
        "use the documented exact single-test process invocation"
    );
    let path = PathBuf::from(
        std::env::var_os("ASTER_T023_MANIFEST").expect("explicit frozen manifest required"),
    );
    let bytes = bounded(&path, MAX_MANIFEST).expect("bounded regular manifest");
    let manifest: Manifest = serde_json::from_slice(&bytes).expect("strict frozen manifest");
    reserve_evidence(&manifest)
        .expect("fresh bounded evidence directory outside inputs and assets");
    let mut report = json!({"schema":"aster.task-matrix-result.v1","status":"preflight","manifest_sha256":digest(&bytes),"prompt_sha256":digest(manifest.prompt.as_bytes()),"manifest":manifest});
    write_report(&manifest.evidence_dir, &report).unwrap();
    let checked = validate(&manifest);
    if let Err(code) = checked {
        report["status"] = json!("preflight_refused");
        report["error_code"] = json!(code);
        write_report(&manifest.evidence_dir, &report).unwrap();
        panic!("preflight refused: {code}");
    }
    let (config, prompt) = checked.unwrap();
    let runtime = observed_run(&manifest, config, prompt).await;
    report["status"] = json!(if runtime.is_ok() {
        "recorded"
    } else {
        "failed"
    });
    match runtime {
        Ok(runtime) => report["runtime"] = runtime,
        Err(code) => report["error_code"] = json!(code),
    }
    report["partial_runtime_file"] = json!(manifest
        .evidence_dir
        .join("runtime-observation.json")
        .is_file()
        .then_some("runtime-observation.json"));
    match receipt_evidence(&manifest.workspace, &manifest.run_id) {
        Ok(value) => report["original_receipts"] = value,
        Err(code) => report["receipt_error"] = json!(code),
    }
    report["immutable_inputs_unchanged"] =
        json!(
            immutable_inputs(&manifest.workspace, &manifest.inputs, &manifest.submissions).is_ok()
        );
    let (asset_files, asset_links) = tree_entries(&manifest.assets.root).unwrap_or_default();
    report["frozen_identity_unchanged"] = json!(
        frozen_files(
            &manifest.source.root,
            &manifest.source.files,
            64 * 1024 * 1024
        )
        .is_ok()
            && frozen_files(
                &manifest.assets.root,
                &manifest.assets.files,
                512 * 1024 * 1024
            )
            .is_ok()
            && asset_files == manifest.assets.files.keys().cloned().collect()
            && asset_links == manifest.assets.symlinks
            && std::env::current_exe()
                .ok()
                .and_then(|path| file_hash(&path, 1024 * 1024 * 1024).ok())
                .as_deref()
                == Some(&manifest.runner_sha256)
    );
    match freeze_submissions(&manifest) {
        Ok(files) => report["submissions"] = files,
        Err(code) => report["submission_error"] = json!(code),
    }
    report["runner_gates_passed"] = json!(
        report.pointer("/runtime/execution_completed") == Some(&json!(true))
            && report["immutable_inputs_unchanged"] == true
            && report["frozen_identity_unchanged"] == true
            && report.get("receipt_error").is_none()
            && report.get("submission_error").is_none()
            && report["original_receipts"]["available"] == true
            && report["submissions"]
                .as_array()
                .is_some_and(|files| files.len() == manifest.submissions.len()
                    && files.iter().all(|file| file["present"] == true))
            && [
                "/runtime/presentation_matches",
                "/runtime/event_window_complete",
                "/runtime/runtime_observations/complete",
                "/runtime/metrics_complete"
            ]
            .iter()
            .all(|pointer| report.pointer(pointer) == Some(&json!(true)))
            && report["runtime"]["final_skill_count"] == 0
    );
    report["status"] = json!(if report["runner_gates_passed"] == true {
        "recorded"
    } else {
        "failed"
    });
    write_report(&manifest.evidence_dir, &report).unwrap();
    assert_eq!(
        report.pointer("/runtime/execution_completed"),
        Some(&json!(true)),
        "Run failed; inspect retained run.json"
    );
    assert_eq!(
        report["immutable_inputs_unchanged"], true,
        "input mutation; evidence retained"
    );
    assert!(
        report.get("receipt_error").is_none() && report.get("submission_error").is_none(),
        "evidence incomplete"
    );
    assert_eq!(
        report["frozen_identity_unchanged"], true,
        "frozen identity drifted; evidence retained"
    );
    assert_eq!(
        report["original_receipts"]["available"], true,
        "original receipt log unavailable"
    );
    assert!(
        report["submissions"]
            .as_array()
            .is_some_and(|files| files.len() == manifest.submissions.len()
                && files.iter().all(|file| file["present"] == true)),
        "declared submission absent; evidence retained"
    );
    for pointer in [
        "/runtime/presentation_matches",
        "/runtime/event_window_complete",
        "/runtime/runtime_observations/complete",
        "/runtime/metrics_complete",
    ] {
        assert_eq!(
            report.pointer(pointer),
            Some(&json!(true)),
            "qualification evidence incomplete: {pointer}"
        );
    }
    assert_eq!(
        report["runtime"]["final_skill_count"], 0,
        "ambient skill registry changed"
    );
}

#[test]
fn strict_manifest_and_frozen_paths_refuse_ambiguity() {
    assert!(serde_json::from_str::<Budget>(r#"{"max_execution_ms":1,"llm_api_timeout_ms":1,"tool_timeout_ms":1,"max_tool_rounds":1,"max_continuation_turns":0,"max_parallel_tasks":1,"thinking_budget":1,"reasoning_effort":"high","reported_token_limit":1,"unknown":true}"#).is_err());
    for name in [
        "",
        "../escape",
        "/absolute",
        "leaf/../../escape",
        "a\\b",
        "a/./b",
        "a//b",
        "a/",
    ] {
        assert!(relative(name).is_err());
    }
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("input.txt"), b"frozen").unwrap();
    let files = BTreeMap::from([("input.txt".into(), digest(b"frozen"))]);
    frozen_files(root.path(), &files, 32).unwrap();
    fs::write(root.path().join("input.txt"), b"changed").unwrap();
    assert_eq!(
        frozen_files(root.path(), &files, 32),
        Err("frozen_file_digest_mismatch")
    );
    assert!(serde_json::from_str::<FrozenTree>(
        r#"{"root":"/tmp","files":{"same":"a","same":"b"},"symlinks":{}}"#
    )
    .is_err());
}

#[test]
fn failure_metadata_is_written_without_raw_model_or_reasoning_content() {
    let root = tempfile::tempdir().unwrap();
    let record = a3s_code_core::run::RunEventRecord {
        sequence: 0,
        timestamp_ms: 0,
        event: AgentEvent::ReasoningDelta {
            text: "PRIVATE_REASONING".into(),
        },
    };
    let metadata = event_metadata(&[record]);
    let report = json!({"status":"failed","error_code":"LLM_ERROR","runtime":metadata});
    write_report(root.path(), &report).unwrap();
    let written =
        String::from_utf8(bounded(&root.path().join("run.json"), MAX_REPORT).unwrap()).unwrap();
    assert!(written.contains("LLM_ERROR") && !written.contains("PRIVATE_REASONING"));
    assert!(write_report(root.path(), &json!({"overflow":"x".repeat(MAX_REPORT)})).is_err());
    assert_eq!(
        bounded(&root.path().join("run.json"), MAX_REPORT).unwrap(),
        written.as_bytes()
    );
}

#[test]
fn model_selection_rejects_fallback_and_effective_endpoint_override() {
    fn declared() -> CodeConfig {
        CodeConfig { providers:vec![serde_json::from_value(json!({"name":"deepseek","apiKey":"DUMMY_NOT_A_CREDENTIAL","baseUrl":ENDPOINT,"models":[{"id":"deepseek-flash"}]})).unwrap()],..Default::default() }
    }
    let selected = select_model(declared()).unwrap();
    assert_eq!(selected.default_model.as_deref(), Some(ROUTE));
    assert_eq!(selected.providers.len(), 1);
    assert_eq!(
        select_model(CodeConfig::default()).unwrap_err(),
        "model_provider_not_uniquely_declared"
    );
    let mut config = declared();
    config.providers[0].models[0].id = "other".into();
    assert_eq!(
        select_model(config).unwrap_err(),
        "model_route_not_uniquely_declared"
    );
    let mut config = declared();
    config.providers[0].models[0].base_url = Some("https://wrong.invalid".into());
    assert_eq!(select_model(config).unwrap_err(), "model_endpoint_mismatch");
    let mut config = declared();
    config.providers[0].api_key = None;
    assert_eq!(
        select_model(config).unwrap_err(),
        "model_credentials_missing"
    );
    let mut config = declared();
    config.providers.push(config.providers[0].clone());
    assert_eq!(
        select_model(config).unwrap_err(),
        "model_provider_not_uniquely_declared"
    );
}

#[test]
fn vm_observation_requires_complete_closed_spans_and_never_infers_from_tool_count() {
    let observations = Arc::new(VmObservations::default());
    let subscriber = tracing_subscriber::registry().with(VmLayer(Arc::clone(&observations)));
    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!(target:"a3s_code_core::sandbox::oci","oci_command");
        {
            let _entered = span.enter();
            for phase in [
                "host_connect",
                "bundle_prepare",
                "vm_create",
                "process_start",
                "collect_wait",
                "vm_delete",
                "host_shutdown",
                "scratch_remove",
            ] {
                tracing::debug!(target:"a3s_code_core::sandbox::oci",phase=phase,elapsed_ms=1.0);
            }
        }
        assert_eq!(
            observations.summary()["vm_count"],
            Value::Null,
            "live span is not settled"
        );
        drop(span);
        assert_eq!(observations.summary()["vm_count"], 1);
        let span = tracing::info_span!(target:"a3s_code_core::sandbox::oci","oci_command");
        span.in_scope(||tracing::debug!(target:"a3s_code_core::sandbox::oci",phase="vm_create",elapsed_ms=1.0));
        drop(span);
        assert_eq!(
            observations.summary()["vm_count"],
            Value::Null,
            "failed phase sequence must stay unknown"
        );
    });
}

#[test]
fn failure_receipts_bind_original_log_without_copying_private_plaintext() {
    let root = tempfile::tempdir().unwrap();
    let session = "failure-receipt";
    let path = a3s_code_core::session_folder::effect_log_dir(root.path(), session)
        .join(format!("{session}.jsonl"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let receipt = json!({"schema":"a3s.code.tool-result.v1","identity":{"turn":1,"cycle":0,"tool_call_id":"call-one","batch_index":0},"prepared_digest":format!("sha256:{}","0".repeat(64)),"outcome":"failed","exit_code":1,"output":"PRIVATE_TOOL_OUTPUT","effects":{"changes":[],"completeness":"complete"}});
    let bytes = format!(
        "{}\n{}\n",
        json!({"seq":0,"type":"model.reasoning","payload":{"text":"PRIVATE_REASONING"}}),
        json!({"seq":1,"type":"tool.result","payload":{"receipt":receipt}})
    );
    fs::write(&path, bytes.as_bytes()).unwrap();
    let evidence = receipt_evidence(root.path(), session).unwrap();
    assert_eq!(evidence["original_log_sha256"], digest(bytes.as_bytes()));
    assert_eq!(evidence["receipts"][0]["outcome"], "failed");
    assert_eq!(
        evidence["receipts"][0]["identity"]["tool_call_id"],
        "call-one"
    );
    let encoded = serde_json::to_string(&evidence).unwrap();
    assert!(!encoded.contains("PRIVATE_TOOL_OUTPUT") && !encoded.contains("PRIVATE_REASONING"));
    assert_eq!(
        receipt_parent_ids(&evidence, "expected-run"),
        Err("parent_receipt_run_identity_mismatch")
    );
    write_report(
        root.path(),
        &json!({"status":"failed","original_receipts":evidence}),
    )
    .unwrap();
    assert_eq!(
        fs::read(path).unwrap(),
        bytes.as_bytes(),
        "original receipt log stays intact"
    );
}

#[test]
fn exact_original_parent_receipts_recover_metrics_without_guessing_unknown_origins() {
    let mut evidence = json!({"available":true,"receipts":[{"identity":{"tool_call_id":"parent-call","admission":{"run_id":"expected-run"}}}]});
    let parents = receipt_parent_ids(&evidence, "expected-run").unwrap();
    let events = vec![
        a3s_code_core::run::RunEventRecord {
            sequence: 0,
            timestamp_ms: 10,
            event: AgentEvent::ToolStart {
                id: "parent-call".into(),
                name: "read".into(),
            },
        },
        a3s_code_core::run::RunEventRecord {
            sequence: 1,
            timestamp_ms: 30,
            event: AgentEvent::ToolEnd {
                id: "parent-call".into(),
                name: "read".into(),
                args: None,
                output: "result".into(),
                exit_code: 0,
                error_kind: None,
                metadata: None,
            },
        },
    ];
    let missing = super::metrics::summarize(&events, &[], Some(0));
    assert!(!measured_tool_metrics(&missing, true));
    assert!(missing["child_tool_calls"].is_null());
    let measured = super::metrics::summarize(&events, &parents, Some(0));
    assert!(measured_tool_metrics(&measured, true));
    assert_eq!(measured["parent_tool_calls"], 1);
    assert_eq!(measured["tool_interval_union_ms"], 20);
    let mut program_events = events.clone();
    if let AgentEvent::ToolEnd { name, metadata, .. } = &mut program_events[1].event {
        *name = "program".into();
        *metadata = Some(
            json!({"program":{"tool_calls":[{"start_offset_ms":1,"duration_ms":5,"output_bytes":4}]}}),
        );
    }
    let measured = super::metrics::summarize(&program_events, &parents, Some(1));
    assert!(measured_tool_metrics(&measured, true));
    assert!(
        measured["child_interval_union_ms"].is_null(),
        "per-program clocks do not establish global child union"
    );
    if let AgentEvent::ToolEnd {
        metadata: Some(metadata),
        ..
    } = &mut program_events[1].event
    {
        metadata["program"]["tool_calls"][0]
            .as_object_mut()
            .unwrap()
            .remove("duration_ms");
    }
    assert!(
        !measured_tool_metrics(
            &super::metrics::summarize(&program_events, &parents, Some(1)),
            true
        ),
        "missing child timing must refuse metric qualification"
    );
    let receipt = evidence["receipts"][0].clone();
    evidence["receipts"].as_array_mut().unwrap().push(receipt);
    assert_eq!(
        receipt_parent_ids(&evidence, "expected-run"),
        Err("duplicate_parent_tool_id")
    );
    assert_eq!(
        receipt_parent_ids(&evidence, "another-run"),
        Err("parent_receipt_run_identity_mismatch")
    );
}

#[test]
fn trimmed_event_windows_cannot_certify_otherwise_complete_metrics() {
    let retained = [
        a3s_code_core::run::RunEventRecord {
            sequence: 1,
            timestamp_ms: 10,
            event: AgentEvent::ToolStart {
                id: "retained-call".into(),
                name: "read".into(),
            },
        },
        a3s_code_core::run::RunEventRecord {
            sequence: 2,
            timestamp_ms: 30,
            event: AgentEvent::ToolEnd {
                id: "retained-call".into(),
                name: "read".into(),
                args: None,
                output: "retained result".into(),
                exit_code: 0,
                error_kind: None,
                metadata: None,
            },
        },
    ];
    let metrics = super::metrics::summarize(&retained, &["retained-call".into()], Some(0));
    assert!(
        measured_tool_metrics(&metrics, true),
        "the retained suffix has complete tool intervals"
    );
    assert!(
        !measured_tool_metrics(&metrics, 3 == retained.len()),
        "an earlier discarded model event makes all Run metrics incomplete"
    );
}

#[test]
fn evidence_reservation_rejects_workspace_mutation_before_manifest_validation() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path().canonicalize().unwrap();
    let workspace = base.join("workspace");
    let assets = base.join("assets");
    fs::create_dir(&workspace).unwrap();
    fs::create_dir(&assets).unwrap();
    let tree = FrozenTree {
        root: assets.clone(),
        files: BTreeMap::new(),
        symlinks: BTreeMap::new(),
    };
    let mut manifest = Manifest {
        schema: "aster.task-matrix-run.v1".into(),
        run_id: "boundary".into(),
        trusted_acl: base.join("trusted.acl"),
        route: ROUTE.into(),
        endpoint: ENDPOINT.into(),
        profile: ToolPresentationModeV1::Code,
        workspace: workspace.clone(),
        inputs: BTreeMap::new(),
        prompt: "frozen literal user prompt".into(),
        source: tree.clone(),
        assets: tree,
        runner_sha256: "0".repeat(64),
        build_features: vec!["default".into()],
        permissions: vec!["read(*)".into()],
        budget: Budget {
            max_execution_ms: 1,
            llm_api_timeout_ms: 1,
            tool_timeout_ms: 1,
            max_tool_rounds: 1,
            max_continuation_turns: 0,
            max_parallel_tasks: 1,
            thinking_budget: 1,
            reasoning_effort: "high".into(),
            reported_token_limit: 1,
        },
        submissions: vec!["result.json".into()],
        evidence_dir: workspace.join("evidence"),
    };
    assert_eq!(
        reserve_evidence(&manifest),
        Err("evidence_boundary_refused")
    );
    assert!(!manifest.evidence_dir.exists());
    manifest.evidence_dir = base.join("evidence");
    reserve_evidence(&manifest).unwrap();
    write_report(
        &manifest.evidence_dir,
        &json!({"status":"preflight_refused","error_code":"run_budget_refused"}),
    )
    .unwrap();
    assert_eq!(fs::read_dir(workspace).unwrap().count(), 0);
    assert_eq!(
        reserve_evidence(&manifest),
        Err("evidence_boundary_refused")
    );
}

#[cfg(unix)]
#[test]
fn asset_manifest_hashes_guest_symlink_identity_without_following_host_paths() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(
        "/unavailable-guest/bin/busybox",
        root.path().join("guest-link"),
    )
    .unwrap();
    let (files, links) = tree_entries(root.path()).unwrap();
    assert!(files.is_empty());
    assert_eq!(
        links["guest-link"],
        digest(b"/unavailable-guest/bin/busybox")
    );
}

#[test]
fn only_explicit_submission_paths_may_change_initial_input_bytes() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("pricing.py"), b"before").unwrap();
    fs::write(root.path().join("immutable.txt"), b"fixed").unwrap();
    let inputs = BTreeMap::from([
        ("pricing.py".into(), digest(b"before")),
        ("immutable.txt".into(), digest(b"fixed")),
    ]);
    frozen_files(root.path(), &inputs, 32 * 1024 * 1024).unwrap();
    fs::write(root.path().join("pricing.py"), b"repaired").unwrap();
    immutable_inputs(root.path(), &inputs, &["pricing.py".into()]).unwrap();
    assert_eq!(
        immutable_inputs(root.path(), &inputs, &[]),
        Err("frozen_file_digest_mismatch")
    );
    fs::write(root.path().join("immutable.txt"), b"changed").unwrap();
    assert_eq!(
        immutable_inputs(root.path(), &inputs, &["pricing.py".into()]),
        Err("frozen_file_digest_mismatch")
    );
}
