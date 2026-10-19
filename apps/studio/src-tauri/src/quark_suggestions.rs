//! Quark search suggestions for the workspace address bar.
//!
//! The renderer cannot read `https://sugs.m.sm.cn` directly. The response has
//! no CORS header, and the shell connect policy does not allow that host.
//! This command fetches only that fixed URL.

use std::process::Command;

const SUGGEST_URL: &str = "https://sugs.m.sm.cn/web";
const SUGGEST_LIMIT: usize = 8;
const QUERY_LIMIT: usize = 200;
const BODY_LIMIT: usize = 65_536;

pub(crate) fn suggestion_url(query: &str) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > QUERY_LIMIT {
        return Err("The suggestion query is empty or too long.".to_owned());
    }
    if query.chars().any(char::is_control) {
        return Err("The suggestion query contains a control character.".to_owned());
    }
    let mut url =
        url::Url::parse(SUGGEST_URL).map_err(|_| "The suggestion URL is invalid.".to_owned())?;
    url.query_pairs_mut().append_pair("q", query);
    Ok(url.into())
}

pub(crate) fn parse_suggestions(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(rows) = value.get("r").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut words = Vec::new();
    for row in rows {
        let Some(word) = row.get("w").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let word = word.trim();
        if word.is_empty()
            || word.chars().count() > QUERY_LIMIT
            || word.chars().any(char::is_control)
            || words.iter().any(|existing: &String| existing == word)
        {
            continue;
        }
        words.push(word.to_owned());
        if words.len() == SUGGEST_LIMIT {
            break;
        }
    }
    words
}

fn fetch_suggestions(query: &str) -> Result<Vec<String>, String> {
    let url = suggestion_url(query)?;
    let curl = if cfg!(windows) {
        "curl.exe"
    } else {
        "/usr/bin/curl"
    };
    let output = Command::new(curl)
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--max-redirs",
            "0",
            "--proto",
            "=https",
            "--max-time",
            "3",
            "--max-filesize",
            "65536",
            "--",
            &url,
        ])
        .output()
        .map_err(|_| "The suggestion request failed.".to_owned())?;
    if !output.status.success() || output.stdout.len() > BODY_LIMIT {
        return Err("The suggestion request failed.".to_owned());
    }
    let body = String::from_utf8(output.stdout)
        .map_err(|_| "The suggestion response is not text.".to_owned())?;
    Ok(parse_suggestions(&body))
}

#[tauri::command]
pub async fn quark_search_suggestions(query: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || fetch_suggestions(&query))
        .await
        .map_err(|_| "The suggestion request failed.".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::{fetch_suggestions, parse_suggestions, suggestion_url};

    #[test]
    fn builds_a_fixed_host_url() {
        let url = suggestion_url(" 天气预报 ").expect("query");
        assert!(url.starts_with("https://sugs.m.sm.cn/web?"));
        assert_eq!(
            url::Url::parse(&url)
                .expect("url")
                .query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.into_owned()),
            Some("天气预报".to_owned())
        );
    }

    #[test]
    fn rejects_an_empty_or_unsafe_query() {
        assert!(suggestion_url("").is_err());
        assert!(suggestion_url("   ").is_err());
        assert!(suggestion_url("line\nbreak").is_err());
        assert!(suggestion_url(&"a".repeat(201)).is_err());
    }

    #[test]
    fn reads_suggestion_words() {
        let body = r#"{"q":"天气","r":[{"w":"天气"},{"w":" 天气预报 "},{"w":""},{"skip":1},{"w":"天气预报"},{"w":"天气之子"}]}"#;
        assert_eq!(
            parse_suggestions(body),
            vec![
                "天气".to_owned(),
                "天气预报".to_owned(),
                "天气之子".to_owned()
            ]
        );
    }

    #[test]
    fn ignores_a_body_that_is_not_a_suggestion_list() {
        assert!(parse_suggestions("not-json").is_empty());
        assert!(parse_suggestions(r#"{"q":"天气"}"#).is_empty());
    }

    #[test]
    fn caps_the_list_at_eight_words() {
        let rows = (0..12)
            .map(|index| format!(r#"{{"w":"word{index}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let words = parse_suggestions(&format!(r#"{{"r":[{rows}]}}"#));
        assert_eq!(words.len(), 8);
        assert_eq!(words[0], "word0");
        assert_eq!(words[7], "word7");
    }

    #[test]
    #[ignore = "calls https://sugs.m.sm.cn"]
    fn live_suggestions_include_a_weather_forecast() {
        let words = fetch_suggestions("天气").expect("suggestions");
        assert!(words.iter().any(|word| word == "天气预报"));
    }
}
