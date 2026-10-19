//! In-process checks for the user's text length and percentage requirements.

use a3s_code_core::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

#[path = "output_check_metrics.rs"]
mod metrics;

pub(crate) struct OutputCheckTool;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Check {
    TextLength { text: String },
    PercentageChange { pairs: Vec<Pair> },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    #[serde(deserialize_with = "Option::<f64>::deserialize")]
    current: Option<f64>,
    #[serde(deserialize_with = "Option::<f64>::deserialize")]
    previous: Option<f64>,
}

#[async_trait]
impl Tool for OutputCheckTool {
    fn name(&self) -> &str {
        "output_check"
    }

    fn description(&self) -> &str {
        "Check text length or compute percentage changes in process, without Bash. text_length counts Unicode characters including punctuation/whitespace, non-whitespace characters, and whitespace-delimited words. percentage_change uses (current-previous)/abs(previous)*100, returns full numeric values and signed prose rounded to at most 2 decimals with ASCII signs (+10%, -50%); explicit user precision still takes precedence. Missing values and zero baselines are not treated as zero changes."
    }

    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{
            "action":{"type":"string","enum":["text_length","percentage_change"]},
            "text":{"type":"string","maxLength":32768,"description":"Required only for text_length."},
            "pairs":{"type":"array","maxItems":512,"items":{
                "type":"object","properties":{
                    "current":{"type":["number","null"]},
                    "previous":{"type":["number","null"]}
                },"required":["current","previous"],"additionalProperties":false}}
        },"required":["action"],"additionalProperties":false,
        "oneOf":[
            {"properties":{"action":{"const":"text_length"}},"required":["text"]},
            {"properties":{"action":{"const":"percentage_change"}},"required":["pairs"]}
        ]})
    }

    fn capabilities(&self, _args: &Value) -> ToolCapabilities {
        ToolCapabilities::parallel_safe_read(1)
    }

    async fn execute(&self, args: &Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let check: Check = match serde_json::from_value(args.clone()) {
            Ok(check) => check,
            Err(error) => {
                return Ok(ToolOutput::error(format!(
                    "Invalid output_check arguments: {error}"
                )))
            }
        };
        let result = match check {
            Check::TextLength { text } => {
                if text.len() > 128 * 1024 || text.chars().count() > 32768 {
                    return Ok(ToolOutput::error(
                        "text exceeds the 32768-character input limit",
                    ));
                }
                let (characters, non_whitespace_characters, whitespace_delimited_words) =
                    metrics::text_counts(&text);
                json!({"characters":characters,"nonWhitespaceCharacters":non_whitespace_characters,"whitespaceDelimitedWords":whitespace_delimited_words})
            }
            Check::PercentageChange { pairs } => {
                if pairs.len() > 512 {
                    return Ok(ToolOutput::error("pairs exceeds the 512-item input limit"));
                }
                let results: Vec<_> = pairs.into_iter().map(|pair| {
                    match (pair.current, pair.previous) {
                        (Some(current), Some(previous)) => match metrics::percentage_change(current, previous) {
                            Ok((value, display)) => json!({"value":value,"display":display}),
                            Err(reason) => json!({"value":null,"display":null,"reason":reason}),
                        },
                        _ => json!({"value":null,"display":null,"reason":"missing value; not provided"}),
                    }
                }).collect();
                json!({"results":results})
            }
        };
        Ok(ToolOutput::success(result.to_string()))
    }
}

#[cfg(test)]
#[path = "output_check_tests.rs"]
mod tests;
