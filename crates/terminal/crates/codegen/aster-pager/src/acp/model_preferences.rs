//! Persist successful picker selections in the ACL read by the next ACP process.

use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use aster_core::CodeConfig;
use aster_core::config::{ConfigSection, ReasoningEffort, rewrite_acl_sections};
use aster_shell::sampling::types::ReasoningEffort as PickerEffort;

const MAX_PREFERENCE_CONFIG_BYTES: u64 = 1024 * 1024;

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
    let mut config = CodeConfig::from_acl(&source).context("parse model configuration")?;
    config.default_model = model.filter(|value| !value.is_empty());
    config.reasoning_effort = Some(
        effort
            .map(|effort| ReasoningEffort::parse(effort.as_ref()))
            .transpose()?
            .unwrap_or(ReasoningEffort::Default),
    );
    let updated = rewrite_acl_sections(
        &source,
        &config,
        &[ConfigSection::DefaultModel, ConfigSection::ReasoningEffort],
    )?;
    aster_config::fs_atomic::write_atomically_bound(&destination, &updated, None)
        .context("save model preference")
}

#[cfg(test)]
#[path = "model_preferences_tests.rs"]
mod tests;
