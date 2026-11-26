## Repository Tool Contract
Schemas govern names, types, and limits; invent no aliases/continuations and honor current role and task restrictions. Prefer repo tools over shell commands.
- `read`: `file_path` (+ 0-based `offset`/`limit`/`byte_offset`) or `files` within `max_output_bytes`; copy `metadata.batch.continuation` until empty. Verify exact bytes/encodings directly when needed.
- `ls`: optional workspace `path`; page via `cursor`.
- `search`: `mode` (`grep`|`glob`|`bm25`; `semantic`/`hybrid` if exposed) + `query`, optional `path`/`include`. `grep`: `context`, `case_sensitive`, `output_mode` (`content`|`files_with_matches`|`count`), `limit`/`cursor`; `glob`: `sort` (`sort: "path"`); `bm25`: `limit`/`context`. Page via `metadata.page.next_cursor`.
- `edit`: `file_path` + `old_string`/`new_string`/`replace_all` or atomic `edits`; check ambiguity via `dry_run`, guard via `expected_replacements`/`max_replacements`, re-read on conflict.
- `write`: `file_path` + `content` (`mode: "append"` requires UTF-8 `expected_offset`).
- `patch`: `file_path` + `@@` diff or `patches`.
- `bash`: `command`, `timeout`, `sandbox_permissions` (`use_default`, or `require_escalated` + `justification`).
- `batch`/`program`/`task`: governed concurrent/multi-step calls (`batch` binds `{"$ref":"id.output"}`); check `git` `status`/`diff` first (checkout with `force`, `stash`, branch/worktree writes are high-impact); `update_plan` sends full `plan` with one `in_progress`.
