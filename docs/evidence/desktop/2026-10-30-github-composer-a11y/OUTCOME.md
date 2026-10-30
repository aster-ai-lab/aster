# Composer accessibility issue fixes

Implemented in Studio `26d3915a3b681eaecd30a3fb520ea3e8836ee3cd` from current Studio main `f6d648642`. Initial parent base: `d6b8e4a88`; landing rebased by fast-forward to `515f0850e`, with root format and workspace checks repeated on that base.

## Result

- [#213](https://github.com/aster-ai-lab/aster/issues/213): quick-panel actions use their visible title as the accessible name and expose the description separately. Builtin launcher actions no longer append a redundant skill category. The registered `ask-form` skill has a translated display title in both composer catalogs. The clear command displays its translated title once. Search and execution retain the actual command and skill identifiers; custom skill names are preserved.
- [#214](https://github.com/aster-ai-lab/aster/issues/214): the Chinese composer and sidebar both say 新建任务.
- Ran the existing i18n generator to repair catalog order. Both catalogs retain exactly 11,510 keys. English values are unchanged; the sole changed Chinese value is `agent.session.new`.
- Corrected the existing AgentComposer test translation fixture to supply the language field that the real translation hook returns. No product fallback was added for a broken mock.

## Verification

| Check | Result |
| --- | --- |
| New regression cases before the fix | 3 failed; 14 passed |
| Focused QuickPanel, home catalog and slash-command suites | 109 passed across 7 files |
| Extended suites including AgentComposer | 258 passed; 7 failed |
| Unmodified main with the corrected locale fixture | The same 7 AgentComposer cases fail; no added failures in the modified source |
| Chromium and WebKit, actual components and full application CSS | 12 assertions each; zero page errors |
| Scoped ESLint | 0 errors; 5 warnings in existing dependencies/import patterns |
| Scoped formatting | Passed |
| i18n check after generator | 23,020 translations passed |
| Root `cargo fmt --all -- --check` | Passed |
| Root `tests/workspace_dependencies.py` | 6 passed |
| Root `tests/studio_checkout.py` | Passed |
| Signed commit | `git verify-commit` passed; DCO signoff present |

Commands used: `pnpm test:renderer` with the seven focused test files/directories, the extended AgentComposer suite, `pnpm lint`, scoped ESLint and Oxfmt, `pnpm exec tsc --noEmit -p tsconfig.web.json`, `pnpm exec tsx scripts/i18n-sync.ts`, `pnpm i18n:check`, the three root checks above, and `node target/issues-a11y/browser.cjs`. The browser fixture uses real QuickPanel rows, the home catalog and slash-command tool; only its synthetic input destination is a fixture. Fixture hashes are in `manifest.json`. Unit tests are committed in Studio.

Evidence stdout copies trim trailing empty lines only; original stdout hashes are recorded in `manifest.json`.

The before/after test comparison and exact failing case names are retained in `verification.json`. Screenshots and semantic trees show the actual browser fixture. No real desktop recording or user document was copied.

## PR processing

- Closed Studio [#1](https://github.com/kense-labs/studio/pull/1), [#2](https://github.com/kense-labs/studio/pull/2), and [#3](https://github.com/kense-labs/studio/pull/3) as obsolete. All 17 target workflow files were removed by desktop migration `e2f0818a8`; no retired automation was restored.
- Refreshed Aster [#21](https://github.com/aster-ai-lab/aster/pull/21) on current main, with signed merge `91a4d96a9`. Its fork checks lack private Studio read credentials. An independent push of the identical commit to `fix/length-salvage-qualified` runs with repository credentials: both desktop builds pass; Rust checks fail on stale icon generation. [Run](https://github.com/aster-ai-lab/aster/actions/runs/41014973573).
- Reviewed [#208](https://github.com/aster-ai-lab/aster/pull/208): its generated SVG changes restore the retired blue artwork despite the current graphite brand contract. Recorded the [blocking generator defect](https://github.com/aster-ai-lab/aster/pull/208#issuecomment-6516842686); did not merge the regression.

## Explicitly not claimed

- Full Studio lint is not green: Oxlint reports existing tsconfig and unawaited-promise errors outside the changed files. The automatic lint edits outside this task were reverted only in this isolated worktree.
- Full web typechecking reports 442 diagnostics outside the touched files. It is not a passing whole-product check.
- The seven baseline AgentComposer failures remain. The original baseline also fails because its locale mock omits `i18n.language`; the one-case original-source failure was confirmed before correcting that fixture.
- The installed A3S Test browser adapter is 0.3.2 while the engine requires >=0.4.0 and <0.5.0. Browser evidence uses the repository's Playwright fallback.
- These are source fixes on main, not a newly published desktop installer. Native macOS accessibility issue #215, Office issues #209–#212, and the other Aster PRs remain open for investigation.
- No Capture takeover source or unrelated shared-workspace edits are included in this landing.
- GitHub reports `unknown_key` for the local signing identity, and its account email is unverified. Local cryptographic signature validation passes; GitHub Verified status and account configuration changes are not claimed.
