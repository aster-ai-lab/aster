//! Frontend replay annotations are not part of an inference message.
//!
//! These exact keys are already treated as non-semantic replay annotations
//! by prefix affinity. Normalize them once before any supplier is selected.
use serde_json::{Map, Value};

const MESSAGE_ANNOTATIONS: &[&str] = &[
    "agent",
    "conversationRequestId",
    "startsNewUserRequest",
    "messageId",
    "model",
    "requestModelId",
    "requestModelName",
    "stepSeq",
    "traceId",
    "argumentsDisplayText",
    "rawUsage",
    "usage",
    "queuePosition",
    "queueTotal",
];

pub(super) fn normalize(document: &mut Map<String, Value>) -> bool {
    let Some(Value::Array(messages)) = document.get_mut("messages") else {
        return false;
    };
    let mut changed = false;
    for message in messages {
        let Some(message) = message.as_object_mut() else {
            continue;
        };
        for key in MESSAGE_ANNOTATIONS {
            changed |= message.remove(*key).is_some();
        }
    }
    changed
}

#[cfg(test)]
#[path = "openai_message_annotations_tests.rs"]
mod tests;
