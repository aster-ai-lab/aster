//! Edit tool - Edit files by string replacement

use crate::tools::types::{Tool, ToolContext, ToolOutput};
use crate::workspace::WorkspaceError;
use anyhow::Result;
use async_trait::async_trait;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing exact strings. Pass old_string/new_string for one replacement, \
         or an `edits` array to change several places in the same file in ONE call: every entry is \
         validated in memory first and the whole call is atomic, so one call per file costs one \
         model round trip instead of one per hunk. The old_string must be unique in the file \
         unless replace_all is true. LF and CRLF match each other, and the file keeps its own line \
         ending. Mixed or CR-only endings are refused."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Required. Path to the file to edit. Always provide this exact field name: 'file_path'."
                },
                "edits": {
                    "type": "array",
                    "minItems": 1,
                    "description": "Several replacements for this one file, applied in order and atomically. \
                     Use this instead of one call per hunk when a change touches more than one place. \
                     Mutually exclusive with old_string/new_string.",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "old_string": {
                                "type": "string",
                                "description": "The exact string to replace. It must be unique unless replace_all=true."
                            },
                            "new_string": {
                                "type": "string",
                                "description": "The replacement string. Empty deletes the match."
                            },
                            "replace_all": {
                                "type": "boolean",
                                "description": "Replace every occurrence of this entry. Default: false."
                            }
                        },
                        "required": ["old_string", "new_string"]
                    }
                },
                "old_string": {
                    "type": "string",
                    "description": "Required for a single replacement (omit when edits is used). The exact string to replace; unique unless replace_all=true."
                },
                "new_string": {
                    "type": "string",
                    "description": "Required for a single replacement (omit when edits is used). The replacement string."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Optional. Replace all occurrences. Default: false."
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "Optional. Preview the exact diff and replacement count without writing. Default: false."
                },
                "max_replacements": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional safety ceiling for the whole call. Reject the edit before writing when it would replace more occurrences."
                },
                "expected_replacements": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional compare guard for the whole call. Reject the edit before writing unless the observed replacement count is exactly this value."
                }
            },
            "required": ["file_path"],
            "examples": [
                {
                    "file_path": "src/lib.rs",
                    "old_string": "old_value",
                    "new_string": "new_value"
                },
                {
                    "file_path": "src/lib.rs",
                    "old_string": "foo",
                    "new_string": "bar",
                    "replace_all": true,
                    "dry_run": true,
                    "max_replacements": 20
                }
            ]
        })
    }

    fn capabilities(&self, args: &serde_json::Value) -> crate::tools::ToolCapabilities {
        let mut capabilities = if args
            .get("dry_run")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            crate::tools::ToolCapabilities::parallel_safe_read(16)
        } else {
            crate::tools::ToolCapabilities::conservative()
        };
        capabilities.output_kind = crate::tools::ToolOutputKind::Diff;
        capabilities
    }

    async fn execute(&self, args: &serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(ToolOutput::error("file_path parameter is required")),
        };
        let requests = match edit_requests(args) {
            Ok(requests) => requests,
            Err(error) => return Ok(ToolOutput::error(error)),
        };
        let edit_count = requests.len();
        let multi = args.get("edits").is_some();

        let dry_run = args
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_replacements = match positive_usize_arg(args, "max_replacements") {
            Ok(value) => value,
            Err(error) => return Ok(ToolOutput::error(error)),
        };
        let expected_replacements = match positive_usize_arg(args, "expected_replacements") {
            Ok(value) => value,
            Err(error) => return Ok(ToolOutput::error(error)),
        };

        let workspace_path = match ctx.resolve_workspace_path(file_path) {
            Ok(path) => path,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to resolve path: {}", e))),
        };
        let display_path = ctx.workspace_services.display_path(&workspace_path);

        let (content, version) = match ctx.workspace_services.read_for_edit(&workspace_path).await {
            Ok(pair) => pair,
            Err(e) => {
                return Ok(ToolOutput::error(format!(
                    "Failed to read file {}: {}",
                    display_path, e
                )));
            }
        };

        // Every entry is applied to the in-memory copy first: a bad hunk anywhere leaves the file
        // untouched, so one call is exactly as safe as a single edit while costing one round trip.
        let mut new_content = content.clone();
        let mut replacement_count = 0usize;
        for (index, request) in requests.iter().enumerate() {
            match crate::text::apply_line_ending_edit(
                &new_content,
                &request.old_string,
                &request.new_string,
                request.replace_all,
            ) {
                Ok(applied) => {
                    new_content = applied.content;
                    replacement_count += applied.count;
                }
                Err(error) => {
                    let label = if multi {
                        format!("edits[{index}]: ")
                    } else {
                        String::new()
                    };
                    let message = match error {
                        crate::text::LineEndingEditError::NotFound => {
                            format!("{label}old_string not found in {display_path}")
                        }
                        crate::text::LineEndingEditError::Ambiguous(count) => format!(
                            "{label}old_string found {count} times in {display_path}. Use replace_all=true to replace all occurrences, or provide a more specific string."
                        ),
                        crate::text::LineEndingEditError::Unsafe => format!(
                            "{label}Refusing to edit {display_path}: the file or the replacement mixes LF and CRLF, or a match would split a CRLF pair. Normalize the file to one line ending and retry."
                        ),
                    };
                    return Ok(ToolOutput::error(message));
                }
            }
        }
        if let Some(expected) = expected_replacements {
            if replacement_count != expected {
                return Ok(ToolOutput::error(format!(
                    "Replacement count mismatch in {display_path}: expected {expected} replacement(s), found {replacement_count}. Re-run dry_run to inspect the current file."
                ))
                .with_metadata(serde_json::json!({
                    "file_path": file_path,
                    "dry_run": dry_run,
                    "expected_replacements": expected,
                    "replacement_count": replacement_count,
                })));
            }
        }
        if let Some(maximum) = max_replacements {
            if replacement_count > maximum {
                return Ok(ToolOutput::error(format!(
                    "Edit would replace {replacement_count} occurrence(s) in {display_path}, which exceeds max_replacements={maximum}."
                ))
                .with_metadata(serde_json::json!({
                    "file_path": file_path,
                    "dry_run": dry_run,
                    "max_replacements": maximum,
                    "replacement_count": replacement_count,
                })));
            }
        }

        let change_metadata = || {
            serde_json::json!({
                "file_path": file_path,
                "before": content,
                "after": new_content,
                "dry_run": dry_run,
                "edit_count": edit_count,
                "replacement_count": replacement_count,
                "expected_replacements": expected_replacements,
                "max_replacements": max_replacements,
            })
        };
        if dry_run {
            let message = if multi {
                format!(
                    "Dry run: would apply {edit_count} edit(s) ({replacement_count} replacement(s)) in {display_path}; nothing was written"
                )
            } else {
                format!(
                    "Dry run: would replace {replacement_count} occurrence(s) in {display_path}; nothing was written"
                )
            };
            return Ok(ToolOutput::success(message).with_metadata(change_metadata()));
        }

        let session_id = ctx.session_id.as_deref().filter(|id| !id.trim().is_empty());
        let newly_acquired = session_id.is_some_and(|id| {
            !crate::external_observation::session_owns_write(id, &ctx.workspace, file_path)
        });
        if let Err(error) = crate::external_observation::claim_bound_write(
            ctx.session_id.as_deref(),
            &ctx.workspace,
            file_path,
        ) {
            return Ok(ToolOutput::error(error));
        }

        match ctx
            .workspace_services
            .write_for_edit(&workspace_path, &new_content, version.as_deref())
            .await
        {
            Ok(_) => {
                let message = if multi {
                    format!(
                        "Applied {edit_count} edit(s) ({replacement_count} replacement(s)) to {display_path}"
                    )
                } else {
                    format!("Replaced {replacement_count} occurrence(s) in {display_path}")
                };
                Ok(ToolOutput::success(message).with_metadata(change_metadata()))
            }
            Err(e) => {
                if newly_acquired {
                    if let Some(id) = session_id {
                        crate::external_observation::release_write_claim(
                            id,
                            &ctx.workspace,
                            file_path,
                        );
                    }
                }
                // Surface the typed kind via ToolOutput.error_kind so SDK
                // callers can react programmatically; the human-readable
                // `content` message stays the same so the model sees the
                // retry hint.
                let typed = crate::tools::ToolErrorKind::from_workspace_error(&e);
                let out = if matches!(e, WorkspaceError::VersionConflict(_)) {
                    ToolOutput::error(format!(
                        "Concurrent modification detected on {}: the file changed between read and write. Re-read the file and retry the edit.",
                        display_path
                    ))
                } else {
                    ToolOutput::error(format!("Failed to write file {}: {}", display_path, e))
                };
                Ok(match typed {
                    Some(kind) => out.with_error_kind(kind),
                    None => out,
                })
            }
        }
    }
}

/// One replacement inside a file edit call.
struct EditRequest {
    old_string: String,
    new_string: String,
    replace_all: bool,
}

/// Accept either the single `old_string`/`new_string` pair or the `edits` array that batches
/// several hunks of the same file into one call. The two forms are mutually exclusive so a
/// half-merged payload cannot silently drop one of them.
fn edit_requests(args: &serde_json::Value) -> std::result::Result<Vec<EditRequest>, String> {
    let single_old = args.get("old_string").and_then(|v| v.as_str());
    let single_new = args.get("new_string").and_then(|v| v.as_str());

    if let Some(value) = args.get("edits") {
        if single_old.is_some() || single_new.is_some() {
            return Err("Provide either old_string/new_string or edits, not both.".to_string());
        }
        let entries = value.as_array().ok_or_else(|| {
            "edits must be an array of {old_string, new_string} entries".to_string()
        })?;
        if entries.is_empty() {
            return Err("edits must contain at least one replacement".to_string());
        }
        let mut requests = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            let old_string = entry
                .get("old_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("edits[{index}].old_string is required"))?;
            if old_string.is_empty() {
                return Err(format!("edits[{index}].old_string must not be empty"));
            }
            let new_string = entry
                .get("new_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("edits[{index}].new_string is required"))?;
            requests.push(EditRequest {
                old_string: old_string.to_string(),
                new_string: new_string.to_string(),
                replace_all: entry
                    .get("replace_all")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            });
        }
        return Ok(requests);
    }

    let old_string = single_old.ok_or_else(|| "old_string parameter is required".to_string())?;
    if old_string.is_empty() {
        return Err("old_string must not be empty".to_string());
    }
    let new_string = single_new.ok_or_else(|| "new_string parameter is required".to_string())?;
    Ok(vec![EditRequest {
        old_string: old_string.to_string(),
        new_string: new_string.to_string(),
        replace_all: args
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }])
}

fn positive_usize_arg(
    args: &serde_json::Value,
    name: &str,
) -> std::result::Result<Option<usize>, String> {
    match args.get(name) {
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or_else(|| format!("{name} must be a positive integer")),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn one_call_applies_several_hunks_atomically() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("verify.py"),
            "def check():
    return 1

result = check()
",
        )
        .unwrap();
        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-multi");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "verify.py",
                    "edits": [
                        { "old_string": "return 1", "new_string": "return 0" },
                        { "old_string": "result = check()", "new_string": "result = check() + 1" }
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        assert!(
            result.content.contains("Applied 2 edit(s)"),
            "{}",
            result.content
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("verify.py")).unwrap(),
            "def check():
    return 0

result = check() + 1
"
        );
        let metadata = result.metadata.expect("diff metadata");
        assert_eq!(metadata["edit_count"], 2);
        assert_eq!(metadata["replacement_count"], 2);
        crate::external_observation::release_session("edit-multi");
    }

    #[tokio::test]
    async fn a_failed_hunk_writes_nothing_and_names_its_index() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("verify.py"),
            "alpha
beta
",
        )
        .unwrap();
        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-atomic");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "verify.py",
                    "edits": [
                        { "old_string": "alpha", "new_string": "ALPHA" },
                        { "old_string": "missing", "new_string": "unused" }
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.content.contains("edits[1]"), "{}", result.content);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("verify.py")).unwrap(),
            "alpha
beta
",
            "an atomic call must not leave the first hunk applied"
        );
        crate::external_observation::release_session("edit-atomic");
    }

    #[tokio::test]
    async fn rejects_a_payload_with_both_edit_forms() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("verify.py"),
            "alpha
",
        )
        .unwrap();
        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-mixed");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "verify.py",
                    "old_string": "alpha",
                    "new_string": "beta",
                    "edits": [{ "old_string": "alpha", "new_string": "gamma" }]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.content.contains("not both"), "{}", result.content);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("verify.py")).unwrap(),
            "alpha
"
        );
        crate::external_observation::release_session("edit-mixed");
    }

    #[tokio::test]
    async fn test_edit_single_replace() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "hello world").unwrap();

        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-test");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "hello",
                    "new_string": "goodbye"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success);
        let content = std::fs::read_to_string(temp.path().join("test.txt")).unwrap();
        assert_eq!(content, "goodbye world");
        crate::external_observation::release_session("edit-test");
    }

    #[tokio::test]
    async fn edit_matches_lf_needle_and_preserves_crlf() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "keep\r\nalpha\r\nbeta\r\n").unwrap();
        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-crlf");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "alpha\nbeta",
                    "new_string": "alpha\ngamma"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        let content = std::fs::read_to_string(temp.path().join("test.txt")).unwrap();
        assert_eq!(content, "keep\r\nalpha\r\ngamma\r\n");
        crate::external_observation::release_session("edit-crlf");
    }

    #[tokio::test]
    async fn edit_refuses_mixed_line_endings() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "alpha\r\nbeta\n").unwrap();
        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-mixed");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "alpha\nbeta",
                    "new_string": "alpha\ngamma"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success, "{}", result.content);
        assert!(
            result.content.contains("mixes LF and CRLF"),
            "{}",
            result.content
        );
        let content = std::fs::read_to_string(temp.path().join("test.txt")).unwrap();
        assert_eq!(content, "alpha\r\nbeta\n");
        crate::external_observation::release_session("edit-mixed");
    }

    #[tokio::test]
    async fn edit_without_a_session_does_not_apply_or_claim_an_owner() {
        let temp = tempfile::tempdir().unwrap();
        let original = "hello world";
        std::fs::write(temp.path().join("test.txt"), original).unwrap();
        let tool = EditTool;
        let unbound = ToolContext::new(temp.path().to_path_buf());

        for session_id in [None, Some("   ")] {
            let ctx = match session_id {
                Some(id) => unbound.clone().with_session_id(id),
                None => unbound.clone(),
            };
            let result = tool
                .execute(
                    &serde_json::json!({
                        "file_path": "test.txt",
                        "old_string": "hello",
                        "new_string": "goodbye"
                    }),
                    &ctx,
                )
                .await
                .unwrap();
            assert!(!result.success, "{session_id:?}");
            assert_eq!(
                std::fs::read_to_string(temp.path().join("test.txt")).unwrap(),
                original,
                "{session_id:?}"
            );
        }

        let bound = unbound.with_session_id("edit-after-unbound");
        let applied = tool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "hello",
                    "new_string": "goodbye"
                }),
                &bound,
            )
            .await
            .unwrap();
        assert!(applied.success, "{}", applied.content);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("test.txt")).unwrap(),
            "goodbye world"
        );
        crate::external_observation::release_session("edit-after-unbound");
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "aaa bbb aaa").unwrap();

        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-test");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "aaa",
                    "new_string": "ccc",
                    "replace_all": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success);
        let content = std::fs::read_to_string(temp.path().join("test.txt")).unwrap();
        assert_eq!(content, "ccc bbb ccc");
    }

    #[tokio::test]
    async fn test_edit_dry_run_returns_diff_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.txt");
        std::fs::write(&path, "alpha alpha").unwrap();
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-test");

        let result = EditTool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "alpha",
                    "new_string": "beta",
                    "replace_all": true,
                    "dry_run": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "alpha alpha");
        let metadata = result.metadata.unwrap();
        assert_eq!(metadata["dry_run"], true);
        assert_eq!(metadata["replacement_count"], 2);
        assert_eq!(metadata["before"], "alpha alpha");
        assert_eq!(metadata["after"], "beta beta");
        assert!(
            EditTool
                .capabilities(&serde_json::json!({"dry_run": true}))
                .read_only
        );
    }

    #[tokio::test]
    async fn test_edit_expected_replacements_mismatch_does_not_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.txt");
        std::fs::write(&path, "alpha alpha").unwrap();
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-test");

        let result = EditTool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "alpha",
                    "new_string": "beta",
                    "replace_all": true,
                    "expected_replacements": 3
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result
            .content
            .contains("expected 3 replacement(s), found 2"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "alpha alpha");
    }

    #[tokio::test]
    async fn test_edit_max_replacements_blocks_oversized_change() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.txt");
        std::fs::write(&path, "alpha alpha alpha").unwrap();
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-test");

        let result = EditTool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "alpha",
                    "new_string": "beta",
                    "replace_all": true,
                    "max_replacements": 2
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.content.contains("exceeds max_replacements=2"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "alpha alpha alpha");
    }

    #[tokio::test]
    async fn test_edit_not_unique() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "aaa bbb aaa").unwrap();

        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-test");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "aaa",
                    "new_string": "ccc"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.content.contains("2 times"));
    }

    #[tokio::test]
    async fn test_edit_not_found() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.txt"), "hello world").unwrap();

        let tool = EditTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-test");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "test.txt",
                    "old_string": "xyz",
                    "new_string": "abc"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn preview_or_miss_does_not_own_a_file_that_was_not_written() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("guest.txt"), "hello").unwrap();
        std::fs::write(temp.path().join("other.txt"), "stay").unwrap();
        std::fs::write(temp.path().join("missing.txt"), "keep").unwrap();
        let owner =
            ToolContext::new(temp.path().to_path_buf()).with_session_id("edit-preview-owner");
        let tool = EditTool;

        let written = tool
            .execute(
                &serde_json::json!({
                    "file_path": "guest.txt",
                    "old_string": "hello",
                    "new_string": "HELLO"
                }),
                &owner,
            )
            .await
            .unwrap();
        assert!(written.success, "{}", written.content);

        let preview = tool
            .execute(
                &serde_json::json!({
                    "file_path": "other.txt",
                    "old_string": "stay",
                    "new_string": "STAY",
                    "dry_run": true
                }),
                &owner,
            )
            .await
            .unwrap();
        assert!(preview.success, "{}", preview.content);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("other.txt")).unwrap(),
            "stay"
        );

        let missed = tool
            .execute(
                &serde_json::json!({
                    "file_path": "missing.txt",
                    "old_string": "absent",
                    "new_string": "nope"
                }),
                &owner,
            )
            .await
            .unwrap();
        assert!(!missed.success);

        let blocked = crate::external_observation::claim_bound_write(
            Some("edit-preview-other"),
            temp.path(),
            "guest.txt",
        );
        assert!(blocked.is_err(), "a real edit must keep the claim");
        assert!(
            crate::external_observation::claim_bound_write(
                Some("edit-preview-other"),
                temp.path(),
                "other.txt",
            )
            .is_ok(),
            "a dry run must not own a file it did not write"
        );
        assert!(
            crate::external_observation::claim_bound_write(
                Some("edit-preview-other"),
                temp.path(),
                "missing.txt",
            )
            .is_ok(),
            "a missed edit must not own a file it did not write"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("guest.txt")).unwrap(),
            "HELLO"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("missing.txt")).unwrap(),
            "keep"
        );

        crate::external_observation::release_session("edit-preview-owner");
        crate::external_observation::release_session("edit-preview-other");
    }

    #[test]
    fn test_edit_schema_is_canonical() {
        let tool = EditTool;
        let params = tool.parameters();
        assert_eq!(params["additionalProperties"], false);
        // The payload is either one replacement or an `edits` batch, so only the path is required.
        assert_eq!(params["required"], serde_json::json!(["file_path"]));
        assert_eq!(params["properties"]["edits"]["type"], "array");
        assert_eq!(params["properties"]["edits"]["minItems"], 1);
        assert_eq!(
            params["properties"]["edits"]["items"]["required"],
            serde_json::json!(["old_string", "new_string"])
        );
        let examples = params["examples"].as_array().unwrap();
        assert_eq!(examples[0]["file_path"], "src/lib.rs");
        assert!(examples[0].get("path").is_none());
        assert_eq!(params["properties"]["dry_run"]["type"], "boolean");
        assert_eq!(
            params["properties"]["max_replacements"]["minimum"],
            serde_json::json!(1)
        );
        assert_eq!(
            params["properties"]["expected_replacements"]["minimum"],
            serde_json::json!(1)
        );
    }

    #[tokio::test]
    async fn test_edit_surfaces_concurrent_modification_as_typed_error() {
        // Mock backend whose write step always reports a version conflict —
        // simulating an S3 If-Match 412 between the read and the write.
        // Verifies that:
        //  (1) edit matches on WorkspaceError::VersionConflict directly,
        //  (2) the user-facing message includes "Concurrent modification"
        //      (so the model can retry) rather than the generic write error.
        use crate::workspace::{
            WorkspaceDirEntry, WorkspaceFileSystem, WorkspaceFileSystemExt, WorkspacePath,
            WorkspaceRef, WorkspaceResult, WorkspaceServices, WorkspaceVersionConflict,
            WorkspaceWriteOutcome,
        };
        use async_trait::async_trait;
        use std::sync::Arc;

        struct AlwaysConflictFs;

        #[async_trait]
        impl WorkspaceFileSystem for AlwaysConflictFs {
            async fn read_text(&self, _path: &WorkspacePath) -> WorkspaceResult<String> {
                Ok("hello world".to_string())
            }
            async fn write_text(
                &self,
                _path: &WorkspacePath,
                content: &str,
            ) -> WorkspaceResult<WorkspaceWriteOutcome> {
                Ok(WorkspaceWriteOutcome {
                    bytes: content.len(),
                    lines: content.lines().count(),
                })
            }
            async fn list_dir(
                &self,
                _path: &WorkspacePath,
            ) -> WorkspaceResult<Vec<WorkspaceDirEntry>> {
                Ok(Vec::new())
            }
        }

        #[async_trait]
        impl WorkspaceFileSystemExt for AlwaysConflictFs {
            async fn read_text_with_version(
                &self,
                _path: &WorkspacePath,
            ) -> WorkspaceResult<(String, String)> {
                Ok(("hello world".to_string(), "v0".to_string()))
            }
            async fn write_text_if_version(
                &self,
                path: &WorkspacePath,
                _content: &str,
                _expected_version: &str,
            ) -> WorkspaceResult<WorkspaceWriteOutcome> {
                Err(WorkspaceError::VersionConflict(WorkspaceVersionConflict {
                    path: path.as_str().to_string(),
                    expected: "v0".to_string(),
                    actual: Some("v-other".to_string()),
                }))
            }
        }

        let backend = Arc::new(AlwaysConflictFs);
        let fs: Arc<dyn WorkspaceFileSystem> = backend.clone();
        let fs_ext: Arc<dyn WorkspaceFileSystemExt> = backend;
        let services = WorkspaceServices::builder(WorkspaceRef::new("mem", "mem://ws"), fs)
            .file_system_ext(fs_ext)
            .build();

        let tool = EditTool;
        let ctx = ToolContext::new(std::env::temp_dir())
            .with_session_id("edit-test")
            .with_workspace_services(services);

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "anything.txt",
                    "old_string": "hello",
                    "new_string": "goodbye",
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            !result.success,
            "edit on conflicting backend must report failure"
        );
        assert!(
            result.content.contains("Concurrent modification"),
            "expected retry-friendly conflict message, got: {}",
            result.content
        );

        // Phase 8: the typed error_kind must also survive end-to-end so SDK
        // callers can branch on it without parsing the string.
        let kind = result
            .error_kind
            .as_ref()
            .expect("edit must surface a typed error_kind for VersionConflict");
        match kind {
            crate::tools::ToolErrorKind::VersionConflict {
                path,
                expected,
                actual,
            } => {
                assert_eq!(path, "anything.txt");
                assert_eq!(expected, "v0");
                assert_eq!(actual.as_deref(), Some("v-other"));
            }
            other => panic!("expected VersionConflict kind, got {other:?}"),
        }

        // The serialised wire shape is the contract SDKs will see. Pin it
        // so any accidental rename / restructure breaks the build.
        let json = serde_json::to_value(kind).unwrap();
        assert_eq!(json["type"], "version_conflict");
        assert_eq!(json["path"], "anything.txt");
        assert_eq!(json["expected"], "v0");
        assert_eq!(json["actual"], "v-other");
    }
}
