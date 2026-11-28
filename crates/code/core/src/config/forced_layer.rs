//! Forced vs preference configuration layers (P1-6).
//!
//! # How forced layers override preference layers
//!
//! Aster already classifies ACL documents as [`LayerAcl::trusted`] (host
//! consented: explicit `--config` / `ASTER_CONFIG` / `~/.aster/config.acl`) or
//! untrusted (workspace-discovered). Only trusted layers may set
//! [`super::HOST_AUTHORITATIVE_BLOCKS`]. After a merged parse,
//! [`super::withhold_untrusted_host_authoritative_blocks`] rewrites those fields
//! from a trusted-only parse so an untrusted preference layer cannot keep a
//! host-authoritative value it attempted to set.
//!
//! Preference keys outside that set may still come from untrusted layers; that
//! is intentional agent configuration, not forced policy.
//!
//! This module does **not** introduce a second JSON sidecar config truth —
//! provenance and digests are over the same ACL layer texts.

use super::{withhold_untrusted_host_authoritative_blocks, CodeConfig, LayerAcl};
use crate::error::CodeError;

/// Queryable provenance snapshot for one layered load.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForcedLayerProvenance {
    /// SHA-256 hex of the concatenated trusted ACL texts (canonical order).
    pub trusted_acl_digest: String,
    /// Host-authoritative block names withheld from untrusted layers.
    pub withheld_host_blocks: Vec<&'static str>,
}

impl ForcedLayerProvenance {
    /// Digest string uses the same `sha256:` prefix convention as capability digests.
    pub fn trusted_digest_prefixed(&self) -> String {
        format!("sha256:{}", self.trusted_acl_digest)
    }
}

/// Apply forced-layer rewrite and return provenance (digest + withheld names).
pub fn apply_forced_layers(config: &mut CodeConfig, layers: &[LayerAcl]) -> ForcedLayerProvenance {
    let trusted_material = layers
        .iter()
        .filter(|layer| layer.trusted)
        .map(|layer| layer.acl.as_str())
        .collect::<Vec<_>>()
        .join("\n---\n");
    let trusted_acl_digest = crate::content_digest::sha256_hex(trusted_material);
    let withheld_host_blocks = withhold_untrusted_host_authoritative_blocks(config, layers);
    ForcedLayerProvenance {
        trusted_acl_digest,
        withheld_host_blocks,
    }
}

/// Parse a trusted-only ACL document; illegal forced values fail closed.
pub fn load_forced_acl(acl: &str) -> Result<CodeConfig, CodeError> {
    CodeConfig::from_acl(acl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SandboxNetworkMode;

    #[test]
    fn illegal_forced_sandbox_value_fails_to_load() {
        let err = load_forced_acl(r#"sandbox { network = "allow-everything" }"#).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("sandbox")
                || message.contains("network")
                || message.contains("Invalid"),
            "{message}"
        );
    }

    #[test]
    fn untrusted_host_block_is_withheld_and_provenance_digest_is_stable() {
        let trusted = LayerAcl {
            trusted: true,
            acl: r#"sandbox { network = "deny" }"#.into(),
        };
        let untrusted = LayerAcl {
            trusted: false,
            acl: r#"sandbox { network = "mediated" network_allow = ["example.com"] }"#.into(),
        };
        let layers = [trusted, untrusted];
        // Start from an untrusted-contaminated merge, then force-rewrite.
        let mut config =
            CodeConfig::from_acl_layers(&[layers[0].acl.as_str(), layers[1].acl.as_str()])
                .unwrap_or_default();
        let first = apply_forced_layers(&mut config, &layers);
        assert_eq!(config.sandbox_network, SandboxNetworkMode::Deny);
        assert!(
            first
                .withheld_host_blocks
                .iter()
                .any(|name| *name == "sandbox"),
            "{:?}",
            first.withheld_host_blocks
        );
        assert!(first.trusted_digest_prefixed().starts_with("sha256:"));

        let mut again = CodeConfig::default();
        let second = apply_forced_layers(&mut again, &layers);
        assert_eq!(first.trusted_acl_digest, second.trusted_acl_digest);
        assert_eq!(first.withheld_host_blocks, second.withheld_host_blocks);
    }
}
