use std::rc::Rc;

use agent_client_protocol as acp;
use anyhow::{Context, Result};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

struct StartupError(String);

impl StartupError {
    fn error(&self) -> acp::Error {
        acp::Error::new(acp::ErrorCode::InternalError.into(), self.0.clone())
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Agent for StartupError {
    async fn initialize(&self, _: acp::InitializeRequest) -> acp::Result<acp::InitializeResponse> {
        Err(self.error())
    }

    async fn authenticate(
        &self,
        _: acp::AuthenticateRequest,
    ) -> acp::Result<acp::AuthenticateResponse> {
        Err(self.error())
    }

    async fn new_session(&self, _: acp::NewSessionRequest) -> acp::Result<acp::NewSessionResponse> {
        Err(self.error())
    }

    async fn prompt(&self, _: acp::PromptRequest) -> acp::Result<acp::PromptResponse> {
        Err(self.error())
    }

    async fn cancel(&self, _: acp::CancelNotification) -> acp::Result<()> {
        Err(self.error())
    }
}

pub(super) async fn report(error: anyhow::Error) -> Result<()> {
    let detail: String = format!("{error:#}").chars().take(4096).collect();
    let agent = Rc::new(StartupError(format!(
        "Cannot load trusted ACL configuration: {detail}. Configure providers and default_model in \
         ~/.aster/config.acl, or explicitly select a reviewed file with \
         `ASTER_CONFIG=/path/to/config.acl just code`."
    )));
    let (_connection, io_task) = acp::AgentSideConnection::new(
        agent,
        tokio::io::stdout().compat_write(),
        tokio::io::stdin().compat(),
        |future| {
            tokio::task::spawn_local(future);
        },
    );
    io_task.await.context("ACP startup error response")
}
