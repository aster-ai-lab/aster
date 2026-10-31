use super::DomainError;

/// JSON text owned by the config aggregate. Parsing stays at the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue(String);

impl ConfigValue {
    pub fn from_text(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigNamespace(String);

impl ConfigNamespace {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        let value = raw.as_ref().trim();
        if !is_valid_namespace(value) {
            return Err(DomainError::validation(
                "namespace must match ^[a-z][a-z0-9_-]{0,63}$",
            ));
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfigKey(String);

impl ConfigKey {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        let value = raw.as_ref().trim();
        if !is_valid_key(value) {
            return Err(DomainError::validation(
                "key must match ^[a-z][a-z0-9._-]{0,127}$",
            ));
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigEntry {
    namespace: ConfigNamespace,
    key: ConfigKey,
    value: ConfigValue,
    revision: i64,
    updated_at_epoch: i64,
    updated_by: String,
}

impl ConfigEntry {
    pub fn new(
        namespace: ConfigNamespace,
        key: ConfigKey,
        value: ConfigValue,
        revision: i64,
        updated_at_epoch: i64,
        updated_by: String,
    ) -> Result<Self, DomainError> {
        if revision <= 0 {
            return Err(DomainError::validation("revision must be positive"));
        }
        if updated_by.len() > 128 {
            return Err(DomainError::validation("updated_by is too long"));
        }
        Ok(Self {
            namespace,
            key,
            value,
            revision,
            updated_at_epoch,
            updated_by,
        })
    }

    pub fn namespace(&self) -> &ConfigNamespace {
        &self.namespace
    }

    pub fn key(&self) -> &ConfigKey {
        &self.key
    }

    pub fn value(&self) -> &ConfigValue {
        &self.value
    }

    pub fn revision(&self) -> i64 {
        self.revision
    }

    pub fn updated_at_epoch(&self) -> i64 {
        self.updated_at_epoch
    }

    pub fn updated_by(&self) -> &str {
        &self.updated_by
    }
}

fn is_valid_namespace(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-')
}

fn is_valid_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..].iter().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-' || *b == b'.'
    })
}
