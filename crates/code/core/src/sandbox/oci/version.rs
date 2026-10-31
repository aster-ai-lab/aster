//! Pair the guest-agent hello with the runtime version of the host binary
//! discovery launches.
//!
//! The host rejects a hello that differs from its compiled
//! `CARGO_PKG_VERSION`. That literal is stored immediately before the stable
//! error text. The guest sends the same kind of literal immediately before
//! its architecture name. The manifest's `sources.agent.version` field is not
//! this comparison.

const HOST_MISMATCH: &[u8] = b"guest agent version does not match the host runtime version";

/// Runtime version required by the OCI host binary `discover` launches.
pub(crate) fn host_runtime_version(host: &[u8]) -> Option<&str> {
    version_immediately_before(host, HOST_MISMATCH)
}

/// Hello version the guest agent sends for `architecture` (`aarch64` here).
pub(crate) fn guest_agent_hello<'a>(agent: &'a [u8], architecture: &str) -> Option<&'a str> {
    let architecture = architecture.as_bytes();
    if architecture.is_empty() {
        return None;
    }
    version_immediately_before(agent, architecture)
}

fn version_immediately_before<'a>(bytes: &'a [u8], marker: &[u8]) -> Option<&'a str> {
    if marker.is_empty() {
        return None;
    }
    let mut search = 0;
    while let Some(offset) = bytes[search..]
        .windows(marker.len())
        .position(|window| window == marker)
    {
        let at = search + offset;
        if let Some(version) = semver_ending_at(bytes, at) {
            return Some(version);
        }
        search = at + 1;
    }
    None
}

fn semver_ending_at(bytes: &[u8], end: usize) -> Option<&str> {
    if end == 0 || end > bytes.len() {
        return None;
    }
    let mut start = end;
    while start > 0 && matches!(bytes[start - 1], b'0'..=b'9' | b'.') {
        start -= 1;
    }
    let text = std::str::from_utf8(&bytes[start..end]).ok()?;
    let mut parts = text.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next()?;
    if parts.next().is_some()
        || [major, minor, patch]
            .into_iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::{guest_agent_hello, host_runtime_version};

    #[test]
    fn reads_the_host_requirement_next_to_the_mismatch_error() {
        let host = b"aster-oci 9.9.9\x00prelude 0.3.6guest agent version does not match the host runtime version";
        assert_eq!(host_runtime_version(host), Some("0.3.6"));
    }

    #[test]
    fn reads_the_guest_hello_glued_to_its_architecture() {
        let agent = b"obinbatchisoaarch64untime share0.3.6aarch64linux-guest";
        assert_eq!(guest_agent_hello(agent, "aarch64"), Some("0.3.6"));
    }

    #[test]
    fn a_mismatch_is_not_the_same_version() {
        let host = b"1.2.3guest agent version does not match the host runtime version";
        let agent = b"1.2.4aarch64";
        assert_ne!(
            host_runtime_version(host),
            guest_agent_hello(agent, "aarch64")
        );
    }

    #[test]
    fn rejects_a_marker_that_has_no_semver() {
        assert_eq!(
            host_runtime_version(b"guest agent version does not match the host runtime version"),
            None
        );
        assert_eq!(guest_agent_hello(b"isoaarch64", "aarch64"), None);
        assert_eq!(guest_agent_hello(b"0.3.6aarch64", ""), None);
        assert_eq!(
            host_runtime_version(
                b"0.3.6.1guest agent version does not match the host runtime version"
            ),
            None
        );
    }
}
