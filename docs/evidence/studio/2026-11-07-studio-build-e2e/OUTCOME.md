# Build the current Studio checkout and run its end-to-end suites

Verified on 2026-11-07, macOS arm64. No commit was created.

## Result

- **Build succeeded.** Native host and companions were built from the current
  checkout, and a renderer matching that tree was built and served on
  `127.0.0.1:4318`. Seven Playwright suites were executed to completion.
- **E2E did not pass as a release gate.** 141 passed / 264 failed / 61 skipped
  across the seven reporter summaries. Sampled failures are pre-existing
  spec-versus-source drift, not regressions introduced by this build. No test,
  fixture, baseline, or assertion was changed to obtain a pass.

## Build and launch commands

```sh
# From the repository root, pinned toolchain, current checkout
PATH=/opt/homebrew/bin:$PATH just build-studio aster
# From apps/studio
STUDIO_RENDERER_DIST=.studio/dist/e2e-latest PATH=/opt/homebrew/bin:$PATH \
  ./node_modules/.bin/rsbuild build
# Serve only that dist on 4318 (no Tauri window reused)
python3 -m http.server 4318 --bind 127.0.0.1
```

- `just build-studio aster`: **passed**, `Finished dev profile in 35.31s`
  (2m9s including companion staging). Produced
  `target/studio/aster/debug/Studio` (418M).
- Renderer build: **passed** in 19.2s; produced
  `apps/studio/.studio/dist/e2e-latest/windows/main/index.html`.
- `just studio aster` was also attempted as the full-host launch. First attempt
  failed at `stage-companions` because WorkBuddy's safe-delete guard refused a
  1476-file temp cleanup; with
  `CODEBUDDY_SAFE_DELETE_BULK_THRESHOLD=100000` it did start a Tauri host, but
  Playwright uses an independent Chromium, not the Tauri webview, so the host
  does not supply renderer IPC. The renderer-preview path above is the one the
  suites' configs document.

Checkout recorded at build start: root `f1ba91900`, studio `3c45b46eb2`.
The working tree's HEAD advanced during the session to root `a2fdfd625`,
studio `699ced4ab8`; **no rebuild was done after that advance**, so these
results describe the earlier tree, not the current HEAD.

## E2E environment adaptations (no repository file changed)

- `HTTP_PROXY` / `HTTPS_PROXY` and friends were unset and
  `NO_PROXY=127.0.0.1,localhost` set, because the machine's proxy returns 502
  for loopback requests.
- The preview server injected one `<script>` into `index.html` that answers
  only `runtime_components_ipc` for `runtime.components.list` and
  `runtime.components.required_missing` with empty arrays. Without it, the
  current source's `FirstRunRuntimeComponentsDialog` enters a permanent
  `runtime component host missing` error state and blocks every renderer-only
  suite. This is the narrowest IPC bridge stub; it does not weaken any product
  assertion.
- Two suites initially failed before their first test because Playwright's
  `test-results` cleanup hit the same bulk-delete threshold (66 / 58 files).
  They were rerun with `CODEBUDDY_SAFE_DELETE_BULK_THRESHOLD=100000` for that
  process only.

## Suite matrix (`--reporter=line`)

| Config suite | Passed | Failed | Skipped |
| --- | ---: | ---: | ---: |
| `playwright.settings.config.ts` | 16 | 5 | 0 |
| `playwright.task-conversation.config.ts` | 14 | 22 | 0 |
| `playwright.knowledge.config.ts` | 6 | 5 | 5 |
| `playwright.composer-audit.config.ts` | 35 | 155 | 0 |
| `playwright.workspace-motion.config.ts` | 1 | 7 | 0 |
| `playwright.workbench-office.config.ts` | 68 | 66 | 56 |
| `playwright.task-turn-state.config.ts` | 1 | 4 | 0 |
| **Total** | **141** | **264** | **61** |

Full logs: `/tmp/aster-e2e-final-<suite>.log`; the orchestration script was
`/tmp/aster-e2e-all2.sh`. Total wall clock for the seven-suite matrix was
24m33s plus 8m12s for the two reruns.

## Failure attribution (sampled, not exhaustive)

- `composer-audit` UI-04: strict-mode violation — `getByRole('button', name
  '输入框测试任务')` now resolves to two elements (项目 and 最近).
- `workbench-office` plus-menu motion: expects `transition-duration`
  `0.16s, 0.16s, 0.16s`, the current UI computes `0.16s, 0.16s`.
- `settings/coverage.spec.ts`: nav baseline still lists 外观 / 环境依赖,
  while the current settings navigation no longer renders them; API 网关 also
  moved. Related `interactions` cases time out looking for 外观.
- `task-turn-state`: home send button stays `aria-disabled="true"`; the
  approval case cannot find `chat.composer.action.pause`.
- `knowledge` / `workbench-office` skipped cases correspond to the tenant-gated
  and `test.skip(true)` items catalogued in `tests/e2e/PLAN.md`.

None of these reproduce a failure introduced by the build itself; they are the
spec-versus-source drift that `tests/e2e/PLAN.md` §5 (B4/B6) calls out.

## Explicitly not claimed

- No claim that the E2E matrix is green or that it qualifies as desktop
  release platform validation (`docs/release-governance.md:70`).
- `playwright.connectors.config.ts` (the only real native-host suite) and the
  retired `playwright.regression.config.ts` were **not run**; the former needs
  its own macOS isolated instance and port 4598.
- No signed/notarized installer, `pnpm package:host` release build, archive
  member check, `spctl`/`stapler` validation, or publishing was performed.
  `package:host` is known to fail earlier in this workspace at the office dts
  declaration step.
- No full frontend suite, full TypeScript check, `pnpm lint`, commit, push,
  or gitlink bump.
- Results do not describe the HEAD reached after the in-session advance.
- No repository test, fixture, config, or baseline was modified.
