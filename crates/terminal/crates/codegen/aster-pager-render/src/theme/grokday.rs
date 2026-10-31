//! Aster Day: graphite product chrome on neutral light surfaces.

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

#[allow(dead_code)]
mod palette {
    use super::*;

    // ── Backgrounds (neutral light grays) ────────────────────────────────
    pub const BG: Color = rgb(255, 255, 255); // #FFFFFF, content white
    pub const BG_DARK: Color = rgb(240, 240, 240); // #f0f0f0
    pub const BG_STORM_DARK: Color = rgb(234, 234, 234); // #eaeaea
    pub const BG_STORM: Color = rgb(246, 246, 246); // #f6f6f6, Aster light canvas
    pub const BG_HIGHLIGHT: Color = rgb(242, 243, 245); // #f2f3f5, selected row

    // ── Text / grays (neutral dark) ──────────────────────────────────────
    pub const FG: Color = rgb(23, 23, 23); // #171717, light Aster primary
    pub const FG_DARK: Color = rgb(63, 63, 63); // #3f3f3f, body text
    pub const FG_GUTTER: Color = rgb(178, 178, 178); // #b2b2b2 — dim
    pub const COMMENT: Color = rgb(118, 118, 118); // #767676 — muted
    pub const DARK3: Color = rgb(142, 142, 142); // #8e8e8e — medium gray
    pub const DARK5: Color = rgb(98, 98, 98); // #626262 — bright gray

    // ── Semantic status colors (not product chrome; chrome stays graphite) ─
    pub const GREEN: Color = rgb(55, 142, 35); // #378E23
    pub const GREEN1: Color = rgb(12, 148, 124); // #0C947C
    pub const MAGENTA: Color = rgb(125, 75, 198); // #7D4BC6
    pub const ORANGE: Color = rgb(195, 105, 30); // #C3691E
    pub const PURPLE: Color = rgb(108, 62, 178); // #6C3EB2
    pub const RED: Color = rgb(205, 48, 72); // #CD3048
    pub const RED1: Color = rgb(175, 35, 35); // #AF2323
    pub const TEAL: Color = rgb(10, 142, 112); // #0A8E70
    pub const YELLOW: Color = rgb(162, 118, 18); // #A27612

    pub const RED_LIGHT: Color = rgb(245, 218, 222); // #F5DADE — diff delete bg
    pub const GREEN_LIGHT: Color = rgb(218, 242, 220); // #DAF2DC — diff insert bg
}
use palette::*;

impl Theme {
    pub const fn grokday() -> Self {
        Self {
            bg_base: BG_STORM,
            bg_light: BG_HIGHLIGHT,
            bg_dark: rgb(228, 228, 228),
            bg_highlight: BG_HIGHLIGHT,
            bg_hover: rgb(208, 208, 208),
            bg_terminal: BG,

            accent_user: FG,
            accent_assistant: FG,
            accent_thinking: FG_DARK,
            accent_tool: DARK5,
            accent_system: FG,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: FG,
            accent_skill: FG,

            text_primary: FG,
            text_secondary: FG_DARK,

            gray_dim: rgb(165, 165, 165), // #a5a5a5 — slightly darker than FG_GUTTER
            gray: COMMENT,
            gray_bright: DARK5,

            command: FG_DARK,
            path: FG_DARK,
            running: FG_DARK,
            warning: YELLOW,

            fuzzy_accent: FG,

            accent_plan: rgb(168, 120, 10), // #A8780A — deep golden

            accent_verify: rgb(120, 80, 160), // deep violet (readable on light bg)

            accent_remember: rgb(76, 175, 80), // #4CAF50 — Material Design green (readable on light bg)

            selection_border: rgb(185, 185, 190),
            prompt_border: rgb(200, 200, 205), // #C8C8CD — dimmer prompt chrome
            prompt_border_active: FG,
            hover_border: rgb(212, 212, 216),

            accent_model: FG_DARK,

            scrollbar_bg: BG_STORM_DARK,
            scrollbar_fg: FG_DARK,

            diff_delete_bg: RED_LIGHT,
            diff_delete_fg: RED,
            diff_insert_bg: GREEN_LIGHT,
            diff_insert_fg: GREEN,
            diff_equal_fg: COMMENT,
            diff_gutter_fg: COMMENT,

            bg_visual: rgb(198, 198, 198),

            paste_bg: BG_HIGHLIGHT,
            paste_fg: FG_DARK,
            paste_dim: FG_GUTTER,

            // Aster Day keeps the neutral ladder: its light accent ramp is tuned for chrome and does not clear
            // the 4.5:1 body-text contrast gate, so reply text stays graphite here.
            md_heading_h1: FG,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: FG,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: FG,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: DARK5,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: COMMENT,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: DARK3,
            md_heading_h6_mod: Modifier::empty(),
            md_code: FG_DARK,
            md_task_checked: GREEN,
            md_task_unchecked: FG_DARK,
            md_muted: COMMENT,
            md_code_bg: rgb(228, 228, 228),
            md_text: FG_DARK,
            md_strong: FG_DARK,
            md_quote: COMMENT,
            md_bullet: COMMENT,
            link_fg: FG_DARK,
        }
    }
}
