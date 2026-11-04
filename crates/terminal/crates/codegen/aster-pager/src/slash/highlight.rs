use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::style::Color;
use unicode_width::UnicodeWidthStr;

use crate::scrollback::text_selection::ResolvedSelectionModel;

pub(super) fn token_ranges(text: &str, known: impl Fn(&str) -> bool) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    for (start, ch) in text.char_indices() {
        if ch != '/'
            || text
                .get(..start)
                .and_then(|s| s.chars().next_back())
                .is_some_and(|c| !c.is_whitespace() && !"`'\"([{（【“‘".contains(c))
        {
            continue;
        }
        let Some(tail) = text.get(start + 1..) else {
            continue;
        };
        let name = tail
            .split(|c: char| c.is_whitespace() || "`'\"()[]{}（）【】“”‘’".contains(c))
            .next()
            .unwrap_or("")
            .trim_end_matches([
                ',', '.', ';', '!', '?', '，', '。', '；', '！', '？', '、', '：',
            ]);
        if !name.is_empty() && known(name) {
            ranges.push(start..start + 1 + name.len());
        }
    }
    ranges
}

pub(super) fn paint_scrollback(
    buf: &mut Buffer,
    model: &ResolvedSelectionModel,
    color: Color,
    dim_from: Option<usize>,
    known: impl Fn(&str) -> bool,
) {
    for range in &model.ranges {
        if dim_from.is_some_and(|start| range.entry_idx >= start) {
            continue;
        }
        let mut text = String::new();
        let mut starts = Vec::with_capacity(range.lines.len());
        // Rejoin soft wraps so a command split across terminal rows still has one identity.
        for (index, line) in range.lines.iter().enumerate() {
            if index > 0 {
                text.push_str(line.joiner_to_previous.as_deref().unwrap_or("\n"));
            }
            starts.push(text.len());
            text.push_str(line.painted_region.as_deref().unwrap_or(&line.text));
        }
        for token in token_ranges(&text, &known) {
            for (line, start) in range.lines.iter().zip(&starts) {
                let painted = line.painted_region.as_deref().unwrap_or(&line.text);
                let from = token.start.saturating_sub(*start).min(painted.len());
                let to = token.end.saturating_sub(*start).min(painted.len());
                if from >= to {
                    continue;
                }
                let (Some(prefix), Some(content)) = (painted.get(..from), painted.get(from..to))
                else {
                    continue;
                };
                let x = line.screen_x.saturating_add(line.selectable_cols.start);
                let left = x.saturating_add(prefix.width() as u16);
                let right = left
                    .saturating_add(content.width() as u16)
                    .min(line.screen_x.saturating_add(line.selectable_cols.end));
                for col in left..right {
                    if let Some(cell) = buf.cell_mut((col, line.screen_y)) {
                        cell.set_fg(color);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "highlight_tests.rs"]
mod tests;
