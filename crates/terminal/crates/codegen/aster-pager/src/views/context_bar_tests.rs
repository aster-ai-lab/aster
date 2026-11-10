use super::*;

#[test]
fn missing_usage_preserves_the_known_limit() {
    let theme = Theme::default();
    for hovered in [false, true] {
        let line = context_bar_line(None, Some(1_000_000), hovered, &theme).unwrap();
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(text, "— / 1.0M");
    }
}
