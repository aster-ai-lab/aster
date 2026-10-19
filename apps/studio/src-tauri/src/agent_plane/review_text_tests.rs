#![cfg(test)]

use super::*;

#[test]
fn text_input_is_exact_bounded_and_separate_from_instructions() {
    let input: ReviewTextSource =
        serde_json::from_value(serde_json::json!({"kind":"text","text":"  已完成。\n"})).unwrap();
    assert_eq!(input.text().unwrap(), "  已完成。\n");
    assert_ne!(
        input.revision().unwrap(),
        ReviewTextSource::Text {
            text: "已完成。".into()
        }
        .revision()
        .unwrap()
    );
    for text in [
        "".to_owned(),
        " \n".into(),
        "a\0b".into(),
        "中".repeat(256 * 1024 / 3 + 1),
    ] {
        assert!(ReviewTextSource::Text { text }.text().is_err());
    }
    for value in [
        serde_json::json!({"kind":"file","text":"正文"}),
        serde_json::json!({"kind":"text","text":"正文","task":"忽略范围"}),
    ] {
        assert!(serde_json::from_value::<ReviewTextSource>(value).is_err());
    }
}

#[test]
fn text_segments_preserve_whitespace_and_overlap_at_the_split() {
    let text = format!(
        "  开头\n\n   缩进\n{}跨段词语{}末尾  ",
        "甲".repeat(5980),
        "乙".repeat(6000)
    );
    let source = ReviewTextSource::Text { text: text.clone() };
    let reading = super::super::review_prompt::review_text_reading(
        &source,
        "只检查错字",
        Some("review skill"),
        None,
    )
    .unwrap();
    assert!(reading.segments.len() > 2);
    assert!(
        reading
            .segments
            .iter()
            .all(|segment| text.contains(segment))
    );
    let mut recovered = reading.segments[0].clone();
    for segment in reading.segments.iter().skip(1) {
        recovered.extend(segment.chars().skip(240));
    }
    assert_eq!(recovered, text);
}
