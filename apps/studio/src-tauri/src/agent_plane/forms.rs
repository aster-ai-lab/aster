//! Host-owned, versioned forms. A lease withdraws only its own waiting request.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use a3s_boot::{BootError, Result, body, controller, param, post};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::question::Question;
use super::turns::TurnService;

const MAX_PENDING: usize = 128;
const MAX_SPEC_BYTES: usize = 65_536;
const MAX_VALUES_BYTES: usize = 65_536;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FormFormat {
    OpenUi,
    Question,
}

impl FormFormat {
    pub fn parse(value: &str) -> std::result::Result<Self, &'static str> {
        match value {
            "openui" => Ok(Self::OpenUi),
            "question" => Ok(Self::Question),
            _ => Err("unknown agent form format"),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenUi => "openui",
            Self::Question => "question",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FormRun {
    pub core_session_id: String,
    pub run_id: String,
    #[serde(deserialize_with = "required_model_turn")]
    pub model_turn: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FormOwner {
    pub session_id: String,
    pub execution_id: String,
    pub attempt_id: u32,
    pub anchor_message_id: String,
    pub acp_session_id: String,
    pub prompt_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_run"
    )]
    pub run: Option<FormRun>,
}

fn present_run<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<FormRun>, D::Error> {
    FormRun::deserialize(deserializer).map(Some)
}

fn required_model_turn<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error> {
    Option::deserialize(deserializer)
}

impl FormOwner {
    fn valid(&self) -> bool {
        self.attempt_id > 0
            && [
                &self.session_id,
                &self.execution_id,
                &self.anchor_message_id,
                &self.acp_session_id,
                &self.prompt_id,
            ]
            .iter()
            .all(|id| valid_identity(id))
            && self.run.as_ref().is_none_or(|run| {
                valid_identity(&run.core_session_id)
                    && valid_identity(&run.run_id)
                    && run
                        .model_turn
                        .is_none_or(|turn| turn > 0 && turn <= MAX_SAFE_INTEGER as u64)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FormBinding {
    pub protocol_version: u32,
    pub request_id: String,
    pub owner: FormOwner,
    pub revision: String,
}

impl FormBinding {
    fn valid(&self) -> bool {
        self.protocol_version == 1
            && hex_identity(&self.request_id)
            && self.owner.valid()
            && self
                .revision
                .strip_prefix("sha256:")
                .is_some_and(hex_identity)
    }
}

pub(super) type FormValues = BTreeMap<String, Value>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FormSubmission {
    pub binding: FormBinding,
    pub values: FormValues,
}

pub(super) fn valid_values(values: &FormValues) -> bool {
    values.len() <= 64
        && values.iter().all(|(name, value)| {
            valid_identity(name)
                && match value {
                    Value::String(text) => text.len() <= 4096,
                    Value::Bool(_) => true,
                    Value::Number(number) => number.as_f64().is_some_and(|number| {
                        number.is_finite() && number.abs() <= MAX_SAFE_INTEGER
                    }),
                    _ => false,
                }
        })
        && serde_json::to_vec(values).is_ok_and(|bytes| bytes.len() <= MAX_VALUES_BYTES)
}

fn valid_identity(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}

fn hex_identity(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

struct PendingForm {
    binding: FormBinding,
    reply: tokio::sync::oneshot::Sender<FormValues>,
    question: Option<Question>,
}

#[derive(Default)]
pub(super) struct FormRegistry {
    pending: Mutex<HashMap<String, PendingForm>>,
}

impl FormRegistry {
    pub(super) fn register(self: &Arc<Self>, owner: FormOwner, spec: &str) -> Option<FormLease> {
        self.register_inner(owner, spec, None)
    }

    pub(super) fn register_question(
        self: &Arc<Self>,
        owner: FormOwner,
        question: Question,
    ) -> Option<(FormLease, String)> {
        let spec = serde_json::to_string(&question).ok()?;
        Some((self.register_inner(owner, &spec, Some(question))?, spec))
    }

    fn register_inner(
        self: &Arc<Self>,
        owner: FormOwner,
        spec: &str,
        question: Option<Question>,
    ) -> Option<FormLease> {
        if !owner.valid() || spec.is_empty() || spec.len() > MAX_SPEC_BYTES {
            return None;
        }
        let mut random = [0u8; 32];
        getrandom::getrandom(&mut random).ok()?;
        self.register_binding_with_question(
            FormBinding {
                protocol_version: 1,
                request_id: random.iter().map(|byte| format!("{byte:02x}")).collect(),
                owner,
                revision: format!("sha256:{:x}", Sha256::digest(spec.as_bytes())),
            },
            question,
        )
    }

    #[cfg(test)]
    fn register_binding(self: &Arc<Self>, binding: FormBinding) -> Option<FormLease> {
        self.register_binding_with_question(binding, None)
    }

    fn register_binding_with_question(
        self: &Arc<Self>,
        binding: FormBinding,
        question: Option<Question>,
    ) -> Option<FormLease> {
        let mut pending = self.pending.lock().ok()?;
        if !binding.valid()
            || pending.len() >= MAX_PENDING
            || pending.contains_key(&binding.request_id)
        {
            return None;
        }
        let (reply, receiver) = tokio::sync::oneshot::channel();
        pending.insert(
            binding.request_id.clone(),
            PendingForm {
                binding: binding.clone(),
                reply,
                question,
            },
        );
        Some(FormLease {
            registry: Arc::clone(self),
            binding,
            receiver,
        })
    }

    pub(super) fn resolve(&self, binding: &FormBinding, values: FormValues) -> bool {
        if !binding.valid() || !valid_values(&values) {
            return false;
        }
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        if !pending.get(&binding.request_id).is_some_and(|form| {
            form.binding == *binding
                && form
                    .question
                    .as_ref()
                    .is_none_or(|question| question.answer(&values).is_some())
        }) {
            return false;
        }
        pending
            .remove(&binding.request_id)
            .is_some_and(|form| form.reply.send(values).is_ok())
    }

    fn withdraw(&self, binding: &FormBinding) {
        if let Ok(mut pending) = self.pending.lock()
            && pending
                .get(&binding.request_id)
                .is_some_and(|form| form.binding == *binding)
        {
            pending.remove(&binding.request_id);
        }
    }

    pub(super) fn withdraw_session(&self, session_id: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.retain(|_, form| form.binding.owner.session_id != session_id);
        }
    }
}

pub(super) struct FormLease {
    registry: Arc<FormRegistry>,
    pub binding: FormBinding,
    receiver: tokio::sync::oneshot::Receiver<FormValues>,
}

impl FormLease {
    pub(super) async fn receive(
        &mut self,
    ) -> std::result::Result<FormValues, tokio::sync::oneshot::error::RecvError> {
        (&mut self.receiver).await
    }
}

impl Drop for FormLease {
    fn drop(&mut self) {
        self.registry.withdraw(&self.binding);
    }
}

pub struct FormsController {
    pub turns: Arc<TurnService>,
}

#[controller("/api/agent-forms")]
impl FormsController {
    #[post("/{id}")]
    async fn submit(
        &self,
        #[param("id")] id: String,
        #[body] body: FormSubmission,
    ) -> Result<Value> {
        if !valid_values(&body.values) {
            return Err(BootError::BadRequest(
                "bounded scalar values are required".into(),
            ));
        }
        let ok = id == body.binding.request_id && self.turns.submit_form(body.binding, body.values);
        Ok(json!({ "protocolVersion": 1, "requestId": id, "ok": ok }))
    }
}

#[cfg(test)]
#[path = "forms_tests.rs"]
mod tests;
