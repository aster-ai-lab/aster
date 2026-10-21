# Existing workspace products / folder action: current-source check

Both original product fixes remain connected in the current Work source. This is a separate compatibility check; its duration is excluded from T01 performance measurements.

- `taskConversation/turn.ts:443` still resolves relative references against the supplied workspace root and deduplicates by the full path key, preserving equal basenames in different directories. Windows drive/UNC separators are handled without changing exact POSIX filenames. `components/AgentSessionMessages.tsx:189` still supplies the session root; `TaskConversationMessage.tsx:1089` still consumes it.
- `WorkbenchSessionFiles.tsx:1957` still renders the accessible `打开所在位置` / `Open location` action beside the session workspace heading for local roots. The handler opens the session root even if a child is selected, with localized errors and stale-session protection.
- `desktopMainBridge.js:1631` still sends the authenticated directory request. `authority.rs:1017` and `:2354` retain the route and authentication classification. `workspace_file.rs:253` retains strict typed payload, canonical containment, directory and unsafe-bundle checks before `no_window::open_target`.
- `no_window.rs:71` retains native platform dispatch: macOS `open`, Windows `ShellExecuteExW` (`open` verb), and Unix/Linux `xdg-open`.

The current code matches the behavior described by the original `docs/evidence/work/2026-10-19-workspace-products-open-folder/OUTCOME.md`. That record has no original source hash manifest, so this check confirms connected behavior, not historical byte identity. Renderer deduplication still makes no filesystem-realpath or case-insensitive-volume identity claim.

## Focused frontend revalidation

Ran the three existing owning suites with the exact name filter stored in `start.json` / `result.json`:

```text
counts workspace-relative|keeps distinct references|uses exact file names|opening the session workspace in the file manager|opens the workspace directory through the authenticated host|rejects an unconfirmed directory open
```

Result: **3 suites passed; 17 passed, 96 unselected/skipped; exit 0**. Vitest duration 4.99 s; process wall time 5.751431041 s. `vitest.log` contains the complete output and existing config/route warnings. All 11 audited source/test files have identical before/after hashes.

Owning files:

- `src/renderer/pages/agents/taskConversation/__tests__/TaskConversationMessage.test.tsx`: five selected deduplication cases, including distinct directories, absent root, Windows and UNC.
- `src/renderer/pages/agents/__tests__/WorkbenchSessionFiles.openFolder.test.tsx`: all six cases, including both languages, selected-child root handling, absent/cloud roots, error localization, and stale completion.
- `src/renderer/shell/desktopMainBridge.test.ts`: six selected platform-path transport / native refusal cases.

## Exact native filters for the fresh root-owned libtest

No Cargo or native test execution was performed here. Each filter below is an exact test name:

```text
workspace_file::tests::opening_a_directory_keeps_its_canonical_path_and_rejects_files_and_bundles
workspace_file::tests::opening_a_directory_rejects_symlinks_outside_the_workspace
authority::tests::workspace_directory_open_authenticates_and_preserves_native_refusals
```

Use `<fresh-work-libtest> --exact <filter> --nocapture`. These tests reject unsafe requests before the platform opener and do not launch Finder. No GUI, Finder, Windows/Linux GUI, model, VM, dependency installation, product source edit, or full-tree typecheck was performed.
