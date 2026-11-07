# Message process aggregation, failure rows, and elapsed time

Verified on 2026-11-06, macOS arm64. No commit was created.

## Result

- Completed reasoning now uses the same disclosure presentation as adjacent
  tool calls inside completed process history. It is no longer rendered as
  directly expanded content.
- Completed process groups and nested tool groups default to collapsed while
  preserving the existing cached manual-expansion behavior.
- Failed tool headers use the full available row width with truncated labels.
  Error details remain collapsed until the user expands the row.
- Elapsed time is right-aligned for completed process groups, nested tool
  groups, individual agent tool calls, and reasoning blocks.
- Tool `startedAt` / `completedAt` values are preserved from Cherry metadata.
  Reasoning uses the existing `thinkingMs` metadata; no new timing state was
  introduced.

## Verification

- `ThinkingBlock.test.tsx`: **20/20 passed**.
- `MessagePartsRenderer.test.tsx`: **93/93 passed**, including completed
  reasoning inside the same collapsed process history as tools.
- `AgentToolRenderer.test.tsx`: **56/56 passed**, including a failed call with
  a full-width one-line header, right-aligned duration, and collapsed error
  details.
- `toolResponse.test.ts`: **32/32 passed**, including execution timestamp
  preservation.
- `ToolBlockGroup.test.tsx`: **32/32 passed**, including nested group duration
  and default collapsed state.
- `MessageProcessGroup.test.tsx`: **2/2 passed**.
- Total: **6 files, 235/235 tests passed**.
- Scoped Oxfmt passed. Oxlint reported **0 errors** and four existing
  `import(default)` React warnings in the touched legacy files; the imports
  were left unchanged rather than rewritten to satisfy the plugin.
- Production Rsbuild output `.studio/dist/message-aggregate-elapsed`: passed.
- `STUDIO_RENDERER_DIST=.studio/dist/message-aggregate-elapsed just build-studio`:
  passed with full companion staging.
- `git -C apps/studio diff --check` passed.

## Explicitly not claimed

- No real-window visual acceptance is claimed: no accessible Aster window was
  available at the final check.
- No new default override of a user's cached manual expansion is claimed.
- No full frontend suite, full TypeScript check, native Linux PTY runtime,
  Windows/Linux GUI run, release installer, or warning-free lint result is
  claimed.
