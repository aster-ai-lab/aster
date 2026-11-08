use super::*;

#[derive(Deserialize)]
struct Golden {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    messages: Vec<TextMessage>,
    mode: String,
    effort: u8,
    prompt: String,
}

#[test]
fn text_rendering_matches_twenty_four_independent_official_v41_encoder_cases() {
    let golden: Golden =
        serde_json::from_str(include_str!("../../fixtures/xq-text-prompts-golden.json")).unwrap();
    assert_eq!(golden.cases.len(), 24);
    for case in golden.cases {
        let mode = ThinkingMode::parse(&case.mode).unwrap();
        assert_eq!(mode.as_str(), case.mode);
        assert_eq!(
            render_text(&case.messages, mode, case.effort).unwrap(),
            case.prompt
        );
    }
}

#[test]
fn unsupported_roles_images_unknown_tokenizers_and_input_limits_fail_closed() {
    for role in ["system", "user", "assistant"] {
        assert_eq!(TextRole::parse(role).unwrap().as_str(), role);
    }
    assert!(TextRole::parse("tool").is_err());
    assert!(ThinkingMode::parse("auto").is_err());
    let user = |content: &str| TextMessage {
        role: "user".into(),
        content: content.into(),
    };
    assert!(render_text(&[], ThinkingMode::Chat, 75).is_err());
    assert!(render_text(&[user("hello")], ThinkingMode::Chat, 0).is_err());
    assert!(render_text(&[user("<｜deepseek_image｜>")], ThinkingMode::Chat, 75).is_err());
    assert!(render_text(&[user(&"x".repeat(65537))], ThinkingMode::Chat, 75).is_err());
    assert!(render_text(&vec![user(""); 129], ThinkingMode::Chat, 75).is_err());
    assert!(XqTextCodec::from_official_bytes(b"{}").is_err());
}
