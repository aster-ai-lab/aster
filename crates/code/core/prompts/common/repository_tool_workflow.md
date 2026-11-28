## Repository Tool Contract
Prefer repository tools. Follow exposed schemas and explicit tool limits. For known paths, use the allowed file operation directly. Discover unknown paths.
- `read` text/images. Follow `metadata.batch.continuation` until empty. Verify exact bytes/encoding when needed.
- Page `ls` with `cursor` and `search` with `metadata.page.next_cursor`. Copy cursors exactly. Use `sort: "path"` for glob path order.
- For needed plans, send all 2–8 steps to `update_plan` with one `in_progress`.
- Find sources with `web_search` and read them with `web_fetch` when exposed.
