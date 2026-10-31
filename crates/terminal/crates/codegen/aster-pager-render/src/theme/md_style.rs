//! Defines the `MarkdownStyle` used by agent message and thinking blocks.
//! Colors come from the `md_*` fields on the current [`Theme`], which are already quantized to the terminal's color capability level.

use aster_markdown::MarkdownStyle;
use anstyle::{Ansi256Color, AnsiColor, Color, Style};

/// Bridge after [`Theme::current()`] quantization. `None` for `Reset`: anstyle has no terminal-default, and unset renders as that default.
fn to_anstyle(c: ratatui::style::Color) -> Option<Color> {
    Some(match c {
        ratatui::style::Color::Reset => return None,
        ratatui::style::Color::Rgb(r, g, b) => Color::Rgb(anstyle::RgbColor(r, g, b)),
        ratatui::style::Color::Indexed(n) => Color::Ansi256(Ansi256Color(n)),
        // Named ANSI colors (from 16-color quantization).
        ratatui::style::Color::Black => Color::Ansi(AnsiColor::Black),
        ratatui::style::Color::Red => Color::Ansi(AnsiColor::Red),
        ratatui::style::Color::Green => Color::Ansi(AnsiColor::Green),
        ratatui::style::Color::Yellow => Color::Ansi(AnsiColor::Yellow),
        ratatui::style::Color::Blue => Color::Ansi(AnsiColor::Blue),
        ratatui::style::Color::Magenta => Color::Ansi(AnsiColor::Magenta),
        ratatui::style::Color::Cyan => Color::Ansi(AnsiColor::Cyan),
        ratatui::style::Color::Gray => Color::Ansi(AnsiColor::White),
        ratatui::style::Color::DarkGray => Color::Ansi(AnsiColor::BrightBlack),
        ratatui::style::Color::LightRed => Color::Ansi(AnsiColor::BrightRed),
        ratatui::style::Color::LightGreen => Color::Ansi(AnsiColor::BrightGreen),
        ratatui::style::Color::LightYellow => Color::Ansi(AnsiColor::BrightYellow),
        ratatui::style::Color::LightBlue => Color::Ansi(AnsiColor::BrightBlue),
        ratatui::style::Color::LightMagenta => Color::Ansi(AnsiColor::BrightMagenta),
        ratatui::style::Color::LightCyan => Color::Ansi(AnsiColor::BrightCyan),
        ratatui::style::Color::White => Color::Ansi(AnsiColor::BrightWhite),
    })
}

/// `anstyle::Style` with the given foreground color (converted from ratatui).
fn fg(c: ratatui::style::Color) -> Style {
    Style::new().fg_color(to_anstyle(c))
}

/// `anstyle::Style` with the given background color (converted from ratatui).
fn bg(c: ratatui::style::Color) -> Style {
    Style::new().bg_color(to_anstyle(c))
}

fn modifier_to_anstyle(m: ratatui::style::Modifier) -> Style {
    let mut s = Style::new();
    if m.contains(ratatui::style::Modifier::BOLD) {
        s = s.bold();
    }
    if m.contains(ratatui::style::Modifier::ITALIC) {
        s = s.italic();
    }
    if m.contains(ratatui::style::Modifier::UNDERLINED) {
        s = s.underline();
    }
    if m.contains(ratatui::style::Modifier::DIM) {
        s = s.dimmed();
    }
    if m.contains(ratatui::style::Modifier::HIDDEN) {
        s = s.hidden();
    }
    if m.contains(ratatui::style::Modifier::CROSSED_OUT) {
        s = s.strikethrough();
    }
    s
}

fn heading_inner_styles(
    colors: [ratatui::style::Color; 6],
    mods: [ratatui::style::Modifier; 6],
) -> [Style; 6] {
    std::array::from_fn(|i| {
        let Some(&color) = colors.get(i) else {
            return Style::new();
        };
        let Some(&m) = mods.get(i) else {
            return Style::new();
        };
        let color_style = fg(color);
        let mod_style = modifier_to_anstyle(m);
        // Combine fg color with modifier effects.
        let mut s = color_style;
        let effects = mod_style.get_effects();
        if !effects.is_plain() {
            s = s.effects(s.get_effects() | effects);
        }
        s
    })
}

/// Build heading outer styles (dimmed and hidden, for syntax markers).
fn heading_outer_styles(colors: [ratatui::style::Color; 6]) -> [Style; 6] {
    colors.map(|c| fg(c).dimmed().hidden())
}

/// The style is built fresh from [`Theme::current()`] on each call.
/// Both the theme construction and style mapping are trivial struct copies.
pub fn style() -> MarkdownStyle {
    build_style_for(&super::Theme::current())
}

fn build_style_for(theme: &super::Theme) -> MarkdownStyle {
    let heading_colors = [
        theme.md_heading_h1,
        theme.md_heading_h2,
        theme.md_heading_h3,
        theme.md_heading_h4,
        theme.md_heading_h5,
        theme.md_heading_h6,
    ];
    let heading_mods = [
        theme.md_heading_h1_mod,
        theme.md_heading_h2_mod,
        theme.md_heading_h3_mod,
        theme.md_heading_h4_mod,
        theme.md_heading_h5_mod,
        theme.md_heading_h6_mod,
    ];

    MarkdownStyle {
        heading_inner: heading_inner_styles(heading_colors, heading_mods),
        heading_outer: heading_outer_styles(heading_colors),
        strong_inner: fg(theme.md_strong).bold(),
        strong_outer: Style::new().dimmed().hidden(),
        emphasis_inner: fg(theme.md_strong).italic(),
        emphasis_outer: Style::new().dimmed().hidden(),
        strikethrough_inner: fg(theme.md_text).strikethrough(),
        strikethrough_outer: Style::new().dimmed().hidden(),
        inline_code_inner: fg(theme.md_code).bold(),
        inline_code_outer: fg(theme.md_code).dimmed().hidden(),
        // The selection-side bar detection in aster-pager (scrollback/blocks/quote_bar.rs, `quote_bar_style`) mirrors this exact style
        // Its end-to-end tests fail if this line changes
        blockquote_outer: fg(theme.md_quote).dimmed(),
        task_checked: fg(theme.md_task_checked),
        task_unchecked: fg(theme.md_task_unchecked).dimmed(),
        list_item: fg(theme.md_bullet),
        rule: fg(theme.md_muted),
        link_outer: fg(theme.md_muted),
        link_text: fg(theme.link_fg).underline(),
        link_url: fg(theme.md_muted),
        link_title: fg(theme.md_heading_h5),
        code_outer: fg(theme.md_code).dimmed().hidden(),
        code_language: fg(theme.md_heading_h3).hidden(),
        code_untagged: fg(theme.md_text),
        code_background: bg(theme.md_code_bg),
        table_outer: fg(theme.md_heading_h2).hidden(),
        text: fg(theme.md_text),
        math: fg(theme.md_text).italic(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anstyle::Effects;

    /// Regression: `Reset` used to fall back to a concrete `AnsiColor::White` (ANSI-7 silver).
    /// That rendered Reset-themed markdown washed-out gray on light terminals and broke the `NO_COLOR` opt-out.
    #[test]
    fn reset_maps_to_no_color() {
        assert_eq!(to_anstyle(ratatui::style::Color::Reset), None);
        assert_eq!(fg(ratatui::style::Color::Reset).get_fg_color(), None);
        assert_eq!(bg(ratatui::style::Color::Reset).get_bg_color(), None);
    }

    /// Spot checks around the Gray/DarkGray naming mismatch between ratatui and anstyle.
    #[test]
    fn named_colors_map_concretely() {
        assert_eq!(
            to_anstyle(ratatui::style::Color::DarkGray),
            Some(Color::Ansi(AnsiColor::BrightBlack))
        );
        assert_eq!(
            to_anstyle(ratatui::style::Color::Gray),
            Some(Color::Ansi(AnsiColor::White))
        );
        assert_eq!(
            to_anstyle(ratatui::style::Color::Red),
            Some(Color::Ansi(AnsiColor::Red))
        );
    }

    #[test]
    fn terminal_default_body_text_has_no_fg() {
        let theme = super::super::Theme::terminal_default();
        assert_eq!(fg(theme.md_text).get_fg_color(), None);
        assert_eq!(bg(theme.md_code_bg).get_bg_color(), None);
    }

    /// Codex transcript parity: `**strong**` and `*emphasis*` carry the emphasis hue, not the body foreground.
    #[test]
    fn emphasis_uses_the_emphasis_color() {
        let theme = super::super::Theme::groknight();
        let style = build_style_for(&theme);
        let expected = to_anstyle(theme.md_strong);
        assert_eq!(style.strong_inner.get_fg_color(), expected);
        assert_eq!(style.emphasis_inner.get_fg_color(), expected);
        assert_ne!(
            style.strong_inner.get_fg_color(),
            style.text.get_fg_color(),
            "bold must not render in the body color"
        );
        assert!(style.strong_inner.get_effects().contains(Effects::BOLD));
        assert!(style.emphasis_inner.get_effects().contains(Effects::ITALIC));
    }

    /// Per-level heading effects follow the Codex role table: H1 underline, H2 plain bold, H3 bold italic, H4+ italic.
    #[test]
    fn heading_effects_follow_the_role_table() {
        let theme = super::super::Theme::groknight();
        let style = build_style_for(&theme);
        let effects = |i: usize| style.heading_inner[i].get_effects();
        assert!(effects(0).contains(Effects::BOLD | Effects::UNDERLINE));
        assert!(effects(1).contains(Effects::BOLD));
        assert!(!effects(1).contains(Effects::ITALIC));
        assert!(effects(2).contains(Effects::BOLD | Effects::ITALIC));
        assert!(effects(3).contains(Effects::ITALIC));
        assert!(!effects(3).contains(Effects::BOLD));
    }

    /// Quote bars and list markers take their own theme slots, so changing them cannot recolor rules or link URLs.
    #[test]
    fn quote_and_bullet_have_dedicated_colors() {
        let theme = super::super::Theme::groknight();
        let style = build_style_for(&theme);
        assert_eq!(
            style.blockquote_outer.get_fg_color(),
            to_anstyle(theme.md_quote)
        );
        assert_eq!(style.list_item.get_fg_color(), to_anstyle(theme.md_bullet));
        assert_eq!(style.rule.get_fg_color(), to_anstyle(theme.md_muted));
    }
}
