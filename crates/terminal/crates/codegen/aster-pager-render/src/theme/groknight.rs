//! Aster Night: graphite surfaces and silver-white product chrome.
//!
//! The canonical palette is defined in RGB (`Color::Rgb`).
//! At startup [`Theme::quantized`] downgrades every color to the terminal's detected capability level (256-color, 16-color, etc.).

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// Keep neutral surfaces legible when terminals quantize the brand accents.
#[allow(dead_code)]
mod palette {
    use super::*;

    // ── Backgrounds ─────────────────────────────────────────────────────
    pub const BG: Color = rgb(10, 10, 10); //  #0a0a0a, Night (terminal bg)
    pub const BG_DARK: Color = rgb(12, 12, 12); //  #0c0c0c, darkest
    pub const BG_STORM_DARK: Color = rgb(17, 17, 17); //  #111111, dark bg
    pub const BG_STORM: Color = rgb(17, 17, 18); // #111112, Aster dark canvas
    pub const BG_HIGHLIGHT: Color = rgb(36, 36, 36); //  #242424, highlight bg

    // ── Text / grays ────────────────────────────────────────────────────
    pub const FG: Color = rgb(245, 245, 245); // #f5f5f5, dark Aster primary
    pub const FG_DARK: Color = rgb(200, 200, 200); // #c8c8c8, secondary text
    pub const FG_GUTTER: Color = rgb(65, 65, 65); //  #414141, dim
    pub const COMMENT: Color = rgb(108, 108, 108); //  #6c6c6c, muted
    pub const DARK3: Color = rgb(90, 90, 90); //  #5a5a5a, medium gray
    pub const DARK5: Color = rgb(120, 120, 120); // #787878, bright gray

    // ── Semantic status colors (not product chrome; chrome stays silver/graphite) ─
    pub const GREEN: Color = rgb(158, 206, 106); // #9ece6a
    pub const GREEN1: Color = rgb(115, 218, 202); // #73daca
    pub const BLUE: Color = rgb(122, 162, 247); // #7aa2f7, links and other markdown chrome that needs a cool hue
    pub const MAGENTA: Color = rgb(187, 154, 247); // #bb9af7
    pub const ORANGE: Color = rgb(255, 158, 100); // #ff9e64
    pub const PURPLE: Color = rgb(157, 124, 216); // #9d7cd8
    pub const RED: Color = rgb(247, 118, 142); // #f7768e
    pub const RED1: Color = rgb(219, 75, 75); // #db4b4b
    pub const TEAL: Color = rgb(26, 188, 156); // #1abc9c
    pub const YELLOW: Color = rgb(224, 175, 104); // #e0af68

    pub const RED_DARK: Color = rgb(66, 14, 20); // #420e14, quantizes to 256-color red, not gray
    pub const GREEN_DARK: Color = rgb(6, 56, 6); // #063806, quantizes to 256-color green, not gray
}
use palette::*;

impl Theme {
    pub const fn groknight() -> Self {
        Self {
            bg_base: BG_STORM,
            bg_light: BG_HIGHLIGHT,
            bg_dark: rgb(28, 28, 28), // lighter than bg_base for visible code blocks
            bg_highlight: BG_HIGHLIGHT,
            bg_hover: rgb(44, 44, 44),
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

            gray_dim: rgb(88, 88, 88), // #585858, slightly brighter than FG_GUTTER
            gray: COMMENT,
            gray_bright: DARK5,

            command: FG_DARK,
            path: FG_DARK,
            running: FG_DARK,
            warning: YELLOW,

            fuzzy_accent: FG,

            accent_plan: rgb(255, 219, 141), // #FFDB8D, golden

            accent_verify: rgb(187, 154, 247), // #bb9af7, violet

            accent_remember: Color::Rgb(139, 195, 74), // #8BC34A, Material Design light green

            selection_border: rgb(60, 60, 65),
            prompt_border: rgb(50, 50, 55), // #323237, dimmer prompt chrome
            prompt_border_active: FG,
            hover_border: rgb(30, 30, 34),

            accent_model: FG_DARK,

            scrollbar_bg: BG_STORM_DARK,
            scrollbar_fg: BG_HIGHLIGHT,

            diff_delete_bg: RED_DARK,
            diff_delete_fg: RED,
            diff_insert_bg: GREEN_DARK,
            diff_insert_fg: GREEN,
            diff_equal_fg: COMMENT,
            diff_gutter_fg: COMMENT,

            bg_visual: rgb(54, 54, 54),

            paste_bg: BG_STORM_DARK,
            paste_fg: FG_DARK,
            paste_dim: FG_GUTTER,

            // Markdown content follows the Codex transcript roles (openai/codex `MarkdownStyles::for_theme`):
            // one hue per heading level, chromatic emphasis, green inline code, blue links, tinted quote bar and
            // list markers. Product chrome above stays silver/graphite; only reply content is chromatic.
            md_heading_h1: RED,
            md_heading_h1_mod: Modifier::BOLD.union(Modifier::UNDERLINED),
            md_heading_h2: ORANGE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: YELLOW,
            md_heading_h3_mod: Modifier::BOLD.union(Modifier::ITALIC),
            md_heading_h4: GREEN,
            md_heading_h4_mod: Modifier::ITALIC,
            md_heading_h5: GREEN1,
            md_heading_h5_mod: Modifier::ITALIC,
            md_heading_h6: MAGENTA,
            md_heading_h6_mod: Modifier::ITALIC,
            md_code: GREEN,
            md_task_checked: GREEN,
            md_task_unchecked: FG_DARK, // text_secondary
            md_muted: COMMENT,
            md_code_bg: rgb(28, 28, 28),
            md_text: FG_DARK,
            md_strong: RED,
            md_quote: MAGENTA,
            md_bullet: GREEN1,
            link_fg: BLUE,
        }
    }
}
