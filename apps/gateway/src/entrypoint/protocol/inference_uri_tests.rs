use super::translated_upstream_uri;
use crate::translation::ResponseTranslation;

#[test]
fn selects_each_attempt_path_and_preserves_the_original_query() {
    for query in ["", "?api-version=fixture-v1&trace=two%20words"] {
        let original: http::Uri = format!("/v1/responses{query}").parse().unwrap();
        for (translation, path) in [
            (ResponseTranslation::ToOpenai, "/v1/messages"),
            (ResponseTranslation::ToAnthropic, "/v1/chat/completions"),
            (
                ResponseTranslation::ResponsesFromChat,
                "/v1/chat/completions",
            ),
        ] {
            assert_eq!(
                translated_upstream_uri(&original, Some(translation), None)
                    .unwrap()
                    .to_string(),
                format!("{path}{query}")
            );
        }
        assert_eq!(
            translated_upstream_uri(&original, None, None).unwrap(),
            original
        );
        assert_eq!(original.to_string(), format!("/v1/responses{query}"));
    }
}
