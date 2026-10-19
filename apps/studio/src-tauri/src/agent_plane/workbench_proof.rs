//! Host-owned turn evidence. Neither model prose nor renderer timing creates proof.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Policy {
    pub review_mode: bool,
    pub artifact_trace: bool,
}

impl Policy {
    pub fn admit_body(body: &Value) -> Result<Self, String> {
        let policy = Self::from_body(body)?;
        if policy.enabled() && body.pointer("/turnContext/agentRuntime").is_some() {
            return Err("This executor cannot provide independent review and native byte provenance. Use the default local agent or disable these options.".into());
        }
        Ok(policy)
    }

    pub fn from_body(body: &Value) -> Result<Self, String> {
        match body.pointer("/turnContext/workbench") {
            Some(value) => {
                serde_json::from_value(value.clone()).map_err(|_| "invalid workbench policy".into())
            }
            None => Ok(Self::default()),
        }
    }

    pub fn enabled(&self) -> bool {
        self.review_mode || self.artifact_trace
    }
}

macro_rules! string_set {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Debug, PartialEq)]
        pub(crate) enum $name { $($variant),+ }
        impl $name {
            pub fn parse(value: &str) -> Result<Self, String> {
                match value { $($wire => Ok(Self::$variant),)+ _ => Err(format!("invalid {}: {value}", stringify!($name))) }
            }
            pub fn as_str(&self) -> &'static str { match self { $(Self::$variant => $wire),+ } }
        }
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> { serializer.serialize_str(self.as_str()) }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

string_set!(ReviewStatus { Off=>"off", Pending=>"pending", Running=>"running", Remediating=>"remediating", OpenQueue=>"open_queue", Completed=>"completed", Failed=>"failed", Cancelled=>"cancelled" });
string_set!(FindingStatus { Pending=>"pending", Addressed=>"addressed", Accepted=>"accepted", Waived=>"waived" });
string_set!(Severity { High=>"high", Warning=>"warning", Info=>"info" });
string_set!(Category { Conclusion=>"conclusion", Citation=>"citation", Change=>"change", Tool=>"tool", Uncertainty=>"uncertainty" });
string_set!(Reviewer { IndependentModel=>"independent-model" });
string_set!(ArtifactStatus { Traced=>"traced", Incomplete=>"incomplete", Failed=>"failed" });
string_set!(ArtifactProcessorStatus { Queued=>"queued", Running=>"running", Succeeded=>"succeeded", Failed=>"failed", Canceled=>"canceled" });
string_set!(ArtifactOriginKind { PaperloomService=>"paperloom-service" });
string_set!(ToolState { Success=>"success", Error=>"error", Cancelled=>"cancelled", Incomplete=>"incomplete" });
string_set!(ProofSchema { V1=>"a3s.studio.workbench-proof.v1" });
string_set!(ProducerStatus { Pending=>"pending", Completed=>"completed", Failed=>"failed", Cancelled=>"cancelled" });
impl Default for ProducerStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Finding {
    pub id: String,
    pub severity: Severity,
    pub category: Category,
    pub claim: String,
    pub evidence_quote: String,
    pub guidance: String,
    pub status: FindingStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Review {
    pub status: ReviewStatus,
    pub findings: Vec<Finding>,
    pub reviewer: Reviewer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ArtifactOrigin {
    pub kind: ArtifactOriginKind,
    pub member_id: String,
    pub workspace_root: String,
    pub source_path: String,
    pub source_revision: String,
    pub source_digest: String,
    pub saved_revision: String,
    pub job_id: String,
    pub job_status: ArtifactProcessorStatus,
    pub attempt: u32,
    pub artifact_key: String,
    pub manifest_digest: String,
    pub connection_revision: String,
    pub package_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ArtifactVersion {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<ArtifactOrigin>,
    pub id: String,
    pub path: String,
    pub name: String,
    pub ordinal: u32,
    pub bytes: u64,
    pub media_type: String,
    pub content_digest: String,
    pub receipt_digest: String,
    pub input_digests: Vec<String>,
    pub workflow_digest: String,
    pub code_digest: String,
    pub environment_digest: String,
    pub environment: String,
    pub execution_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub created_at: String,
    pub status: ArtifactStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ToolEvidence {
    pub id: String,
    pub name: String,
    pub state: ToolState,
    pub input: String,
    pub output: String,
}

pub(crate) struct ArtifactEvidence {
    pub id: String,
    pub text: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewContext {
    pub prompt: String,
    pub components: Vec<String>,
    pub host: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Proof {
    pub schema: ProofSchema,
    pub session_id: String,
    pub message_id: String,
    pub run_id: String,
    #[serde(default)]
    pub producer_status: ProducerStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ReviewContext>,
    pub policy: Policy,
    pub model: String,
    pub source_text: String,
    pub user_text: String,
    pub tools: Vec<ToolEvidence>,
    pub artifacts: Vec<ArtifactVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_error: Option<String>,
    pub review: Review,
}

impl Proof {
    pub fn new(
        session: &str,
        message: &str,
        run: &str,
        model: &str,
        user: &str,
        policy: Policy,
    ) -> Self {
        let status = if policy.review_mode {
            ReviewStatus::Pending
        } else {
            ReviewStatus::Off
        };
        Self {
            schema: ProofSchema::V1,
            session_id: session.into(),
            message_id: message.into(),
            run_id: run.into(),
            producer_status: ProducerStatus::Pending,
            org_id: None,
            context: None,
            policy,
            model: model.into(),
            source_text: String::new(),
            user_text: user.into(),
            tools: Vec::new(),
            artifacts: Vec::new(),
            trace_error: None,
            review: Review {
                status,
                findings: Vec::new(),
                reviewer: Reviewer::IndependentModel,
                started_at: None,
                finished_at: None,
                error: None,
            },
        }
    }

    pub fn from_parts(parts: &[Value]) -> Option<Self> {
        let part = parts
            .iter()
            .find(|part| part["type"] == "data-workbench-proof")?;
        let proof: Self = serde_json::from_value(part.get("data")?.clone()).ok()?;
        Some(proof)
    }
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "workbench_proof_tests.rs"]
mod tests;
