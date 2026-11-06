# AccountCenterMenu startup syntax failure

Verified on 2026-11-06, macOS arm64, against the owner's actual default Aster
window and renderer on port 4318. No commit was created.

## Result

- The default window remained on the static `#spinner` because WebKit rejected
  the main JavaScript bundle before `MainApp` mounted.
- The WebKit console reported:
  `SyntaxError: Unexpected token ','. Cannot declare a lexical variable twice: 'highlightId'.`
- `ScopeTab` in `AccountCenterMenu.tsx` declared `highlightId` twice in both its
  destructured parameters and TypeScript parameter type. The duplicate entries
  were removed. Scoped Oxfmt also reordered one existing UI import.

## Verification

1. Reproduced on the default Aster process, PID 87127: only the static boot
   spinner was present in the accessibility tree.
2. Opened the real WebKit inspector on that window and read the syntax error at
   the generated `ScopeTab` parameter list. The same duplicate was present in
   the served `/static/js/index.js`.
3. `pnpm exec vitest run src/renderer/components/account/AccountCenterMenu.test.tsx`
   passed **16/16 tests in 1/1 file** after the fix.
4. `pnpm exec oxfmt --check src/renderer/components/account/AccountCenterMenu.tsx`
   passed after scoped formatting.
5. `pnpm exec oxlint src/renderer/components/account/AccountCenterMenu.tsx`
   passed with **0 warnings and 0 errors**.
6. The default dev server rebuilt successfully and served a bundle without the
   duplicate parameter. Reloading the original white window mounted the real
   task list, message content, composer, and workspace controls. The inspector
   was then closed, leaving the recovered default window visible.
7. `git -C apps/studio diff --check` passed.

Recovered-window capture: `artifacts/default-window-recovered.png`.

## Explicitly not claimed

- No full `pnpm lint`, full frontend suite, production renderer build, native
  rebuild, Windows/Linux runtime, or release installer pass is claimed for this
  two-line syntax fix.
- Existing unrelated i18n missing-key messages, workspace resource 404s, and
  `backup.get_auto_sync_state` route errors remain; they did not prevent the
  recovered first screen and were not changed.
- The earlier isolated-instance check alone was not valid evidence for the
  default instance. The accepted recovery evidence here comes from the original
  default white window after the fix.
