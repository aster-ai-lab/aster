use super::*;

#[test]
fn prompt_info_distinguishes_missing_usage_and_zero() {
    let mut agent = super::tests::agent();
    let theme = Theme::current();
    let area = Rect::new(0, 0, 120, 1);
    for available in [false, true] {
        agent.context_state = Some(aster_shell::session::ContextInfo {
            total: 1_000_000,
            usage_available: Some(available),
            ..Default::default()
        });
        let mut buffer = Buffer::empty(area);
        render_prompt_info(&mut buffer, area, &agent, 0, "/transcript", &theme);
        let text: String = (0..area.width)
            .filter_map(|x| buffer.cell((x, 0)).map(|cell| cell.symbol()))
            .collect();
        if available {
            assert!(text.contains("0 / 1.0M (0%)"), "{text}");
        } else {
            assert!(text.contains("— / 1.0M"), "{text}");
            assert!(!text.contains('%'), "{text}");
        }
    }
}
