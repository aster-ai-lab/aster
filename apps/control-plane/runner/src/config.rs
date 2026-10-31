//! `runner.acl`: one `runner` block, closed attribute set.

use std::collections::HashMap;
use std::path::PathBuf;

use aster_acl::{parse_acl, Value};

pub const ATTRIBUTE_SET: &str = "api_url, state_file, work_dir, image, box_cli, cpus, memory, \
poll_seconds and heartbeat_seconds";
const ATTRIBUTES: [&str; 9] = [
    "api_url",
    "state_file",
    "work_dir",
    "image",
    "box_cli",
    "cpus",
    "memory",
    "poll_seconds",
    "heartbeat_seconds",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    /// Base URL without a trailing `/`.
    pub api_url: String,
    pub state_file: PathBuf,
    pub work_dir: PathBuf,
    pub image: String,
    pub box_cli: Option<PathBuf>,
    pub cpus: u32,
    pub memory: String,
    pub poll_seconds: u64,
    pub heartbeat_seconds: u64,
}

pub fn parse_config(text: &str) -> Result<RunnerConfig, String> {
    let document = parse_acl(text).map_err(|err| format!("runner acl: {err}"))?;
    let [block] = document.blocks.as_slice() else {
        return Err("runner acl must hold exactly one `runner` block".into());
    };
    if block.name != "runner" || !block.labels.is_empty() {
        return Err("runner acl must hold exactly one `runner` block".into());
    }
    if !block.blocks.is_empty()
        || block
            .attributes
            .keys()
            .any(|key| !ATTRIBUTES.contains(&key.as_str()))
    {
        return Err(format!("runner only accepts {ATTRIBUTE_SET}"));
    }
    let attributes = &block.attributes;
    let api_url = api_url(&required_string(attributes, "api_url")?)?;
    let state_file = absolute(attributes, "state_file")?;
    let work_dir = absolute(attributes, "work_dir")?;
    let image = required_string(attributes, "image")?;
    valid_image(&image)?;
    let box_cli = match optional_string(attributes, "box_cli")? {
        Some(path) => Some(absolute_path("box_cli", &path)?),
        None => None,
    };
    let memory = optional_string(attributes, "memory")?.unwrap_or_else(|| "2g".into());
    valid_memory(&memory)?;
    Ok(RunnerConfig {
        api_url,
        state_file,
        work_dir,
        image,
        box_cli,
        cpus: integer(attributes, "cpus", 1, 16, 2)? as u32,
        memory,
        poll_seconds: integer(attributes, "poll_seconds", 1, 60, 5)?,
        heartbeat_seconds: integer(attributes, "heartbeat_seconds", 5, 60, 30)?,
    })
}

fn optional_string(
    attributes: &HashMap<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match attributes.get(key) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("runner {key} must be a string")),
    }
}

fn required_string(attributes: &HashMap<String, Value>, key: &str) -> Result<String, String> {
    optional_string(attributes, key)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("runner {key} is required"))
}

fn absolute(attributes: &HashMap<String, Value>, key: &str) -> Result<PathBuf, String> {
    absolute_path(key, &required_string(attributes, key)?)
}

fn absolute_path(key: &str, value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!("runner {key} must be an absolute path"))
    }
}

fn integer(
    attributes: &HashMap<String, Value>,
    key: &str,
    min: u64,
    max: u64,
    default: u64,
) -> Result<u64, String> {
    let refusal = || format!("runner {key} must be an integer {min}..={max}");
    match attributes.get(key) {
        None => Ok(default),
        Some(Value::Number(value))
            if value.fract() == 0.0 && *value >= min as f64 && *value <= max as f64 =>
        {
            Ok(*value as u64)
        }
        Some(_) => Err(refusal()),
    }
}

fn api_url(value: &str) -> Result<String, String> {
    let trimmed = value.trim_end_matches('/');
    let rest = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"));
    match rest {
        Some(rest) if !rest.is_empty() && !trimmed.chars().any(char::is_whitespace) => {
            Ok(trimmed.to_string())
        }
        _ => Err("runner api_url must be an http:// or https:// URL".into()),
    }
}

/// Non-empty, no whitespace, not a flag.
pub fn valid_image(image: &str) -> Result<(), String> {
    if image.is_empty() || image.starts_with('-') || image.chars().any(char::is_whitespace) {
        Err("runner image must be an image reference without whitespace".into())
    } else {
        Ok(())
    }
}

/// `^[1-9][0-9]{0,5}[mg]$`.
fn valid_memory(memory: &str) -> Result<(), String> {
    let (digits, unit) = memory.split_at(memory.len().saturating_sub(1));
    let shaped = matches!(unit, "m" | "g")
        && (1..=6).contains(&digits.len())
        && digits.chars().all(|ch| ch.is_ascii_digit())
        && !digits.starts_with('0');
    if shaped {
        Ok(())
    } else {
        Err("runner memory must match ^[1-9][0-9]{0,5}[mg]$".into())
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
