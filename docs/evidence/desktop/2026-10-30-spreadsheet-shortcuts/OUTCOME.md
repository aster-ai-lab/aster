# Spreadsheet shortcut help — issue #212

The Chinese Fortune locale now uses `Ctrl / ⌘`, `⇧`, and Chinese names for
delete, backspace, return and tab. All 13 shortcut descriptions and the initial
screen-reader introduction are covered. Focus locking still says
`⇧ + Ctrl + F`: its actual implementation accepts Ctrl, not Meta.

The source change extends Office's existing handwritten Fortune patch, for
both ESM and CommonJS. The installed bundles and Studio lockfile were generated
through the existing pnpm workflow. No keyboard implementation, formula logic,
workbook data, dependency version or runtime localization wrapper was added.
Studio commit: `d97043d7d4c3593428cd78680e48ff9b1422ef92`.

## Verification

macOS arm64, Node 26.8.1, pinned pnpm 12.6.0, Playwright 1.62.1.
Chromium 151.0.7922.34 and WebKit 26.5. Commands below ran from `apps/studio`:

| Check | Observed result |
| --- | --- |
| `pnpm exec vitest run --project scripts scripts/__tests__/officeShortcutLocale.test.ts` before | 1 failed against the real installed CommonJS locale |
| Same regression after | 1 passed |
| `node target/issue212/browser.cjs before` | Both Chinese cases failed the translation contract; both English cases passed |
| Same browser script with `after` | All 4 engine/language combinations passed |
| `pnpm install --lockfile-only --ignore-scripts --offline` | Only the existing core patch fingerprint changed in 4 lockfile references |
| `pnpm install --frozen-lockfile --ignore-scripts --offline` | Passed; 2 packages reused, 0 downloaded |
| `pnpm --filter @a3s-lab/office build` | Passed, including the Rust/Wasm kernel build and library declaration generation |
| `pnpm exec rsbuild build` | Production renderer passed; build warnings retained in the raw log |
| Scoped Oxlint, ESLint and Oxfmt checks for the new regression | Passed |
| Parent and Studio `git diff --check` | Passed |

The browser fixture renders the real `@fortune-sheet/react` Workbook with
Office's installed dependency. Each engine checks all 13 accessible list items,
the introduction, and real Ctrl+Shift+F focus-lock toggles. English help is
compared in full before and after. Both sheets retain their names, ordering,
active status and stored values (`原始数据`: 12; `月度汇总`: 34).

The source audit re-opened all 70 files in the old and new installed core
packages: 68 are byte-identical; the ESM and CommonJS modules each change exactly
14 corresponding locale values. The production renderer chunk also contains
all 14 values. `verification.json` records the source, archive-entry and artifact
hashes; every retained gzip and all 33 tar entries were reopened and compared
with their original bytes.

Two fixture assumptions were corrected before freezing the final before/after
comparison: an actively edited formula input intercepts the focus chord, and a
valid explicit selection suppresses the introduction. The final probe focuses
the actual shortcut region and uses Fortune's default selection. Its inputs,
expectations and deadlines are identical before and after; only the core
package alias selects the retained baseline or candidate. Diagnostic receipts
are retained separately in the browser archive and are not counted as product
regressions.

The existing Office build regenerated a tracked kernel artifact. No Rust
kernel source belongs to this locale fix, so that artifact was restored to its
previous committed bytes and is excluded from the change; both hashes remain
in `kernel-rebuild.json`. Generated renderer/library output is not committed.
All owned test browsers and the dedicated Vite server were closed.

## Explicitly not claimed

No native VoiceOver speech, Windows/Linux runtime, full Studio lint/test pass,
green global CI, installer rebuild, installed application update or new release
is claimed. A3S Test could not start because the installed browser adapter is
0.3.2 and requires >=0.4.0,<0.5.0; existing Playwright provided the browser proof.
This change does not claim fixes for the separate save-status, name-box,
sheet-navigation or sidebar-title issues (#209, #210, #211, #215).
