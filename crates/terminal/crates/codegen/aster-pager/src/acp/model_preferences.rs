//! Persist successful picker selections in the ACL read by the next ACP process.

use std::io::Read;
use std::path::Path;

use agent_client_protocol as acp;
use anyhow::{Context, Result, bail};
use aster_acl::Value;
use aster_shell::sampling::types::ReasoningEffort as PickerEffort;
use aster_shell::session::{
    info::Info,
    storage::{JsonlStorageAdapter, StorageAdapter},
};

const MAX_PREFERENCE_CONFIG_BYTES: u64 = 1024 * 1024;

pub(crate) async fn persist_session(
    cwd: &Path,
    id: acp::SessionId,
    model: &acp::ModelId,
    effort: Option<PickerEffort>,
) -> std::io::Result<()> {
    let storage = JsonlStorageAdapter::with_root(aster_shell::util::grok_home::grok_home());
    let info = Info {
        id,
        cwd: cwd.to_string_lossy().into_owned(),
    };
    storage.init_session(&info, model.clone()).await?;
    storage
        .update_current_model_and_agent(&info, model, None, Some(effort))
        .await
}

pub(crate) async fn add_session_selection(
    cwd: &Path,
    id: &acp::SessionId,
    meta: &mut Option<acp::Meta>,
) {
    let storage = JsonlStorageAdapter::with_root(aster_shell::util::grok_home::grok_home());
    let info = Info {
        id: id.clone(),
        cwd: cwd.to_string_lossy().into_owned(),
    };
    let summary = match storage.load_summary(&info).await {
        Ok(summary) => summary,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            tracing::warn!(%error, "session model selection could not be read");
            return;
        }
    };
    let model_id = summary.current_model_id.0.as_ref();
    if !model_id.contains('/') {
        return;
    }
    let meta = meta.get_or_insert_with(acp::Meta::new);
    meta.insert("aster/sessionModelId".into(), model_id.into());
    if let Some(effort) = summary.recorded_reasoning_effort() {
        meta.insert("reasoningEffortModelId".into(), model_id.into());
        meta.insert(
            "reasoningEffort".into(),
            effort
                .map_or_else(|| "default".to_string(), |effort| effort.to_string())
                .into(),
        );
    }
}

pub(crate) async fn persist(model: Option<String>, effort: Option<PickerEffort>) -> Result<()> {
    let path = crate::views::model_config::default_config_path();
    tokio::task::spawn_blocking(move || save_to(&path, model, effort))
        .await
        .context("model preference save task failed")?
}

fn save_to(path: &Path, model: Option<String>, effort: Option<PickerEffort>) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).context("create model configuration directory")?;
    }
    let destination = aster_config::fs_atomic::bind_follow_destination(path)?;
    let mut source = String::new();
    match std::fs::File::open(destination.as_path()) {
        Ok(file) => {
            file.take(MAX_PREFERENCE_CONFIG_BYTES + 1)
                .read_to_string(&mut source)?;
            if source.len() as u64 > MAX_PREFERENCE_CONFIG_BYTES {
                bail!("model configuration exceeds 1 MiB");
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("read model configuration"),
    }
    let default_model = model.filter(|value| !value.is_empty());
    let effort_token = match effort {
        Some(effort) => normalize_reasoning_effort(effort.as_ref())?,
        None => "default",
    };
    let updated = crate::views::model_config::rewrite_top_level_scalars(
        &source,
        &[
            (
                &["default_model", "defaultModel"],
                "default_model",
                default_model.map(Value::String),
            ),
            (
                &["reasoning_effort", "reasoningEffort"],
                "reasoning_effort",
                Some(Value::String(effort_token.to_string())),
            ),
        ],
    )
    .map_err(anyhow::Error::msg)?;
    aster_config::fs_atomic::write_atomically_bound(&destination, &updated, None)
        .context("save model preference")
}

fn normalize_reasoning_effort(value: &str) -> Result<&'static str> {
    match value {
        "default" => Ok("default"),
        "none" => Ok("none"),
        "minimal" => Ok("minimal"),
        "low" => Ok("low"),
        "medium" => Ok("medium"),
        "high" => Ok("high"),
        "xhigh" => Ok("xhigh"),
        "max" => Ok("max"),
        "ultracode" => Ok("ultracode"),
        _ => bail!(
            "reasoning_effort must be one of default, none, minimal, low, medium, high, xhigh, max, ultracode"
        ),
    }
}

#[cfg(test)]
#[path = "model_preferences_tests.rs"]
mod tests;
