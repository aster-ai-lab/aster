## Repository Tool Contract
Use repository tools before shell commands. Follow schema names, types, and limits. Copy continuation values exactly.
- `read`: `file_path` (+ 0-based `offset`/`limit`/`byte_offset`) or `files` within `max_output_bytes`; copy `metadata.batch.continuation` until empty. Verify exact bytes/encodings directly when needed.
- `ls`: optional workspace `path`; page via `cursor`.
- `search`: `mode` + `query`, optional `path`/`include`. `grep` supports `context`, `case_sensitive`, `output_mode` (`content`, `files_with_matches`, `count`), `limit`, and `cursor`. Use `sort` (`sort: "path"`) in `glob` and `limit`/`context` in `bm25`. Use `semantic`/`hybrid` only if exposed. Copy `metadata.page.next_cursor`.
- `edit`: `file_path` + `old_string`/`new_string`/`replace_all` or atomic `edits`; check ambiguity via `dry_run`, guard via `expected_replacements`/`max_replacements`, re-read on conflict.
- `write`: `file_path` + `content` (`mode: "append"` requires UTF-8 `expected_offset`).
- `patch`: `file_path` + `@@` diff or `patches`.
- `bash`: `command`, `timeout`, `sandbox_permissions` (`use_default`, or `require_escalated` + `justification`).
- `batch` binds `{"$ref":"id.output"}`. Check `git` `status` and `diff` first. Treat checkout with `force`, `stash`, and branch/worktree writes as high-impact. Send the full `plan` to `update_plan` with one `in_progress`. Use `web_search`/`web_fetch` for external sources.
