//! Logged-in Quark Kbot sessions over a user-enabled local CDP connection.
//!
//! This adapter never reads cookies, profile databases, history or existing
//! conversations. Each call creates a separate new-conversation window and
//! closes only that page. The connected browser process is never terminated.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use tracing::warn;

use crate::cdp::{browser_error, CdpConnection, LocalCdpEndpoint};
use crate::{UseError, UseResult};

const QUARK_NEW_CONVERSATION: &str = "https://kbot.quark.cn/quarkchat";
const MAX_PROMPT_BYTES: usize = 1024 * 1024;
const MAX_ANSWER_BYTES: usize = 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarkModelReply {
    pub text: String,
}

#[derive(Default)]
struct QuarkTargetState {
    connection: Option<CdpConnection>,
    target_id: Option<String>,
    session_id: Option<String>,
}

/// One Aster-owned Kbot page. Dropping it schedules page-only cleanup.
pub struct QuarkModelPage {
    state: Arc<Mutex<QuarkTargetState>>,
}

impl std::fmt::Debug for QuarkModelPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let open = self
            .state
            .try_lock()
            .map(|guard| guard.target_id.is_some())
            .unwrap_or(true);
        f.debug_struct("QuarkModelPage")
            .field("open", &open)
            .finish_non_exhaustive()
    }
}

impl QuarkModelPage {
    /// Probe the account through a fresh page without reading an existing
    /// conversation. The owned page is closed before this returns.
    pub async fn probe_logged_in(endpoint: LocalCdpEndpoint, timeout: Duration) -> UseResult<()> {
        tokio::time::timeout(timeout, async {
            let page = Self::connect(endpoint, timeout).await?;
            page.close().await;
            Ok(())
        })
        .await
        .map_err(|_| UseError::new("use.browser.timeout", "Quark login probe timed out"))?
    }

    /// Close only this page and release Aster's CDP connection.
    pub async fn close(self) {
        close_target_state(&self.state).await;
    }

    pub async fn connect(endpoint: LocalCdpEndpoint, timeout: Duration) -> UseResult<Self> {
        tokio::time::timeout(timeout, Self::connect_inner(endpoint, timeout))
            .await
            .map_err(|_| {
                UseError::new(
                    "use.browser.timeout",
                    "Timed out opening a Quark Kbot conversation",
                )
            })?
    }

    async fn connect_inner(endpoint: LocalCdpEndpoint, timeout: Duration) -> UseResult<Self> {
        let websocket = endpoint.websocket_url().await?;
        let mut connection = CdpConnection::connect(&websocket).await.map_err(|error| {
            browser_error(format!(
                "Failed to connect to the local browser: {}",
                error.message
            ))
        })?;

        // Navigate explicitly after target creation so Page.navigate does not race target setup.
        let created = connection
            .call(
                "Target.createTarget",
                json!({
                    "url": "about:blank",
                    "newWindow": true,
                    "background": false
                }),
            )
            .await
            .map_err(|error| {
                browser_error(format!("Failed to open a Quark window: {}", error.message))
            })?;
        let target_id = created
            .get("targetId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| browser_error("Failed to open a Quark window: missing targetId"))?;

        let attached = match connection
            .call(
                "Target.attachToTarget",
                json!({
                    "targetId": target_id,
                    "flatten": true
                }),
            )
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let _ = connection
                    .call("Target.closeTarget", json!({ "targetId": target_id }))
                    .await;
                connection.close_socket().await;
                return Err(browser_error(format!(
                    "Failed to attach to Quark window: {}",
                    error.message
                )));
            }
        };
        let Some(session_id) = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            let _ = connection
                .call("Target.closeTarget", json!({ "targetId": target_id }))
                .await;
            connection.close_socket().await;
            return Err(browser_error(
                "Failed to attach to Quark window: missing sessionId",
            ));
        };

        let owned = Self {
            state: Arc::new(Mutex::new(QuarkTargetState {
                connection: Some(connection),
                target_id: Some(target_id),
                session_id: Some(session_id),
            })),
        };
        owned
            .call_session("Page.navigate", json!({ "url": QUARK_NEW_CONVERSATION }))
            .await
            .map_err(|error| {
                browser_error(format!("Failed to open Quark Kbot: {}", error.message))
            })?;
        owned.wait_for_editor(timeout).await?;
        Ok(owned)
    }

    pub async fn complete<F>(
        &self,
        prompt: &str,
        timeout: Duration,
        mut on_delta: F,
    ) -> UseResult<QuarkModelReply>
    where
        F: FnMut(String) -> UseResult<()>,
    {
        if prompt.trim().is_empty() {
            return Err(browser_error("Quark request has no user text"));
        }
        if prompt.len() > MAX_PROMPT_BYTES {
            return Err(browser_error("Quark request exceeds the 1 MiB input limit"));
        }
        tokio::time::timeout(timeout, self.complete_inner(prompt, timeout, &mut on_delta))
            .await
            .map_err(|_| UseError::new("use.browser.timeout", "Quark Kbot generation timed out"))?
    }

    async fn complete_inner<F>(
        &self,
        prompt: &str,
        timeout: Duration,
        on_delta: &mut F,
    ) -> UseResult<QuarkModelReply>
    where
        F: FnMut(String) -> UseResult<()>,
    {
        let focused: bool = self
            .evaluate_value(
                "(() => { const el = document.querySelector('.searchBarInput[contenteditable=\"true\"]'); if (!el) return false; el.click(); el.focus(); return true; })()",
            )
            .await
            .map_err(|error| browser_error(format!("Quark editor was not found: {}", error.message)))?;
        if !focused {
            return Err(browser_error("Quark editor was not found"));
        }
        self.call_session("Input.insertText", json!({ "text": prompt }))
            .await
            .map_err(|error| {
                browser_error(format!(
                    "Quark prompt could not be entered: {}",
                    error.message
                ))
            })?;
        let encoded_prompt = serde_json::to_string(prompt)
            .map_err(|error| browser_error(format!("Failed to encode Quark prompt: {error}")))?;
        let matches: bool = self
            .evaluate_value(&format!(
                "(() => {{ const editor = document.querySelector('.searchBarInput[contenteditable=\"true\"]'); return !!editor && editor.innerText.startsWith({encoded_prompt}); }})()"
            ))
            .await
            .map_err(|error| browser_error(format!("Quark prompt verification failed: {}", error.message)))?;
        if !matches {
            return Err(browser_error(
                "Quark editor did not retain the complete prompt; nothing was submitted",
            ));
        }
        self.call_session(
            "Input.dispatchKeyEvent",
            json!({
                "type": "rawKeyDown",
                "key": "Enter",
                "code": "Enter",
                "windowsVirtualKeyCode": 13,
                "text": "\r"
            }),
        )
        .await
        .and(
            self.call_session(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "keyUp",
                    "key": "Enter",
                    "code": "Enter",
                    "windowsVirtualKeyCode": 13
                }),
            )
            .await,
        )
        .map_err(|error| {
            browser_error(format!(
                "Quark prompt could not be submitted: {}",
                error.message
            ))
        })?;

        let deadline = Instant::now() + timeout;
        let mut streamed = String::new();
        loop {
            let answer = self.read_answer().await?;
            if answer.count > 1 {
                return Err(browser_error(
                    "Quark returned multiple answers in an isolated new conversation",
                ));
            }
            if let Some(text) = answer.text {
                if text.len() > MAX_ANSWER_BYTES {
                    return Err(browser_error("Quark answer exceeds the 1 MiB output limit"));
                }
                if let Some(delta) = monotonic_delta(&streamed, &text)? {
                    on_delta(delta)?;
                    streamed = text;
                }
                if answer.complete {
                    if streamed.trim().is_empty() {
                        return Err(browser_error("Quark returned an empty answer"));
                    }
                    return Ok(QuarkModelReply { text: streamed });
                }
            }
            if Instant::now() >= deadline {
                return Err(UseError::new(
                    "use.browser.timeout",
                    "Quark Kbot did not complete its answer before the deadline",
                ));
            }
            sleep(POLL_INTERVAL).await;
        }
    }

    async fn wait_for_editor(&self, timeout: Duration) -> UseResult<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let ready: bool = self
                .evaluate_value(
                    "(() => [...document.querySelectorAll('.searchBarInput[contenteditable=\"true\"]')].some(element => element.offsetWidth > 0 && element.offsetHeight > 0))()",
                )
                .await
                .map_err(|error| browser_error(format!("Quark readiness check failed: {}", error.message)))?;
            if ready {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(UseError::new(
                    "use.browser.quark_login_required",
                    "Quark Kbot did not expose its new-conversation editor; sign in to Quark and retry. If already signed in, the Kbot page structure may have changed",
                ));
            }
            sleep(POLL_INTERVAL).await;
        }
    }

    async fn read_answer(&self) -> UseResult<RawAnswer> {
        self.evaluate_value(
            r#"(() => {
              const answers = [...document.querySelectorAll('.answer-common-card .qk-markdown')]
                .filter(element => element.offsetWidth > 0 && element.offsetHeight > 0);
              return {
                count: answers.length,
                text: answers.length === 1 ? answers[0].innerText : null,
                complete: answers.length === 1 && answers[0].classList.contains('qk-markdown-complete')
              };
            })()"#,
        )
        .await
        .map_err(|error| browser_error(format!("Failed to read the Quark answer: {}", error.message)))
    }

    async fn call_session(&self, method: &str, params: Value) -> UseResult<Value> {
        let mut guard = self.state.lock().await;
        let session_id = guard.session_id.clone().ok_or_else(|| {
            UseError::new(
                "use.browser.page_closed",
                "The Quark conversation page is closed",
            )
        })?;
        let connection = guard.connection.as_mut().ok_or_else(|| {
            UseError::new(
                "use.browser.page_closed",
                "The Quark conversation page is closed",
            )
        })?;
        connection.call_session(&session_id, method, params).await
    }

    async fn evaluate_value<T: for<'de> Deserialize<'de>>(&self, expression: &str) -> UseResult<T> {
        let result = self
            .call_session(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true
                }),
            )
            .await?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(browser_error(format!("Quark JS exception: {exception}")));
        }
        let raw = result
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null);
        serde_json::from_value(raw).map_err(|error| {
            browser_error(format!("Quark evaluation response was invalid: {error}"))
        })
    }
}

impl Drop for QuarkModelPage {
    fn drop(&mut self) {
        let state = Arc::clone(&self.state);
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            warn!("Cannot schedule Quark page cleanup without a Tokio runtime");
            return;
        };
        runtime.spawn(async move {
            close_target_state(&state).await;
        });
    }
}

async fn close_target_state(state: &Arc<Mutex<QuarkTargetState>>) {
    let mut guard = state.lock().await;
    let target_id = guard.target_id.take();
    guard.session_id = None;
    if let Some(mut connection) = guard.connection.take() {
        if let Some(target_id) = target_id {
            if let Err(error) = connection
                .call("Target.closeTarget", json!({ "targetId": target_id }))
                .await
            {
                warn!(
                    "Failed to close the Aster-owned Quark page: {}",
                    error.message
                );
            }
        }
        connection.close_socket().await;
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAnswer {
    count: usize,
    text: Option<String>,
    #[serde(default)]
    complete: bool,
}

fn monotonic_delta(previous: &str, current: &str) -> UseResult<Option<String>> {
    if current == previous {
        return Ok(None);
    }
    if !current.starts_with(previous) {
        return Err(browser_error(
            "Quark replaced part of an answer after it was streamed",
        ));
    }
    Ok(Some(current[previous.len()..].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_deltas_never_repeat_or_rewrite_streamed_text() {
        assert_eq!(monotonic_delta("", "你好").unwrap(), Some("你好".into()));
        assert_eq!(monotonic_delta("你好", "你好").unwrap(), None);
        assert_eq!(
            monotonic_delta("你好", "你好，夸克").unwrap(),
            Some("，夸克".into())
        );
        assert!(monotonic_delta("abc", "xbc").is_err());
        assert!(monotonic_delta("界", "界面").is_ok());
    }

    #[tokio::test]
    #[ignore = "requires the user's logged-in Quark browser on local CDP"]
    async fn live_login_probe_reports_availability() {
        let endpoint = LocalCdpEndpoint::parse("http://127.0.0.1:9222").unwrap();
        QuarkModelPage::probe_logged_in(endpoint, Duration::from_secs(10))
            .await
            .unwrap();
    }
}
