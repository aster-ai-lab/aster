//! Write tool - Write content to files

use crate::tools::types::{Tool, ToolContext, ToolOutput};
use crate::workspace::WorkspaceError;
use anyhow::Result;
use async_trait::async_trait;

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write a file (default overwrite), or append bounded chunks after the first write with expected_offset for safe retries."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "File path."
                },
                "content": {
                    "type": "string",
                    "description": "Full replacement/first chunk for overwrite; next chunk for append."
                },
                "mode": {
                    "type": "string",
                    "enum": ["overwrite", "append"],
                    "default": "overwrite",
                    "description": "overwrite replaces the file (default); append adds a chunk and requires expected_offset."
                },
                "expected_offset": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Append requires the stored UTF-8 byte size. Chunks adopt existing line endings; continue from the reported size. Matching already-applied chunks retry successfully; other mismatches fail. Overwrite may omit this or send neutral 0 (ignored)."
                }
            },
            "required": ["file_path", "content"],
            "examples": [
                {
                    "file_path": "notes.txt",
                    "content": "hello world"
                },
                {
                    "file_path": "report.html",
                    "content": "<section>next chunk</section>",
                    "mode": "append",
                    "expected_offset": 8192
                }
            ]
        })
    }

    fn capabilities(&self, args: &serde_json::Value) -> crate::tools::ToolCapabilities {
        let mut capabilities = crate::tools::ToolCapabilities::conservative();
        capabilities.idempotent = true;
        capabilities.resumable =
            args.get("mode").and_then(|value| value.as_str()) == Some("append");
        capabilities.output_kind = crate::tools::ToolOutputKind::Diff;
        capabilities
    }

    async fn execute(&self, args: &serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(ToolOutput::error("file_path parameter is required")),
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return Ok(ToolOutput::error("content parameter is required")),
        };

        let mode = match args.get("mode").and_then(|value| value.as_str()) {
            None | Some("overwrite") => "overwrite",
            Some("append") => "append",
            Some(other) => {
                return Ok(ToolOutput::error(format!(
                    "mode must be 'overwrite' or 'append', got '{other}'"
                )));
            }
        };

        let expected_offset = match (mode, args.get("expected_offset")) {
            ("append", Some(value)) => match value
                .as_u64()
                .and_then(|offset| usize::try_from(offset).ok())
            {
                Some(offset) => Some(offset),
                None => {
                    return Ok(ToolOutput::error(
                        "expected_offset must be a non-negative byte offset that fits this platform",
                    ));
                }
            },
            ("append", None) => {
                return Ok(ToolOutput::error(
                    "expected_offset parameter is required in append mode",
                ));
            }
            // Some model providers materialize every optional integer field in
            // a tool schema with its minimum value. Accepting the neutral value
            // keeps overwrite interoperable without weakening append's
            // compare-and-append contract.
            ("overwrite", Some(value)) if value.as_u64() == Some(0) => None,
            ("overwrite", Some(_)) => {
                return Ok(ToolOutput::error(
                    "expected_offset must be omitted or 0 in overwrite mode",
                ));
            }
            _ => None,
        };

        let workspace_path = match ctx.resolve_workspace_path(file_path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to resolve path: {}", e))),
        };

        if let Some(expected_offset) = expected_offset {
            let display_path = ctx.workspace_services.display_path(&workspace_path);
            let (mut current, version) = match ctx
                .workspace_services
                .read_for_edit(&workspace_path)
                .await
            {
                Ok(pair) => pair,
                Err(error) => {
                    return Ok(ToolOutput::error(format!(
                        "Failed to append to file {}: {}. Start a segmented write with mode='overwrite'.",
                        display_path, error
                    )));
                }
            };

            // The chunk joins the file in the file's newline style, so a retry
            // is judged against the prefix it was appended to.
            let content = match current.get(..expected_offset) {
                Some(prefix) => crate::text::append_newline_style(prefix, content),
                None => std::borrow::Cow::Borrowed(content),
            };
            let resulting_size = match expected_offset.checked_add(content.len()) {
                Some(size) => size,
                None => {
                    return Ok(ToolOutput::error(
                        "append size exceeds the supported byte range",
                    ));
                }
            };
            if current.len() != expected_offset {
                let already_applied = current.len() == resulting_size
                    && current
                        .as_bytes()
                        .get(expected_offset..)
                        .is_some_and(|suffix| suffix == content.as_bytes());
                if already_applied {
                    return Ok(append_success_output(
                        file_path,
                        &display_path,
                        expected_offset,
                        content.len(),
                        resulting_size,
                        true,
                    ));
                }
                return Ok(ToolOutput::error(format!(
                    "Append offset mismatch for {}: expected {} bytes, found {} bytes. Read the current file tail or resume from the reported size; the file was not changed.",
                    display_path,
                    expected_offset,
                    current.len()
                )));
            }

            current.push_str(&content);
            let newly_acquired = match begin_write_claim(ctx, file_path) {
                Ok(acquired) => acquired,
                Err(output) => return Ok(output),
            };
            return match ctx
                .workspace_services
                .write_for_edit(&workspace_path, &current, version.as_deref())
                .await
            {
                Ok(_) => Ok(append_success_output(
                    file_path,
                    &display_path,
                    expected_offset,
                    content.len(),
                    resulting_size,
                    false,
                )),
                Err(error) => {
                    release_new_write_claim(ctx, file_path, newly_acquired);
                    let typed = crate::tools::ToolErrorKind::from_workspace_error(&error);
                    let output = if matches!(error, WorkspaceError::VersionConflict(_)) {
                        ToolOutput::error(format!(
                            "Concurrent modification detected on {} while appending at byte offset {}. Re-read the file and retry from its current size.",
                            display_path, expected_offset
                        ))
                    } else {
                        ToolOutput::error(format!(
                            "Failed to append to file {}: {}",
                            display_path, error
                        ))
                    };
                    Ok(match typed {
                        Some(kind) => output.with_error_kind(kind),
                        None => output,
                    })
                }
            };
        }

        // Read existing content for diff metadata (if file exists)
        let fs = ctx.workspace_services.fs();
        let path_for_before = workspace_path.clone();
        let fs_for_before = fs.clone();
        let before_content = ctx
            .workspace_services
            .run_with_timeout("read_text", async move {
                fs_for_before.read_text(&path_for_before).await
            })
            .await
            .ok();

        let path_for_write = workspace_path.clone();
        // An LF body from the model must not flip an existing CRLF file to LF,
        // and a CRLF body must not flip an LF file. Mixed endings are written
        // unchanged. Append adapts its chunk the same way, and reports stored bytes.
        let content_for_write = match before_content.as_deref() {
            Some(existing) => crate::text::preserve_newline_style(existing, content).into_owned(),
            None => content.to_string(),
        };
        let written = content_for_write.clone();
        let newly_acquired = match begin_write_claim(ctx, file_path) {
            Ok(acquired) => acquired,
            Err(output) => return Ok(output),
        };
        match ctx
            .workspace_services
            .run_with_timeout("write_text", async move {
                fs.write_text(&path_for_write, &written).await
            })
            .await
        {
            Ok(outcome) => {
                // Attach diff metadata
                // Keep creation explicit instead of asking renderers to infer
                // it from a missing `before` field.  The latter is ambiguous
                // for older adapters that may omit one side of a payload.
                let created = before_content.is_none();
                let mut metadata = serde_json::Map::new();
                metadata.insert("file_path".to_string(), serde_json::json!(file_path));
                metadata.insert("created".to_string(), serde_json::json!(created));
                metadata.insert("after".to_string(), serde_json::json!(content_for_write));
                if let Some(before) = before_content {
                    metadata.insert("before".to_string(), serde_json::json!(before));
                }

                Ok(ToolOutput::success(format!(
                    "Wrote {} bytes ({} lines) to {}",
                    outcome.bytes,
                    outcome.lines,
                    ctx.workspace_services.display_path(&workspace_path)
                ))
                .with_metadata(serde_json::Value::Object(metadata)))
            }
            Err(e) => {
                release_new_write_claim(ctx, file_path, newly_acquired);
                Ok(ToolOutput::error(format!(
                    "Failed to write file {}: {}",
                    ctx.workspace_services.display_path(&workspace_path),
                    e
                )))
            }
        }
    }
}

#[allow(clippy::result_large_err)]
fn begin_write_claim(ctx: &ToolContext, file_path: &str) -> std::result::Result<bool, ToolOutput> {
    let session_id = ctx.session_id.as_deref().filter(|id| !id.trim().is_empty());
    let newly_acquired = session_id.is_some_and(|id| {
        !crate::external_observation::session_owns_write(id, &ctx.workspace, file_path)
    });
    crate::external_observation::claim_bound_write(
        ctx.session_id.as_deref(),
        &ctx.workspace,
        file_path,
    )
    .map_err(ToolOutput::error)?;
    Ok(newly_acquired)
}

fn release_new_write_claim(ctx: &ToolContext, file_path: &str, newly_acquired: bool) {
    if !newly_acquired {
        return;
    }
    if let Some(id) = ctx.session_id.as_deref().filter(|id| !id.trim().is_empty()) {
        crate::external_observation::release_write_claim(id, &ctx.workspace, file_path);
    }
}

fn append_success_output(
    file_path: &str,
    display_path: &str,
    offset: usize,
    appended_bytes: usize,
    resulting_size: usize,
    already_applied: bool,
) -> ToolOutput {
    let mut metadata = serde_json::Map::new();
    metadata.insert("file_path".to_string(), serde_json::json!(file_path));
    metadata.insert("mode".to_string(), serde_json::json!("append"));
    metadata.insert("offset".to_string(), serde_json::json!(offset));
    metadata.insert(
        "appended_bytes".to_string(),
        serde_json::json!(appended_bytes),
    );
    metadata.insert(
        "resulting_size".to_string(),
        serde_json::json!(resulting_size),
    );
    metadata.insert(
        "already_applied".to_string(),
        serde_json::json!(already_applied),
    );

    let content = if already_applied {
        format!(
            "Append chunk already applied at byte offset {} in {}; no duplicate content was written (file size: {} bytes)",
            offset, display_path, resulting_size
        )
    } else {
        format!(
            "Appended {} bytes at byte offset {} to {} (file size: {} bytes)",
            appended_bytes, offset, display_path, resulting_size
        )
    };
    ToolOutput::success(content).with_metadata(serde_json::Value::Object(metadata))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_new_file() {
        let temp = tempfile::tempdir().unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");

        let result = tool
            .execute(
                &serde_json::json!({"file_path": "new.txt", "content": "hello world"}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            result
                .metadata
                .as_ref()
                .and_then(|value| value.get("created")),
            Some(&serde_json::Value::Bool(true)),
        );
        let content = std::fs::read_to_string(temp.path().join("new.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");

        let result = tool
            .execute(
                &serde_json::json!({"file_path": "sub/dir/file.txt", "content": "nested"}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success);
        let content = std::fs::read_to_string(temp.path().join("sub/dir/file.txt")).unwrap();
        assert_eq!(content, "nested");
    }

    #[tokio::test]
    async fn test_write_overwrite_accepts_provider_materialized_zero_offset() {
        let temp = tempfile::tempdir().unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "provider-compatible.txt",
                    "content": "hello",
                    "mode": "overwrite",
                    "expected_offset": 0
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("provider-compatible.txt")).unwrap(),
            "hello"
        );
    }

    #[tokio::test]
    async fn test_write_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("existing.txt"), "old").unwrap();

        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");

        let result = tool
            .execute(
                &serde_json::json!({"file_path": "existing.txt", "content": "new"}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(
            result
                .metadata
                .as_ref()
                .and_then(|value| value.get("created")),
            Some(&serde_json::Value::Bool(false)),
        );
        let content = std::fs::read_to_string(temp.path().join("existing.txt")).unwrap();
        assert_eq!(content, "new");
    }

    #[tokio::test]
    async fn overwrite_keeps_an_existing_crlf_file_crlf() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("win.txt"), "alpha\r\nbeta\r\n").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-crlf");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "win.txt",
                    "content": "alpha\ngamma\n"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        let content = std::fs::read_to_string(temp.path().join("win.txt")).unwrap();
        assert_eq!(content, "alpha\r\ngamma\r\n");
        crate::external_observation::release_session("write-crlf");
    }

    #[tokio::test]
    async fn new_file_keeps_the_authored_line_ending() {
        let temp = tempfile::tempdir().unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-lf");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "new.txt",
                    "content": "alpha\nbeta\n"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        let content = std::fs::read_to_string(temp.path().join("new.txt")).unwrap();
        assert_eq!(content, "alpha\nbeta\n");
        crate::external_observation::release_session("write-lf");
    }

    #[tokio::test]
    async fn test_write_append_reconstructs_large_content() {
        let temp = tempfile::tempdir().unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");
        let chunk = "0123456789abcdef".repeat(1024);
        let expected = chunk.repeat(16);

        tool.execute(
            &serde_json::json!({
                "file_path": "large.txt",
                "content": chunk,
                "mode": "overwrite"
            }),
            &ctx,
        )
        .await
        .unwrap();

        for index in 1..16 {
            let result = tool
                .execute(
                    &serde_json::json!({
                        "file_path": "large.txt",
                        "content": chunk,
                        "mode": "append",
                        "expected_offset": index * chunk.len()
                    }),
                    &ctx,
                )
                .await
                .unwrap();
            assert!(result.success, "{}", result.content);
        }

        let content = std::fs::read_to_string(temp.path().join("large.txt")).unwrap();
        assert_eq!(content, expected);
    }

    #[tokio::test]
    async fn append_onto_a_crlf_file_keeps_it_editable() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("win.txt");
        std::fs::write(&path, "alpha\r\nbeta\r\n").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-append-crlf");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "win.txt",
                    "content": "gamma\ndelta\n",
                    "mode": "append",
                    "expected_offset": 13
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        let expected = "alpha\r\nbeta\r\ngamma\r\ndelta\r\n";
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
        let metadata = result.metadata.expect("append metadata");
        assert_eq!(metadata["appended_bytes"], 14);
        assert_eq!(metadata["resulting_size"], expected.len());

        let edited = crate::tools::builtin::edit::EditTool
            .execute(
                &serde_json::json!({
                    "file_path": "win.txt",
                    "old_string": "delta",
                    "new_string": "omega"
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(edited.success, "{}", edited.content);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "alpha\r\nbeta\r\ngamma\r\nomega\r\n"
        );
        crate::external_observation::release_session("write-append-crlf");
    }

    #[tokio::test]
    async fn crlf_append_retry_is_idempotent_and_chunks_may_split_a_pair() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("win.txt");
        std::fs::write(&path, "a\r\n").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-append-retry");
        let append = |content: &str, offset: usize| {
            serde_json::json!({
                "file_path": "win.txt",
                "content": content,
                "mode": "append",
                "expected_offset": offset
            })
        };

        let first = append("b\nc\n", 3);
        assert!(tool.execute(&first, &ctx).await.unwrap().success);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\r\nb\r\nc\r\n");

        let retry = tool.execute(&first, &ctx).await.unwrap();
        assert!(retry.success, "{}", retry.content);
        assert!(
            retry.content.contains("already applied"),
            "{}",
            retry.content
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\r\nb\r\nc\r\n");

        assert!(tool.execute(&append("d\r", 9), &ctx).await.unwrap().success);
        assert!(
            tool.execute(&append("\ne\n", 11), &ctx)
                .await
                .unwrap()
                .success
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "a\r\nb\r\nc\r\nd\r\ne\r\n"
        );
        crate::external_observation::release_session("write-append-retry");
    }

    #[tokio::test]
    async fn append_onto_an_lf_file_stays_lf() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("unix.txt");
        std::fs::write(&path, "a\nb\n").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-append-lf");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "unix.txt",
                    "content": "c\r\nd\r\n",
                    "mode": "append",
                    "expected_offset": 4
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.content);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nb\nc\nd\n");
        crate::external_observation::release_session("write-append-lf");
    }

    #[tokio::test]
    async fn test_write_append_rejects_offset_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("existing.txt"), "prefix").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "existing.txt",
                    "content": "suffix",
                    "mode": "append",
                    "expected_offset": 3
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(
            result.content.contains("offset mismatch"),
            "{}",
            result.content
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("existing.txt")).unwrap(),
            "prefix"
        );
    }

    #[tokio::test]
    async fn test_write_append_retry_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("existing.txt"), "prefixsuffix").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");
        let args = serde_json::json!({
            "file_path": "existing.txt",
            "content": "suffix",
            "mode": "append",
            "expected_offset": 6
        });

        let result = tool.execute(&args, &ctx).await.unwrap();

        assert!(result.success, "{}", result.content);
        assert!(
            result.content.contains("already applied"),
            "{}",
            result.content
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("existing.txt")).unwrap(),
            "prefixsuffix"
        );
    }

    #[tokio::test]
    async fn test_write_append_metadata_is_bounded() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("existing.txt"), "prefix").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-test");

        let result = tool
            .execute(
                &serde_json::json!({
                    "file_path": "existing.txt",
                    "content": "suffix",
                    "mode": "append",
                    "expected_offset": 6
                }),
                &ctx,
            )
            .await
            .unwrap();

        let metadata = result.metadata.expect("append metadata");
        assert_eq!(metadata["mode"], "append");
        assert_eq!(metadata["offset"], 6);
        assert_eq!(metadata["appended_bytes"], 6);
        assert_eq!(metadata["resulting_size"], 12);
        assert!(metadata.get("before").is_none());
        assert!(metadata.get("after").is_none());
    }

    #[tokio::test]
    async fn test_write_missing_params() {
        let tool = WriteTool;
        let ctx = ToolContext::new(std::path::PathBuf::from("/tmp")).with_session_id("write-test");

        let result = tool.execute(&serde_json::json!({}), &ctx).await.unwrap();
        assert!(!result.success);

        let result = tool
            .execute(&serde_json::json!({"file_path": "x"}), &ctx)
            .await
            .unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn rejected_write_does_not_own_a_file_that_was_not_written() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("guest.txt"), "keep").unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-reject-owner");
        let result = tool
            .execute(&serde_json::json!({"file_path": "guest.txt"}), &ctx)
            .await
            .unwrap();
        assert!(!result.success);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("guest.txt")).unwrap(),
            "keep"
        );
        assert!(
            crate::external_observation::claim_bound_write(
                Some("write-reject-other"),
                temp.path(),
                "guest.txt",
            )
            .is_ok(),
            "a rejected write must not own a file it did not write"
        );
        crate::external_observation::release_session("write-reject-owner");
        crate::external_observation::release_session("write-reject-other");
    }

    #[tokio::test]
    async fn write_without_a_session_does_not_claim_an_anonymous_owner() {
        let temp = tempfile::tempdir().unwrap();
        let tool = WriteTool;
        let ctx = ToolContext::new(temp.path().to_path_buf());
        let result = tool
            .execute(
                &serde_json::json!({"file_path": "new.txt", "content": "hello"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.content.contains("session id"));
        assert!(!temp.path().join("new.txt").exists());
    }

    #[tokio::test]
    async fn write_spelling_does_not_hide_another_sessions_file() {
        let temp = tempfile::tempdir().unwrap();
        let guest = temp.path().join("guest.txt");
        let tool = WriteTool;
        let owner = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-owner");
        let applied = tool
            .execute(
                &serde_json::json!({"file_path": "guest.txt", "content": "spell-token-2a91"}),
                &owner,
            )
            .await
            .unwrap();
        assert!(applied.success, "owner write failed: {applied:?}");

        let other = ToolContext::new(temp.path().to_path_buf()).with_session_id("write-other");
        for spelling in ["./guest.txt", "subdir/../guest.txt"] {
            let denied = tool
                .execute(
                    &serde_json::json!({
                        "file_path": spelling,
                        "content": "hidden-spell-2a91",
                    }),
                    &other,
                )
                .await
                .unwrap();
            assert!(!denied.success, "{spelling} hid the claim");
            assert_eq!(std::fs::read_to_string(&guest).unwrap(), "spell-token-2a91");
        }

        let again = tool
            .execute(
                &serde_json::json!({"file_path": "./guest.txt", "content": "owner-again-2a91"}),
                &owner,
            )
            .await
            .unwrap();
        assert!(
            again.success,
            "owner spelling should still apply: {again:?}"
        );
        assert_eq!(std::fs::read_to_string(&guest).unwrap(), "owner-again-2a91");
        crate::external_observation::release_session("write-owner");
    }

    #[test]
    fn test_write_schema_is_canonical() {
        let tool = WriteTool;
        let params = tool.parameters();
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(
            params["required"],
            serde_json::json!(["file_path", "content"])
        );
        assert_eq!(
            params["properties"]["mode"]["enum"],
            serde_json::json!(["overwrite", "append"])
        );
        assert!(params["properties"].get("expected_offset").is_some());
        let examples = params["examples"].as_array().unwrap();
        assert_eq!(examples[0]["file_path"], "notes.txt");
        assert!(examples[0].get("path").is_none());
    }
}
