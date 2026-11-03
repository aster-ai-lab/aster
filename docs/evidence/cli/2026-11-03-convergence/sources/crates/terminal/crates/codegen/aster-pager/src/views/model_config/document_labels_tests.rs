use super::*;

#[test]
fn quoted_provider_labels_preserve_unicode_and_escaped_quotes() {
    assert_eq!(
        parse_first_quoted("providers \"模型🌏\" {"),
        Some("模型🌏".into())
    );
    assert_eq!(
        parse_first_quoted("providers \"模型\\\"机\" {"),
        Some("模型\"机".into())
    );
    assert_eq!(
        parse_first_quoted("providers '本地模型' {"),
        Some("本地模型".into())
    );
    assert!(parse_first_quoted("providers \"unterminated").is_none());
}
