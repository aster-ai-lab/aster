//! Local account sources surfaced in the `/config` opt-in UI (`import_accounts`).
//!
//! Mirrors `aster_core::config::AccountImport` without linking `aster-core` into
//! the pager binary. Parity with `aster_core` is verified in dev tests.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum AccountImport {
    Grok,
    Cursor,
    Codex,
    Quark,
}

impl AccountImport {
    // `/config` renders these three UI checkboxes while `Quark` stays parseable
    // for round-trip parity with `aster_core::config::AccountImport`.
    pub(crate) const ALL: [Self; 3] = [Self::Grok, Self::Cursor, Self::Codex];

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "grok" => Ok(Self::Grok),
            "cursor" => Ok(Self::Cursor),
            "codex" => Ok(Self::Codex),
            "quark" => Ok(Self::Quark),
            other => Err(format!(
                "unknown account import {other:?} (closed set: grok, cursor, codex, quark)"
            )),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Grok => "grok",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::Quark => "quark",
        }
    }
}
