# Former host behavior ported from AsterDream

Desktop (`apps/desktop`, package `a3s-desktop`, product string Former) now
carries the AsterDream host behaviors that were still missing on this
side. The 2026-10-16 Desktop-to-AsterDream sync stays where it is.

What landed:

- `host_for_session` always wraps the caller with the Former delivery
  contract, then `session_new_params` copies that host into
  `_meta.a3sHost`. Caller guidelines stay capped at 8000 characters.
  The ACP `include_str` still reads the Work contract file.
- A home draft stays visually idle while the previous task keeps
  streaming. A plain home send creates its own task. If that create
  returns the same id as a session that already has messages or is
  streaming, the send toasts `agent.session.create.error.failed` and
  returns. A home send with no host and no task, and with no code plan
  and no deck, toasts `workbench.default_agent.preparing`. Code-plan
  and deck sends still place a worktree.
- The home permission mode is snapshotted before task creation and
  stored per member and session. `plan` remains a stored posture.
  The pending composer key stays
  `workbench.composer.permission-mode:v1:`.
- `file.select` keeps its hidden input on `document.body` until the
  selection settles. An attachment failure toasts
  `chat.input.file_error`, clears the selecting state, and restores
  focus. `createComposerInputAdapter().focus` returns when the editor
  is destroyed.
- Ending a session records its id for 5 seconds. Undo removes that id.
  A finished turn shows the last successful write per path under
  `workbench.turn.changed_files`.
- The authorization menu uses a module-level 500 ms
  `performance.now()` suppress. Native menu labels load from the
  renderer locale catalogs and substitute `{{product}}`.
- Default-agent provisioning is cached per member, with one in-flight
  promise and a runtime-unavailable flag. An omitted member id shares
  the `local` bucket. An empty member id refuses.

Already present, so left in place: conversation titles via
`visible_title`, and the workbench command palette (Shift+K).

`cargo fmt -p a3s-desktop` adjusted wrapping in
`delivery_contract_tests.rs`, `menu_labels_tests.rs`, and `lib.rs`.
A following `cargo fmt -p a3s-desktop -- --check` exited 0.

## Commands and counts

macOS, Rust 1.97.1, debug test profile. Renderer tests ran from
`apps/desktop` with `./node_modules/.bin/vitest run`. Cargo filters were
separate invocations. A single filter containing `|` selected 0 tests.

```
./node_modules/.bin/vitest run src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx
```

149 passed, 0 failed, 8.78s. This run is after the home-draft guard
was limited to sends that have no code plan and no deck.

In the same session, one vitest invocation ran the permission picker,
the default-agent hook, and `AgentComposer.test.tsx` together. At that
moment the composer file had 2 failures. Those two cases were re-run
after the guard change, then the composer file above passed as a whole.
The other two files in that invocation were:

- `WorkbenchPermissionPicker.test.tsx`: 12 passed.
- `useWorkbenchDefaultAgent.test.ts`: 20 passed. Empty-member refusal
  logged `Cannot provision a workbench agent without an explicit member scope`.

Focused renderer cases in the same session:

- `workbenchPermissionPostures.test.ts`: 1 passed.
- `desktopMainBridge.test.ts`: 1 passed, 26 skipped by the name filter.
- `taskConversation/__tests__/turnFileEdits.test.ts`: 1 passed.
- `WorkbenchCommandPalette.test.tsx`: 5 passed.

```
cargo test -p a3s-desktop --locked --lib delivery_contract -- --test-threads=1
```

3 passed, 0 failed, 805 filtered out. Repeated after `cargo fmt`.

```
cargo test -p a3s-desktop --locked --lib tobot_persona_changes_the_role -- --test-threads=1
```

1 passed, 0 failed.

```
cargo test -p a3s-desktop --locked --lib native_menu_uses_the_same_catalog -- --test-threads=1
```

1 passed, 0 failed. Repeated after `cargo fmt`.

```
cargo test -p a3s-desktop --locked --lib standing_host_context_stays_out_of_the_user_text -- --test-threads=1
```

1 passed, 0 failed, 0.79s.

```
cargo test -p a3s-desktop --locked --lib tobot_warm_keeps_one_a3s_code_process -- --test-threads=1
```

1 passed, 0 failed, 0.30s.

## Theme

The new work-page chrome uses the roles already in the root `DESIGN.md`.
The settled-task heading uses the same 12px `#8a8a8a` day heading as the
task list. The undo action is the 13px `#8a8a8a` text button, and it
turns `#171717` on hover. The changed-files block reuses the product
row: 36px, radius 8px, 2px gap, 12px muted label, and the path uses
`--code-font-family`. Tenant brand color is not applied to these rows.

## Review fixes

Two review bugs were fixed in the same port.

- A fallback-card send in code mode or plain chat now returns the plan
  or the text with `cleared: false`. `finishFallbackSend` places a
  checkout before `homeTask.create`, writes the code session and the
  draft cache, marks the pending home submit, adopts the task, and
  only then clears the draft. A missing task toasts
  `workbench.default_agent.preparing` and keeps the text. A worktree
  with no project folder toasts the existing need-project sentence
  and keeps the text. Deck sends still create, arm, and clear first.
  The fallback card does not read a pending project path.
- The changed-files row stays empty while the latest turn is active,
  including `awaiting-approval`. `taskTurnPresentation` returns
  `active` from `isTurnActive` on that latest row. Older rows still
  show their files. `TURN_STATE` is unchanged.

```
cd apps/desktop
./node_modules/.bin/vitest run src/renderer/pages/agents/__tests__/homeComposerChrome.test.tsx -t "keeps the code plan|keeps 日常办公"
```

2 passed, 7 skipped, 6.52s.

```
./node_modules/.bin/vitest run src/renderer/pages/agents/__tests__/WorkbenchHome.test.tsx -t "fallback"
```

5 passed, 90 skipped, 6.15s. The five are the existing
"does not write the fallback draft" case plus the four new send cases.

```
./node_modules/.bin/vitest run src/renderer/pages/agents/taskConversation/__tests__/TaskConversationMessage.test.tsx -t "hides changed files"
```

1 passed, 66 skipped, 4.14s.

```
./node_modules/.bin/vitest run src/renderer/pages/agents/__tests__/composerModeHome.e2e.test.tsx -t "fallback card|applet window would refuse"
```

2 passed, 7 skipped, 6.13s.

## Later contract clauses and the plus-menu upload

The Former delivery contract now includes the later AsterDream clauses.
The heading stays `Former delivery requirements`. The check sentence is
`Check each in the reply and files before finishing.` Trimmed length is
3944 characters. `8000 + 2 + 3944 = 11946`, inside the 12000 ACP field.
Desktop still reads its own contract file.

The plus menu always shows an upload row. The row is a `type="button"`
control with `aria-haspopup="dialog"`. It stays disabled until a popover
attachment launcher is registered and a quick panel is present. A click
closes the menu and dispatches
`{ source: 'popover', inputAdapter, quickPanel }`. The row uses the
existing gray field `#f5f5f5`, muted `#8a8a8a`, ink `#171717`, and hover
`#f7f7f7`. The fallback home card and `renderLayout({})` have no quick
panel, so the row stays disabled there and those renders do not throw.

```
cargo test -p a3s-desktop --locked --lib delivery_contract
```

3 passed, 0 failed, 0.00s. The compile finished in 37.95s.
`rustfmt --check --edition 2024` on `delivery_contract_tests.rs` exited 0.

```
cd apps/desktop
./node_modules/.bin/vitest run src/renderer/pages/agents/__tests__/WorkbenchComposerOptions.test.tsx src/renderer/pages/agents/__tests__/homeComposerChrome.test.tsx -t "opens a menu|opens the registered|shows 工作区 on the shipped"
```

3 passed, 8 skipped, 5.27s. The three are the unresolved upload row,
the registered picker, and the shipped tools row.

## Explicitly not claimed

No Desktop GUI launch, no `just dev`, and no `tilt up`. No browser
pass. Desktop is a Tauri host, so these hermetic renderer and library
tests are the check. No commit. No `cargo fmt --all`. The pre-existing
`a3s-desktop` lib-test warnings, including the unused `Ordering` import,
were left as they are.

The full `desktopMainBridge.test.ts` file was not run; 26 tests stayed
skipped by the name filter. The review-fix vitest runs used name
filters, so the rest of those four files stayed skipped. Renderer
typecheck and eslint were not run.
The persona and warm harness tests were not repeated after `cargo fmt`;
`turns.rs` was already formatted.

Paperloom, team, and the `plan` posture stay. Export-logs and
open-logs menu items were not added. The About page version, the
Desktop meta plan, JSON-render UI, local-knowledge dialogs, MaaS
retirement, task corpus, and CUA were left untouched. The command
palette was not expanded. The Presenton `generate_deck` prompt, the
debug model seed, signed DMG packaging, and the read-only audits were
not ported. The upload row was not exercised in the Desktop window.
