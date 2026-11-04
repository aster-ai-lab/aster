# Office outer-chrome remaining (post Tabs/Segmented/SearchSelect)

Date: 2026-11-04. Builds on `2026-11-03-arc-ui-full-compliance/office-chrome-decisions.md`.

| Control | Status | Notes |
| --- | --- | --- |
| Select / picker | **Done** | `office-select` → `SearchSelect` |
| Tabs | **Motion shared** | Office `Tabs` uses `SelectionHighlight` + Arc morph; ribbon plate CSS preserved |
| Segmented control | **Motion shared** | Same SelectionHighlight bridge |
| Dialog | **Motion aligned** | Keeps Office focus/popover policy; uses `aster-dialog-*` + `DIALOG_UNMOUNT_DELAY_MS` |
| Button / IconButton | **Press motion shared** | Keep `ds-button` / loading roles; `whileTap` uses `motionTokens.spring.snappy` |
| Tooltip | **Shared** | `OfficeTooltip` → primitives `Tooltip` (no arrow, Office bubble class, floating portal) |
| Popover / portals | **Enter + exit presence** | `aster-popup-rise` + `usePopupPresence` (200ms hold, `data-state` open/closed); layered dismiss / portal policy kept (no Radix rewrite yet) |
| Engines | **Retain** | Document / calc / edit kernels |

### Deferred: Radix Popover rewrite

Blocked until a design preserves all of:

1. `openPopoverLayers` stacking + `hasOpenOfficePopover()` for dialog close gating
2. Floating placement + `PORTAL_INHERITED_CUSTOM_PROPERTIES` on portal root
3. Render-prop trigger API used by ribbon / toolbar call sites
4. Shared 160/200 popup motion (already landed via `usePopupPresence`)

Unblock evidence: a failing→passing Office suite that keeps (1)–(4) while the panel is a primitives/Radix `Popover` surface; `packages/office/tests/popover-presence.test.tsx` alone is insufficient.

Until then, Office Popover is **motion-aligned, owner-custom** — not a second product chrome kit.

### Deferred: uiarc.dev screenshot parity (non-Office visual QA)

No screenshot / image-diff harness exists under `packages/ui` yet. Owner + motion contract for product chrome is gated by unit tests (`motion-tokens.test.ts`, ownership gates). Gallery pixel parity remains a separate visual QA track.
