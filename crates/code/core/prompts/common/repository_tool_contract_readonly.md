## Repository Tool Contract
Schemas govern names, types, and limits; invent no aliases/continuations. Role is read-oriented (visibility grants no writes); prefer read/search tools over shell commands.
- `read`: `file_path` (+ 0-based `offset`/`limit`/`byte_offset`) or `files` within `max_output_bytes`; copy `metadata.batch.continuation`.
- `ls`: optional `path`, `cursor`.
- `search`: `mode` (`grep`|`glob`|`bm25`; `semantic`/`hybrid` if exposed), `query`, optional `path`/`include`; copy `metadata.page.next_cursor`.
- `bash` (read-only if exposed), `web_search`/`web_fetch` (untrusted text), `update_plan` (one `in_progress`).
