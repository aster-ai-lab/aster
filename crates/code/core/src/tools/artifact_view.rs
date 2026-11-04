//! Model-facing pages over retained `aster://tool-output/...` artifacts.
//!
//! A lossy tool-result transform keeps the original bytes in the session
//! artifact store. `read` and `search` page that exact content so the model
//! can recover an omitted region instead of re-running the command.

use super::builtin::grep::GrepOutputMode;
use super::pagination::PageRequest;
use super::types::{ToolContext, ToolOutput};
use super::{MAX_LINE_LENGTH, MAX_READ_LINES};
use crate::text::truncate_utf8;

pub(crate) const TOOL_OUTPUT_URI_PREFIX: &str = "aster://tool-output/";
const MAX_GREP_CONTEXT: usize = 8;
const MAX_GREP_OUTPUT_BYTES: usize = 64 * 1_024;

pub(crate) fn is_tool_output_uri(path: &str) -> bool {
    path.starts_with(TOOL_OUTPUT_URI_PREFIX)
}

fn load(ctx: &ToolContext, uri: &str) -> Result<String, String> {
    let store = ctx.artifact_store.as_ref().ok_or_else(|| {
        format!("{uri} is not readable here: this session retains tool output outside the local artifact store")
    })?;
    store.get(uri).map(|artifact| artifact.content).ok_or_else(|| {
        format!("{uri} is not retained in this session (evicted by the artifact limit or never produced here)")
    })
}

fn display_line(line: &str) -> &str {
    truncate_utf8(line, MAX_LINE_LENGTH)
}

/// One `read` page of an artifact, numbered like a workspace file read.
pub(crate) fn read_page(
    ctx: &ToolContext,
    uri: &str,
    offset: usize,
    limit: usize,
    byte_offset: usize,
) -> ToolOutput {
    let content = match load(ctx, uri) {
        Ok(content) => content,
        Err(error) => return ToolOutput::error(error),
    };
    let lines = content.lines().collect::<Vec<_>>();
    let limit = limit.min(MAX_READ_LINES);
    if offset > lines.len() {
        return ToolOutput::error(format!(
            "Offset {offset} exceeds artifact length ({} lines)",
            lines.len()
        ));
    }
    let end = offset.saturating_add(limit).min(lines.len());
    let page = match super::builtin::read::numbered_page(
        &lines[offset..end],
        offset,
        byte_offset,
        ctx.result_budget_bytes(),
    ) {
        Ok(page) => page,
        Err(error) => return ToolOutput::error(error),
    };
    let end = offset + page.returned;
    let mut output = page.text;
    let next_offset = (end < lines.len()).then_some(end);
    if let Some(next) = next_offset {
        output.push_str(&format!(
            "\n... (more lines available; continue with offset={next})\n"
        ));
    } else if output.is_empty() {
        output.push_str("(end of artifact)\n");
    }
    ToolOutput::success(output).with_metadata(serde_json::json!({
        "source_anchors": [uri],
        "range": {
            "offset": offset,
            "returned_lines": end - offset,
            "next_offset": next_offset,
            "eof": next_offset.is_none(),
            "total_lines": lines.len(),
        }
    }))
}

/// Regular-expression line search inside one artifact.
pub(crate) fn grep(
    ctx: &ToolContext,
    uri: &str,
    pattern: &str,
    case_insensitive: bool,
    context: usize,
    output_mode: GrepOutputMode,
    page_request: PageRequest,
) -> ToolOutput {
    let content = match load(ctx, uri) {
        Ok(content) => content,
        Err(error) => return ToolOutput::error(error),
    };
    let regex = match regex::RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
    {
        Ok(regex) => regex,
        Err(error) => return ToolOutput::error(format!("invalid regular expression: {error}")),
    };
    let lines = content.lines().collect::<Vec<_>>();
    let context = context.min(MAX_GREP_CONTEXT);
    let matches = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| regex.is_match(line).then_some(index))
        .collect::<Vec<_>>();
    let search = serde_json::json!({
        "matching_lines": matches.len(),
        "matching_files": usize::from(!matches.is_empty()),
        "truncated": false,
    });
    if output_mode == GrepOutputMode::Summary {
        return ToolOutput::success(format!(
            "{} matching line(s) in {} file(s)",
            matches.len(),
            usize::from(!matches.is_empty())
        ))
        .with_metadata(serde_json::json!({
            "output_mode": output_mode.as_str(), "source_anchors":[uri], "search":search,
        }));
    }
    if matches!(
        output_mode,
        GrepOutputMode::Count | GrepOutputMode::FilesWithMatches
    ) {
        let paths = if matches.is_empty() {
            vec![]
        } else {
            vec![uri]
        };
        let page = match page_request.page(paths) {
            Ok(page) => page,
            Err(error) => return ToolOutput::error(error),
        };
        let output = if page.items.is_empty() {
            "No matches found".into()
        } else if output_mode == GrepOutputMode::Count {
            format!("{uri}: {} matching line(s)", matches.len())
        } else {
            uri.into()
        };
        return ToolOutput::success(output).with_metadata(serde_json::json!({
            "output_mode":output_mode.as_str(), "source_anchors":page.items,
            "search":search, "page":page.metadata(), "matches":matches.len(),
        }));
    }
    let mut page = match page_request.page(matches) {
        Ok(page) => page,
        Err(error) => return ToolOutput::error(error),
    };
    let mut output = String::new();
    let mut shown = 0usize;
    let mut printed_until = 0usize;
    let mut budget_hit = false;
    let budget = MAX_GREP_OUTPUT_BYTES
        .min(ctx.result_budget_bytes())
        .saturating_sub(super::builtin::read::PAGE_CONTINUATION_RESERVE_BYTES);
    for &index in &page.items {
        let start = index.saturating_sub(context).max(printed_until);
        let end = (index + context + 1).min(lines.len());
        let mut entry = String::new();
        if printed_until > 0 && start > printed_until {
            entry.push_str("--\n");
        }
        for (line_index, line) in lines.iter().enumerate().take(end).skip(start) {
            let separator = if line_index == index { ':' } else { '-' };
            entry.push_str(&format!(
                "{}{separator}{}\n",
                line_index + 1,
                display_line(line)
            ));
        }
        if output.len() + entry.len() > budget {
            budget_hit = true;
            if shown > 0 {
                break;
            }
            // A wide context must not hide the match or prevent cursor progress.
            let label = format!("{}:", index + 1);
            let bytes = budget.saturating_sub(label.len() + 1).min(MAX_LINE_LENGTH);
            entry = format!("{label}{}\n", truncate_utf8(lines[index], bytes));
        }
        output.push_str(&entry);
        printed_until = end;
        shown += 1;
        if budget_hit {
            break;
        }
    }
    page.items.truncate(shown);
    let next = page.offset + shown;
    page.next_cursor = (next < page.total_items).then(|| next.to_string());
    if let Some(cursor) = &page.next_cursor {
        let reason = if budget_hit { "output budget" } else { "limit" };
        output.push_str(&format!(
            "\n... {}/{} matches ({reason}). Continue with cursor={cursor}; read exact lines with read file_path={uri} offset=<line-1>.\n",
            shown,
            page.total_items
        ));
    } else if budget_hit {
        output.push_str(&format!(
            "\nMatch context clipped; read exact lines with read file_path={uri} offset=<line-1>.\n"
        ));
    }
    if output.is_empty() {
        output = format!("No matches in {uri} ({} lines)", lines.len());
    }
    ToolOutput::success(output).with_metadata(serde_json::json!({
        "output_mode": output_mode.as_str(),
        "source_anchors": [uri],
        "matches": page.total_items,
        "shown_matches": shown,
        "search":search,
        "page":page.metadata(),
    }))
}

#[cfg(test)]
#[path = "artifact_view_tests.rs"]
mod tests;
