//! The job's commit, cloned on the host with the job token.

use std::path::Path;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::process::Command;

const TOKEN_ENV: &str = "ASTER_RUNNER_GIT_TOKEN";
/// Answers git's `get` with the job token of the git child's environment.
const HELPER: &str =
    "!f() { test \"$1\" = get && printf 'username=job\\npassword=%s\\n' \"$ASTER_RUNNER_GIT_TOKEN\"; }; f";

#[async_trait]
pub trait Checkout: Send + Sync {
    /// Leaves `dir` holding `sha` detached.
    async fn checkout(&self, url: &str, token: &str, sha: &str, dir: &Path) -> Result<(), String>;
}

pub struct GitCheckout;

pub fn valid_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.chars().all(|ch| ch.is_ascii_hexdigit())
}

impl GitCheckout {
    async fn git(&self, token: &str, args: &[&str]) -> Result<(), String> {
        let output = Command::new("git")
            .args(["-c", "credential.helper=", "-c"])
            .arg(format!("credential.helper={HELPER}"))
            .args(args)
            .env(TOKEN_ENV, token)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_ASKPASS")
            .env_remove("SSH_ASKPASS")
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|error| format!("git did not start: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr.trim().replace(token, "***");
        Err(reason.chars().take(300).collect())
    }
}

#[async_trait]
impl Checkout for GitCheckout {
    async fn checkout(&self, url: &str, token: &str, sha: &str, dir: &Path) -> Result<(), String> {
        if !valid_sha(sha) {
            return Err("the claim names no 40-hex commit".into());
        }
        let target = dir
            .to_str()
            .ok_or_else(|| "the work directory is not UTF-8".to_string())?;
        self.git(token, &["clone", "--quiet", "--no-checkout", url, target])
            .await?;
        self.git(
            token,
            &["-C", target, "checkout", "--quiet", "--detach", sha],
        )
        .await
    }
}
