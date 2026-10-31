//! Directory admission over HTTP. aster-api owns who may log in.

use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;

use crate::contexts::proof::directory_contract::{
    parse_directory_http_body, DirectoryAdmitRequest, DirectoryDecision, DirectoryStatusRequest,
};
use crate::contexts::proof::domain::{
    Admission, AdmitQuery, AuthError, DirectoryAdmission, Plane, SubjectStatus,
};

pub struct HttpDirectory {
    base: String,
    token: String,
    client: reqwest::Client,
}

impl HttpDirectory {
    pub fn new(base: impl Into<String>, token: impl Into<String>) -> Result<Self, AuthError> {
        let token = token.into();
        if token.is_empty() {
            return Err(AuthError::storage(
                "directory client requires an internal token",
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| AuthError::storage(err.to_string()))?;
        Ok(Self {
            base: base.into().trim_end_matches('/').to_owned(),
            token,
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }

    async fn decision(
        &self,
        url: String,
        body: impl Serialize,
    ) -> Result<DirectoryDecision, AuthError> {
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|err| AuthError::storage(err.to_string()))?;
        if !response.status().is_success() {
            return Err(AuthError::storage(format!(
                "directory admission returned {}",
                response.status()
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|err| AuthError::storage(err.to_string()))?;
        parse_directory_http_body(&bytes).map_err(AuthError::storage)
    }
}

#[async_trait]
impl DirectoryAdmission for HttpDirectory {
    async fn admit(&self, query: AdmitQuery) -> Result<Admission, AuthError> {
        let decision = self
            .decision(
                self.url("/internal/auth/admit"),
                DirectoryAdmitRequest {
                    plane: query.plane.as_str().to_owned(),
                    identifier: query.identifier,
                    org_id: query.org_id,
                },
            )
            .await?;
        Ok(Admission {
            subject_id: decision.subject_id,
            org_id: decision.org_id,
            active: decision.active,
            console_login: decision.console_login,
        })
    }

    async fn subject_status(
        &self,
        plane: Plane,
        subject_id: Uuid,
    ) -> Result<SubjectStatus, AuthError> {
        let decision = self
            .decision(
                self.url(&format!("/internal/auth/subjects/{subject_id}/status")),
                DirectoryStatusRequest {
                    plane: plane.as_str().to_owned(),
                },
            )
            .await?;
        Ok(SubjectStatus {
            active: decision.active,
            console_login: decision.console_login,
            org_id: decision.org_id,
        })
    }
}
