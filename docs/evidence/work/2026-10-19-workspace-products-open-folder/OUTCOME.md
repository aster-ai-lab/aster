# Workspace products and native folder open

## Result

The completed T01 conversation now shows **工作产物 · 2**: one `pipeline.py`
and one `results.json`. Relative and absolute references use the existing
session workspace context for identity. Different directories and exact POSIX
file names remain distinct; no filename or basename guessing is used.

The session workspace heading has an accessible folder action at its right
edge. It opens the session root, including when a child directory is selected.
It is absent for missing and cloud workspaces. Errors remain visible in the
file panel, and a previous session's delayed error does not affect a new one.

The real desktop file bridge lacked `file.openPath`; adding only the button
was insufficient. Directory opening now uses an authenticated local
`POST /api/workspace-file/open-directory` route, a strict typed payload,
canonical containment and directory checks, then the existing platform opener:
macOS `open`, Windows `ShellExecuteExW` with the `open` verb, or Linux
`xdg-open`. Existing unsafe bundle rejection and file-open behavior remain.

## Verification

Commands run from `apps/work` unless stated otherwise:

| Command | Result |
| --- | --- |
| `node_modules/.bin/vitest run --project renderer src/renderer/pages/agents/taskConversation/__tests__/TaskConversationMessage.test.tsx -t 'counts workspace-relative'` before the fix | Failed as expected: 5 rows instead of 3. |
| `node_modules/.bin/vitest run --project renderer src/renderer/pages/agents/taskConversation/__tests__/TaskConversationMessage.test.tsx src/renderer/pages/agents/__tests__/taskConversationWorkspace.test.tsx src/renderer/pages/agents/messages/__tests__/agentMessageListAdapter.test.tsx` | 3 suites, 103 tests passed after the final change. |
| `node_modules/.bin/vitest run --project renderer src/renderer/shell/studioMainBridge.test.ts src/renderer/pages/agents/__tests__/WorkbenchSessionFiles.openFolder.test.tsx` | 2 suites, 36 tests passed. Includes macOS/Windows/Linux path strings, authenticated transport, failures, cloud/missing roots, and stale errors. |
| Session-files main/export/open-folder suites | Initial 34 tests passed; subsequent cloud-root addition passed in the 6-test open-folder suite. |
| Root: `cargo test -p aster-dream --lib workspace_file` | 13 tests passed. Includes directory canonicalization, spaces/Unicode, non-directory, bundle, traversal, missing path, and escaping symlink cases. |
| Root: `cargo test -p aster-dream --lib authority::tests::workspace_directory_open_authenticates_and_preserves_native_refusals` | 1 real HTTP test passed: missing bearer 401, hostile origin 403, unsafe bundle 403, unknown field 400, and non-directory 400. |
| `node_modules/.bin/oxfmt --check` on the 7 changed renderer/bridge source and test files | Passed. |
| Focused ESLint on changed renderer/bridge source and tests | No errors; existing cross-page import warnings remain. |
| Focused `rustfmt --check --config skip_children=true` on the two native files | Passed. |
| Root: `cargo fmt --all -- --check` | Failed on existing formatting differences elsewhere in the working tree; unrelated files were not reformatted. |
| Root: `just build-work` | Passed: ACP, Office, then AsterDream host. |

The shell has no `pnpm` command, so installed local binaries were used.
The repository's mutating full lint command was not run across unrelated dirty
files. Full TypeScript checking was not repeated; the preceding task had
already recorded unrelated baseline failures.

## Desktop evidence

CUA operated the current development app in the background using accessibility
actions. After replacing and signing the development host and restarting the
4318 renderer server to rebuild its inlined bridge script:

- AsterDream's T01 transcript exposed `工作产物 · 2`, `打开 pipeline.py`, and
  `打开 results.json`.
- The file pane exposed the new `打开所在位置` button beside `会话工作区`.
- Clicking it opened a Finder window named `T01-workspace`, containing exactly
  the visible entries `input`, `instruction.md`, `output`, and `pipeline.py`.
- The current session root was `/Users/roylin/code/tasks/T01-workspace`.

The updated development bundle is
`/Users/roylin/Library/Application Support/dev.a3s.studio/aster/AsterDream.app`.
The installed `/Applications/AsterDream.app` was not replaced. No additional
model call was needed and the T01 input or generated submission was not edited.

## Explicitly not claimed

- Windows and Linux GUI execution was not performed on this Mac. Their existing
  native opener branches were inspected; transport tests preserve their paths.
- No clean full-workspace lint/typecheck/format gate is claimed.
- No filesystem realpath identity for symlink aliases or case-insensitive
  mounts is claimed by renderer product deduplication. The native boundary
  performs canonical containment for opening directories.
- This change does not add general file opening to the previously missing
  bridge method; the newly supported operation is directory opening.
