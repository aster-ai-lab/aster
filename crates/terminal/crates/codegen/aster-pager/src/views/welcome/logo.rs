//! Aster star, tilted orbital ring and comet trails in a bounded frame cache.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::render::color::blend_color;
use crate::theme::Theme;

const LOGO: &str = include_str!("../../../assets/logo/aster.txt");
const LOGO_SMALL: &str = include_str!("../../../assets/logo/aster-small.txt");

/// Height at or above which the small logo is shown (below it, no logo).
const SMALL_LOGO_MIN_HEIGHT: u16 = 20;
/// Height at or above which the full logo is shown.
const FULL_LOGO_MIN_HEIGHT: u16 = 26;

/// Which logo art the stacked column shows.
/// The terminal height picks the tier; the stacked layout steps it down only while the column would not fit beside the draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoTier {
    Full,
    Compact,
    Hidden,
}

impl LogoTier {
    pub fn for_height(window_height: u16) -> Self {
        Self::for_height_and_hidden(window_height, logo_hidden())
    }

    /// Takes an explicit visibility flag so layout tests can drive it directly.
    fn for_height_and_hidden(window_height: u16, hidden: bool) -> Self {
        if hidden || window_height < SMALL_LOGO_MIN_HEIGHT {
            Self::Hidden
        } else if window_height < FULL_LOGO_MIN_HEIGHT {
            Self::Compact
        } else {
            Self::Full
        }
    }

    fn art(self) -> Option<&'static str> {
        match self {
            Self::Full => Some(LOGO),
            Self::Compact => Some(LOGO_SMALL),
            Self::Hidden => None,
        }
    }

    pub fn rows(self) -> u16 {
        self.art().map_or(0, count_lines)
    }

    /// The next smaller tier; `None` once hidden.
    pub fn step_down(self) -> Option<Self> {
        match self {
            Self::Full => Some(Self::Compact),
            Self::Compact => Some(Self::Hidden),
            Self::Hidden => None,
        }
    }
}

fn pick_logo(window_height: u16) -> Option<&'static str> {
    pick_logo_for(window_height, logo_hidden())
}

fn pick_logo_for(window_height: u16, hidden: bool) -> Option<&'static str> {
    LogoTier::for_height_and_hidden(window_height, hidden).art()
}

/// ASCII glyphs remain readable on legacy consoles.
fn logo_hidden() -> bool {
    false
}

fn non_empty_lines(logo: &str) -> impl Iterator<Item = &str> {
    logo.lines().filter(|l| !l.is_empty())
}

fn count_lines(logo: &str) -> u16 {
    non_empty_lines(logo).count() as u16
}

fn visual_width(logo: &str) -> u16 {
    non_empty_lines(logo)
        .map(unicode_width::UnicodeWidthStr::width)
        .max()
        .unwrap_or(24) as u16
}

/// Animation phase in seconds since the first render.
/// The phase is wall-clock based so the shimmer speed is independent of the frame rate.
fn anim_phase_secs() -> f32 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Shimmer redraw cadence in frames per second.
/// The sweep is slow, so a few fps looks smooth while sparing the long-lived welcome screen from full-rate repaints.
const SHIMMER_FPS: f32 = 12.0;

/// Quantized shimmer frame for the current wall-clock phase.
/// The welcome screen redraws only when this advances, throttling the animation to ~`SHIMMER_FPS` rather than the full event-loop tick rate.
/// The frame is pinned to 0 when the logo is hidden.
pub fn shimmer_frame() -> u64 {
    if logo_hidden() {
        return 0;
    }
    (anim_phase_secs() * SHIMMER_FPS) as u64
}

/// Per-glyph shine opacity in `[0, 1]` at normalized diagonal position `diag` (0 is bottom-left, 1 is top-right) and animation time `secs`.
/// A raised-cosine band sweeps from bottom-left to top-right and parks off-screen between sweeps; a gentle global pulse breathes underneath it.
/// 0 keeps the resting gray, 1 is full bright.
fn shine_opacity(diag: f32, secs: f32) -> f32 {
    const BAND: f32 = 0.38; // half-width of the shine band; wider means a more gradual falloff
    const CYCLE: f32 = 4.0; // seconds for one sweep plus its rest
    const SWEEP_FRAC: f32 = 0.32; // portion of the cycle spent sweeping (~1.3s glint, rest idles)
    const SHINE: f32 = 0.33; // peak shine strength
    const PULSE: f32 = 0.06; // global breathing amount
    const PULSE_SECS: f32 = 4.0; // breathing period

    let p = (secs % CYCLE) / CYCLE;
    let q = (p / SWEEP_FRAC).min(1.0); // parks the band off-screen during the rest
    let band_pos = -BAND + q * (1.0 + 2.0 * BAND);
    let pulse = PULSE * (0.5 - 0.5 * (std::f32::consts::TAU * secs / PULSE_SECS).cos());

    let d = (diag - band_pos).abs();
    let shine = if d < BAND {
        0.5 * (1.0 + (std::f32::consts::PI * d / BAND).cos())
    } else {
        0.0
    };
    (pulse + SHINE * shine).clamp(0.0, 1.0)
}

const FRAME_COUNT: usize = 96;

type LogoFrame = Vec<(u16, u16, char, f32)>;

fn cached_frames(logo: &'static str) -> &'static [LogoFrame] {
    use std::sync::OnceLock;
    static FULL: OnceLock<Vec<LogoFrame>> = OnceLock::new();
    static COMPACT: OnceLock<Vec<LogoFrame>> = OnceLock::new();
    let cache = if logo == LOGO { &FULL } else { &COMPACT };
    cache.get_or_init(|| {
        let rows = count_lines(logo) as usize;
        let cols = visual_width(logo) as usize;
        (0..FRAME_COUNT)
            .map(|frame| {
                let phase = frame as f32 / FRAME_COUNT as f32 * std::f32::consts::TAU;
                let mut cells = vec![None; rows * cols];
                let center_x = (cols - 1) as f32 / 2.0;
                let center_y = (rows - 1) as f32 / 2.0;
                let radius = center_x - 1.0;
                let tilt = -0.19 + 0.10 * phase.sin();
                let depth = center_y * (0.35 + 0.12 * phase.cos());
                for layer in [false, true] {
                    for step in 0..192 {
                        let angle = step as f32 / 192.0 * std::f32::consts::TAU;
                        if (angle.sin() > 0.0) != layer {
                            continue;
                        }
                        let x = radius * angle.cos();
                        let y = depth * angle.sin() + tilt * x;
                        let col = (center_x + x).round() as usize;
                        let row = (center_y + y).round().clamp(0.0, (rows - 1) as f32) as usize;
                        let trail = (1.0 - (phase - angle).rem_euclid(std::f32::consts::TAU) / 0.9)
                            .max(0.0);
                        let opacity = 0.10 + 0.85 * trail.powi(2);
                        let ch = if trail > 0.9 {
                            'o'
                        } else if trail > 0.55 {
                            '*'
                        } else if layer {
                            ':'
                        } else {
                            '.'
                        };
                        let cell = &mut cells[row * cols + col];
                        if layer
                            && trail < 0.65
                            && cell.is_some_and(|(ch, _)| matches!(ch, '+' | '*' | '#' | '@'))
                        {
                            continue;
                        }
                        if cell.is_none_or(|(_, previous)| opacity > previous) {
                            *cell = Some((ch, opacity));
                        }
                    }
                    if !layer {
                        for (row, line) in non_empty_lines(logo).enumerate() {
                            for (col, ch) in line.chars().enumerate() {
                                if !matches!(ch, '+' | '*' | '#') {
                                    continue;
                                }
                                let diag = (col + rows - 1 - row) as f32 / (cols + rows) as f32;
                                let opacity = shine_opacity(diag, frame as f32 / SHIMMER_FPS);
                                let ch = match ch {
                                    '#' if opacity > 0.25 => '@',
                                    '*' if opacity > 0.25 => '#',
                                    other => other,
                                };
                                cells[row * cols + col] = Some((ch, opacity));
                            }
                        }
                    }
                }
                cells
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, cell)| {
                        cell.map(|(ch, opacity)| {
                            ((index % cols) as u16, (index / cols) as u16, ch, opacity)
                        })
                    })
                    .collect()
            })
            .collect()
    })
}

fn render_into(area: Rect, buf: &mut Buffer, theme: &Theme, logo: &'static str) {
    let frames = cached_frames(logo);
    let inset = area.width.saturating_sub(visual_width(logo)) / 2;
    let frame = (shimmer_frame() % FRAME_COUNT as u64) as usize;
    let Some(glyphs) = frames.get(frame) else {
        return;
    };
    for &(col, row, ch, opacity) in glyphs {
        let col = col + inset;
        if col >= area.width || row >= area.height {
            continue;
        }
        let base = if matches!(ch, '@' | '#' | '*' | '+') {
            theme.accent_system
        } else {
            theme.text_secondary
        };
        let color =
            crate::theme::quantize(blend_color(base, theme.text_primary, opacity).unwrap_or(base));
        if let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) {
            cell.set_char(ch).set_fg(color);
        }
    }
}

pub fn logo_line_count(window_height: u16) -> u16 {
    pick_logo(window_height).map_or(0, count_lines)
}

pub fn logo_visual_width(window_height: u16) -> u16 {
    pick_logo(window_height).map_or(24, visual_width)
}

pub fn render_logo(area: Rect, buf: &mut Buffer, theme: &Theme, window_height: u16) {
    if let Some(logo) = pick_logo(window_height) {
        render_into(area, buf, theme, logo);
    }
}

/// Paint the tier the layout reserved rows for, so the art can never outgrow its slot.
pub fn render_logo_tier(area: Rect, buf: &mut Buffer, theme: &Theme, tier: LogoTier) {
    if let Some(logo) = tier.art() {
        render_into(area, buf, theme, logo);
    }
}

/// The hero box always shows the full logo: it is laid out beside the menu, so it fits whenever the box does.
/// These report and render that logo directly, independent of the height-based [`pick_logo`] tiers used by the stacked layout.
/// When [`logo_hidden`], they report 0 and render nothing.
pub fn full_logo_line_count() -> u16 {
    full_logo_line_count_for(logo_hidden())
}

fn full_logo_line_count_for(hidden: bool) -> u16 {
    if hidden { 0 } else { count_lines(LOGO) }
}

pub fn full_logo_visual_width() -> u16 {
    full_logo_visual_width_for(logo_hidden())
}

fn full_logo_visual_width_for(hidden: bool) -> u16 {
    if hidden { 0 } else { visual_width(LOGO) }
}

pub fn render_full_logo(area: Rect, buf: &mut Buffer, theme: &Theme) {
    if !logo_hidden() {
        render_into(area, buf, theme, LOGO);
    }
}

/// Line count of the small ASCII logo used in minimal's committed welcome card.
pub fn compact_logo_line_count() -> u16 {
    if logo_hidden() {
        0
    } else {
        count_lines(LOGO_SMALL)
    }
}

/// Render the small ASCII logo (centered) into `area` for minimal's welcome card.
/// No-op when the logo is hidden.
pub fn render_compact_logo(area: Rect, buf: &mut Buffer, theme: &Theme) {
    if !logo_hidden() {
        render_into(area, buf, theme, LOGO_SMALL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbital_frames_are_cached_bounded_ascii_with_moving_geometry() {
        for logo in [LOGO, LOGO_SMALL] {
            assert!(logo.is_ascii());
            let frames = cached_frames(logo);
            let stars = non_empty_lines(logo)
                .enumerate()
                .flat_map(|(y, line)| {
                    line.chars().enumerate().filter_map(move |(x, ch)| {
                        matches!(ch, '+' | '*' | '#').then_some((x as u16, y as u16))
                    })
                })
                .collect::<Vec<_>>();
            assert!(std::ptr::eq(frames, cached_frames(logo)));
            let bytes = frames
                .iter()
                .map(|frame| frame.len() * std::mem::size_of::<(u16, u16, char, f32)>())
                .sum::<usize>();
            assert!(bytes <= 512 * 1024, "a logo cache must stay below 512 KiB");
            let shape =
                |frame: &LogoFrame| frame.iter().map(|&(x, y, _, _)| (x, y)).collect::<Vec<_>>();
            for frame in frames {
                let visible_stars = stars
                    .iter()
                    .filter(|&&(x, y)| {
                        frame.iter().any(|&(col, row, ch, _)| {
                            col == x && row == y && matches!(ch, '+' | '*' | '#' | '@')
                        })
                    })
                    .count();
                assert!(
                    visible_stars * 100 >= stars.len() * 85,
                    "orbit must not obscure the star silhouette"
                );
                assert!(
                    frame.len() <= usize::from(visual_width(logo)) * usize::from(count_lines(logo))
                );
                assert!(
                    frame
                        .iter()
                        .all(|&(x, y, ch, opacity)| x < visual_width(logo)
                            && y < count_lines(logo)
                            && ch.is_ascii()
                            && (0.0..=1.0).contains(&opacity))
                );
            }
            assert!(frames.iter().any(|frame| shape(frame) != shape(&frames[0])));
        }
    }

    #[test]
    fn logo_is_centered_in_a_wide_slot() {
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);
        render_into(area, &mut buf, &Theme::default(), LOGO_SMALL);
        let inset = (80 - visual_width(LOGO_SMALL)) / 2;
        assert!(buf.content.iter().any(|cell| cell.symbol() != " "));
        assert!(
            (inset + visual_width(LOGO_SMALL)..80)
                .all(|x| (0..10).all(|y| buf[(x, y)].symbol() == " "))
        );
        assert!((0..inset).all(|x| (0..10).all(|y| buf[(x, y)].symbol() == " ")));
    }

    #[test]
    fn logo_rendering_clips_to_the_reserved_area() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 20));
        render_into(Rect::new(2, 3, 18, 5), &mut buf, &Theme::default(), LOGO);
        assert!(buf.content.iter().any(|cell| cell.symbol() != " "));
        for y in 0..20 {
            for x in 0..40 {
                if !(2..20).contains(&x) || !(3..8).contains(&y) {
                    assert_eq!(buf[(x, y)].symbol(), " ");
                }
            }
        }
    }

    #[test]
    fn logo_sizes_by_height() {
        assert!(pick_logo_for(SMALL_LOGO_MIN_HEIGHT - 1, false).is_none());
        assert_eq!(
            pick_logo_for(SMALL_LOGO_MIN_HEIGHT, false),
            Some(LOGO_SMALL)
        );
        assert_eq!(
            pick_logo_for(FULL_LOGO_MIN_HEIGHT - 1, false),
            Some(LOGO_SMALL)
        );
        assert_eq!(pick_logo_for(FULL_LOGO_MIN_HEIGHT, false), Some(LOGO));
    }

    // Explicitly hidden layout tiers must reserve no logo rows.
    #[test]
    fn explicitly_hidden_logo_reserves_no_rows_at_every_height() {
        for h in [0, SMALL_LOGO_MIN_HEIGHT, FULL_LOGO_MIN_HEIGHT, u16::MAX] {
            assert!(pick_logo_for(h, true).is_none(), "height {h}");
        }
    }

    #[test]
    fn hero_box_always_uses_full_logo() {
        // The box renders the full logo regardless of height (it's laid out beside the menu), and it's the large variant, never the small one
        assert_eq!(full_logo_line_count_for(false), count_lines(LOGO));
        assert_eq!(full_logo_visual_width_for(false), visual_width(LOGO));
        assert!(full_logo_line_count_for(false) > count_lines(LOGO_SMALL));
        assert!(full_logo_visual_width_for(false) > visual_width(LOGO_SMALL));
    }

    #[test]
    fn full_logo_helpers_collapse_when_hidden() {
        assert_eq!(full_logo_line_count_for(true), 0);
        assert_eq!(full_logo_visual_width_for(true), 0);
    }

    #[test]
    fn shine_opacity_stays_in_unit_range() {
        let mut secs = 0.0;
        while secs < 10.0 {
            for i in 0..=20 {
                let diag = i as f32 / 20.0;
                let op = shine_opacity(diag, secs);
                assert!(
                    (0.0..=1.0).contains(&op),
                    "opacity {op} out of range at diag {diag}, secs {secs}"
                );
            }
            secs += 0.13;
        }
    }

    #[test]
    fn shine_band_sweeps_across() {
        // The brightest point along the diagonal advances from left to right as the sweep progresses through its active phase
        let brightest = |secs: f32| -> f32 {
            (0..=100)
                .map(|i| i as f32 / 100.0)
                .max_by(|a, b| {
                    shine_opacity(*a, secs)
                        .partial_cmp(&shine_opacity(*b, secs))
                        .unwrap()
                })
                .unwrap()
        };
        let early = brightest(0.1);
        let mid = brightest(0.4);
        let late = brightest(0.7);
        assert!(early < mid, "early {early} should precede mid {mid}");
        assert!(mid < late, "mid {mid} should precede late {late}");
    }

    #[test]
    fn shine_rests_dim_between_sweeps() {
        // During the rest phase the band is parked off-screen, so an interior glyph falls back to at most the gentle pulse, never full bright
        let op = shine_opacity(0.5, 6.0); // secs % 4.0 = 2.0, past SWEEP_FRAC, in the rest phase
        assert!(op < 0.2, "resting opacity {op} should stay dim");
    }
}
