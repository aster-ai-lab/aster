//! Pure proof decisions. No IO.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use super::Plane;

pub const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 14;
pub const CHALLENGE_TTL_SECS: i64 = 5 * 60;
pub const ACCESS_TOKEN_TTL_SECS: i64 = 5 * 60;
pub const REFRESH_TTL_SECS: i64 = 60 * 60 * 24 * 14;
pub const AUTH_CODE_TTL_SECS: i64 = 120;
pub const LOCKOUT_FAILURES: u32 = 5;
pub const LOCKOUT_SECS: i64 = 15 * 60;

pub const ATTESTATION_CLAIMS: &[&str] = &["sub", "plane", "org_id", "auth_time", "amr"];
pub const PROTOCOL_CLAIMS: &[&str] = &["iss", "aud", "exp", "iat", "nonce", "azp"];

pub fn token_prefix(plane: Plane) -> &'static str {
    match plane {
        Plane::Member => "aster_ms_",
        Plane::User => "aster_us_",
    }
}

pub fn plane_of_token(token: &str) -> Option<Plane> {
    if token.starts_with(token_prefix(Plane::Member)) {
        Some(Plane::Member)
    } else if token.starts_with(token_prefix(Plane::User)) {
        Some(Plane::User)
    } else {
        None
    }
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn random_token(plane: Plane) -> String {
    format!("{}{}", token_prefix(plane), random_hex(32))
}

pub fn random_secret() -> String {
    random_hex(32)
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut buf);
    hex::encode(buf)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expiry {
    Live,
    Expired,
}

pub fn expiry(expires_at: i64, now: i64) -> Expiry {
    if expires_at <= now {
        Expiry::Expired
    } else {
        Expiry::Live
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockoutSnapshot {
    pub failures: u32,
    pub locked_until: i64,
    /// Version read from the row. The store accepts the write only when it still matches.
    pub version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockoutDecision {
    Open,
    Locked { until: i64 },
}

pub fn lockout_decision(snapshot: Option<LockoutSnapshot>, now: i64) -> LockoutDecision {
    match snapshot {
        Some(snapshot) if snapshot.locked_until > now => LockoutDecision::Locked {
            until: snapshot.locked_until,
        },
        _ => LockoutDecision::Open,
    }
}

pub fn record_failure(snapshot: Option<LockoutSnapshot>, now: i64) -> LockoutSnapshot {
    let version = snapshot.map(|snapshot| snapshot.version).unwrap_or(0);
    let mut failures = snapshot
        .filter(|snapshot| snapshot.locked_until <= now)
        .map(|snapshot| snapshot.failures)
        .unwrap_or(0);
    failures = failures.saturating_add(1);
    let locked_until = if failures >= LOCKOUT_FAILURES {
        now.saturating_add(LOCKOUT_SECS)
    } else {
        0
    };
    LockoutSnapshot {
        failures,
        locked_until,
        version,
    }
}

pub fn record_success(snapshot: Option<LockoutSnapshot>) -> LockoutSnapshot {
    LockoutSnapshot {
        failures: 0,
        locked_until: 0,
        version: snapshot.map(|snapshot| snapshot.version).unwrap_or(0),
    }
}

pub fn pkce_s256(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn pkce_matches(challenge: &str, verifier: &str) -> bool {
    let actual = pkce_s256(verifier);
    if actual.len() != challenge.len() {
        return false;
    }
    subtle::ConstantTimeEq::ct_eq(actual.as_bytes(), challenge.as_bytes()).into()
}

pub fn claims_within_allow_list(keys: &[String]) -> bool {
    !keys.is_empty()
        && keys.iter().all(|key| {
            ATTESTATION_CLAIMS.contains(&key.as_str()) || PROTOCOL_CLAIMS.contains(&key.as_str())
        })
        && keys.iter().any(|key| key == "sub")
        && keys.iter().any(|key| key == "plane")
        && keys.iter().any(|key| key == "auth_time")
        && keys.iter().any(|key| key == "amr")
        && keys
            .iter()
            .all(|key| !key.contains("role") && !key.contains("grant") && key != "permissions")
}

/// RFC 6238 TOTP, HMAC-SHA1, 30-second step, 6 digits.
pub fn totp_code(secret: &[u8], unix_secs: i64) -> u32 {
    let counter = (unix_secs.max(0) as u64) / 30;
    let mut msg = [0u8; 8];
    msg.copy_from_slice(&counter.to_be_bytes());
    let mac = hmac_sha1(secret, &msg);
    let offset = (mac[19] & 0x0f) as usize;
    let bin = u32::from_be_bytes([
        mac[offset] & 0x7f,
        mac[offset + 1],
        mac[offset + 2],
        mac[offset + 3],
    ]);
    bin % 1_000_000
}

pub fn totp_matches(secret: &[u8], unix_secs: i64, code: &str) -> bool {
    let Ok(presented) = code.trim().parse::<u32>() else {
        return false;
    };
    if presented > 999_999 {
        return false;
    }
    for skew in [-1_i64, 0, 1] {
        let at = unix_secs.saturating_add(skew * 30);
        if totp_code(secret, at) == presented {
            return true;
        }
    }
    false
}

fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha1::Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_prefix_rejects_the_other_plane() {
        let member = random_token(Plane::Member);
        let user = random_token(Plane::User);
        assert_eq!(plane_of_token(&member), Some(Plane::Member));
        assert_eq!(plane_of_token(&user), Some(Plane::User));
        assert_ne!(plane_of_token(&member), Some(Plane::User));
    }

    #[test]
    fn lockout_opens_until_the_shared_threshold() {
        let mut snapshot = None;
        for _ in 0..(LOCKOUT_FAILURES - 1) {
            snapshot = Some(record_failure(snapshot, 1_000));
            assert_eq!(lockout_decision(snapshot, 1_000), LockoutDecision::Open);
        }
        snapshot = Some(record_failure(snapshot, 1_000));
        assert!(matches!(
            lockout_decision(snapshot, 1_000),
            LockoutDecision::Locked { .. }
        ));
        assert_eq!(
            lockout_decision(snapshot, 1_000 + LOCKOUT_SECS),
            LockoutDecision::Open
        );
    }

    #[test]
    fn pkce_s256_is_stable_and_rejects_a_different_verifier() {
        let verifier = "abcdefghijklmnopqrstuvwxyz0123456789ABCD";
        let challenge = pkce_s256(verifier);
        assert!(pkce_matches(&challenge, verifier));
        assert!(!pkce_matches(
            &challenge,
            "abcdefghijklmnopqrstuvwxyz0123456789ABCE"
        ));
    }

    #[test]
    fn allow_list_rejects_role_and_grant_claims() {
        assert!(claims_within_allow_list(&[
            "sub".into(),
            "plane".into(),
            "org_id".into(),
            "auth_time".into(),
            "amr".into(),
            "iss".into(),
        ]));
        assert!(!claims_within_allow_list(&[
            "sub".into(),
            "plane".into(),
            "auth_time".into(),
            "amr".into(),
            "roles".into(),
        ]));
        assert!(!claims_within_allow_list(&[
            "sub".into(),
            "plane".into(),
            "auth_time".into(),
            "amr".into(),
            "grant".into(),
        ]));
    }

    #[test]
    fn totp_accepts_the_current_counter() {
        let secret = b"12345678901234567890";
        let now = 1_700_000_000;
        let code = format!("{:06}", totp_code(secret, now));
        assert!(totp_matches(secret, now, &code));
        assert!(!totp_matches(secret, now, "000000") || totp_code(secret, now) == 0);
    }
}
