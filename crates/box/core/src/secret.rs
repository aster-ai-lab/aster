//! Non-sensitive control metadata for transient Secret environment bindings.

use serde::{Deserialize, Serialize};

/// Runtime control key carrying only environment names and guest file paths.
pub const SECRET_ENVIRONMENT_MANIFEST: &str = "ASTER_BOX_SECRET_ENV_V1";

/// Reserved in-guest root for transient Secret files.
pub const SECRET_GUEST_ROOT: &str = "/.aster-box-secrets";

/// Validate one POSIX-style process environment variable name.
pub fn validate_environment_variable_name(variable: &str) -> Result<(), String> {
    let mut bytes = variable.bytes();
    let Some(first) = bytes.next() else {
        return Err("Secret environment variable name must not be empty".into());
    };
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || variable.len() > 255
    {
        return Err("Secret environment variable name is invalid".into());
    }
    Ok(())
}

/// One non-sensitive environment binding consumed by guest-init immediately
/// before it launches the workload process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretEnvironmentBinding {
    pub variable: String,
    pub path: String,
}

impl SecretEnvironmentBinding {
    pub fn validate(&self) -> Result<(), String> {
        validate_environment_variable_name(&self.variable)?;
        let normalized_path = self.path.strip_prefix('/').is_some_and(|relative| {
            !relative.is_empty()
                && relative
                    .split('/')
                    .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
        });
        if !normalized_path
            || self.path.len() > 4096
            || self.path.contains([':', '\0'])
            || self.path.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err("Secret environment file path is invalid".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_binding_accepts_only_canonical_linux_paths_and_names() {
        let binding = SecretEnvironmentBinding {
            variable: "ASTER_PROVIDER_TOKEN".into(),
            path: format!("/.aster-box-secrets/{}/000.secret", "a".repeat(64)),
        };
        binding.validate().unwrap();

        for variable in ["", "9TOKEN", "TOKEN-NAME"] {
            let mut invalid = binding.clone();
            invalid.variable = variable.into();
            assert!(invalid.validate().is_err(), "accepted {variable:?}");
        }

        for path in [
            "relative/000.secret",
            "/.aster-box-secrets//000.secret",
            "/.aster-box-secrets/./000.secret",
            "/.aster-box-secrets/../000.secret",
            "/.aster-box-secrets/value:000.secret",
        ] {
            let mut invalid = binding.clone();
            invalid.path = path.into();
            assert!(invalid.validate().is_err(), "accepted {path:?}");
        }
    }
}
