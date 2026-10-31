//! The API the runner talks to, behind a trait so the job loop is tested
//! without a server.

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::protocol::{
    ClaimedJob, CompletionBody, Envelope, Heartbeat, JobStatus, LogBody, TokenBody,
};

const PIPELINES: &str = "/v1/api/admin/pipelines";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// The token or credential was refused (401/403).
    #[error("refused by the API")]
    Unauthorized,
    /// The job already ended on the server (409).
    #[error("the job already ended")]
    Conflict,
    #[error("{0}")]
    Other(String),
}

#[async_trait]
pub trait RunnerApi: Send + Sync {
    /// With a registration token, the answer carries the credential.
    async fn heartbeat(&self, token: &str) -> Result<Heartbeat, ApiError>;
    async fn claim(&self, credential: &str) -> Result<Option<ClaimedJob>, ApiError>;
    async fn append_log(&self, job_id: &str, job_token: &str, chunk: &str) -> Result<(), ApiError>;
    async fn complete(
        &self,
        job_id: &str,
        credential: &str,
        status: JobStatus,
    ) -> Result<(), ApiError>;
}

pub struct HttpApi {
    client: reqwest::Client,
    base: String,
}

impl HttpApi {
    pub fn new(api_url: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|error| format!("http client: {error}"))?;
        Ok(Self {
            client,
            base: format!("{api_url}{PIPELINES}"),
        })
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .json(body)
            .send()
            .await
            .map_err(|error| ApiError::Other(format!("POST {path}: {}", error.without_url())))?;
        let status = response.status().as_u16();
        match status {
            200..=299 => response
                .json::<Envelope<T>>()
                .await
                .map(|envelope| envelope.data)
                .map_err(|error| ApiError::Other(format!("POST {path}: {error}"))),
            401 | 403 => Err(ApiError::Unauthorized),
            409 => Err(ApiError::Conflict),
            _ => {
                let message = response
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|body| body.get("message")?.as_str().map(str::to_string))
                    .unwrap_or_default();
                Err(ApiError::Other(format!(
                    "POST {path}: HTTP {status} {message}"
                )))
            }
        }
    }
}

#[async_trait]
impl RunnerApi for HttpApi {
    async fn heartbeat(&self, token: &str) -> Result<Heartbeat, ApiError> {
        self.post("/runner-heartbeats", &TokenBody { token }).await
    }

    async fn claim(&self, credential: &str) -> Result<Option<ClaimedJob>, ApiError> {
        self.post("/job-claims", &TokenBody { token: credential })
            .await
    }

    async fn append_log(&self, job_id: &str, job_token: &str, chunk: &str) -> Result<(), ApiError> {
        self.post::<_, serde_json::Value>(
            &format!("/jobs/{job_id}/logs"),
            &LogBody {
                token: job_token,
                chunk,
            },
        )
        .await
        .map(|_| ())
    }

    async fn complete(
        &self,
        job_id: &str,
        credential: &str,
        status: JobStatus,
    ) -> Result<(), ApiError> {
        self.post::<_, serde_json::Value>(
            &format!("/jobs/{job_id}/completions"),
            &CompletionBody {
                token: credential,
                status: status.as_str(),
            },
        )
        .await
        .map(|_| ())
    }
}
