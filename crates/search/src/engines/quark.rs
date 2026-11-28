//! Quark search results rendered by the user's logged-in browser.
//!
//! Quark embeds each result's structured `initialData` in a JSON hydration
//! script. Reading those values avoids coupling result identity to layout-only
//! CSS classes. Unknown card shapes are skipped; unrelated pages fail closed.

use crate::html_engine::{
    HtmlEngine, HtmlParser, SearchResponseSpec, selector, validate_search_response,
};
use crate::{EngineCategory, EngineConfig, Result, SearchQuery, SearchResult};
use scraper::Html;
use serde_json::Value;

pub struct QuarkParser;
pub type Quark = HtmlEngine<QuarkParser>;

impl Quark {
    pub fn new(fetcher: std::sync::Arc<dyn crate::PageFetcher>) -> Self {
        HtmlEngine::with_fetcher(QuarkParser, fetcher)
    }
}

impl HtmlParser for QuarkParser {
    fn default_config() -> EngineConfig {
        EngineConfig {
            name: "Quark".to_string(),
            shortcut: "quark".to_string(),
            categories: vec![EngineCategory::General],
            weight: 1.0,
            timeout: 10,
            enabled: true,
            paging: true,
            safesearch: false,
        }
    }

    fn build_url(&self, query: &SearchQuery) -> String {
        let mut url = url::Url::parse("https://quark.sm.cn/s").expect("static Quark search URL");
        url.query_pairs_mut().append_pair("q", &query.query);
        if query.page > 1 {
            url.query_pairs_mut()
                .append_pair("pg", &query.page.to_string());
        }
        url.into()
    }

    fn validate(&self, html: &str) -> Result<()> {
        validate_search_response(
            html,
            SearchResponseSpec {
                engine: "Quark",
                result_selectors: &["script[type=\"application/json\"][data-used-by=\"hydrate\"]"],
                empty_selectors: &["#results:empty", ".no-result", "#content_none"],
                challenge_selectors: &[
                    "form[action*=\"/captcha/\"]",
                    "iframe[src*=\"/captcha/\"]",
                    "script[src*=\"captcha\"]",
                ],
            },
        )
    }

    fn parse(&self, html: &str) -> Result<Vec<SearchResult>> {
        let document = Html::parse_document(html);
        let cards = selector("script[type=\"application/json\"][data-used-by=\"hydrate\"]")?;
        let mut results = Vec::new();
        for card in document.select(&cards) {
            let raw = card.text().collect::<String>();
            let Ok(payload) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            let Some(initial) = payload.pointer("/data/initialData") else {
                continue;
            };
            if let Some(result) = result_from_initial_data(initial) {
                if !results
                    .iter()
                    .any(|current: &SearchResult| current.url == result.url)
                {
                    results.push(result);
                }
            }
        }
        Ok(results)
    }
}

fn result_from_initial_data(initial: &Value) -> Option<SearchResult> {
    let url = first_string(
        initial,
        &[
            "/url",
            "/source_url",
            "/titleProps/dest_url",
            "/summaryProps/dest_url",
            "/nuProps/ru",
            "/nuProps/nu",
        ],
    )?;
    let parsed = url::Url::parse(&url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }
    let title = first_string(
        initial,
        &["/title", "/titleProps/content", "/titleProps/text"],
    )
    .map(|text| html_text(&text))?;
    if title.trim().is_empty() {
        return None;
    }
    let content = first_string(
        initial,
        &["/desc", "/summaryProps/content", "/text", "/summary"],
    )
    .map(|text| html_text(&text))
    .unwrap_or_default();
    Some(SearchResult::new(url, title, content))
}

fn first_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn html_text(value: &str) -> String {
    Html::parse_fragment(value)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;

    const RESULTS: &str = r##"
      <div id="results">
        <script type="application/json" data-used-by="hydrate">{"data":{"initialData":{"titleProps":{"content":"<em>Rust</em> 程序设计语言","dest_url":"https://www.rust-lang.org/zh-CN/"},"summaryProps":{"content":"构建可靠且高效软件能力的<em>语言</em>。"}}}}</script>
        <script type="application/json" data-used-by="hydrate">{"data":{"initialData":{"title":"Rust 编程视频","desc":"入门介绍","source_url":"https://www.bilibili.com/video/example"}}}</script>
      </div>
    "##;

    #[test]
    fn parses_structured_result_cards_without_layout_classes() {
        let results = QuarkParser.parse(RESULTS).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust 程序设计语言");
        assert_eq!(results[0].url, "https://www.rust-lang.org/zh-CN/");
        assert_eq!(results[0].content, "构建可靠且高效软件能力的语言。");
        assert_eq!(results[1].title, "Rust 编程视频");
        assert_eq!(results[1].url, "https://www.bilibili.com/video/example");
    }

    #[test]
    fn skips_unknown_duplicate_and_non_http_cards() {
        let html = r##"
          <script type="application/json" data-used-by="hydrate">{"data":{"initialData":{"title":"recommendation"}}}</script>
          <script type="application/json" data-used-by="hydrate">{"data":{"initialData":{"title":"bad","url":"javascript:alert(1)"}}}</script>
          <script type="application/json" data-used-by="hydrate">{"data":{"initialData":{"title":"one","url":"https://example.com"}}}</script>
          <script type="application/json" data-used-by="hydrate">{"data":{"initialData":{"title":"duplicate","url":"https://example.com"}}}</script>
        "##;
        let results = QuarkParser.parse(html).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "one");
    }

    #[test]
    fn validates_results_empty_state_and_parser_drift() {
        assert!(QuarkParser.validate(RESULTS).is_ok());
        assert!(
            QuarkParser
                .validate(r#"<main id="results"></main>"#)
                .is_ok()
        );
        let error = QuarkParser
            .validate(r#"<html><body><main>home</main></body></html>"#)
            .unwrap_err();
        assert_eq!(error.kind(), "invalid_response");
        let challenge = QuarkParser
            .validate(r#"<form action="/captcha/check"></form>"#)
            .unwrap_err();
        assert_eq!(challenge.kind(), "challenge");
    }

    #[test]
    fn query_url_preserves_utf8_and_page() {
        let query = SearchQuery::new("Rust 编程").with_page(2);
        let url = url::Url::parse(&QuarkParser.build_url(&query)).unwrap();
        assert_eq!(url.origin().ascii_serialization(), "https://quark.sm.cn");
        assert_eq!(url.path(), "/s");
        let pairs = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            pairs.get("q").map(|value| value.as_ref()),
            Some("Rust 编程")
        );
        assert_eq!(pairs.get("pg").map(|value| value.as_ref()), Some("2"));
    }

    #[test]
    fn engine_is_explicitly_named_quark() {
        let engine = Quark::new(std::sync::Arc::new(crate::HttpFetcher::new()));
        assert_eq!(engine.name(), "Quark");
        assert_eq!(engine.shortcut(), "quark");
        assert!(engine.is_enabled());
    }
}
