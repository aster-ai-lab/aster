# Shared context-menu dismissal

## Result

The canonical `packages/ui` ContextMenu now uses the same popup state and
dismissal helper as DropdownMenu, Popover and HoverCard. A window-directed
background click and application blur close the open context menu. Existing
opening pointer-up protection, forwarded refs, consumer capture handlers,
portalled submenus and outside-click focus behavior are retained.

The installed Radix ContextMenu implementations in both the root workspace
(2.4.1) and Studio (2.3.3) already support controlled `open`. This change uses
that existing API and adds no dependency or version upgrade.

The shared Vitest setup now registers the declared Jest DOM matchers against
the owning Vitest expectation object. The installed Jest DOM `/vitest` entry
otherwise resolved a different peer Vitest and produced `Invalid Chai property`
before testing any popup behavior. Assertions were not removed or relaxed.

## Verification

Run from `packages/ui`:

```sh
node node_modules/vitest/vitest.mjs run \
  src/components/primitives/__tests__/popup-dismissal.test.tsx \
  src/components/primitives/__tests__/context-menu.test.tsx \
  src/components/primitives/__tests__/dropdown-menu.test.tsx \
  src/components/primitives/__tests__/hover-card.test.tsx \
  src/components/primitives/__tests__/popover.test.tsx
node node_modules/typescript/bin/tsc --noEmit -p tsconfig.json --pretty false
```

- The new background case failed alone before the implementation change: the
  real context menu remained in the document after `click(window)`.
- After the change: **5 files, 35 tests passed**; the shared UI TypeScript check
  passed with no diagnostics.
- Studio's focused permission/options/composer/visual-feedback regression:
  **4 files, 48 tests passed** after resolving its nested source merge.
- A broader shared run including Switch and Tooltip produced **96 passed,
  1 failed**. The remaining Tooltip case was rerun alone and still fails: its
  existing assertion expects only `own-hint`, while installed Radix appends
  its own tooltip ID to the caller-provided `aria-describedby`. That assertion
  remains unchanged and the failure remains open.

## Explicitly not claimed

These are real shared-component tests in jsdom and a package typecheck, not
native WebKit acceptance. They do not certify every product popup call site,
the full Studio renderer typecheck, the Dashboard vendored copy, signed native
artifacts, or release readiness. Those delivery gates remain pending.
