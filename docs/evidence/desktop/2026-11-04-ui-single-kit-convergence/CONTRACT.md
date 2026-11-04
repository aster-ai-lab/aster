# Single-kit owner + ArcUI motion contract

Authoritative for the `/goal` “全面收敛到一套UI，对齐ArcUI设计风格和动效”. Complements `DESIGN.md` §5–6 and `inventory.json`.

## Sole product owner

| Layer | Role | Product may import? |
| --- | --- | --- |
| `@aster-ai-lab/ui` barrel / `components/primitives/*` | Shared control API | **Yes** (preferred) |
| `components/console/*` | Domain composites (SearchSelect, tables, wizards, FormDialog) | **Yes**, shrink-only for anything that duplicates a primitive |
| `components/composites/*` | Product composites (ConfirmDialog, …) | **Yes** |
| `components/arc/**` | CSS skin + `motionTokens` only (no React `.tsx`) | **No** product import of Arc React; CSS/`motionTokens` via primitives |
| Office `design-system/primitives` | Document outer chrome (migrating) | Prefer bridge to shared kit |
| `form` / `flow-ui` / `ce` / `intelligent-ui` | Domain runtimes | Domain APIs only — no second Button/Dialog/Tabs kit |

## Motion alignment (ArcUI ↔ Aster)

Values live in `packages/ui/src/components/arc/lib/motion-tokens.ts` and must match `DESIGN.md`:

| Recipe | Token / behavior | Owner |
| --- | --- | --- |
| Shared highlight | `motionTokens.spring.morph` (`visualDuration: 0.42`, bounce 0) | `SelectionHighlight` + `LayoutGroup` + `layoutDependency` |
| Segments / Tabs plate | Same morph spring; plate scoped per instance | `SegmentedControl`, `Tabs` (primitives) |
| Tabs panel exchange | Directional fade + optional height morph (`duration.standard` / `spring.smooth`) | `TabsContent` (primitives; Arc recipe) |
| Dialog / popup enter | 160ms (`duration.fast`) | primitives Dialog + dialog-motion.css |
| Dialog / popup exit | 200ms (`duration.exit`) | same |
| Press / glyph | `spring.snappy` 260ms | `primitives/arc-button` (via primitives Button) |

Reduced motion: plate does not travel; panels exchange in place with duration 0; dialogs skip leave delay per existing contract.

## Dual-family close order

From `inventory.json` dual families — product-facing first:

1. **tabs** — primitives owns API + Arc CSS + Arc panel motion + shared-highlight plate (`default` shell / `plain` rail); product must not freeze the plate with CSS `data-[state=active]` fills or `TabsTrigger asChild`; arc React Tabs is gallery-only ✅
2. **segmented-control** — primitives + Arc CSS + SelectionHighlight; Office DS also shares SelectionHighlight ✅
3. **button** — primitives owns press/morph (`arc-button.tsx`) + Arc CSS ✅ (Office keeps `ds-button` for document chrome)
4. **dialog** — primitives Dialog is product chrome; console `Dialog`/`FormDialog` now wrap primitives ✅; Office Dialog keeps focus/popover policy but uses shared scrim/surface motion tokens
5. **select** — product pickers = SearchSelect; `RADIX_SELECT_DEBT=[]` in Studio `noNativeSelect.test.ts` ✅
6. **arc React prune** — **zero** `components/arc/**/*.tsx`; `motionTokens` + CSS modules only; button React lives in `primitives/arc-button.tsx` (2026-11-04)
7. **toast** — Console `Toast`/`InlineBanner` → primitives toast + Alert ✅
8. **progress** — Console `RingProgress` → primitives `CircularProgress` ✅
9. **pagination / FilterChip** — domain APIs keep page math / chip labels; active plate uses `SelectionHighlight` ✅
10. **command** — Console `CommandPalette` → primitives `CommandDialog` / cmdk ✅
11. **table (simple)** — Console `Table` / `ConsoleTable` → primitives `Table*` + Card ✅

Full residual classification: [`console-residual.md`](./console-residual.md).

## Gates

- Studio product sources must not match deep imports of `@aster-ai-lab/ui/components/arc/` or relative `components/arc/*.tsx` (except tests/stories).
- Within `packages/ui`, `components/arc/**` must contain **no** `.tsx` React modules (CSS + `motionTokens` only).
- Studio product sources must not import removed colliding `Console*` UI aliases — use primitives/composites owners, `OffsetPagination`, and `Textarea.Input`.
