# Data model and contract

- Reuse stored id/content/tags/importance/memory_type/timestamp/meta exactly.
  Unknown type, invalid timestamp or metadata is a named decode refusal.
- Management request closed set: list(query, optional stored type, cursor),
  read(id), delete(id, revision). Unknown fields and operations refuse.
- List pages are engine-snapshot versioned. Cursor binds query/type/revision and
  offset; writes expire it. Maximum 50 rows per page and bounded record/collection
  bytes/count; exhausted limits produce an explicit error, never silent omission.
- Details include exact ID, content, stored metadata and revision. Content is text,
  not HTML/remote Markdown. Display metadata as stored, without provenance guesses.
- Delete uses a digest of exact stored fields, under the same mutation lock as
  agent writes. No clear-all endpoint; no automatic retry after unknown outcomes.
- Native request contains workspaceRoot and the owner request, never a memory
  directory or tenant ID. Native host derives profile/memory/workspace-hash and
  returns canonical workspace and actual storage directory as scope.
- UI selections/cursors live only in the current mounted scope. Failed refresh
  retains inspected content but disables deletion until a fresh read succeeds.
- Returning to Memory revalidates the catalog and restarts paging; its filters
  remain. Knowledge's mounted Office draft/selection/scroll stay intact.
