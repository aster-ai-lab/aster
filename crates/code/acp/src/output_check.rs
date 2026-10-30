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
    TextLength {
        text: String,
    },
    PercentageChange {
        pairs: Vec<Pair>,
    },
    DateSpan {
        start: String,
        end: String,
        endpoint_counting: String,
    },
    SequenceChanges {
        values: Vec<Option<f64>>,
    },
    RecordProvenance {
        source: Vec<Value>,
        output: Vec<Value>,
        key: String,
        #[serde(default)]
        compare_fields: Vec<String>,
        #[serde(default)]
        allow_derived_keys: Vec<Value>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    #[serde(deserialize_with = "Option::<f64>::deserialize")]
    current: Option<f64>,
    #[serde(deserialize_with = "Option::<f64>::deserialize")]
    previous: Option<f64>,
}

fn provenance_key(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn provenance_index<'a>(
    records: &'a [Value],
    key: &str,
) -> Result<(std::collections::BTreeMap<String, &'a Value>, Vec<String>), String> {
    let mut indexed = std::collections::BTreeMap::new();
    let mut duplicates = Vec::new();
    for (position, record) in records.iter().enumerate() {
        let identifier = record
            .get(key)
            .and_then(provenance_key)
            .ok_or_else(|| format!("record {position} has no valid {key} key"))?;
        if indexed.insert(identifier.clone(), record).is_some() {
            duplicates.push(identifier);
        }
    }
    Ok((indexed, duplicates))
}

fn record_provenance(
    source: &[Value],
    output: &[Value],
    key: &str,
    compare_fields: &[String],
    allow_derived_keys: &[Value],
) -> Result<Value, String> {
    if source.len() > 2048 || output.len() > 2048 {
        return Err("source and output must each contain at most 2048 records".into());
    }
    if key.trim().is_empty() || key.chars().count() > 64 || compare_fields.len() > 64 {
        return Err("key must be nonempty and at most 64 characters; compare_fields is limited to 64 fields".into());
    }
    if compare_fields
        .iter()
        .any(|field| field.is_empty() || field.chars().count() > 64)
    {
        return Err("compare_fields entries must contain 1..64 characters".into());
    }
    if allow_derived_keys.len() > 256 {
        return Err("allow_derived_keys exceeds the 256-item limit".into());
    }
    let allowed: std::collections::BTreeSet<String> = allow_derived_keys
        .iter()
        .map(|value| {
            provenance_key(value).ok_or_else(|| {
                "allow_derived_keys entries must be nonempty strings or numbers".to_owned()
            })
        })
        .collect::<Result<_, _>>()?;
    let (source_records, duplicate_source_keys) = provenance_index(source, key)?;
    let (output_records, duplicate_output_keys) = provenance_index(output, key)?;
    let mut missing = Vec::new();
    let mut changed = Vec::new();
    for (identifier, source_record) in &source_records {
        let Some(output_record) = output_records.get(identifier) else {
            missing.push(identifier.clone());
            continue;
        };
        if compare_fields.is_empty() {
            continue;
        }
        let differences: Vec<_> = compare_fields
            .iter()
            .filter(|field| source_record.get(*field) != output_record.get(*field))
            .cloned()
            .collect();
        if !differences.is_empty() {
            changed.push(json!({"key":identifier,"fields":differences}));
        }
    }
    let unexpected: Vec<_> = output_records
        .keys()
        .filter(|identifier| {
            !source_records.contains_key(*identifier) && !allowed.contains(*identifier)
        })
        .cloned()
        .collect();
    let valid = missing.is_empty()
        && unexpected.is_empty()
        && changed.is_empty()
        && duplicate_source_keys.is_empty()
        && duplicate_output_keys.is_empty();
    Ok(json!({
        "valid":valid,
        "sourceRecords":source.len(),
        "outputRecords":output.len(),
        "missing":missing,
        "unexpected":unexpected,
        "duplicateSourceKeys":duplicate_source_keys,
        "duplicateOutputKeys":duplicate_output_keys,
        "changed":changed,
    }))
}

#[async_trait]
impl Tool for OutputCheckTool {
    fn name(&self) -> &str {
        "output_check"
    }

    fn description(&self) -> &str {
        "Check text length, percentage changes, date spans or every adjacent sequence change in process, without Bash. text_length counts Unicode characters including punctuation/whitespace, non-whitespace characters, and whitespace-delimited words. percentage_change uses (current-previous)/abs(previous)*100, returns full numeric values and signed prose rounded to at most 2 decimals with ASCII signs (+10%, -50%); explicit user precision still takes precedence. date_span requires exact YYYY-MM-DD dates and explicit elapsed or inclusive endpoint counting; reversed dates are refused. sequence_changes returns every adjacent difference and every decrease with zero-based indices; missing values leave the complete-sequence conclusion unknown. record_provenance compares source and output records by key and reports missing, unexpected, duplicate and changed records; allow_derived_keys is only for task-authorized derived records. Missing values and zero baselines are never converted to zero changes. These measurements do not establish business meaning or resolve conflicting user requirements; compare every final claim with its source and counting rule."
    }

    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{
            "action":{"type":"string","enum":["text_length","percentage_change","date_span","sequence_changes","record_provenance"]},
            "text":{"type":"string","maxLength":32768,"description":"Required only for text_length."},
            "start":{"type":"string","minLength":10,"maxLength":10,"description":"Required for date_span: YYYY-MM-DD."},
            "end":{"type":"string","minLength":10,"maxLength":10,"description":"Required for date_span: YYYY-MM-DD, on or after start."},
            "endpoint_counting":{"type":"string","enum":["elapsed","inclusive"],"description":"Required for date_span. elapsed counts end minus start; inclusive counts both endpoints."},
            "values":{"type":"array","minItems":2,"maxItems":512,"items":{"type":["number","null"]},"description":"Required for sequence_changes, in source order; null is missing."},
            "source":{"type":"array","maxItems":2048,"items":{"type":"object"},"description":"Source records for record_provenance."},
            "output":{"type":"array","maxItems":2048,"items":{"type":"object"},"description":"Output records for record_provenance."},
            "key":{"type":"string","minLength":1,"maxLength":64,"description":"Record identifier field for record_provenance."},
            "compare_fields":{"type":"array","maxItems":64,"items":{"type":"string","minLength":1,"maxLength":64},"description":"Fields to compare; omitted compares all source fields except the key."},
            "allow_derived_keys":{"type":"array","maxItems":256,"items":{"type":["string","number"]},"description":"Task-authorized derived record keys."},
            "pairs":{"type":"array","maxItems":512,"items":{
                "type":"object","properties":{
                    "current":{"type":["number","null"]},
                    "previous":{"type":["number","null"]}
                },"required":["current","previous"],"additionalProperties":false}}
        },"required":["action"],"additionalProperties":false,
        "oneOf":[
            {"properties":{"action":{"const":"text_length"},"text":{}},"required":["text"],"additionalProperties":false},
            {"properties":{"action":{"const":"percentage_change"},"pairs":{}},"required":["pairs"],"additionalProperties":false},
            {"properties":{"action":{"const":"date_span"},"start":{},"end":{},"endpoint_counting":{}},"required":["start","end","endpoint_counting"],"additionalProperties":false},
            {"properties":{"action":{"const":"sequence_changes"},"values":{}},"required":["values"],"additionalProperties":false},
            {"properties":{"action":{"const":"record_provenance"},"source":{},"output":{},"key":{},"compare_fields":{},"allow_derived_keys":{}},"required":["source","output","key"],"additionalProperties":false}
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
            Check::DateSpan {
                start,
                end,
                endpoint_counting,
            } => {
                let counting = match metrics::EndpointCounting::parse(&endpoint_counting) {
                    Ok(value) => value,
                    Err(reason) => return Ok(ToolOutput::error(reason)),
                };
                match metrics::date_span(&start, &end, counting) {
                    Ok(days) => {
                        json!({"start":start,"end":end,"endpoint_counting":counting.as_str(),"days":days})
                    }
                    Err(reason) => return Ok(ToolOutput::error(reason)),
                }
            }
            Check::SequenceChanges { values } => {
                if !(2..=512).contains(&values.len()) {
                    return Ok(ToolOutput::error("values must contain 2..512 items"));
                }
                let mut steps = Vec::with_capacity(values.len() - 1);
                let mut decreases = Vec::new();
                let mut complete = true;
                for (index, pair) in values.windows(2).enumerate() {
                    let mut step = json!({"from_index":index,"to_index":index+1,"previous":pair[0],"current":pair[1]});
                    match (pair[0], pair[1]) {
                        (Some(previous), Some(current))
                            if current.is_finite() && previous.is_finite() =>
                        {
                            let difference = current - previous;
                            if !difference.is_finite() {
                                return Ok(ToolOutput::error(
                                    "sequence difference exceeds finite numeric range",
                                ));
                            }
                            step["difference"] = json!(difference);
                            if current < previous {
                                decreases.push(json!({"from_index":index,"to_index":index+1}));
                            }
                            match metrics::percentage_change(current, previous) {
                                Ok((value, display)) => {
                                    step["percentage_change"] = json!(value);
                                    step["display"] = json!(display);
                                }
                                Err(reason) => {
                                    step["percentage_change"] = Value::Null;
                                    step["reason"] = json!(reason);
                                }
                            }
                        }
                        _ => {
                            complete = false;
                            step["difference"] = Value::Null;
                            step["percentage_change"] = Value::Null;
                            step["reason"] = json!("missing value; not provided");
                        }
                    }
                    steps.push(step);
                }
                json!({"steps":steps,"decreases":decreases,"all_steps_observed":complete,"non_decreasing":if complete { json!(decreases.is_empty()) } else { Value::Null }})
            }
            Check::RecordProvenance {
                source,
                output,
                key,
                compare_fields,
                allow_derived_keys,
            } => {
                let compare_fields = if compare_fields.is_empty() {
                    let mut fields = std::collections::BTreeSet::new();
                    for object in source.iter().filter_map(Value::as_object) {
                        fields.extend(object.keys().filter(|field| *field != &key).cloned());
                    }
                    fields.into_iter().collect()
                } else {
                    compare_fields
                };
                match record_provenance(
                    &source,
                    &output,
                    &key,
                    &compare_fields,
                    &allow_derived_keys,
                ) {
                    Ok(result) => result,
                    Err(reason) => return Ok(ToolOutput::error(reason)),
                }
            }
        };
        Ok(ToolOutput::success(result.to_string()))
    }
}

#[cfg(test)]
#[path = "output_check_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "output_check_sequence_tests.rs"]
mod sequence_tests;
