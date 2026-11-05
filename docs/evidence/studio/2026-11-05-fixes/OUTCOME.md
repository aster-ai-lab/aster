# Studio data, search, model, translation, and focus fixes

Verified on 2026-11-05, macOS arm64, using the initialized `apps/studio`
submodule. Unrelated checkout changes were preserved; no commit was created.

## Result

- Data settings imports the shared legacy IndexedDB name, removing the temporal
  dead-zone crash when the page initializes.
- The native agent plane exposes `/api/search/entities` and
  `/api/search/contents`, including entity/session filtering, full-text session
  messages, timestamps, pagination, case preservation, and optional-field
  omission. The renderer serializes nested search parameters as JSON.
- Translation languages come from the shared built-in catalog instead of a
  nonexistent backend language route.
- Home model selection updates the default model; existing sessions continue to
  update their agent model.
- Global search records the last focused element and restores it only after
  `POPUP_EXIT_MS`, when Radix's focus scope has been removed. Restoring in the
  earlier close-promise animation frame was too early and lost the native focus
  check. The renderer unit test covers Escape and focus restoration.
- Scoped Oxfmt was applied only to the six touched frontend files that needed it.

## Build and automated verification

Commands run from the repository root unless a working directory is named.

1. `just build-studio` passed after the final focus change. Existing Rust
   warnings and the linker compact-unwind warning remain.
2. From `apps/studio`, the matching renderer build passed:

   ```bash
   ASTER_TENANT_SLUG=aster \
   STUDIO_RENDERER_DIST=/Users/roylin/code/aster/apps/studio/.studio/dist/agent-desktop-e2e \
   /opt/homebrew/bin/node node_modules/@rsbuild/core/bin/rsbuild.js build
   ```

   Rsbuild reported existing dynamic-dependency and route-file warnings.
3. From `apps/studio`, the final focused frontend run passed **85/85 executed
   tests (1 skipped) in 4/4 files**:

   ```bash
   /opt/homebrew/bin/node node_modules/vitest/vitest.mjs run \
     src/renderer/pages/settings/DataSettings/__tests__/BasicDataSettings.test.tsx \
     src/renderer/data/__tests__/studioAgentPlane.test.ts \
     src/renderer/pages/agents/__tests__/AgentChatSettingsPanel.test.tsx \
     src/renderer/components/GlobalSearch/__tests__/GlobalSearchPopup.escape.test.tsx
   ```

4. `cargo test --locked -p aster-studio --lib controllers_search_tests -- --nocapture`
   passed **1/1** against the final search implementation:
   `search_routes_filter_entities_and_page_session_messages ... ok`.
5. `CARGO_TARGET_DIR=/Users/roylin/code/aster/target/studio/aster cargo clippy --locked -p aster-studio --all-targets --no-deps`
   completed successfully in 1 m 43 s. It reported **310 warnings (246
   duplicates)** across the existing package/test graph; this is a successful
   lint command, not a warning-free codebase.
6. Scoped Oxfmt passed on all eight touched frontend files. The Python native
   driver compiled successfully. Diff checks and the unrelated current Cargo
   format blocker are recorded below.

## Native agent-desktop evidence

The earlier native run from this checkout passed:

- `SET-NAV-01`: data settings recovered; network search, document processing,
  data, and about pages were reachable.
- `COMP-01`: GPT-6.1 Sol → GPT-6.1 Terra → Sol changed and restored correctly.
- `SET-03E`: translation initialized without an error and the popup closed by
  keyboard.

The final isolated runs used only headless semantic observe → act → verify;
no coordinate or headed fallback was used.

- `BOOT-01` passed on `fork7` after its first-run component and permission
  dialogs completed (`converged-boot-fork7-final/core-navigation.json`).
- `NAV-03` passed on `fork6`: query `0101`, dialog closed, draft restored, and
  Composer reported `focused` (`converged-nav03-stable-driver/core-navigation.json`).
  The driver now focuses text fields explicitly and activates the window once
  per replacement instead of resetting DOM focus before every character.
- `NAV-06` passed on `fork7`: the real backend returned the synthetic session
  and agent, Return opened the session, closed the dialog, and exposed the task
  workspace control (`converged-nav06-clean/core-navigation.json`). The local
  fixture used a placeholder model, so its expected unavailable-model status
  was visible; no generation request was sent.

A single-instance display callback restored accessibility on `fork6`; no native
startup code was changed. `fork7` used a workspace-local runtime descriptor to
create only loopback fixture data. Session deletion returned HTTP 405, so the
synthetic agent/session remains in the stopped, isolated `fork7` profile, not in
the owner's default profile. The test process group and local runtime/fixture
descriptors were removed. A later combined run did not pass its home-navigation
setup from the placeholder-model error page; only the individual passes above
are claimed.

## Other check results and limits

- Direct `tsc --noEmit -p tsconfig.web.json --composite false` failed on the
  existing frontend graph. Two diagnostics are in touched files but on unchanged
  lines: duplicate `workspace` at `AgentChatSettingsPanel.test.tsx:366` and a
  duplicate JSX attribute at `BasicDataSettings.tsx:385`. No whole-web typecheck
  pass is claimed.
- Scoped Oxlint reported one existing error for the intentionally discarded
  `nativeExecution: _` binding at `studioAgentPlane.ts:986` and one React import
  warning in `BasicDataSettings.tsx`; it is not a passing lint result.
- Two attempted `pnpm exec` checks triggered the package manager's broken
  `node-gyp` path under `npm.asar`. They did not provide lint/format results and
  were replaced with direct local checker invocation. `package.json` and
  `pnpm-lock.yaml` were already modified before those attempts and were not
  edited by this task.
- `cargo fmt --all -- --check` passed earlier. The final invocation was blocked
  by an unrelated concurrent checkout change: `apps/gateway/src/wire.rs` does not
  exist. That unrelated module was not restored or edited.
- `git diff --check` and `git -C apps/studio diff --check` passed after the final
  documentation and formatting changes.

## Explicitly not claimed

- No combined `NAV-03 + NAV-06` pass is claimed; the passing evidence is from the
  individual native scenarios listed above. No model generation or task execution
  is claimed for the placeholder-model fixture.
- No real external model request, provider credential check, Windows/Linux
  runtime, release, deployment, full frontend test suite, full typecheck, final
  workspace-wide Cargo format pass, or warning-free lint result is claimed.
- Unrelated type/lint errors, the missing gateway module, native accessibility
  implementation, and broken package-manager native-module installation were
  not fixed. The isolated `fork7` profile retains its two synthetic records.
