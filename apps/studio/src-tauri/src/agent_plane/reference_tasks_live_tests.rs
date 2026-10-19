#![cfg(test)]

use std::{path::PathBuf, sync::Arc, time::Duration};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::{AgentCatalog, EventHub, PlaneConfig, PlaneStore, SessionCatalog, TurnService};

const MODEL: &str = "boyue::deepseek-v4-flash";

static REVIEW_RESPONSES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static REVIEW_TOOL_UPDATES: std::sync::Mutex<Vec<serde_json::Value>> =
    std::sync::Mutex::new(Vec::new());
pub(super) static REVIEW_DOCUMENT_READS: std::sync::Mutex<Vec<PathBuf>> =
    std::sync::Mutex::new(Vec::new());
static REVIEW_QUESTIONS: std::sync::Mutex<Vec<serde_json::Value>> =
    std::sync::Mutex::new(Vec::new());

pub(super) fn record_review_permission(params: &serde_json::Value) {
    if std::env::var_os("A3S_STUDIO_REFERENCE_JOB").is_some()
        && params
            .pointer("/_meta/question/allowFreeText")
            .is_some_and(serde_json::Value::is_boolean)
    {
        REVIEW_QUESTIONS.lock().unwrap().push(params.clone());
    }
}

pub(super) fn record_review_response(text: &str) {
    if std::env::var_os("A3S_STUDIO_REFERENCE_JOB").is_some() {
        REVIEW_RESPONSES.lock().unwrap().push(text.to_owned());
    }
}

pub(super) fn record_review_document_read(path: &std::path::Path) {
    REVIEW_DOCUMENT_READS.lock().unwrap().push(path.to_owned());
}

pub(super) fn record_review_tool_update(params: &serde_json::Value) {
    if std::env::var_os("A3S_STUDIO_REFERENCE_JOB").is_some()
        && matches!(
            params
                .pointer("/update/sessionUpdate")
                .and_then(serde_json::Value::as_str),
            Some("tool_call" | "tool_call_update")
        )
    {
        REVIEW_TOOL_UPDATES.lock().unwrap().push(params.clone());
    }
}

pub(super) fn publish_reference_review(job: &std::path::Path) -> serde_json::Value {
    use a3s_use_reconcile::{
        PackageFile, PackageSpec, Payload, ReconcileStore, SurfaceKind, SurfaceSpec, files_sha256,
    };

    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../use-registry/packages/review");
    let names = [
        "a3s-use-extension.acl",
        "agents/legal-expert/INSTRUCTIONS.md",
        "skills/review/SKILL.md",
    ];
    let files: Vec<_> = names
        .iter()
        .map(|name| PackageFile {
            path: (*name).to_owned(),
            bytes: std::fs::read(source.join(name)).expect("review package source"),
        })
        .collect();
    let acl = std::str::from_utf8(&files[0].bytes).unwrap();
    a3s_acl::parse(acl).expect("review package is ACL");
    let surface = |kind, id: &str, required: &[&str]| {
        let payload: Vec<_> = files
            .iter()
            .filter(|file| required.contains(&file.path.as_str()))
            .map(|file| PackageFile {
                path: file.path.clone(),
                bytes: file.bytes.clone(),
            })
            .collect();
        SurfaceSpec {
            kind,
            id: id.into(),
            sha256: files_sha256(&payload).unwrap(),
            entry: None,
            payload: Payload::Files(payload),
        }
    };
    let store = ReconcileStore::open(job.join("package-generations")).unwrap();
    let published = store
        .apply_sync(&PackageSpec {
            package_id: "use/a3s/review".into(),
            surfaces: vec![
                surface(SurfaceKind::Skill, "review", &[names[0], names[2]]),
                surface(SurfaceKind::Agent, "legal-expert", &[names[0], names[1]]),
            ],
        })
        .unwrap();
    store
        .publish_selection_sync("studio-installed-plugins", None, &[published.cursor()])
        .unwrap();
    assert!(crate::review_plugin::docx_review_instructions(Some(job), "legal-expert").is_some());
    json!({"authority":"controlled test publication; not a signed registry or product-installation claim", "sha256":files.iter().map(|file| (file.path.clone(), json!(format!("{:x}", Sha256::digest(&file.bytes))))).collect::<serde_json::Map<String,serde_json::Value>>()})
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real Boyue model; requires an owned reference job and BOYUE_API_KEY"]
async fn reference_task_boyue_live() {
    let job = PathBuf::from(std::env::var_os("A3S_STUDIO_REFERENCE_JOB").expect("job directory"))
        .canonicalize()
        .expect("existing job");
    assert!(job.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(
        job.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("a3s-studio-reference-")
    );
    assert!(job.join(".owned").is_file());
    let instruction = std::fs::read_to_string(job.join("instruction.txt")).expect("instruction");
    assert!(!instruction.trim().is_empty());
    let workspace = job.join("workspace").canonicalize().expect("workspace");
    assert!(workspace.starts_with(&job));
    let state = job.join("state");
    std::fs::create_dir(&state).expect("fresh job state; retain previous attempts");
    let review = job.join("readonly-review").is_file();
    let expects_clarification = job.join("expect-clarification").is_file();
    let question_reply = std::fs::read(job.join("answer-question.json"))
        .ok()
        .map(|bytes| {
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .expect("explicit question reply fixture")
        });
    let publication = review.then(|| publish_reference_review(&job));
    let prompt = if review {
        instruction.replace(
            "{{WORKSPACE}}",
            workspace.to_str().expect("UTF-8 reference workspace"),
        )
    } else {
        instruction.clone()
    };

    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/harbor/native-tb.acl");
    let acl = std::fs::read_to_string(source).expect("existing model ACL");
    let document = a3s_acl::parse(&acl).expect("valid model ACL");
    let provider = document
        .blocks
        .iter()
        .find(|block| block.name == "providers" && block.labels == ["boyue"])
        .expect("declared Boyue provider");
    assert!(
        provider
            .blocks
            .iter()
            .any(|block| block.name == "models" && block.labels == ["deepseek-v4-flash"])
    );
    let base = provider
        .attributes
        .get("base_url")
        .and_then(a3s_acl::Value::as_str)
        .expect("declared base URL")
        .to_owned();
    assert!(
        matches!(provider.attributes.get("api_key"), Some(a3s_acl::Value::Call(name,args)) if name == "env" && args == &[a3s_acl::Value::String("BOYUE_API_KEY".into())])
    );
    let key = std::env::var("BOYUE_API_KEY").expect("Boyue evaluation credential");
    assert!(!key.trim().is_empty());
    let secret = key.clone();
    let binary = std::env::var_os("A3S_STUDIO_REFERENCE_ACP")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/a3s-code-acp")
        })
        .canonicalize()
        .expect("workspace ACP binary");
    let artifact = std::env::var_os("A3S_STUDIO_REFERENCE_AGENT_ARTIFACT")
        .map(PathBuf::from)
        .unwrap_or_else(|| binary.clone());
    let binary_digest = format!("{:x}", Sha256::digest(std::fs::read(&artifact).unwrap()));
    let launcher_digest = format!("{:x}", Sha256::digest(std::fs::read(&binary).unwrap()));

    let store = Arc::new(PlaneStore::open(Some(state.clone())));
    let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
    let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
    let hub = Arc::new(EventHub::new());
    let permission_mode = if review {
        "default"
    } else {
        "bypassPermissions"
    };
    let agent = agents.create(&json!({"type":"a3s-code","name":"Reference evaluation","model":MODEL,"configuration":{"permission_mode":permission_mode}})).unwrap();
    let session = sessions.create(&json!({"agentId":agent.id,"name":"Reference evaluation","workspace":{"type":"user","workspace":{"id":"reference-workspace","name":"Reference evaluation","path":workspace,"type":"user"}}})).unwrap();
    let config = Arc::new(PlaneConfig {
        root: Some(state.clone()),
        credentials: Arc::new(move |provider| {
            (provider == "boyue").then(|| crate::providers::ProviderCredentials {
                base_url: base.clone(),
                api_key: key.clone(),
            })
        }),
        binary: Some(binary),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    let turns = Arc::new(TurnService::new(
        Arc::clone(&store),
        agents,
        sessions,
        config,
        Arc::clone(&hub),
    ));
    let mut events = hub.subscribe();
    let started = std::time::Instant::now();
    let mut body = json!({"userMessageParts":[{"type":"text","text":prompt}]});
    if review {
        body["turnContext"] = json!({"agentId":"legal-expert"});
        if job.join("text-review").is_file() {
            body["turnContext"]["reviewSource"] = json!({"kind":"text","text":std::fs::read_to_string(workspace.join("inputs/source.txt")).expect("explicit pasted review fixture")});
        }
    }
    turns
        .open(&session.id, &body)
        .expect("native Studio turn accepted");
    let mut review_forms = Vec::new();
    let mut question_replies = Vec::new();
    let mut question_reattachments = Vec::new();
    let mut review_retries = Vec::new();
    let outcome = tokio::time::timeout(Duration::from_secs(900), async {
        loop {
            let event = events
                .recv()
                .await
                .map_err(|_| "event stream closed".to_owned())?;
            if event.pointer("/payload/chunk/type").and_then(serde_json::Value::as_str) == Some("data-retry") {
                review_retries.push(event["payload"]["chunk"].clone());
            }
            if let Some(reply) = &question_reply
                && event.pointer("/payload/chunk/type").and_then(serde_json::Value::as_str) == Some("data-form")
                && event.pointer("/payload/chunk/data/state").and_then(serde_json::Value::as_str) == Some("form-requested")
            {
                if !question_replies.is_empty() || event["payload"]["chunk"]["data"]["format"] != "question" {
                    return Err("expected one typed question".to_owned());
                }
                let requested = &event["payload"]["chunk"];
                events = hub.subscribe();
                let attached = turns.attach(&session.id);
                let replayed = attached["bufferedChunks"].as_array().and_then(|chunks| chunks.iter().rev().find(|payload| payload["chunk"]["type"] == "data-form")).map(|payload| &payload["chunk"]).ok_or("waiting question missing on reattach")?;
                turns.settle_orphan_pending(&session.id);
                let persisted = store.messages(&session.id);
                let saved = persisted.iter().flat_map(|message| message.parts()).find(|part| part["type"] == "data-form" && part["data"]["id"] == requested["id"]).ok_or("waiting question missing from checkpoint")?;
                if replayed != requested || saved["data"] != requested["data"] { return Err("recovered question differs from the native request".into()); }
                question_reattachments.push(json!({"requestId":requested["id"],"checkpointMatches":true,"bufferedRequestMatches":true,"reconnectedBeforeAnswer":true}));
                let data = &replayed["data"];
                let question: super::question::Question = serde_json::from_str(data["spec"].as_str().ok_or("missing question spec")?).map_err(|error| error.to_string())?;
                let binding: super::forms::FormBinding = serde_json::from_value(data["binding"].clone()).map_err(|error| error.to_string())?;
                if binding.owner.session_id != session.id { return Err("question owner mismatch".into()); }
                let values: super::forms::FormValues = if let Some(label) = reply.get("choiceLabel").and_then(serde_json::Value::as_str) {
                    let label = label.replace("{{WORKSPACE}}", workspace.to_str().ok_or("workspace encoding")?);
                    let choices: Vec<_> = question.choices.iter().filter(|choice| choice.label == label).collect();
                    if choices.len() != 1 { return Err("expected exact unique choice label".into()); }
                    serde_json::from_value(json!({"choice":choices[0].id,"answer":""})).unwrap()
                } else {
                    serde_json::from_value(json!({"choice":"","answer":reply.get("answer").and_then(serde_json::Value::as_str).ok_or("missing answer")?})).unwrap()
                };
                if !turns.submit_form(binding.clone(), values.clone()) { return Err("question submission refused".into()); }
                review_forms.push(event["payload"]["chunk"].clone());
                question_replies.push(json!({"binding":binding,"values":values}));
            }
            if expects_clarification
                && event
                    .pointer("/payload/chunk/type")
                    .and_then(serde_json::Value::as_str)
                    == Some("data-form")
                && event
                    .pointer("/payload/chunk/data/state")
                    .and_then(serde_json::Value::as_str)
                    == Some("form-requested")
                && event
                    .pointer("/payload/chunk/data/binding/owner/sessionId")
                    .and_then(serde_json::Value::as_str)
                    == Some(session.id.as_str())
            {
                review_forms.push(event["payload"]["chunk"].clone());
                return Ok("requires-input".to_owned());
            }
            if expects_clarification
                && event
                    .pointer("/payload/chunk/type")
                    .and_then(serde_json::Value::as_str)
                    == Some("tool-approval-request")
                && !REVIEW_QUESTIONS.lock().unwrap().is_empty()
            {
                return Ok("requires-input".to_owned());
            }
            if event["channel"] == "ai.stream.done" {
                return Ok(event["payload"]["status"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_owned());
            }
            if event["channel"] == "ai.stream.error" {
                return Err(event["payload"]["error"]["message"]
                    .as_str()
                    .unwrap_or("native turn failed")
                    .to_owned());
            }
        }
    })
    .await;
    turns.abort(&session.id);
    turns.close_test_runtimes().await;
    let (status, error) = match outcome {
        Ok(Ok(status)) => (status, None),
        Ok(Err(reason)) => ("failed".to_owned(), Some(reason)),
        Err(_) => (
            "timeout".to_owned(),
            Some("native turn timed out".to_owned()),
        ),
    };
    let messages = store.messages(&session.id);
    let report = json!({"schema":"a3s.studio.reference-turn.v1","model":"boyue/deepseek-v4-flash","nativeModel":MODEL,"status":status,"error":error,"elapsedSeconds":started.elapsed().as_secs(),"acpSha256":binary_digest,"launcherSha256":launcher_digest,"modelAclSha256":format!("{:x}",Sha256::digest(acl.as_bytes())),"instructionSha256":format!("{:x}",Sha256::digest(instruction.as_bytes())),"userPromptSha256":format!("{:x}",Sha256::digest(prompt.as_bytes())),"reviewPublication":publication,"messages":messages});
    let mut report = report;
    report["reviewResponses"] = json!(REVIEW_RESPONSES.lock().unwrap().clone());
    report["reviewQuestions"] = json!(REVIEW_QUESTIONS.lock().unwrap().clone());
    report["reviewForms"] = json!(review_forms);
    report["questionReplies"] = json!(question_replies);
    report["questionReattachments"] = json!(question_reattachments);
    report["reviewRetries"] = json!(review_retries);
    report["reviewToolUpdates"] = json!(REVIEW_TOOL_UPDATES.lock().unwrap().clone());
    report["reviewDocumentReads"] = json!(
        REVIEW_DOCUMENT_READS
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.starts_with(&workspace))
            .collect::<Vec<_>>()
    );
    report["reviewMemory"] = std::fs::read(
        state
            .join("review-memory")
            .join(format!("{}.json", session.id)),
    )
    .ok()
    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    .unwrap_or(serde_json::Value::Null);
    let redacted = serde_json::to_string_pretty(&report)
        .unwrap()
        .replace(&secret, "[redacted]");
    std::fs::write(job.join("turn-result.json"), redacted).unwrap();
    // The generated ACL contains a credential; it is not retained evaluation evidence.
    std::fs::remove_dir_all(state.join("runtime")).unwrap();
    assert!(
        status == "success" || (expects_clarification && status == "requires-input"),
        "inspect the redacted turn result"
    );
    assert!(messages.iter().any(|message| message.role == "assistant"
        && (message.status == "success" || status == "requires-input")));
    eprintln!("REFERENCE_TURN model=boyue/deepseek-v4-flash status={status} child=closed");
}
