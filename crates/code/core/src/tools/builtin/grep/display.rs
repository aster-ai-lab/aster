//! Source locations for the model view of paginated native grep context.

use crate::workspace::{escape_control_chars_for_display, WorkspacePath};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const GREP_DISPLAY_LAYOUT_KEY: &str = "aster_grep_display_layout_v1";
const DISPLAY_LEGEND: &str =
    "Context lines use the file path on the matching line in the same grep block.\n";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DisplayRun {
    path: String,
    end: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DisplayLayout {
    digest: String,
    runs: Vec<DisplayRun>,
}

pub(super) fn extend_run(runs: &mut Vec<DisplayRun>, path: &WorkspacePath, end: usize) {
    if let Some(last) = runs.last_mut().filter(|last| last.path == path.as_str()) {
        last.end = end;
    } else {
        runs.push(DisplayRun {
            path: path.as_str().to_owned(),
            end,
        });
    }
}

pub(super) fn layout(body: &str, runs: Vec<DisplayRun>) -> Option<Value> {
    if runs.is_empty() {
        return None;
    }
    serde_json::to_value(DisplayLayout {
        digest: digest(body),
        runs,
    })
    .ok()
}

/// Keep every original match location. Elide path prefixes on its context lines
/// only when the originating tool bound the complete block to that same path.
pub(crate) fn project_grep_display(body: &str, layout: Value) -> Option<String> {
    let layout: DisplayLayout = serde_json::from_value(layout).ok()?;
    let end = layout.runs.last()?.end;
    let (matches, footer) = body.split_at_checked(end)?;
    if layout.digest != digest(matches) {
        return None;
    }

    let mut compact = String::from(DISPLAY_LEGEND);
    let mut start = 0;
    for run in layout.runs {
        let rows = matches
            .get(start..run.end)
            .filter(|rows| rows.ends_with("--\n"))?;
        let path = escape_control_chars_for_display(&run.path);
        let mut saw_match = false;
        for row in rows.split_inclusive('\n') {
            if row == "--\n" {
                if !saw_match {
                    return None;
                }
                saw_match = false;
                compact.push_str(row);
                continue;
            }
            let marker = row.get(..1).filter(|marker| matches!(*marker, ">" | " "))?;
            let numbered = row[1..].strip_prefix(&path)?.strip_prefix(':')?;
            let (line, _) = numbered.split_once(": ")?;
            if line.is_empty() || !line.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            if marker == ">" {
                if saw_match {
                    return None;
                }
                saw_match = true;
                compact.push_str(row);
            } else {
                compact.push(' ');
                compact.push_str(numbered);
            }
        }
        start = run.end;
    }
    (compact.len() < matches.len()).then(|| {
        compact.push_str(footer);
        compact
    })
}

fn digest(body: &str) -> String {
    crate::content_digest::digest_bytes("aster.code.grep-display.v1", body.as_bytes())
}
