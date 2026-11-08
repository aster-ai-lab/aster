//! Bounded text-only adaptation of DeepSeek's MIT-licensed V4.1 encoder.
//! Source revision 1bae0e1088993b9b82de08f59de5be5a34e42170,
//! encoding/encoding.py SHA-256 13231c592e958fb39e2af6001dae69d90e7a36b265702b9b390236fb387599e2.
//! See licenses/DeepSeek-MIT.txt. Tool/image formats require their own adapters.

use aster_power::error::{PowerError, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokenizers::Tokenizer;

const BOS: &str = "<｜begin▁of▁sentence｜>";
const EOS: &str = "<｜end▁of▁sentence｜>";
const USER: &str = "<｜User｜>";
const ASSISTANT: &str = "<｜Assistant｜>";
const SYSTEM: &str = "<｜System｜>";
const MAX_PROMPT_BYTES: usize = 65536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    System,
    User,
    Assistant,
}
impl TextRole {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            _ => Err(invalid("text role requires system, user or assistant")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThinkingMode {
    Chat,
    Thinking,
}
impl ThinkingMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "chat" => Ok(Self::Chat),
            "thinking" => Ok(Self::Thinking),
            _ => Err(invalid("thinking mode requires chat or thinking")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Thinking => "thinking",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextMessage {
    pub role: String,
    pub content: String,
}

/// Matches the official encoder with drop_thinking=true, text roles only,
/// no tools, no context continuation and an explicit numeric effort budget.
pub fn render_text(messages: &[TextMessage], mode: ThinkingMode, effort: u8) -> Result<String> {
    if messages.is_empty() || messages.len() > 128 || !(1..=100).contains(&effort) {
        return Err(invalid(
            "text prompt requires 1..128 messages and effort 1..100",
        ));
    }
    let roles = messages
        .iter()
        .map(|message| TextRole::parse(&message.role))
        .collect::<Result<Vec<_>>>()?;
    let total = messages.iter().try_fold(0_usize, |total, message| {
        total
            .checked_add(message.content.len())
            .ok_or_else(|| invalid("text input bytes overflowed"))
    })?;
    if total > MAX_PROMPT_BYTES {
        return Err(invalid("text prompt exceeds its input byte bound"));
    }
    let last_user = roles
        .iter()
        .enumerate()
        .rfind(|&(index, role)| *role == TextRole::User || (index > 0 && *role == TextRole::System))
        .map(|(index, _)| index);
    let mut output = String::with_capacity(total + messages.len() * 64 + 256);
    output.push_str(BOS);
    for (index, (message, &role)) in messages.iter().zip(&roles).enumerate() {
        if message.content.contains("<｜deepseek_image｜>") {
            return Err(invalid(
                "text profile refuses the explicit image special token",
            ));
        }
        if index == 0 && (mode == ThinkingMode::Thinking || role == TextRole::System) {
            output.push_str(SYSTEM);
        }
        if index == 0 && mode == ThinkingMode::Thinking {
            output.push_str(&format!("Reasoning Effort: {effort} (range 1-100, the higher the value, the more thorough the reasoning)\n\n"));
        }
        match role {
            TextRole::System => {
                if index > 0 {
                    output.push_str(SYSTEM);
                }
                output.push_str(&message.content);
            }
            TextRole::User => {
                output.push_str(USER);
                output.push_str(&message.content);
            }
            TextRole::Assistant => {
                if mode == ThinkingMode::Thinking && last_user.is_none_or(|last| index > last) {
                    output.push_str("</think>");
                }
                output.push_str(&message.content);
                output.push_str(EOS);
            }
        }
        if (index + 1 == messages.len() || roles[index + 1] == TextRole::Assistant)
            && (role == TextRole::User || (role == TextRole::System && index > 0))
        {
            output.push_str(ASSISTANT);
            output.push_str(
                if mode == ThinkingMode::Thinking && last_user.is_some_and(|last| index >= last) {
                    "<think>"
                } else {
                    "</think>"
                },
            );
        }
        if output.len() > MAX_PROMPT_BYTES {
            return Err(invalid("rendered text prompt exceeds its byte bound"));
        }
    }
    Ok(output)
}

/// Verified official tokenizer for actual serving; the compressed Engram map
/// remains a separate model artifact. No generic-tokenizer fallback exists.
pub struct XqTextCodec {
    tokenizer: Tokenizer,
}
impl XqTextCodec {
    pub fn from_official_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty()
            || bytes.len() > crate::tokenizer::MAX_TOKENIZER_BYTES
            || format!("{:x}", Sha256::digest(bytes)) != crate::tokenizer::OFFICIAL_TOKENIZER_SHA256
        {
            return Err(invalid(
                "text tokenizer differs from its pinned bounded official artifact",
            ));
        }
        let tokenizer =
            Tokenizer::from_bytes(bytes).map_err(|_| invalid("invalid official text tokenizer"))?;
        if tokenizer.get_vocab_size(true) != crate::tokenizer::RAW_VOCABULARY
            || tokenizer.token_to_id(BOS) != Some(0)
            || tokenizer.token_to_id(EOS) != Some(1)
        {
            return Err(invalid(
                "official tokenizer vocabulary or BOS/EOS identity changed",
            ));
        }
        Ok(Self { tokenizer })
    }
    pub fn encode(&self, prompt: &str, maximum: usize) -> Result<Vec<u32>> {
        if prompt.is_empty() || prompt.len() > MAX_PROMPT_BYTES || maximum == 0 {
            return Err(invalid(
                "tokenization input exceeds its nonempty byte/token bound",
            ));
        }
        let encoded = self
            .tokenizer
            .encode(prompt, false)
            .map_err(|_| invalid("official tokenization failed"))?;
        let ids = encoded.get_ids();
        if ids.is_empty() || ids.len() > maximum || ids.contains(&129264) {
            return Err(invalid(
                "encoded prompt exceeds the text context or contains image tokens",
            ));
        }
        Ok(ids.to_vec())
    }
    pub fn tokenizer(&self) -> &Tokenizer {
        &self.tokenizer
    }
}

fn invalid(message: &str) -> PowerError {
    PowerError::InvalidRequest(message.into())
}

#[cfg(test)]
#[path = "prompt/tests.rs"]
mod tests;
