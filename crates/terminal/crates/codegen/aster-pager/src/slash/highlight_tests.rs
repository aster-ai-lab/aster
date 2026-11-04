use super::*;
use crate::acp::model_state::ModelState;
use crate::scrollback::render::ScratchBuffer;
use crate::scrollback::{RenderBlock, ScrollbackPane, ScrollbackState};
use crate::slash::{SlashController, SlashState};
use ratatui::layout::Rect;

#[test]
fn syntax_uses_the_live_catalog_without_arming_mentions() {
    let mut controller = SlashController::with_builtins(".".into());
    controller
        .registry_mut()
        .set_acp_commands(&[agent_client_protocol::AvailableCommand::new(
            "acme:review",
            "Review",
        )]);
    let state = SlashState::default();
    let text =
        "输入 `/model`，然后 (/acme:review). /exit https://a/model /model/file /model.md /unknown";
    controller.refresh(&state, text, text.len(), &ModelState::default());
    let snapshot = state.snapshot();
    let tokens: Vec<_> = snapshot
        .syntax_tokens
        .iter()
        .map(|r| text.get(r.clone()).unwrap())
        .collect();
    assert_eq!(tokens, ["/model", "/acme:review", "/exit"]);
    assert!(
        snapshot.recognized_tokens.is_empty(),
        "syntax must not change execution eligibility"
    );
    controller.registry_mut().set_acp_commands(&[]);
    controller.refresh(&state, text, text.len(), &ModelState::default());
    assert_eq!(
        state.snapshot().syntax_tokens.len(),
        2,
        "removed commands stop highlighting"
    );
}

#[test]
fn rendered_user_and_streaming_markdown_commands_keep_their_text_and_layout() {
    let mut controller = SlashController::with_builtins(".".into());
    controller
        .registry_mut()
        .set_acp_commands(&[agent_client_protocol::AvailableCommand::new(
            "acme:review",
            "Review",
        )]);
    for width in [18, 80] {
        let area = Rect::new(0, 0, width, 25);
        let mut state = ScrollbackState::new();
        state.push_block(RenderBlock::user_prompt("选择 /model 和 /acme:review"));
        let mut message = crate::scrollback::blocks::AgentMessageBlock::streaming();
        message.push_chunk("使用 `/mo");
        message.push_chunk("del`，再运行 /acme:review。\n路径 /model/file 不变。");
        state.push_block(RenderBlock::AgentMessage(message));
        state.prepare_layout(area.width, area.height);
        let mut buf = Buffer::empty(area);
        let output = ScrollbackPane::new().render_with_scratch(
            area,
            &mut buf,
            &state,
            &mut ScratchBuffer::new(),
        );
        let before = buf.clone();
        let model_before = output.selection_model.clone();
        // Other rendering tests change the process-wide theme; test the real painter with a fixed accent.
        let color = Color::Magenta;
        let known = |name: &str| controller.registry().get_for_dispatch(name).is_some();
        paint_scrollback(&mut buf, &output.selection_model, color, None, known);
        let mut highlighted = String::new();
        for (old, new) in before.content.iter().zip(&buf.content) {
            assert_eq!(old.symbol(), new.symbol());
            assert_eq!(old.bg, new.bg);
            assert_eq!(old.modifier, new.modifier);
            if old.fg != new.fg {
                assert_eq!(new.fg, color);
            }
            if new.fg == color {
                highlighted.push_str(new.symbol());
            }
        }
        assert_eq!(output.selection_model, model_before);
        assert!(
            highlighted.contains("/model"),
            "width={width}: {highlighted:?}"
        );
        assert!(
            highlighted.contains("/acme:review"),
            "width={width}: {highlighted:?}"
        );
        let mut dimmed = before.clone();
        paint_scrollback(&mut dimmed, &output.selection_model, color, Some(0), known);
        assert_eq!(dimmed, before, "rewind dimming must win over syntax");
    }
}
