#![cfg(test)]

use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    EmptyConnection,
    IncompleteRequest,
    ReadFailed,
    HeaderBudget,
    Target,
    Framing,
    Unauthorized,
    LengthRequired,
    BodyBudget,
    BodyJson,
    ModelOutputBudget,
    RequestBudget,
}

impl Reason {
    pub const ALL: [Self; 12] = [
        Self::EmptyConnection,
        Self::IncompleteRequest,
        Self::ReadFailed,
        Self::HeaderBudget,
        Self::Target,
        Self::Framing,
        Self::Unauthorized,
        Self::LengthRequired,
        Self::BodyBudget,
        Self::BodyJson,
        Self::ModelOutputBudget,
        Self::RequestBudget,
    ];

    pub fn parse(value: &str) -> Result<Self, &'static str> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
            .ok_or("unknown relay rejection reason; expected the closed relay admission reason set")
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmptyConnection => "empty_connection",
            Self::IncompleteRequest => "incomplete_request",
            Self::ReadFailed => "read_failed",
            Self::HeaderBudget => "header_budget",
            Self::Target => "target",
            Self::Framing => "framing",
            Self::Unauthorized => "unauthorized",
            Self::LengthRequired => "length_required",
            Self::BodyBudget => "body_budget",
            Self::BodyJson => "body_json",
            Self::ModelOutputBudget => "model_output_budget",
            Self::RequestBudget => "request_budget",
        }
    }

    pub fn http_status(self) -> u16 {
        match self {
            Self::Unauthorized => 401,
            Self::Target => 404,
            Self::LengthRequired => 411,
            Self::HeaderBudget | Self::BodyBudget => 413,
            Self::RequestBudget => 429,
            _ => 400,
        }
    }
}
