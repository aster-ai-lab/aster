use agent_client_protocol as acp;
use aster_fast_worktree::{
    resolve_grok_home, GcOptions, ListFilter, RegistryOpen, WorktreeDb, WorktreeKind,
};

fn response<T: serde::Serialize>(result: T) -> acp::Result<acp::ExtResponse> {
    let raw = serde_json::value::to_raw_value(&serde_json::json!({
        "result": result,
        "error": null,
    }))
    .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

fn internal(error: impl std::fmt::Display) -> acp::Error {
    acp::Error::internal_error().data(error.to_string())
}

fn params<'a, T: serde::Deserialize<'a>>(args: &'a acp::ExtRequest) -> acp::Result<T> {
    serde_json::from_str(args.params.get())
        .map_err(|error| acp::Error::invalid_params().data(error.to_string()))
}

async fn blocking<T>(
    operation: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> acp::Result<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(internal)?
        .map_err(internal)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListParams {
    #[serde(default)]
    repo: Option<String>,
    #[serde(default, rename = "type")]
    kinds: Vec<String>,
    #[serde(default)]
    include_all: bool,
}

pub(super) async fn list(args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
    let request: ListParams = params(args)?;
    let kind = match request.kinds.as_slice() {
        [] => None,
        [value] => Some(WorktreeKind::from_str_opt(value).ok_or_else(|| {
            acp::Error::invalid_params().data(format!("unknown worktree type: {value}"))
        })?),
        _ => {
            return Err(acp::Error::invalid_params()
                .data("the current registry accepts at most one worktree type filter"));
        }
    };
    let filter = ListFilter {
        repo_name: request.repo,
        kind,
        include_dead: request.include_all,
        ..ListFilter::default()
    };
    let home = resolve_grok_home().map_err(internal)?;
    let records = blocking(move || match WorktreeDb::open_read_only(&home) {
        RegistryOpen::Opened { db, .. } => db.list(&filter),
        RegistryOpen::Absent { .. } => Ok(Vec::new()),
        RegistryOpen::Busy { error, .. } | RegistryOpen::Failed { error, .. } => Err(error),
    })
    .await?;
    response(records)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GcParams {
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    max_age: Option<String>,
    #[serde(default)]
    force: bool,
}

fn max_age_secs(value: &str) -> acp::Result<i64> {
    let value = value.trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix('d') {
        (number, 86_400)
    } else if let Some(number) = value.strip_suffix('h') {
        (number, 3_600)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1)
    } else {
        return Err(acp::Error::invalid_params().data(format!(
            "invalid duration: {value} (expected e.g. 7d, 24h, 30m, 60s)"
        )));
    };
    number
        .parse::<i64>()
        .ok()
        .filter(|number| *number >= 0)
        .and_then(|number| number.checked_mul(multiplier))
        .ok_or_else(|| acp::Error::invalid_params().data(format!("invalid duration: {value}")))
}

pub(super) async fn gc(args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
    let request: GcParams = params(args)?;
    let options = GcOptions {
        max_age_secs: request.max_age.as_deref().map(max_age_secs).transpose()?,
        force: request.force,
        dry_run: request.dry_run,
        ..GcOptions::default()
    };
    let report = blocking(move || {
        let db = WorktreeDb::open_default()?;
        aster_fast_worktree::gc_worktrees(&db, &options)
    })
    .await?;
    response(report)
}

pub(super) fn db_path(args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
    params::<serde_json::Value>(args)?;
    response(serde_json::json!({
        "path": resolve_grok_home().map_err(internal)?.join("worktrees.db"),
    }))
}

pub(super) fn unsupported(operation: &str) -> acp::Result<acp::ExtResponse> {
    Err(acp::Error::method_not_found().data(format!(
        "aster-acp cannot {operation}; create the worktree with system Git, then start Aster with `aster --cwd <path>`"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_session_creation_names_a_supported_fallback() {
        let error = unsupported("resume a session in a worktree").unwrap_err();
        assert_eq!(error.code, acp::ErrorCode::MethodNotFound);
        let message = error.data.unwrap().to_string();
        assert!(message.contains("system Git"), "{message}");
        assert!(message.contains("aster --cwd <path>"), "{message}");
    }

    #[test]
    fn gc_duration_is_bounded_and_rejects_invalid_values() {
        assert_eq!(max_age_secs("7d").unwrap(), 7 * 86_400);
        assert_eq!(max_age_secs("24h").unwrap(), 24 * 3_600);
        assert_eq!(max_age_secs("0s").unwrap(), 0);
        for invalid in ["", "bad", "7x", "-1d", "999999999999999999999d"] {
            assert!(max_age_secs(invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
