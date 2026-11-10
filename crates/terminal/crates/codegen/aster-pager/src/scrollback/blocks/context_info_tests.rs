use super::tests::{all_text, test_theme};
use super::*;

#[test]
fn hides_unreported_breakdown() {
    let mut snapshot = ContextInfo::from_notification(17_000, 1_048_576);
    snapshot.turn_count = 2;
    let block = ContextInfoBlock::new(snapshot, "fixture/model");
    let lines = block.build_lines(&test_theme(), BarLayout::WIDE);
    let text = all_text(&lines);
    assert!(text.contains("Reported input"), "{text}");
    assert!(text.contains("17.0k"), "{text}");
    assert!(text.contains("Turns: 2"), "{text}");
    for unreported in [
        "System prompt",
        "Messages",
        "Reasoning/overhead",
        "Tool definitions",
        "Auto-compact",
        "Tool calls:",
        "Compactions:",
    ] {
        assert!(!text.contains(unreported), "{unreported}: {text}");
    }
}

#[test]
fn missing_usage_is_not_free_context() {
    let snapshot = ContextInfo {
        total: 1_048_576,
        usage_available: Some(false),
        ..Default::default()
    };
    let block = ContextInfoBlock::new(snapshot, "fixture/model");
    let text = all_text(&block.build_lines(&test_theme(), BarLayout::WIDE));
    assert!(text.contains("Usage not reported"), "{text}");
    assert!(text.contains("1.0m token window"), "{text}");
    assert!(!text.contains("Free"), "{text}");
    assert!(!text.contains("0 /"), "{text}");
    assert!(!text.contains('%'), "{text}");
}

#[test]
fn reported_zero_remains_a_measurement() {
    let snapshot = ContextInfo::from_notification(0, 1_048_576);
    let block = ContextInfoBlock::new(snapshot, "fixture/model");
    let text = all_text(&block.build_lines(&test_theme(), BarLayout::WIDE));
    assert!(text.contains("Reported input"), "{text}");
    assert!(text.contains("Free"), "{text}");
    assert!(!text.contains("Usage not reported"), "{text}");
}
