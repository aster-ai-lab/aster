//! Workspace mutation paths taken from git effects, never from command text.
//!
//! Dirty-line porcelain catches uncommitted writes. A clean checkout can change
//! tracked files without leaving porcelain dirty, so a changed HEAD also
//! contributes `git diff --name-only` between the two revisions when this
//! command ran a HEAD-moving git subcommand. A parallel commit in the same
//! worktree moves HEAD while this command only reads git; those paths belong
//! to that commit.

mod identity;

pub(crate) use identity::observe_file_identity;
use identity::*;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
#[cfg(any(unix, windows, test))]
use std::io::Read;
use std::path::Path;
use std::time::Duration;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::{directory_entries, node_stamp, observe_identity_windows};

/// `git status -u` on a large worktree takes longer than a few hundred
/// milliseconds. A shorter budget made porcelain return nothing, and the
/// file walk then hit its cap and marked a real git tree incomplete.
/// Five seconds covers that status. A read that still misses inside a
/// worktree stays incomplete and does not walk the tree.
const GIT_QUERY_TIMEOUT: Duration = Duration::from_secs(5);
/// Bound subprocess output before allocating authority baselines. Overflow is
/// an unreadable query, never a successful prefix of the repository state.
const GIT_OUTPUT_BYTE_CAP: u64 = 4 * 1024 * 1024;

/// Identity of a path already named by porcelain. A later write, mode change, or
/// type change that keeps the same status line is still a mutation. This is
/// not a second effect identity and not a full-tree hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ContentStamp {
    pub path: String,
    pub present: bool,
    pub len: u64,
    pub modified: u128,
    #[serde(default)]
    pub changed: u128,
    #[serde(default)]
    pub node: (u64, u64),
    #[serde(default)]
    pub mode: u32,
    #[serde(default)]
    pub kind: u8,
    #[serde(default)]
    pub content_digest: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Snapshot {
    porcelain: Option<Vec<String>>,
    head: Option<String>,
    stamps: Vec<ContentStamp>,
    /// Stamps do not cover every dirty path. A stamp budget or a failed
    /// identity read sets this. It is not, by itself, a missed status list.
    stamps_incomplete: bool,
    /// Status or the hidden-index query failed. A list that only exceeded
    /// the stamp budget stays false: the status diff is still the command delta.
    #[serde(default)]
    stamp_read_failed: bool,
    /// The hidden-index list exceeded its bound, so unnamed writes are possible.
    hidden_index_budget: bool,
    /// Status hit its record or byte cap. The prefix is not a diff input.
    #[serde(default)]
    status_budget: bool,
    /// This directory is a git worktree, including when `status` itself
    /// failed. A failed git read must not fall through to the file walk.
    git_worktree: bool,
    /// The parent repository ignores this entire workspace root.
    // shortcut: ignored descendants of an ordinary Git root need a separate coverage proof.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    ignored_root: bool,
}

impl Snapshot {
    pub(crate) fn observes_git(&self) -> bool {
        self.git_worktree || self.porcelain.is_some()
    }

    /// Digest, counts, and read flags. The prepared fact is bounded at 1 MiB.
    /// The status list and stamps stay in memory for the command delta.
    fn receipt_summary(&self) -> Value {
        let mut summary = serde_json::json!({
            "head": self.head,
            "git_worktree": self.git_worktree,
            "status_count": self.porcelain.as_ref().map(Vec::len),
            "status_digest": self.porcelain.as_ref().map(|lines| digest_status(lines)),
            "status_complete": self.porcelain.is_some(),
            "status_budget": self.status_budget,
            "stamp_count": self.stamps.len(),
            "unhashed_stamp_count": self.stamps.iter().filter(|stamp| stamp.present && stamp.kind == 0 && stamp.content_digest.is_none()).count(),
            "stamp_digest": digest_stamps(&self.stamps),
            "stamps_incomplete": self.stamps_incomplete,
            "stamp_read_failed": self.stamp_read_failed,
            "hidden_index_budget": self.hidden_index_budget,
        });
        if self.ignored_root {
            summary["ignored_root"] = Value::Bool(true);
        }
        summary
    }
}

/// One before/after observation of a workspace. Git uses porcelain; other
/// roots use a bounded file index. Callers attach the result as
/// `changed_paths` and must not parse command text.
pub(crate) struct Watch {
    snapshot: Snapshot,
    files: Option<FileIndex>,
}

fn digest_status(lines: &[String]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"status\0");
    for line in lines {
        digest.update(line.as_bytes());
        digest.update([0]);
    }
    format!("sha256:{:x}", digest.finalize())
}

fn digest_stamps(stamps: &[ContentStamp]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"stamps\0");
    for stamp in stamps {
        digest.update(serde_json::to_vec(stamp).unwrap_or_default());
        digest.update([0]);
    }
    format!("sha256:{:x}", digest.finalize())
}

impl Watch {
    /// Authority baseline for the prepared fact. A dirty status list is not
    /// copied in: the fact keeps its digest, and the watch keeps the lines.
    pub(crate) fn receipt_baseline(&self) -> Value {
        serde_json::json!({
            "git": self.snapshot.receipt_summary(),
            "nongit": self.files.as_ref().map(FileIndex::summary),
        })
    }

    pub(crate) async fn start(root: &Path) -> Self {
        let snapshot = snapshot(root).await;
        let files = if snapshot.observes_git() && !snapshot.ignored_root {
            None
        } else {
            Some(index_files(root).with_content(root))
        };
        Self { snapshot, files }
    }

    fn child_baseline(&self, task_id: &str) -> ChildMarker {
        // Git cannot reconstruct a file-index baseline after its owner is lost.
        let nongit = self.files.is_some();
        ChildMarker {
            task_id: task_id.to_string(),
            porcelain: if nongit {
                None
            } else {
                self.snapshot.porcelain.clone()
            },
            head: if nongit {
                None
            } else {
                self.snapshot.head.clone()
            },
            nongit,
        }
    }

    pub(crate) async fn finish(self, root: &Path) -> Vec<String> {
        self.finish_with_revision(root, true).await
    }

    /// `attribute_revision` is false when this command did not run a
    /// HEAD-moving git subcommand. A parallel commit's clean paths are then
    /// left off the command delta.
    pub(crate) async fn finish_with_revision(
        self,
        root: &Path,
        attribute_revision: bool,
    ) -> Vec<String> {
        self.finish_observed_with_revision(root, attribute_revision)
            .await
            .paths
    }

    /// Preserve read failures and bounded-walk truncation for durable receipts.
    /// Known paths remain useful even when the observation is incomplete.
    pub(crate) async fn finish_observed_with_revision(
        self,
        root: &Path,
        attribute_revision: bool,
    ) -> PathDelta {
        self.observe_delta(root, attribute_revision).await
    }

    /// Bind after-images to byte identities already captured by this watch.
    /// An uncaptured pre-image stays unknown; no second baseline read is made.
    /// A clean Git path absent from dirty stamps is not evidence of absence.
    pub(crate) async fn finish_effects_with_revision(
        self,
        root: &Path,
        attribute_revision: bool,
    ) -> aster_effect::ToolEffectObservationV1 {
        let delta = self.observe_delta(root, attribute_revision).await;
        let mut effects = observe_path_effects(root, delta);
        let stamps = self
            .files
            .as_ref()
            .and_then(|files| files.content.as_deref())
            .unwrap_or(&self.snapshot.stamps);
        let before: HashMap<&str, &ContentStamp> = stamps
            .iter()
            .map(|stamp| (stamp.path.as_str(), stamp))
            .collect();
        for change in &mut effects.changes {
            change.before = match before.get(change.path.as_str()) {
                Some(stamp) => tool_file_identity(stamp),
                None if self.files.as_ref().is_some_and(|files| {
                    !files.incomplete
                        && files
                            .entries
                            .binary_search_by(|entry| entry.path.cmp(&change.path))
                            .is_err()
                }) =>
                {
                    Some(aster_effect::ToolFileIdentityV1 {
                        kind: aster_effect::ToolFileKind::Missing,
                        content_digest: None,
                        mode: None,
                    })
                }
                None => None,
            };
        }
        if effects.exceeds_canonical_bound() {
            return if effects.completeness == aster_effect::ToolObservationCompleteness::Incomplete
                && !effects.budget_exceeded
            {
                aster_effect::ToolEffectObservationV1::incomplete(
                    "tool_effect_observation_incomplete: pre-image identity or canonical size bound failed",
                )
            } else {
                aster_effect::ToolEffectObservationV1::budget_exceeded()
            };
        }
        if effects.validate().is_err() {
            return aster_effect::ToolEffectObservationV1::incomplete(
                "tool_effect_observation_incomplete: pre-image identity or canonical size bound failed",
            );
        }
        effects
    }

    async fn observe_delta(&self, root: &Path, attribute_revision: bool) -> PathDelta {
        let mut observed = command_delta(root, &self.snapshot, attribute_revision).await;
        if let Some(files) = self.files.as_ref() {
            let files = file_delta(root, files);
            if files.incomplete {
                observed.incomplete = true;
                observed.budget_exceeded = false;
            } else if files.budget_exceeded {
                observed.budget_exceeded = true;
                observed.paths.clear();
            } else {
                observed.paths.extend(files.paths);
            }
        }
        observed.paths.sort();
        observed.paths.dedup();
        observed
    }
}

/// Paths the gate can bind, or an explicit failure to re-read the tree.
/// An incomplete read is not an empty mutation and is not a smaller digest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PathDelta {
    pub paths: Vec<String>,
    pub incomplete: bool,
    /// A known cap was hit. Distinct from [`Self::incomplete`], which is a
    /// failed read. The run scan owns that gap. It is not an invocation debt.
    pub budget_exceeded: bool,
    /// Whole-workspace coverage hit a status, stamp, or hidden-index cap.
    /// A revision-name list alone does not set this coverage gap.
    pub status_budget: bool,
}

/// The one typed byte observer used by foreground and background receipts.
/// An unsafe or failed read stays Unknown, never an invented Missing identity.
pub(crate) fn observe_path_effects(
    root: &Path,
    delta: PathDelta,
) -> aster_effect::ToolEffectObservationV1 {
    observe_path_effects_within(root, delta, CHANGED_PATH_BYTE_CAP)
}

fn observe_path_effects_within(
    root: &Path,
    mut delta: PathDelta,
    mut budget: u64,
) -> aster_effect::ToolEffectObservationV1 {
    use aster_effect::{
        ToolEffectObservationV1, ToolFileIdentityV1, ToolFileKind, ToolObservationCompleteness,
        ToolPathChangeV1,
    };
    let mut invalid_scope = false;
    delta.paths.retain(|path| {
        if path.len() > OBSERVATION_PATH_BYTE_CAP
            || path.contains('\0')
            || Path::new(path).components().any(|component| {
                !matches!(
                    component,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
        {
            // A harness-looking escape must not become a complete empty
            // observation merely because metadata paths are normally hidden.
            invalid_scope = true;
            false
        } else {
            !is_harness_path(path)
        }
    });
    delta.incomplete |= invalid_scope;
    delta.paths.sort();
    delta.paths.dedup();
    if !delta.incomplete && delta.paths.len() > aster_effect::MAX_TOOL_EFFECT_PATHS {
        return ToolEffectObservationV1::budget_exceeded();
    }
    let mut effects = ToolEffectObservationV1::complete(Vec::new());
    if delta.paths.len() > aster_effect::MAX_TOOL_EFFECT_PATHS {
        delta.incomplete = true;
        delta.paths.truncate(aster_effect::MAX_TOOL_EFFECT_PATHS);
    }
    for path in delta.paths {
        if path.is_empty() || path.len() > OBSERVATION_PATH_BYTE_CAP {
            delta.incomplete = true;
            continue;
        }
        let after = match observe_file_identity(root, &path, &mut budget)
            .and_then(|stamp| tool_file_identity(&stamp))
        {
            Some(identity) => identity,
            None => {
                delta.incomplete = true;
                ToolFileIdentityV1 {
                    kind: ToolFileKind::Unknown,
                    content_digest: None,
                    mode: None,
                }
            }
        };
        effects.changes.push(ToolPathChangeV1 {
            path,
            before: None,
            after,
        });
    }
    if delta.incomplete {
        effects.completeness = ToolObservationCompleteness::Incomplete;
        effects.reason = Some(
            "tool_effect_observation_incomplete: workspace read or observation budget failed"
                .into(),
        );
        effects.budget_exceeded = false;
    }
    if delta.budget_exceeded && !delta.incomplete {
        effects.completeness = ToolObservationCompleteness::Incomplete;
        effects.budget_exceeded = true;
        effects.reason = Some("tool_effect_observation_incomplete: workspace metadata coverage exceeded its path budget".into());
    }
    if effects.exceeds_canonical_bound() {
        return if delta.incomplete {
            ToolEffectObservationV1::incomplete(
                "tool_effect_observation_incomplete: typed receipt contract or canonical size bound failed",
            )
        } else {
            ToolEffectObservationV1::budget_exceeded()
        };
    }
    if effects.validate().is_err() {
        return ToolEffectObservationV1::incomplete(
            "tool_effect_observation_incomplete: typed receipt contract or canonical size bound failed",
        );
    }
    effects
}

pub(crate) async fn snapshot(root: &Path) -> Snapshot {
    let status = read_status(root).await;
    let porcelain = status.records;
    let status_budget = status.budget;
    let git_worktree = porcelain.is_some() || status_budget || inside_work_tree(root).await;
    let ignored = if git_worktree {
        workspace_ignored(root).await
    } else {
        Some(false)
    };
    let ignored_root = ignored == Some(true);
    let mut byte_budget = OBSERVATION_BYTE_CAP;
    let mut path_budget = FILE_WALK_CAP;
    let (mut stamps, mut stamps_incomplete, stamp_failed) = match &porcelain {
        Some(lines) => stamp_lines(root, lines, &mut byte_budget, &mut path_budget),
        // Status missed inside a worktree. That is not an empty non-git tree.
        None => (Vec::new(), false, false),
    };
    // A missed status or a broken hidden-index query cannot prove an empty
    // delta. Hitting the stamp budget on a fully read status list cannot
    // either be treated as a missed read: that froze every later mutating
    // call on a worktree that was already dirtier than FILE_WALK_CAP.
    let mut stamp_read_failed = stamp_failed
        || (porcelain.is_none() && !status_budget && git_worktree)
        || ignored.is_none();
    let mut hidden_index_budget = false;
    if porcelain.is_some() {
        match hidden_index_paths(root).await {
            HiddenIndex::Listed(paths, parse_incomplete) => {
                hidden_index_budget |= parse_incomplete && paths.len() == FILE_WALK_CAP;
                stamp_read_failed |= parse_incomplete && !hidden_index_budget;
                stamps_incomplete |= parse_incomplete;
                let (extra, extra_incomplete, extra_failed) =
                    stamp_paths(root, &paths, &mut byte_budget, &mut path_budget);
                stamps_incomplete |= extra_incomplete;
                stamp_read_failed |= extra_failed;
                stamps.extend(extra);
                stamps.sort_by(|left, right| left.path.cmp(&right.path));
                stamps.dedup_by(|left, right| left.path == right.path);
            }
            HiddenIndex::OverCap => {
                stamps_incomplete = true;
                hidden_index_budget = true;
            }
            HiddenIndex::Failed => {
                stamp_read_failed = true;
                stamps_incomplete = true;
            }
        }
    }
    stamps_incomplete |= stamp_read_failed || status_budget;
    Snapshot {
        porcelain,
        head: revision(root).await,
        stamps,
        stamps_incomplete,
        stamp_read_failed,
        hidden_index_budget,
        status_budget,
        git_worktree,
        ignored_root,
    }
}

async fn workspace_ignored(root: &Path) -> Option<bool> {
    let status = tokio::time::timeout(
        GIT_QUERY_TIMEOUT,
        tokio::process::Command::new("git")
            .args(["check-ignore", "--quiet", "--no-index", "."])
            .env("GIT_OPTIONAL_LOCKS", "0")
            .current_dir(root)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .status(),
    )
    .await;
    match status {
        Ok(Ok(status)) => match status.code() {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

async fn inside_work_tree(root: &Path) -> bool {
    let Some(output) = git(root, &["rev-parse", "--is-inside-work-tree"]).await else {
        return false;
    };
    String::from_utf8_lossy(&output).trim() == "true"
}

pub(crate) async fn lines(root: &Path) -> Option<Vec<String>> {
    read_status(root).await.records
}

struct StatusRead {
    records: Option<Vec<String>>,
    budget: bool,
}

/// A complete status list, a known cap, or a failed read. A cap does not
/// keep the parsed prefix: comparing prefixes invents paths.
async fn read_status(root: &Path) -> StatusRead {
    let Some(prefix) = workspace_git_prefix(root).await else {
        return StatusRead {
            records: None,
            budget: false,
        };
    };
    match git_capture(root, &["status", "--porcelain", "-z", "-u", "--", "."]).await {
        GitCapture::Failed => StatusRead {
            records: None,
            budget: false,
        },
        GitCapture::OverCap => StatusRead {
            records: None,
            budget: true,
        },
        GitCapture::Bytes(output) => {
            let parsed = porcelain_records(&output);
            if parsed.rejected {
                StatusRead {
                    records: None,
                    budget: false,
                }
            } else if parsed.budget {
                StatusRead {
                    records: None,
                    budget: true,
                }
            } else {
                StatusRead {
                    records: scope_porcelain_records(parsed.records, &prefix),
                    budget: false,
                }
            }
        }
    }
}

/// Porcelain always names paths relative to the repository top, even when
/// invoked from a nested workspace. Keep the exact prefix from this cwd;
/// never join a repository-relative path onto the smaller workspace root.
async fn workspace_git_prefix(root: &Path) -> Option<String> {
    let output = git(root, &["rev-parse", "--show-prefix"]).await?;
    let prefix = std::str::from_utf8(&output).ok()?.strip_suffix('\n')?;
    if prefix.len() > OBSERVATION_PATH_BYTE_CAP
        || prefix.contains('\n')
        || (!prefix.is_empty() && !prefix.ends_with('/'))
    {
        return None;
    }
    Some(prefix.to_string())
}

fn bind_git_path<'a>(path: &'a str, prefix: &str) -> Result<Option<&'a str>, ()> {
    let Some(relative) = path.strip_prefix(prefix) else {
        return Ok(None);
    };
    if relative.is_empty()
        || relative.len() > OBSERVATION_PATH_BYTE_CAP
        || Path::new(relative)
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(());
    }
    Ok(Some(relative))
}

fn scope_porcelain_records(records: Vec<String>, prefix: &str) -> Option<Vec<String>> {
    let mut scoped = Vec::new();
    for record in records {
        let status = record.get(..2)?;
        for path in paths_of(&record)? {
            if let Some(relative) = bind_git_path(&path, prefix).ok()? {
                scoped.push(format!("{status} {}", quote_git_path(relative)));
            }
        }
    }
    Some(scoped)
}

pub(crate) async fn delta(root: &Path, before: Snapshot) -> PathDelta {
    command_delta(root, &before, true).await
}

async fn command_delta(root: &Path, before: &Snapshot, attribute_revision: bool) -> PathDelta {
    let after = snapshot(root).await;
    if before.status_budget || after.status_budget {
        let failed = before.stamp_read_failed || after.stamp_read_failed;
        return PathDelta {
            paths: Vec::new(),
            incomplete: failed,
            budget_exceeded: !failed,
            status_budget: !failed,
        };
    }
    let names = match (&before.head, &after.head) {
        (Some(before_head), Some(after_head)) if before_head != after_head => {
            names_between(root, before_head, after_head).await
        }
        _ => RevisionNames::Listed(Vec::new()),
    };
    // A failed revision read stays incomplete. An unread move is not "no paths".
    // A name list past the stamp cap is a budget, not a missed read.
    let names_for_combine = if attribute_revision || matches!(names, RevisionNames::Unread) {
        names.clone()
    } else {
        RevisionNames::Listed(Vec::new())
    };
    let listed = match &names_for_combine {
        RevisionNames::Listed(names) => Some(names.clone()),
        RevisionNames::Budget => Some(Vec::new()),
        RevisionNames::Unread => None,
    };
    let mut observed = combine_delta(
        before.porcelain.as_deref(),
        after.porcelain.as_deref(),
        before.head.as_deref(),
        after.head.as_deref(),
        listed,
    );
    if matches!(names, RevisionNames::Budget) {
        observed.budget_exceeded = true;
    }
    observed.incomplete |= before.stamp_read_failed
        || after.stamp_read_failed
        || before.ignored_root != after.ignored_root;
    // A stable status line needs both metadata stamps. New status paths are
    // already candidates; an unstamped existing dirty path is a real gap.
    let mut coverage_budget = before.hidden_index_budget || after.hidden_index_budget;
    if before.stamps_incomplete || after.stamps_incomplete {
        let before_stamps: HashSet<_> = before
            .stamps
            .iter()
            .map(|stamp| stamp.path.as_str())
            .collect();
        let after_stamps: HashSet<_> = after
            .stamps
            .iter()
            .map(|stamp| stamp.path.as_str())
            .collect();
        if let (Some(old), Some(new)) = (
            before.porcelain.as_deref().and_then(porcelain_paths),
            after.porcelain.as_deref().and_then(porcelain_paths),
        ) {
            let old: HashSet<_> = old.into_iter().collect();
            coverage_budget |= new.iter().any(|path| {
                !is_harness_path(path)
                    && old.contains(path)
                    && (!before_stamps.contains(path.as_str())
                        || !after_stamps.contains(path.as_str()))
            });
        }
    }
    if observed.incomplete {
        observed.budget_exceeded = false;
    } else if coverage_budget {
        observed.budget_exceeded = true;
    }
    observed
        .paths
        .extend(content_changes(&before.stamps, &after.stamps));
    if !attribute_revision && !observed.incomplete {
        if let (RevisionNames::Listed(names), Some(lines)) = (&names, after.porcelain.as_deref()) {
            if let Some(still_dirty) = porcelain_paths(lines) {
                drop_foreign_revision(&mut observed.paths, names, &still_dirty);
            }
        }
    }
    observed.paths.sort();
    observed.paths.dedup();
    observed
}

/// Paths a parallel commit made clean are that commit's effect. A path this
/// command left dirty stays, even if the same name is in the revision diff.
fn drop_foreign_revision(
    paths: &mut Vec<String>,
    revision_paths: &[String],
    still_dirty: &[String],
) {
    if revision_paths.is_empty() {
        return;
    }
    paths.retain(|path| {
        let from_revision = revision_paths.iter().any(|name| name == path);
        !from_revision || still_dirty.iter().any(|dirty| dirty == path)
    });
}

fn porcelain_paths(lines: &[String]) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    for line in lines {
        paths.extend(paths_of(line)?);
    }
    paths.sort();
    paths.dedup();
    Some(paths)
}

/// Join a before/after git read. A missing re-read is incomplete. Every path
/// git already returned is kept; dropping a suffix would let a waiver of the
/// prefix close the gate.
fn combine_delta(
    before_porcelain: Option<&[String]>,
    after_porcelain: Option<&[String]>,
    before_head: Option<&str>,
    after_head: Option<&str>,
    names_between: Option<Vec<String>>,
) -> PathDelta {
    let mut incomplete = false;
    let mut paths = match (before_porcelain, after_porcelain) {
        (Some(before_lines), Some(after_lines)) => {
            let parsed = diff_porcelain(before_lines, after_lines);
            incomplete |= parsed.incomplete;
            parsed.paths
        }
        (Some(_), None) => {
            incomplete = true;
            Vec::new()
        }
        _ => Vec::new(),
    };
    match (before_head, after_head) {
        (Some(before_head), Some(after_head)) if before_head != after_head => match names_between {
            Some(names) => paths.extend(names),
            None => incomplete = true,
        },
        (Some(_), None) => incomplete = true,
        _ => {}
    }
    paths.sort();
    paths.dedup();
    PathDelta {
        paths,
        incomplete,
        budget_exceeded: false,
        status_budget: false,
    }
}

#[allow(dead_code)] // bash unit tests compare porcelain lines through this helper
pub(crate) fn changed_paths(before: &[String], after: &[String]) -> Vec<String> {
    diff_porcelain(before, after).paths
}

fn diff_porcelain(before: &[String], after: &[String]) -> PathDelta {
    let before_set: HashSet<&str> = before.iter().map(String::as_str).collect();
    let after_set: HashSet<&str> = after.iter().map(String::as_str).collect();
    let mut paths = Vec::new();
    let mut incomplete = false;
    for line in after
        .iter()
        .filter(|line| !before_set.contains(line.as_str()))
        .chain(
            before
                .iter()
                .filter(|line| !after_set.contains(line.as_str())),
        )
    {
        match paths_of(line) {
            Some(found) => paths.extend(found),
            None => incomplete = true,
        }
    }
    paths.sort();
    paths.dedup();
    PathDelta {
        paths,
        incomplete,
        budget_exceeded: false,
        status_budget: false,
    }
}

pub(crate) const FILE_WALK_CAP: usize = 4_096;
/// Status lines kept from one `git status`. The stamp cap stays smaller.
/// A dirty worktree past FILE_WALK_CAP is still a complete status read as
/// long as it fits here and in the git output byte cap. Past this bound the
/// read is missed, not a silent prefix.
pub(crate) const STATUS_RECORD_CAP: usize = 65_536;
/// Entries a non-git walk may stat. Stat-only, so a dependency tree fits.
const NONGIT_WALK_CAP: usize = 1 << 18;
const OBSERVATION_PATH_BYTE_CAP: usize = 4_096;
/// Bytes a baseline may hash. Baselines run before every mutating call.
pub(crate) const OBSERVATION_BYTE_CAP: u64 = 64 * 1024 * 1024;
/// Bytes one receipt may hash for its changed paths. A changed file without
/// byte identity leaves an obligation that nothing can discharge, so this
/// covers the data files a task writes; beyond it the receipt is incomplete.
const CHANGED_PATH_BYTE_CAP: u64 = 1 << 30;

/// Non-git fallback. Git roots use [`delta`]; this only covers workspaces
/// porcelain cannot see. A truncated walk is incomplete, not an empty
/// mutation: directory order is unstable, so a prefix must not become paths.
/// Like git's index, a node's stat identity is the change detector: ctime
/// moves on every content, mode or link write and no write can set it back,
/// so no bytes are read here. Changed paths get their content identity from
/// [`observe_path_effects`]. Directory errors and unsupported node kinds
/// remain incomplete.
pub(crate) struct FileIndex {
    entries: Vec<NodeStamp>,
    /// A directory could not be read. Distinct from a walk that hit its cap.
    incomplete: bool,
    /// The walk hit [`NONGIT_WALK_CAP`]. The prefix is not a diff input.
    budget: bool,
    /// Before-content of a small tree, kept as receipt recovery evidence.
    content: Option<Vec<ContentStamp>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct NodeStamp {
    pub path: String,
    pub kind: u8,
    pub dev: u64,
    pub ino: u64,
    pub mode: u32,
    pub len: u64,
    pub modified: (i64, i64),
    pub changed: (i64, i64),
}

/// The prepared fact is bounded at 1 MiB; leave room for the rest of it.
const CONTENT_BASELINE_BYTE_CAP: usize = 512 * 1024;

impl FileIndex {
    /// Hash a small tree's bytes once, before the call runs. A larger tree,
    /// or one the byte budget cannot cover, keeps only its stat digest.
    pub(crate) fn with_content(mut self, root: &Path) -> Self {
        if self.entries.len() > FILE_WALK_CAP {
            return self;
        }
        let mut budget = OBSERVATION_BYTE_CAP;
        self.content = self
            .entries
            .iter()
            .map(|entry| observe_file_identity(root, &entry.path, &mut budget))
            .collect();
        self
    }

    fn summary(&self) -> Value {
        if let Some(content) = &self.content {
            let full = serde_json::json!({
                "entries": content,
                "incomplete": self.incomplete,
                "budget_exceeded": self.budget,
            });
            if serde_json::to_vec(&full).is_ok_and(|bytes| bytes.len() <= CONTENT_BASELINE_BYTE_CAP)
            {
                return full;
            }
        }
        let mut digest = Sha256::new();
        for entry in &self.entries {
            digest.update(serde_json::to_vec(entry).unwrap_or_default());
            digest.update(b"\n");
        }
        serde_json::json!({
            "entry_count": self.entries.len(),
            "stat_digest": format!("sha256:{:x}", digest.finalize()),
            "content_omitted": true,
            "incomplete": self.incomplete,
            "budget_exceeded": self.budget,
        })
    }
}

pub(crate) fn index_files(root: &Path) -> FileIndex {
    index_files_within(root, NONGIT_WALK_CAP)
}

fn index_files_within(root: &Path, cap: usize) -> FileIndex {
    let mut entries = Vec::new();
    let mut incomplete = false;
    let mut budget = false;
    let mut visited = 0;
    let mut stack = vec![std::path::PathBuf::new()];
    while let Some(dir) = stack.pop() {
        let Some((found, directory_failed, capped)) = directory_entries(root, &dir, cap - visited)
        else {
            incomplete = true;
            continue;
        };
        incomplete |= directory_failed;
        budget |= capped;
        for (name, stat) in found {
            visited += 1;
            if name == ".git"
                || name == ".aster"
                || name == ".aster"
                || name == ".aster-agent-retrievals"
            {
                continue;
            }
            let path = dir.join(name);
            let Some(stamp) = path
                .to_str()
                .and_then(|relative| node_stamp(relative, &stat))
            else {
                incomplete = true;
                continue;
            };
            if stamp.kind == 1 {
                stack.push(path);
            }
            entries.push(stamp);
        }
        if budget || (visited >= cap && !stack.is_empty()) {
            budget = true;
            break;
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    FileIndex {
        entries,
        incomplete,
        budget,
        content: None,
    }
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast, clippy::useless_conversion)] // libc stat field widths vary by target.
fn node_stamp(relative: &str, stat: &libc::stat) -> Option<NodeStamp> {
    if relative.is_empty() || relative.len() > OBSERVATION_PATH_BYTE_CAP {
        return None;
    }
    let kind = match stat.st_mode & libc::S_IFMT {
        libc::S_IFREG => 0,
        libc::S_IFDIR => 1,
        libc::S_IFLNK => 2,
        _ => return None,
    };
    let (modified_nsec, changed_nsec) = stat_subseconds(stat);
    Some(NodeStamp {
        path: relative.to_string(),
        kind,
        dev: stat.st_dev as u64,
        ino: stat.st_ino as u64,
        mode: stat.st_mode as u32,
        len: u64::try_from(stat.st_size).ok()?,
        modified: (i64::from(stat.st_mtime), i64::try_from(modified_nsec).ok()?),
        changed: (i64::from(stat.st_ctime), i64::try_from(changed_nsec).ok()?),
    })
}

#[cfg(not(any(unix, windows)))]
fn node_stamp(_relative: &str, _stat: &()) -> Option<NodeStamp> {
    None
}

pub(crate) fn file_delta(root: &Path, before: &FileIndex) -> PathDelta {
    let after = index_files(root);
    if before.incomplete || after.incomplete {
        return PathDelta {
            paths: Vec::new(),
            incomplete: true,
            budget_exceeded: false,
            status_budget: false,
        };
    }
    if before.budget || after.budget {
        // Directory order is unstable past the cap. Comparing a prefix would
        // invent paths; returning a complete empty delta would hide a write.
        return PathDelta {
            paths: Vec::new(),
            incomplete: false,
            budget_exceeded: true,
            status_budget: false,
        };
    }
    let before_map: HashMap<&str, &NodeStamp> = before
        .entries
        .iter()
        .map(|stamp| (stamp.path.as_str(), stamp))
        .collect();
    let after_map: HashMap<&str, &NodeStamp> = after
        .entries
        .iter()
        .map(|stamp| (stamp.path.as_str(), stamp))
        .collect();
    let mut paths = Vec::new();
    for (path, stamp) in &after_map {
        if before_map.get(path) != Some(stamp) {
            paths.push((*path).to_string());
        }
    }
    for path in before_map.keys() {
        if !after_map.contains_key(path) {
            paths.push((*path).to_string());
        }
    }
    paths.sort();
    paths.dedup();
    PathDelta {
        paths,
        incomplete: false,
        budget_exceeded: false,
        status_budget: false,
    }
}

/// Baseline taken at run start. Queried again at the gate so a writer that
/// never published a path still opens the same ledger. `.aster` and `.git` are
/// harness metadata, not source mutations.
pub(crate) struct RunWatch {
    snapshot: Snapshot,
    files: Option<FileIndex>,
}

impl RunWatch {
    pub(crate) async fn start(root: &Path) -> Self {
        let snapshot = snapshot(root).await;
        let files = if snapshot.observes_git() && !snapshot.ignored_root {
            None
        } else {
            Some(index_files(root))
        };
        Self { snapshot, files }
    }

    pub(crate) async fn baseline_changes(&self, root: &Path) -> PathDelta {
        let mut observed = command_delta(root, &self.snapshot, true).await;
        if !observed.incomplete && !observed.status_budget {
            // A complete status list is sufficient for path-level baseline
            // observation even when secondary content stamps hit their cap.
            observed.budget_exceeded = false;
        }
        if self.snapshot.ignored_root {
            // The file index owns this root's paths; Git omits ignored files.
            observed.paths.clear();
        }
        observed.paths = source_paths(observed.paths);
        observed
    }

    pub(crate) fn porcelain(&self) -> Option<Vec<String>> {
        self.snapshot.porcelain.clone()
    }

    /// Status itself was not read. A stamp budget on an already-dirty tree
    /// is not this failure.
    pub(crate) fn stamp_read_failed(&self) -> bool {
        self.snapshot.stamp_read_failed
    }

    pub(crate) fn status_budget(&self) -> bool {
        self.snapshot.status_budget
    }

    pub(crate) fn head(&self) -> Option<String> {
        self.snapshot.head.clone()
    }

    pub(crate) fn stamps(&self) -> Vec<ContentStamp> {
        self.snapshot.stamps.clone()
    }

    pub(crate) fn file_changes(&self, root: &Path) -> PathDelta {
        let Some(files) = self.files.as_ref() else {
            return PathDelta::default();
        };
        let mut observed = file_delta(root, files);
        observed.paths = source_paths(observed.paths);
        observed
    }
}

/// Run-level gate input. A path-list cap on one command is not this budget.
/// Status and the non-git walk are. A failed read wins over either budget.
pub(crate) fn fold_run_observation(
    mut observed: PathDelta,
    status_failed: bool,
    status_budget: bool,
    files: Option<PathDelta>,
) -> PathDelta {
    if status_failed {
        observed.incomplete = true;
        observed.budget_exceeded = false;
        observed.status_budget = false;
    } else if status_budget || observed.status_budget {
        // The baseline or the fresh status read hit its cap. Drop any prefix.
        observed.paths.clear();
        observed.incomplete = false;
        observed.budget_exceeded = true;
        observed.status_budget = true;
    } else {
        // A revision-name cap still leaves the status diff usable.
        observed.budget_exceeded = false;
        observed.status_budget = false;
    }
    if !observed.status_budget && observed.paths.is_empty() && !observed.incomplete {
        if let Some(files) = files {
            if files.incomplete {
                observed.incomplete = true;
                observed.budget_exceeded = false;
            } else if files.budget_exceeded {
                observed.paths.clear();
                observed.budget_exceeded = true;
            } else {
                observed.paths = files.paths;
            }
        }
    }
    observed
}

pub(crate) async fn baseline_delta(
    root: &Path,
    porcelain: Option<Vec<String>>,
    head: Option<String>,
    stamps: Vec<ContentStamp>,
) -> PathDelta {
    let mut observed = delta(root, snapshot_from_parts(porcelain, head, stamps)).await;
    observed.paths = source_paths(observed.paths);
    observed
}

/// Paths present on both sides whose stat changed. A stable status line is
/// not proof the file is unchanged.
/// Metadata-only stamps include ctime and node identity. Missing stamps remain
/// an explicit coverage gap in command_delta, never a complete empty delta.
fn content_changes(before: &[ContentStamp], after: &[ContentStamp]) -> Vec<String> {
    let before_map: HashMap<&str, &ContentStamp> = before
        .iter()
        .map(|stamp| (stamp.path.as_str(), stamp))
        .collect();
    let mut paths = Vec::new();
    for stamp in after {
        let Some(previous) = before_map.get(stamp.path.as_str()) else {
            continue;
        };
        if *previous != stamp {
            paths.push(stamp.path.clone());
        }
    }
    paths
}

fn source_paths(mut paths: Vec<String>) -> Vec<String> {
    paths.retain(|path| !is_harness_path(path));
    paths
}

pub(crate) fn is_harness_path(path: &str) -> bool {
    // Separators belong to the host filesystem. A Unix backslash is a literal
    // byte in the name, so `.aster\\file` is not the `.aster` directory.
    // `__pycache__` is Python's own bytecode side effect, not a user edit.
    Path::new(path).components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if name == ".aster"
                    || name == ".aster"
                    || name == ".aster-agent-retrievals"
                    || name == ".git"
                    || name == "__pycache__"
        )
    })
}

const MODEL_CHANGED_PATH_CAP: usize = 32;

pub(crate) fn attach(metadata: &mut Option<Value>, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    let truncated = paths.len() > MODEL_CHANGED_PATH_CAP;
    let shown = if truncated {
        &paths[..MODEL_CHANGED_PATH_CAP]
    } else {
        paths
    };
    let mut value = metadata.take().unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("changed_paths".to_string(), serde_json::json!(shown));
        if truncated {
            object.insert("changed_paths_truncated".to_string(), Value::Bool(true));
        }
    }
    *metadata = Some(value);
}

async fn revision(root: &Path) -> Option<String> {
    let output = git(root, &["rev-parse", "HEAD"]).await?;
    let revision = String::from_utf8_lossy(&output).trim().to_string();
    if revision.is_empty() {
        None
    } else {
        Some(revision)
    }
}

#[derive(Clone)]
enum RevisionNames {
    Listed(Vec<String>),
    Budget,
    Unread,
}

async fn names_between(root: &Path, before: &str, after: &str) -> RevisionNames {
    let Some(prefix) = workspace_git_prefix(root).await else {
        return RevisionNames::Unread;
    };
    let Some(output) = git(
        root,
        &[
            "diff",
            "--name-only",
            "--no-relative",
            "-z",
            before,
            after,
            "--",
            ".",
        ],
    )
    .await
    else {
        return RevisionNames::Unread;
    };
    let mut names = Vec::new();
    for name in output
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let Ok(name) = std::str::from_utf8(name) else {
            return RevisionNames::Unread;
        };
        if name.is_empty() || name.contains('\n') || name.len() > OBSERVATION_PATH_BYTE_CAP {
            return RevisionNames::Unread;
        }
        if names.len() == FILE_WALK_CAP {
            return RevisionNames::Budget;
        }
        match bind_git_path(name, &prefix) {
            Ok(Some(relative)) => names.push(relative.to_string()),
            Ok(None) => {}
            Err(()) => return RevisionNames::Unread,
        }
    }
    RevisionNames::Listed(names)
}

async fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    match git_capture(root, args).await {
        GitCapture::Bytes(bytes) => Some(bytes),
        GitCapture::OverCap | GitCapture::Failed => None,
    }
}

#[derive(Debug)]
enum GitCapture {
    Bytes(Vec<u8>),
    OverCap,
    Failed,
}

async fn git_capture(root: &Path, args: &[&str]) -> GitCapture {
    // Dropping the timeout future must kill git. A live `status` otherwise
    // keeps `index.lock` and the next read waits inside its own budget.
    let captured = tokio::time::timeout(GIT_QUERY_TIMEOUT, async {
        let mut child = match tokio::process::Command::new("git")
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .current_dir(root)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => child,
            Err(_) => return GitCapture::Failed,
        };
        let captured = match child.stdout.take() {
            Some(stdout) => read_git_output(stdout).await,
            None => GitCapture::Failed,
        };
        // An over-cap read must not wait for git. The pipe is full, and
        // waiting would turn the cap into a timeout failure.
        if matches!(captured, GitCapture::OverCap) {
            drop(child);
            return GitCapture::OverCap;
        }
        match child.wait().await {
            Ok(status) if status.success() => captured,
            _ => GitCapture::Failed,
        }
    })
    .await;
    match captured {
        Ok(capture) => capture,
        Err(_) => GitCapture::Failed,
    }
}

async fn read_git_output(reader: impl tokio::io::AsyncRead + Unpin) -> GitCapture {
    use tokio::io::AsyncReadExt;
    let mut output = Vec::new();
    match reader
        .take(GIT_OUTPUT_BYTE_CAP + 1)
        .read_to_end(&mut output)
        .await
    {
        Ok(_) if output.len() as u64 <= GIT_OUTPUT_BYTE_CAP => GitCapture::Bytes(output),
        Ok(_) => GitCapture::OverCap,
        Err(_) => GitCapture::Failed,
    }
}

struct PorcelainParse {
    records: Vec<String>,
    budget: bool,
    rejected: bool,
}

/// Status lines from `git status --porcelain -z`. A rename is two NUL fields.
/// An unreadable record is a rejected read, not an absent mutation.
/// Hitting the record cap is a budget and does not keep a usable prefix.
fn porcelain_records(output: &[u8]) -> PorcelainParse {
    let mut records = Vec::new();
    let mut budget = false;
    let mut rejected = false;
    let mut parts = output
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty());
    while let Some(part) = parts.next() {
        if records.len() == STATUS_RECORD_CAP {
            budget = true;
            break;
        }
        if part.len() > OBSERVATION_PATH_BYTE_CAP + 3 {
            rejected = true;
            continue;
        }
        let Ok(text) = std::str::from_utf8(part) else {
            rejected = true;
            continue;
        };
        if text.len() < 4 || part[2] != b' ' {
            rejected = true;
            continue;
        }
        let Some(status) = text.get(..2) else {
            rejected = true;
            continue;
        };
        let Some(path) = text.get(3..) else {
            rejected = true;
            continue;
        };
        let renamed = status.contains('R') || status.contains('C');
        if renamed {
            if records.len() + 2 > STATUS_RECORD_CAP {
                budget = true;
                break;
            }
            let Some(dest) = parts.next() else {
                rejected = true;
                if !path.is_empty() {
                    records.push(format!("{status} {}", quote_git_path(path)));
                }
                continue;
            };
            let Ok(dest) = std::str::from_utf8(dest) else {
                rejected = true;
                continue;
            };
            if dest.is_empty() || dest.len() > OBSERVATION_PATH_BYTE_CAP {
                rejected = true;
            } else {
                records.push(format!("{status} {}", quote_git_path(dest)));
            }
            if !path.is_empty() {
                records.push(format!("D  {}", quote_git_path(path)));
            }
            continue;
        }
        if path.is_empty() {
            rejected = true;
            continue;
        }
        records.push(format!("{status} {}", quote_git_path(path)));
    }
    PorcelainParse {
        records,
        budget,
        rejected,
    }
}

/// NUL porcelain fields are exact names, never C-quoted. Encode them for the
/// existing line representation so quotes, spaces and arrows stay literal.
fn quote_git_path(path: &str) -> String {
    let mut quoted = String::with_capacity(path.len() + 2);
    quoted.push('"');
    for character in path.chars() {
        match character {
            '\\' | '"' => {
                quoted.push('\\');
                quoted.push(character);
            }
            '\0'..='\u{1f}' | '\u{7f}' => {
                quoted.push_str(&format!("\\{:03o}", character as u32));
            }
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

/// Paths named by one porcelain line. `None` means the line could not be
/// decoded, which is not the same as “no path”.
fn paths_of(line: &str) -> Option<Vec<String>> {
    if line.len() < 4 {
        return None;
    }
    let status = line.get(..2)?;
    let body = line.get(3..)?.trim();
    if body.is_empty() {
        return None;
    }
    let pieces = if status.contains('R') || status.contains('C') {
        match split_arrow(body) {
            Some((from, to)) => vec![from, to],
            None => vec![body],
        }
    } else {
        vec![body]
    };
    let mut paths = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let path = decode_git_path(piece)?;
        if path.is_empty() || path.contains('\n') {
            return None;
        }
        paths.push(path);
    }
    Some(paths)
}

fn split_arrow(body: &str) -> Option<(&str, &str)> {
    let bytes = body.as_bytes();
    let mut index = 0;
    let mut quoted = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            quoted = true;
            index += 1;
            continue;
        }
        if bytes.get(index..index + 4) == Some(b" -> ") {
            let from = body[..index].trim();
            let to = body[index + 4..].trim();
            if from.is_empty() || to.is_empty() {
                return None;
            }
            return Some((from, to));
        }
        index += 1;
    }
    None
}

fn decode_git_path(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if !raw.starts_with('"') {
        return Some(raw.to_string());
    }
    if raw.len() < 2 || !raw.ends_with('"') {
        return None;
    }
    let inner = &raw[1..raw.len() - 1];
    let mut bytes = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut encoded = [0; 4];
            bytes.extend(ch.encode_utf8(&mut encoded).as_bytes());
            continue;
        }
        let next = chars.next()?;
        match next {
            '\\' | '"' => bytes.push(next as u8),
            'n' => bytes.push(b'\n'),
            't' => bytes.push(b'\t'),
            'r' => bytes.push(b'\r'),
            'a' => bytes.push(0x07),
            'b' => bytes.push(0x08),
            'f' => bytes.push(0x0c),
            'v' => bytes.push(0x0b),
            digit if digit.is_digit(8) => {
                let mut value = digit as u8 - b'0';
                for _ in 0..2 {
                    let Some(candidate) = chars.clone().next() else {
                        break;
                    };
                    if !candidate.is_digit(8) {
                        break;
                    }
                    chars.next();
                    value = value
                        .saturating_mul(8)
                        .saturating_add(candidate as u8 - b'0');
                }
                bytes.push(value);
            }
            _ => return None,
        }
    }
    String::from_utf8(bytes).ok()
}

/// A background child that shares the parent workspace. The completion gate
/// waits for this slot; it is not a second effect identity.
struct ChildSlot {
    watch: std::sync::Mutex<Option<Watch>>,
    porcelain: std::sync::Mutex<Option<Vec<String>>>,
    head: std::sync::Mutex<Option<String>>,
    nongit: std::sync::atomic::AtomicBool,
    settled: std::sync::Mutex<Option<Vec<String>>>,
    abandoned: std::sync::atomic::AtomicBool,
    notify: tokio::sync::Notify,
    settlement_requested: Option<tokio_util::sync::CancellationToken>,
}

fn child_slots() -> &'static std::sync::Mutex<HashMap<String, std::sync::Arc<ChildSlot>>> {
    static SLOTS: std::sync::OnceLock<
        std::sync::Mutex<HashMap<String, std::sync::Arc<ChildSlot>>>,
    > = std::sync::OnceLock::new();
    SLOTS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn lock_slots() -> std::sync::MutexGuard<'static, HashMap<String, std::sync::Arc<ChildSlot>>> {
    child_slots()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) struct ChildMarker {
    pub task_id: String,
    pub porcelain: Option<Vec<String>>,
    pub head: Option<String>,
    pub nongit: bool,
}

/// Attach a watch that already started, before the writer is spawned.
pub(crate) fn install_workspace_child(task_id: &str, watch: Watch) {
    let marker = watch.child_baseline(task_id);
    lock_slots().insert(
        task_id.to_string(),
        std::sync::Arc::new(ChildSlot {
            watch: std::sync::Mutex::new(Some(watch)),
            porcelain: std::sync::Mutex::new(marker.porcelain),
            head: std::sync::Mutex::new(marker.head),
            nongit: std::sync::atomic::AtomicBool::new(marker.nongit),
            settled: std::sync::Mutex::new(None),
            abandoned: std::sync::atomic::AtomicBool::new(false),
            notify: tokio::sync::Notify::new(),
            settlement_requested: None,
        }),
    );
}

/// Reserve before the child can write. The gate observes this id, not command text.
pub(crate) fn reserve_workspace_child(task_id: &str) {
    reserve_workspace_child_inner(task_id, None);
}

pub(crate) fn reserve_workspace_child_with_settlement(
    task_id: &str,
    settlement_requested: tokio_util::sync::CancellationToken,
) {
    reserve_workspace_child_inner(task_id, Some(settlement_requested));
}

fn reserve_workspace_child_inner(
    task_id: &str,
    settlement_requested: Option<tokio_util::sync::CancellationToken>,
) {
    lock_slots().insert(
        task_id.to_string(),
        std::sync::Arc::new(ChildSlot {
            watch: std::sync::Mutex::new(None),
            porcelain: std::sync::Mutex::new(None),
            head: std::sync::Mutex::new(None),
            nongit: std::sync::atomic::AtomicBool::new(false),
            settled: std::sync::Mutex::new(None),
            abandoned: std::sync::atomic::AtomicBool::new(false),
            notify: tokio::sync::Notify::new(),
            settlement_requested,
        }),
    );
}

/// Ask an explicitly background command to stop before the completion gate
/// waits for its final observation. This live signal is never recovery proof.
pub(crate) fn request_workspace_child_settlement(task_id: &str) {
    if let Some(slot) = lock_slots().get(task_id) {
        if let Some(requested) = &slot.settlement_requested {
            requested.cancel();
        }
    }
}

pub(crate) async fn begin_workspace_child(task_id: &str, root: &Path) {
    let watch = Watch::start(root).await;
    let Some(slot) = lock_slots().get(task_id).cloned() else {
        return;
    };
    let marker = watch.child_baseline(task_id);
    *slot
        .porcelain
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = marker.porcelain;
    *slot
        .head
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = marker.head;
    slot.nongit
        .store(marker.nongit, std::sync::atomic::Ordering::SeqCst);
    *slot
        .watch
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(watch);
}

pub(crate) fn child_marker(task_id: &str) -> Option<ChildMarker> {
    let slot = lock_slots().get(task_id).cloned()?;
    if slot
        .settled
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .is_some()
    {
        return None;
    }
    let porcelain = slot
        .porcelain
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let head = slot
        .head
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let nongit = slot.nongit.load(std::sync::atomic::Ordering::SeqCst);
    Some(ChildMarker {
        task_id: task_id.to_string(),
        porcelain,
        head,
        nongit,
    })
}

pub(crate) fn peek_settled_workspace_child(task_id: &str) -> Option<Vec<String>> {
    lock_slots()
        .get(task_id)?
        .settled
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) fn take_settled_workspace_child(task_id: &str) -> Option<Vec<String>> {
    let mut slots = lock_slots();
    let slot = slots.get(task_id).cloned()?;
    let paths = slot
        .settled
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()?;
    slots.remove(task_id);
    Some(paths)
}

pub(crate) fn workspace_child_pending(task_id: &str) -> bool {
    lock_slots().contains_key(task_id)
}

pub(crate) struct WorkspaceChildGuard {
    task_id: String,
    settled: bool,
}

impl WorkspaceChildGuard {
    pub(crate) fn new(task_id: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            settled: false,
        }
    }

    pub(crate) fn settle_paths(&mut self, paths: Vec<String>) {
        settle_workspace_child_paths(&self.task_id, paths);
        self.settled = true;
    }
}

impl Drop for WorkspaceChildGuard {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        let Some(slot) = lock_slots().get(&self.task_id).cloned() else {
            return;
        };
        slot.abandoned
            .store(true, std::sync::atomic::Ordering::SeqCst);
        slot.notify.notify_waiters();
    }
}

pub(crate) async fn settle_workspace_child_guard(guard: &mut WorkspaceChildGuard, root: &Path) {
    settle_workspace_child(&guard.task_id, root).await;
    guard.settled = true;
}

pub(crate) async fn settle_workspace_child(task_id: &str, root: &Path) {
    let Some(slot) = lock_slots().get(task_id).cloned() else {
        return;
    };
    let watch = slot
        .watch
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    let paths = if let Some(watch) = watch {
        watch.finish(root).await
    } else {
        Vec::new()
    };
    *slot
        .settled
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(paths);
    slot.notify.notify_waiters();
}

/// Settle using the command's existing observation rather than taking a second
/// workspace snapshot. The reserved child remains pending until this call.
pub(crate) fn settle_workspace_child_paths(task_id: &str, paths: Vec<String>) {
    let Some(slot) = lock_slots().get(task_id).cloned() else {
        return;
    };
    slot.watch
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    *slot
        .settled
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(paths);
    slot.notify.notify_waiters();
}

pub(crate) async fn await_workspace_child(
    task_id: &str,
    root: &Path,
    cancel: &tokio_util::sync::CancellationToken,
) -> Option<Vec<String>> {
    loop {
        if let Some(paths) = take_settled_workspace_child(task_id) {
            return Some(paths);
        }
        let Some(slot) = lock_slots().get(task_id).cloned() else {
            return Some(Vec::new());
        };
        if slot.abandoned.load(std::sync::atomic::Ordering::SeqCst) {
            return Some(finish_abandoned_workspace_child(task_id, root).await);
        }
        if cancel.is_cancelled() {
            return None;
        }
        let notified = slot.notify.notified();
        tokio::pin!(notified);
        if let Some(paths) = take_settled_workspace_child(task_id) {
            return Some(paths);
        }
        if slot.abandoned.load(std::sync::atomic::Ordering::SeqCst) {
            return Some(finish_abandoned_workspace_child(task_id, root).await);
        }
        if cancel.is_cancelled() {
            return None;
        }
        tokio::select! {
            _ = &mut notified => {}
            _ = cancel.cancelled() => return None,
        }
    }
}

async fn finish_abandoned_workspace_child(task_id: &str, root: &Path) -> Vec<String> {
    let watch = {
        let slots = lock_slots();
        let Some(slot) = slots.get(task_id).cloned() else {
            return Vec::new();
        };
        let watch = slot
            .watch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        watch
    };
    let paths = if let Some(watch) = watch {
        watch.finish(root).await
    } else {
        Vec::new()
    };
    lock_slots().remove(task_id);
    paths
}

pub(crate) fn snapshot_from_parts(
    porcelain: Option<Vec<String>>,
    head: Option<String>,
    stamps: Vec<ContentStamp>,
) -> Snapshot {
    // A stored head without porcelain means status missed. That is not an
    // empty tree, and it must not be reread as a non-git walk.
    let git_worktree = porcelain.is_some() || head.is_some();
    let stamps_incomplete = porcelain.is_none() && head.is_some();
    Snapshot {
        porcelain,
        head,
        stamps,
        stamps_incomplete,
        stamp_read_failed: stamps_incomplete,
        hidden_index_budget: false,
        status_budget: false,
        git_worktree,
        ignored_root: false,
    }
}

#[cfg(test)]
mod receipt_tests;

#[cfg(all(test, unix))]
mod bounded_stamp_tests;

#[cfg(test)]
mod tests;
