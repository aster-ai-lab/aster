use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One immutable fact-log generation used by transcript cursors and forks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptSourceCut {
    pub seq: u64,
    pub digest: String,
}

impl From<a3s_effect::LogCut> for TranscriptSourceCut {
    fn from(cut: a3s_effect::LogCut) -> Self {
        Self {
            seq: cut.seq,
            digest: cut.digest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptCursor {
    pub session_id: String,
    pub after_entry_id: String,
    pub source_cut: TranscriptSourceCut,
}

#[derive(Debug, Clone)]
pub struct TranscriptPageRequest {
    pub cursor: Option<TranscriptCursor>,
    pub limit: usize,
}

impl Default for TranscriptPageRequest {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptEntryKind {
    User,
    Assistant,
    Tool,
    Compaction,
    TurnComplete,
    LegacyHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptSourceSpan {
    pub first_seq: u64,
    pub last_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
    pub text: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptToolResult {
    pub tool_call_id: String,
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TranscriptEntryPayload {
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    Tool {
        calls: Vec<TranscriptToolCall>,
        results: Vec<TranscriptToolResult>,
        complete: bool,
    },
    Compaction {
        summary: String,
        kept_lines: usize,
    },
    TurnComplete {
        terminal: String,
        effect_digest: Option<String>,
        forced_stop: Option<String>,
        log_cut: TranscriptSourceCut,
    },
    /// A known historical shape that this closed read model cannot represent
    /// losslessly. Its presence is explicit; it is not rendered as a complete
    /// typed user/assistant/tool entry.
    LegacyHistory {
        source_kind: String,
        fact_key: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display: Option<TranscriptLegacyHistoryDisplay>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptEntry {
    pub entry_id: String,
    pub kind: TranscriptEntryKind,
    pub fact_seq: u64,
    pub source_span: TranscriptSourceSpan,
    pub digest: String,
    pub payload: TranscriptEntryPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptPage {
    pub session_id: String,
    pub entries: Vec<TranscriptEntry>,
    pub next_cursor: Option<TranscriptCursor>,
    pub source_cut: TranscriptSourceCut,
}

pub const LEGACY_STUDIO_MIGRATION_SCHEMA_V1: &str = "a3s.code.legacy-studio-migration.v1";

/// Closed input role for imported Studio product history. This is deliberately
/// not an execution role or an assistant terminal status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyStudioRole {
    User,
    Assistant,
}

/// Only the ordinary Studio text shape is admitted by the v1 migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyStudioEntryKind {
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyStudioAttachmentMetadata {
    pub name: String,
    pub media_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyStudioEntry {
    pub id: String,
    pub kind: LegacyStudioEntryKind,
    pub role: LegacyStudioRole,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<LegacyStudioAttachmentMetadata>,
}

/// One offline, single-writer import. `backup_reference` is an opaque host
/// identifier; Core never interprets it as a filesystem or deletion path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyStudioMigrationRequest {
    pub source_session_id: String,
    pub target_session_id: String,
    pub expected_workspace: PathBuf,
    pub source_digest: String,
    pub backup_reference: String,
    pub entries: Vec<LegacyStudioEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyStudioMigrationCompletion {
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyStudioMigrationReceipt {
    pub schema: String,
    pub source_session_id: String,
    pub target_session_id: String,
    pub imported_count: usize,
    pub unmapped_count: usize,
    pub conflicting_count: usize,
    pub non_executable_count: usize,
    pub source_digest: String,
    pub target_digest: String,
    pub completion: LegacyStudioMigrationCompletion,
    pub backup_reference: String,
}

/// Display-only payload for imported history. `role` controls Studio rendering;
/// it never turns the enclosing LegacyHistory entry into an execution boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptLegacyHistoryDisplay {
    pub source_entry_id: String,
    pub role: LegacyStudioRole,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<LegacyStudioAttachmentMetadata>,
}

/// A non-destructive fork pinned to the current source generation and one
/// completed transcript boundary. The child uses the same workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixForkRequest {
    pub boundary_entry_id: String,
    pub expected_cut: TranscriptSourceCut,
    pub target_session_id: String,
    pub expected_workspace: PathBuf,
}
