//! End-to-end frame checks for the Cursor-like borderless follow-up composer.
use super::{AgentView, AppRenderParams, BannerSlotParams, test_fixtures};
use crate::actions::ActionRegistry;
use crate::scrollback::render::ScratchBuffer;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn draw(agent: &mut AgentView, width: u16, height: u16) -> Buffer {
    agent.last_terminal_size = (width, height);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let mut scratch = ScratchBuffer::new();
    let registry = ActionRegistry::defaults();
    agent.draw(
        area,
        &mut buf,
        &registry,
        &mut scratch,
        None,
        false,
        BannerSlotParams::none(),
        false,
        &mut Vec::new(),
        AppRenderParams::default(),
    );
    buf
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect()
}

#[test]
fn followup_composer_pads_arrow_inside_fill_with_status_on_page() {
    let _guard = crate::theme::cache::pin_theme();
    let theme = Theme::current();
    let mut agent = test_fixtures::make_agent();
    // Session-bound so the follow-up composer (not welcome) paints.
    agent.session.session_id = Some(agent_client_protocol::SessionId::new("composer-layout"));
    agent.session.models.current =
        Some(agent_client_protocol::ModelId::new("kimi/K3.1 Pro"));
    agent.session.yolo_mode = true;

    let buf = draw(&mut agent, 100, 36);
    let ta = agent.prompt.textarea_area();
    assert!(
        ta.width > 0 && ta.height >= 1,
        "composer textarea must be laid out, got {ta:?}"
    );

    let draft_y = ta.y;
    let draft = row_text(&buf, draft_y);
    let lead = crate::glyphs::composer_lead_arrow()
        .chars()
        .next()
        .unwrap_or('\u{2192}');
    assert!(
        draft.contains("Add a follow-up"),
        "placeholder on the elevated draft row, got {draft:?} y={draft_y}"
    );
    assert!(
        draft.contains(lead),
        "Cursor-like → lead arrow must be present on the left, got {draft:?}"
    );
    assert!(
        !draft.contains('❯'),
        "left ❯ must not appear on the Cursor-like bar, got {draft:?}"
    );
    assert!(
        !draft.contains('\u{2191}'),
        "right ↑ submit chip must not appear, got {draft:?}"
    );
    let lead_at = draft.find(lead).expect("lead");
    let ph_at = draft.find("Add a follow-up").expect("placeholder");
    assert!(
        lead_at < ph_at,
        "→ must sit to the left of the placeholder, got {draft:?}"
    );
    assert_eq!(
        buf.cell((ta.x, draft_y)).map(|c| c.bg),
        Some(theme.bg_light),
        "draft row must be elevated fill"
    );
    assert!(
        draft_y > 0,
        "draft must sit below an inner top pad row"
    );
    let top_pad = buf.cell((ta.x, draft_y - 1)).expect("top pad");
    if crate::glyphs::upper_half_block().is_some() {
        assert_eq!(top_pad.symbol(), "\u{2580}", "top pad is a half-block halo");
        assert_eq!(top_pad.bg, theme.bg_light);
    } else {
        assert_eq!(top_pad.bg, theme.bg_light, "legacy full-cell top pad");
    }
    let bottom_pad_y = draft_y + ta.height;
    let bottom_pad = buf.cell((ta.x, bottom_pad_y)).expect("bottom pad");
    if crate::glyphs::upper_half_block().is_some() {
        assert_eq!(
            bottom_pad.symbol(),
            "\u{2580}",
            "bottom pad is a half-block halo"
        );
        assert_eq!(bottom_pad.fg, theme.bg_light);
        assert_eq!(bottom_pad.bg, theme.bg_base);
    } else {
        assert_eq!(bottom_pad.bg, theme.bg_light, "legacy full-cell bottom pad");
    }
    let status_y = bottom_pad_y + 1;
    let status = row_text(&buf, status_y);
    assert!(
        status.contains("kimi") || status.contains("K3.1") || status.contains("yolo"),
        "model/mode status under the bar, got {status:?} y={status_y}"
    );
    assert_eq!(
        buf.cell((ta.x, status_y)).map(|c| c.bg),
        Some(theme.bg_base),
        "status must sit on the page canvas immediately under the fill"
    );
    assert!(
        !status.contains('\u{2500}')
            && !status.contains('\u{256d}')
            && !status.contains('\u{2570}'),
        "no box border glyphs around the Cursor-like composer, got {status:?}"
    );
}
