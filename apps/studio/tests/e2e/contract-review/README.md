# Contract review in the composer

The package extends the installed `@` agent list. It has no standalone UI or
mini-app window. The former package-picker fixture is retired.

From `apps/desktop`, with the AsterDream renderer running on port 4537:

```sh
DESKTOP_E2E_BASE_URL=http://127.0.0.1:4537 ./node_modules/.bin/playwright test \
  --config playwright.workbench-office.config.ts contract-review-composer.spec.ts
a3s-test check tests/e2e/contract-review/no-window.acl --json
a3s-test run tests/e2e/contract-review/no-window.acl \
  --browser-driver standalone \
  --browser-executable /opt/homebrew/bin/agent-browser --json
```

The browser tests use the real renderer, composer, Word importer, TipTap
attention decorations and Office comment pane. Catalog, message history, file
bytes and file-watch availability are controlled service boundaries. The Word
bytes come from `reference-alignment/cases/review-basic`. Tests verify `@`
selection, inline path binding, no window invocation, verified bytes, host
segment progress, actual comment placement, attention, visible comments, and
unchanged read-only source.

The ACL case proves the retired standalone URL no longer exposes a review
uploader. It does not install a plugin or call a model. The standalone driver
is the existing admitted agent-browser 0.26.0, protocol 15; the installed
a3s-browser 0.3.2 is incompatible with this test-engine build.

Renderer/native sibling tests cover install/uninstall refresh, picker cancel,
late results, source bounds, empty/failed/stopped outcomes, source revision
changes, progressive host reads and finding validation. Native-window and
fresh real-model acceptance remain separate gates.
