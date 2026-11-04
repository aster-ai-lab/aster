# Single-kit ArcUI convergence

Date: 2026-11-04. Host: macOS arm64. Goal: one product UI kit aligned with ArcUI look and motion.

## Objective (unchanged)

Studio / Console product chrome converges on **`@aster-ai-lab/ui` as the sole shared owner**, with visual and motion behavior aligned to ArcUI / `DESIGN.md` (shared-highlight ~420ms morph, dialog/popup 160/200, segmented/tabs plate travel). Parallel primitives / console / arc React owners and product bypass chrome shrink to zero for general controls. Office outer chrome migrates per control. Domain packages (`form`, `flow-ui`+`ce`, `intelligent-ui`) keep domain APIs but must not invent a second general chrome kit.

## Frozen inventory (source)

Machine snapshot: [`inventory.json`](./inventory.json). Owner + motion rules: [`CONTRACT.md`](./CONTRACT.md).

| Finding | Count / note |
| --- | --- |
| Product kit package | `@aster-ai-lab/ui` only for Studio shell (~800+ imports) |
| Families with **both** `primitives/*.tsx` and `arc/**/*.tsx` | **0** (button React absorbed into `primitives/arc-button.tsx`; arc is CSS + `motionTokens` only) |
| Console modules re-exported on the same barrel (`Console*` / SearchSelect / …) | **51** alias export lines |
| Studio deep-import of `components/console/*` | essentially **PopConfirm** + barrel `SearchSelect` |
| Studio import of `components/arc` React | **0** (`components/arc/**/*.tsx` count = 0) |
| Studio `role="tablist"` surfaces | **13** (most use kit Tabs / SelectionHighlight / WorkspaceTab; Plan editor panels now use SelectionHighlight) |
| Office design-system primitives | **13** files; picker → SearchSelect; Tabs + SegmentedControl now use shared `SelectionHighlight` |

## Owner rules (binding)

See [`CONTRACT.md`](./CONTRACT.md). Summary: product never imports `components/arc/**` React; primitives own shared API + Arc skin/motion; console composites are shrink-only when they duplicate primitives.

## Progress

- Goal armed; inventory frozen; contract written.
- Prior: Tabs / SegmentedControl / SelectionHighlight morph spring (`visualDuration: 0.42`) + LayoutGroup.
- Earlier this day
  - Primitives `TabsContent` ports Arc directional panel exchange + height morph (`motionTokens`).
  - Studio gate `singleKitOwnership.test.ts` bans product Arc React deep-imports; UI package gate limits Arc React to primitives/arc.
  - `components/arc/index.ts` quarantined / `@deprecated` for product Arc* exports.
  - Plan editor inspector tabs → `SelectionHighlight` + `LayoutGroup`.
  - Office Tabs + SegmentedControl → shared `SelectionHighlight` (ribbon plate preserved).
  - Barrel exports `SelectionHighlight`.
  - Console `Dialog` / `FormDialog` / `Drawer` / `DropdownMenu` / `Textarea` / `FormField` / `PopConfirm` → primitives + Arc motion.
  - Office Button press / Tooltip / Popover exit presence aligned.
  - Pruned unused Arc React twins — only `arc/button` + `motionTokens` remain; CSS modules keep the skin.
- **Prior this day** — Console Toast/FilterChip/Pagination/RingProgress/CommandPalette/Table bridged; residual classified; MigrationApp / DisplayLanguage / WebSearch → SearchSelect; `singleKitOwnership` restored; Office remaining tracked.
- **Prior this day (motion + Select debt)**
  - Fixed product `SelectionHighlight` call sites missing required `layoutDependency` (WorkspaceTab, WorkbenchLineagePane, WorkbenchModelPicker, TasksSettings status nav, PluginStore category, ConnectorManage transport).
  - Added Arc plate travel: Plan editor inspector tabs, InboxHome lane tabs, Org repo/CR underline tabs, AccountCenterMenu scope/join segmented tabs, HarnessFigure story tabs.
  - Radix Select → SearchSelect across product debt list including TasksSettings, AssistantEditDialog, CodeBlockNodeView.
  - `RADIX_SELECT_DEBT` is now **[]** (product `SelectTrigger` count = 0).
- **This turn (barrel preferred names + plate keys)**
  - Barrel preferred `OffsetPagination`; dropped colliding `Console*` aliases (`ConsoleDialog` / `ConsoleTable` / `ConsoleToastProvider` / …) and `ConsoleTextarea`; `generate-barrel` restores `rowMotion` public exports.
  - Studio + dashboard `CollectionPagination` → `OffsetPagination`; `QuestionForm` → `Textarea.Input`.
  - `WorkspaceEditorTabs` and Workbench paperloom/presentation/plugin tabs set `data-workspace-tab-key` so plate `layoutDependency` is stable.
  - Product `SelectionHighlight` imports use the public barrel (no deep `primitives/selection-highlight` paths).
  - Studio ownership gate bans product `ConsolePagination` / `ConsoleTextarea`.
- **This turn (non-Office close-out)**
  - Absorbed Arc Button React into `primitives/arc-button.tsx`; deleted `components/arc/button/button.tsx`.
  - `components/arc/**/*.tsx` = **0**; `arc/index.ts` exports `motionTokens` only.
  - Studio ownership gate asserts zero Arc React modules; `motion-tokens.test.ts` locks DESIGN.md 160/200 / morph 0.42 / snappy 0.26.
- **This turn (Tabs plate best practice)**
  - Kit `Tabs` / `TabsTrigger`: `variant="plain"` for rails; label wrap + z-index; `asChild` skips plate (Slot single-child).
  - Studio: `EditDialogShared` vertical rail → plain + transparent active (no `asChild` MenuItem fill); `ConnectorManageDialog` transport → `SelectionHighlight` for all tenants; `ImportAssistantDialog` → plain rail (dropped CSS `data-[state=active]:bg-secondary`); sidebar selected plate via morph (`SidebarMenuMotion`).
  - Capabilities connector paste tests wait for Arc `TabsContent` exit before asserting the other panel’s fields.

## Verification

| Check | Result |
| --- | --- |
| Inventory + CONTRACT | written under this evidence dir |
| `packages/ui` tabs + selection-highlight + motion-tokens | **pass** (14 tests) this turn |
| Studio `singleKitOwnership` + `noNativeSelect` | **pass** this turn; `RADIX_SELECT_DEBT=[]`; arc `.tsx` = [] |
| Studio TasksSettings suite | **pass** (103 tests) this turn |
| Studio Capabilities connector tab / transport cases | **pass** (paste + import + Aster transport); 3 install-row cases still race on `aria-label=installing` (unrelated to Tabs plate) |
| EditDialogShared + ImportAssistant overlay | **pass**; EditDialogs group-field combobox cases fail on SearchSelect role (pre-existing vs Tabs) |
| Public barrel colliding `Console*` aliases | **removed** |
| **Non-Office owner + motion contract** | **complete** (see CONTRACT dual-family list; Office rows deferred) |
| Full product+Office single-kit completion | **not claimed** — Office Popover Radix / Office Button visual swap remain |

## Explicitly not claimed

- Office Popover Radix rewrite (exit presence landed; layered dismiss kept custom — see `office-remaining.md`)
- Full Office Button visual swap onto Arc Button (press motion only)
- Remaining domain console modules listed in `console-residual.md` (tables/wizards/pickers/layout) — intentional composites, not a second chrome kit
- Visual screenshot parity with uiarc.dev gallery (no image-diff harness yet; motion numbers are gated in unit tests instead)
- **Office outer-chrome complete** — see `office-remaining.md`
