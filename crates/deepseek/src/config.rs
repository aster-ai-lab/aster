//! Model-owned ACL input. Parsing does not load weights or activate a backend.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use aster_acl::{AttributeSchema, Block, Document, ParseLimits, Schema, Value, ValueSchema};
use aster_power::error::{PowerError, Result};

use crate::model::{DeepSeekModel, NativeExecution, H200_DEVICE_COUNT, MAX_CONTEXT_TOKENS};

pub const CONFIG_SCHEMA: &str = "aster.deepseek.native.v1";
const ACL_LIMITS: ParseLimits = ParseLimits {
    max_document_bytes: 16 * 1024,
    max_nesting_depth: 4,
    max_collection_items: 16,
    max_token_bytes: 4096,
    max_diagnostics: 8,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeModelConfig {
    model: DeepSeekModel,
    execution: NativeExecution,
    checkpoint_dir: PathBuf,
    devices: Vec<usize>,
    context_tokens: usize,
}

impl NativeModelConfig {
    pub fn from_acl(input: &str) -> Result<Self> {
        let document = aster_acl::parse_with_limits(input, ACL_LIMITS)
            .map_err(|error| PowerError::Acl(error.to_string()))?;
        let mut schema = Schema::new();
        for name in ["schema", "model", "execution", "checkpoint_dir", "revision"] {
            schema = schema.attribute(name, AttributeSchema::required(ValueSchema::string()));
        }
        schema = schema
            .attribute(
                "devices",
                AttributeSchema::required(ValueSchema::list(ValueSchema::number())),
            )
            .attribute(
                "context_tokens",
                AttributeSchema::required(ValueSchema::number()),
            );
        let report = aster_acl::validate_document_with_limits(&document, &schema, ACL_LIMITS);
        if !report.is_empty() {
            return Err(PowerError::Acl(format!(
                "native model ACL does not satisfy its closed schema: {:?}",
                report.diagnostics
            )));
        }
        let field = |name: &str| -> Result<&Value> {
            document
                .blocks
                .iter()
                .find(|block| block.name == name)
                .and_then(|block| block.attributes.get(name))
                .ok_or_else(|| config_error("native ACL field is missing"))
        };
        let string = |name: &str| -> Result<&str> {
            field(name)?
                .as_str()
                .ok_or_else(|| config_error("native ACL field requires a string"))
        };
        if string("schema")? != CONFIG_SCHEMA {
            return Err(config_error(
                "unsupported native ACL schema; expected aster.deepseek.native.v1",
            ));
        }
        let model = DeepSeekModel::parse(string("model")?)?;
        if string("revision")? != model.revision() {
            return Err(config_error(
                "native checkpoint revision is not the supported immutable revision",
            ));
        }
        let execution = NativeExecution::parse(string("execution")?)?;
        let directory = string("checkpoint_dir")?;
        if directory.is_empty() || directory.contains('\0') {
            return Err(config_error(
                "checkpoint_dir requires a nonempty filesystem path",
            ));
        }
        let Value::List(values) = field("devices")? else {
            return Err(config_error("native devices require a list"));
        };
        let devices = values
            .iter()
            .map(|value| integer(value, 0, H200_DEVICE_COUNT - 1))
            .collect::<Result<Vec<_>>>()?;
        if devices.len() != H200_DEVICE_COUNT
            || devices.iter().copied().collect::<BTreeSet<_>>().len() != H200_DEVICE_COUNT
        {
            return Err(config_error(
                "native devices must list visible CUDA ordinals 0 through 7 exactly once",
            ));
        }
        let context_tokens = integer(field("context_tokens")?, 1, MAX_CONTEXT_TOKENS)?;
        Ok(Self {
            model,
            execution,
            checkpoint_dir: directory.into(),
            devices,
            context_tokens,
        })
    }

    pub fn to_acl(&self) -> String {
        let fields = [
            ("schema", Value::String(CONFIG_SCHEMA.into())),
            ("model", Value::String(self.model.as_str().into())),
            ("execution", Value::String(self.execution.as_str().into())),
            (
                "checkpoint_dir",
                Value::String(self.checkpoint_dir.to_string_lossy().into_owned()),
            ),
            ("revision", Value::String(self.model.revision().into())),
            (
                "devices",
                Value::List(
                    self.devices
                        .iter()
                        .map(|value| Value::Number(*value as f64))
                        .collect(),
                ),
            ),
            ("context_tokens", Value::Number(self.context_tokens as f64)),
        ];
        let document = Document {
            blocks: fields
                .into_iter()
                .map(|(name, value)| Block {
                    name: name.into(),
                    labels: Vec::new(),
                    blocks: Vec::new(),
                    attributes: HashMap::from([(name.into(), value)]),
                })
                .collect(),
        };
        aster_acl::generate(&document)
    }

    pub fn model(&self) -> DeepSeekModel {
        self.model
    }
    pub fn execution(&self) -> NativeExecution {
        self.execution
    }
    pub fn checkpoint_dir(&self) -> &Path {
        &self.checkpoint_dir
    }
    pub fn devices(&self) -> &[usize] {
        &self.devices
    }
    pub fn context_tokens(&self) -> usize {
        self.context_tokens
    }
}

fn integer(value: &Value, minimum: usize, maximum: usize) -> Result<usize> {
    match value.as_number() {
        Some(number)
            if number.is_finite()
                && number.fract() == 0.0
                && number >= minimum as f64
                && number <= maximum as f64 =>
        {
            Ok(number as usize)
        }
        _ => Err(config_error(
            "native numeric field must be an integer within its declared bound",
        )),
    }
}

fn config_error(message: &str) -> PowerError {
    PowerError::Config(message.to_owned())
}

#[cfg(test)]
mod tests;
