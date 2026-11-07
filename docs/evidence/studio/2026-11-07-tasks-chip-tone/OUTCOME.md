# Give the /tasks command chip its own color

Verified on 2026-11-07, macOS arm64. No commit was created.

## Result

- `CommandComposerToken` gave `/clear` (red), `/compact` (blue) and `/goal`
  (amber) dedicated tones and routed every other command — including `/memory`
  and `/tasks` — to one violet fallback. The 后台任务 and 记忆管理 chips
  therefore looked identical in the slash palette.
- `/tasks` now has its own teal tone, in the same light/dark chip palette
  style as the existing command tones: `bg-[#e9f6f6]`, `text-[#2b6f76]`,
  `ring-[#c5e3e6]` with hover and dark variants. `/memory` and all other
  commands keep the violet fallback. Icon, label and command text are
  unchanged.
- A regression case asserts `/tasks` has the teal classes, does not have the
  violet background, and that `/memory` still has the violet tone.

## Verification

- `ComposerToken.test.tsx`: **70/70 passed** (includes the new case).
- `homeSlashCatalog.test.tsx`: **12/12 passed**.
- Scoped Oxfmt `--check` and Oxlint `--deny-warnings` on both changed files:
  **passed**, 0 warnings / 0 errors. `git diff --check`: **passed**.
- `STUDIO_RENDERER_DIST=.studio/dist/slash-chip-tone rsbuild build`:
  **passed** (21.8s).
- Chromium rendering against that production CSS measured
  `/memory` as `rgb(243, 239, 250)` / `rgb(117, 83, 159)` and `/tasks` as
  `rgb(233, 246, 246)` / `rgb(43, 111, 118)`. [preview.png](preview.png)
  shows the two chips side by side.
- `slashCommandsTool.test.tsx`: **4 pre-existing failures** (it still expects
  the older plain-string label 清空对话历史 / 清空对话, while the tool now
  renders a `ComposerToken`). Unrelated to this tone change; left untouched.

## Explicitly not claimed

- No live Studio window or GUI/e2e run; the visual check loads the real
  production stylesheet against a faithful static replication of both chips.
- No full frontend suite, full TypeScript check, `pnpm lint`, or commit.
- No change to any other command's tone, icon, label, or behavior.
