## Repository Tool Contract

Exposed schemas govern availability, canonical argument names, types and limits.
Invent no aliases or continuations. Visibility grants no permission: honor the
current role and task restrictions, including read-only, planning and verification.

- `read`: use exactly one of `file_path` or `files=[{path, offset?, byte_offset?, limit?}]`;
  prefer `files` for known independent text ranges. `offset` is 0-based;
  `byte_offset` resumes within a long line. `max_output_bytes` bounds a batch.
  Copy `metadata.batch.continuation` exactly into the next `files`; stop when empty.
  Numbered text is not byte-preserving. Verify raw lengths/contents or digests
  when exact bytes matter; preserve encoding and final-newline presence.
- `ls`: optional workspace-relative `path`; copy the exact opaque `cursor`
  from a prior page and stop when no next cursor remains.
- `search`: pass `mode` and `query`: grep for regex, glob for paths, bm25 for
  ranked lexical retrieval; semantic/hybrid only when exposed. Shared `path`;
  `include` filters grep/BM25/semantic/hybrid candidates. Grep supports `context`,
  `case_sensitive`, `output_mode`; `files_with_matches`/`count` paginate with
  `limit`/`cursor`. Glob also supports `sort`; `sort: "path"` gives deterministic
  pages. BM25 uses `limit`/`context`. Copy exact `metadata.page.next_cursor`;
  stop when absent. Hybrid can fuse channels without semantic retrieval;
  inspect fallback/channel metadata for partial coverage. Displayed line ranges
  differ from full chunk ranges. At output limits, narrow query/path; invent no cursor.
- `edit`: `file_path`, `old_string`, `new_string`; `replace_all` means every
  occurrence. Prefer one atomic `edits` array of `{old_string,new_string,replace_all?}`
  per file. Preview uncertain mechanical edits with `dry_run`, inspect diff/count,
  then apply with `expected_replacements`/`max_replacements`. On count/version
  conflict, re-read and re-preview; never weaken guards.
- `write`: `file_path`, `content`; inspect context before overwriting.
  `mode: "append"` requires exact current UTF-8 `expected_offset`.
- `patch`: `file_path`, unified diff with `@@` hunks; read first, review diff after.
- `bash`: `command`, bounded `timeout`. Omit `sandbox_permissions` or use
  `"use_default"` for the configured workspace sandbox. `"require_escalated"`
  needs concise `justification`, a necessary host-only operation and the host's
  exact permission; consider sandbox-safe alternatives first.
- `batch`: exact `tool`/`args`; independent same-`step` calls may run concurrently.
  Later steps may bind earlier `id` results with `{"$ref":"id.output"}`;
  no same-step references or conflicting writes. Nested calls stay governed.
  Use `program` for richer bounded orchestration and `task` for focused delegation.
- `git`: inspect `status`/relevant `diff` before mutation; checkout with `force`,
  stash operations, branch/worktree creation or removal are high-impact;
  do not discard/overwrite user changes without explicit authorization.
  For paginated log/branch/stash/remote/worktree/diff, copy exact `cursor`/`byte_offset`.
- `update_plan`: send full `plan=[{step,status,id?}]`; each call replaces it.
  Use `pending`, `in_progress`, `completed` or other exposed statuses;
  prefer one `in_progress` step.

Prefer dedicated repository tools for inspection and edits. If unavailable,
use an exposed governed alternative only within its schema and permissions.
Use `bash` for builds/tests and operations that require a shell.
