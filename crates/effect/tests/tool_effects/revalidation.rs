#![cfg(test)]

use super::*;
use sha2::{Digest, Sha256};

struct FileTools {
    root: std::path::PathBuf,
    checks: Arc<AtomicUsize>,
    complete: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Update {
    None,
    NewBytes,
    SameBytes,
    Incomplete,
}

fn file_identity(bytes: &[u8]) -> ToolFileIdentityV1 {
    ToolFileIdentityV1 {
        kind: ToolFileKind::File,
        content_digest: Some(format!("sha256:{:x}", Sha256::digest(bytes))),
        mode: None,
    }
}

impl ToolRunner for FileTools {
    fn run(
        &self,
        _: ToolCall,
        _: CancellationToken,
    ) -> a3s_effect::coding::BoxFuture<Result<Value, ActorError>> {
        unreachable!("the fixture executes only its typed prepared effect")
    }

    fn prepare(
        self: Arc<Self>,
        request: ToolPreparationRequest,
        _: CancellationToken,
    ) -> a3s_effect::coding::BoxFuture<Result<PreparedToolExecution, ActorError>> {
        Box::pin(async move {
            let digest = tool_args_digest(&request.call.args)?;
            let prepared = ToolPreparedV1 {
                schema: TOOL_PREPARED_SCHEMA.into(),
                identity: request.identity,
                tool_name: request.call.name.clone(),
                original_args_digest: digest.clone(),
                effective_args_digest: digest,
                recovery: ToolRecoveryPolicy::NeverReplay,
                baseline: None,
            };
            let bound = prepared.clone();
            let call = request.call;
            let execute = Effect::from_async_observed(move |_services, _cancel| {
                let runner = self.clone();
                let prepared = bound.clone();
                let call = call.clone();
                async move {
                    let path = runner.root.join("state.txt");
                    let before = std::fs::read(&path).unwrap();
                    let mut effects = match call.name.as_str() {
                        "verify" => {
                            runner.checks.fetch_add(1, Ordering::SeqCst);
                            ToolEffectObservationV1::complete(vec![])
                        }
                        "replace" => {
                            let after = call.args["content"].as_str().unwrap().as_bytes();
                            std::fs::write(&path, after).unwrap();
                            ToolEffectObservationV1::complete(vec![ToolPathChangeV1 {
                                path: "state.txt".into(),
                                before: Some(file_identity(&before)),
                                after: file_identity(&std::fs::read(&path).unwrap()),
                            }])
                        }
                        _ => panic!("unknown fixture tool"),
                    };
                    if !runner.complete {
                        effects.completeness = ToolObservationCompleteness::Incomplete;
                        effects.reason = Some("controlled incomplete observation".into());
                    }
                    effects.validate().unwrap();
                    Ok(ToolExecutionResultV1 {
                        schema: TOOL_RESULT_SCHEMA.into(),
                        identity: prepared.identity.clone(),
                        prepared_digest: prepared.digest().map_err(Exit::Fail)?,
                        outcome: ToolExecutionOutcome::Succeeded,
                        exit_code: Some(0),
                        output: json!(String::from_utf8(std::fs::read(path).unwrap()).unwrap()),
                        effects,
                    })
                }
            });
            Ok(PreparedToolExecution { prepared, execute })
        })
    }
}

async fn revalidate(case: Case, update: Update) -> (Vec<ToolExecutionResultV1>, usize) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("state.txt"), b"version-0").unwrap();
    let checks = Arc::new(AtomicUsize::new(0));
    let mut decisions = VecDeque::new();
    for index in 0..4 {
        if update != Update::None && index > 0 {
            let mut replace = call(
                &format!("replace-{index}"),
                json!({"content":format!("version-{}", if update == Update::SameBytes { 0 } else { index })}),
            );
            replace.name = "replace".into();
            decisions.push_back(ModelDecision::Tool {
                call: replace,
                also: vec![],
            });
        }
        let mut verify = call(&format!("verify-{index}"), json!({"file_path":"state.txt"}));
        verify.name = "verify".into();
        decisions.push_back(ModelDecision::Tool {
            call: verify,
            also: vec![],
        });
    }
    let services = Arc::new(CodingServices {
        completion: Arc::new(ScriptModel(Mutex::new(decisions))),
        tools: Arc::new(FileTools {
            root: workspace.path().to_path_buf(),
            checks: checks.clone(),
            complete: update != Update::Incomplete,
        }),
        compactor: Arc::new(NoCompact),
    });
    let config = HarnessConfig::new(20, usize::MAX, LIMIT, 1, vec![], vec![])
        .unwrap()
        .with_duplicate_threshold(3);
    let settled = ingest_coding(
        &coding_actor(config),
        case.log.as_ref(),
        services,
        THREAD,
        message_fact("revalidate", "Check the current file after each update"),
        LIMIT,
    )
    .await
    .unwrap();
    assert!(settled.view.schema_error.is_none());
    (receipts(&settled.log), checks.load(Ordering::SeqCst))
}

#[tokio::test]
async fn actual_byte_change_allows_identical_revalidation_after_the_repeat_limit() {
    for case in [Case::memory(), Case::file()] {
        let (results, checks) = revalidate(case, Update::NewBytes).await;
        let changes: Vec<_> = results
            .iter()
            .flat_map(|result| &result.effects.changes)
            .collect();
        assert_eq!(changes.len(), 3);
        assert!(changes
            .iter()
            .all(|change| change.before.as_ref().unwrap().content_digest
                != change.after.content_digest));
        assert!(results
            .iter()
            .all(|result| result.effects.completeness == ToolObservationCompleteness::Complete));
        assert_eq!(
            checks, 4,
            "a new byte identity permits a fresh verification of identical arguments"
        );
        let verify: Vec<_> = results
            .iter()
            .filter(|result| result.identity.tool_call_id.starts_with("verify-"))
            .collect();
        assert_eq!(verify.len(), 4);
        for (index, result) in verify.iter().enumerate() {
            assert_eq!(result.outcome, ToolExecutionOutcome::Succeeded);
            assert_eq!(result.output, json!(format!("version-{index}")));
        }
    }
}

#[tokio::test]
async fn unchanged_bytes_keep_the_identical_call_repeat_limit() {
    for case in [Case::memory(), Case::file()] {
        let (results, checks) = revalidate(case, Update::None).await;
        assert_eq!(checks, 3);
        assert_eq!(results.len(), 4);
        assert!(results
            .iter()
            .all(|result| result.effects.changes.is_empty()));
        assert_eq!(
            results.last().unwrap().outcome,
            ToolExecutionOutcome::Denied
        );
    }
}

#[tokio::test]
async fn same_byte_rewrites_and_incomplete_changes_do_not_reset_the_repeat_limit() {
    for update in [Update::SameBytes, Update::Incomplete] {
        for case in [Case::memory(), Case::file()] {
            let (results, checks) = revalidate(case, update).await;
            assert_eq!(checks, 3);
            let fourth = results
                .iter()
                .find(|result| result.identity.tool_call_id == "verify-3")
                .unwrap();
            assert_eq!(fourth.outcome, ToolExecutionOutcome::Denied);
        }
    }
}
