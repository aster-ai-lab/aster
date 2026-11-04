use super::is_input_frame;

#[test]
fn welcome_readiness_uses_the_input_placeholder_instead_of_its_decoration() {
    for input in [
        "→ Type a message...",
        "❯ Type a message...",
        "Type a message...",
    ] {
        assert!(is_input_frame(&format!("Aster 0.1.6\n{input}\nmock/gpt")));
    }
}

#[test]
fn welcome_readiness_refuses_partial_and_error_frames() {
    for screen in [
        "",
        "Aster 0.1.6\nLoading...",
        "→ Type a message...",
        "Aster failed to launch: missing configuration",
        "Add a follow-up",
    ] {
        assert!(!is_input_frame(screen), "unexpected ready frame: {screen}");
    }
}

#[test]
fn a_resumed_session_uses_its_follow_up_composer_and_product_key_hints() {
    assert!(is_input_frame(
        "→ Add a follow-up\nmock/gpt\nShift+Tab:mode  │  Ctrl+.:shortcuts"
    ));
}
