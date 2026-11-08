//! Node verification adapted from mise v2026.10.25 (MIT); see licenses/mise-MIT.txt.

use std::io::Cursor;

use pgp::composed::{Deserializable, DetachedSignature, SignedPublicKey};
use pgp::packet::Signature;
use pgp::types::{Fingerprint, KeyDetails, KeyId, VerifyingKey};

/// Recoverable failures in Node's release-signature verification.
#[derive(Debug, thiserror::Error)]
pub enum NodeSignatureError {
    #[error("Node signing metadata exceeds its size limit")]
    InputLimit,
    #[error("no trusted Node signing keys are available")]
    NoTrustedKeys,
    #[error("Node release has no detached signature")]
    NoSignature,
    #[error("Node signature verification exceeds its attempt budget")]
    AttemptLimit,
    #[error("Node signature does not match a trusted release key")]
    InvalidSignature,
    #[error("invalid Node signing metadata: {0}")]
    Parse(#[from] pgp::errors::Error),
}

pub(super) fn verify_node(data: &[u8], signature: &[u8]) -> Result<(), NodeSignatureError> {
    if data.len() > 1024 * 1024 || signature.len() > 64 * 1024 {
        return Err(NodeSignatureError::InputLimit);
    }
    let keys = public_keys(include_str!("node.asc"))?;
    if keys.is_empty() {
        return Err(NodeSignatureError::NoTrustedKeys);
    }
    let signatures = signatures(signature)?;
    if signatures.is_empty() {
        return Err(NodeSignatureError::NoSignature);
    }
    let mut attempts = keys.iter().map(|key| 1 + key.public_subkeys.len()).sum();
    for signature in &signatures {
        if against_keys(signature, &keys, data, &mut attempts, true)? {
            return Ok(());
        }
    }
    for signature in signatures
        .iter()
        .filter(|signature| !has_issuer(&signature.signature))
    {
        if against_keys(signature, &keys, data, &mut attempts, false)? {
            return Ok(());
        }
    }
    Err(NodeSignatureError::InvalidSignature)
}

fn against_keys(
    signature: &DetachedSignature,
    keys: &[SignedPublicKey],
    data: &[u8],
    attempts: &mut usize,
    require_issuer: bool,
) -> Result<bool, NodeSignatureError> {
    for key in keys {
        if (!require_issuer
            || issuer_matches(
                &signature.signature,
                &key.fingerprint(),
                &key.legacy_key_id(),
            ))
            && verify_candidate(signature, key, data, attempts)?
        {
            return Ok(true);
        }
        for subkey in &key.public_subkeys {
            if (!require_issuer
                || issuer_matches(
                    &signature.signature,
                    &subkey.fingerprint(),
                    &subkey.legacy_key_id(),
                ))
                && verify_candidate(signature, subkey, data, attempts)?
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn verify_candidate(
    signature: &DetachedSignature,
    key: &impl VerifyingKey,
    data: &[u8],
    attempts: &mut usize,
) -> Result<bool, NodeSignatureError> {
    *attempts = attempts
        .checked_sub(1)
        .ok_or(NodeSignatureError::AttemptLimit)?;
    Ok(signature.signature.verify(key, Cursor::new(data)).is_ok())
}

fn issuer_matches(signature: &Signature, fingerprint: &Fingerprint, key_id: &KeyId) -> bool {
    signature
        .issuer_fingerprint()
        .into_iter()
        .any(|issuer| issuer == fingerprint)
        || signature
            .issuer_key_id()
            .into_iter()
            .any(|issuer| issuer == key_id)
}

fn has_issuer(signature: &Signature) -> bool {
    !signature.issuer_fingerprint().is_empty() || !signature.issuer_key_id().is_empty()
}

fn public_keys(input: &str) -> Result<Vec<SignedPublicKey>, NodeSignatureError> {
    let begin = "-----BEGIN PGP PUBLIC KEY BLOCK-----";
    let end = "-----END PGP PUBLIC KEY BLOCK-----";
    let mut blocks = Vec::new();
    let mut current = None;
    for line in input.lines() {
        let line = line.trim_start_matches('\u{feff}');
        if line.trim() == begin {
            current = Some(String::new());
        }
        if let Some(block) = current.as_mut() {
            block.push_str(line);
            block.push('\n');
        }
        if line.trim() == end {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
        }
    }
    let mut keys = Vec::new();
    for block in blocks {
        let (parsed, _) = SignedPublicKey::from_string_many(&block)?;
        for key in parsed {
            keys.push(key?);
        }
    }
    Ok(keys)
}

fn signatures(data: &[u8]) -> Result<Vec<DetachedSignature>, NodeSignatureError> {
    let armored = data
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|start| {
            data.get(start..)
                .is_some_and(|rest| rest.starts_with(b"-----BEGIN"))
        });
    if armored {
        let (parsed, _) = DetachedSignature::from_armor_many(data)?;
        parsed
            .collect::<Result<Vec<_>, _>>()
            .map_err(NodeSignatureError::Parse)
    } else {
        DetachedSignature::from_bytes_many(data)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(NodeSignatureError::Parse)
    }
}

#[cfg(test)]
mod tests;
