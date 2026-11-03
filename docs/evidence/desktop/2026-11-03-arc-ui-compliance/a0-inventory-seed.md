# A0 inventory seed (from ArcUI compliance audit + B0–B4)

Status: **seed only** — not a frozen A0 checklist. Promote rows into
`specs/<A0-batch>/contracts/` when A0 starts; do not treat this file as A0 complete.

Frozen identity for this seed: local Aster Studio renderer + `@aster-ai-lab/ui` as of
2026-11-03 compliance work. Build/git SHAs are not frozen here.

## Surface classes (route × control family)

| Class | Representative paths | Control notes after B0–B4 | A0 follow-up |
| --- | --- | --- | --- |
| Settings pickers | Provider drawer, Tasks, Usage, WebSearch, Channels, Language, Appearance, Data, Device, MCP, OCR/FileProcessing | Product `SearchSelect`; `RADIX_SELECT_DEBT=[]` | A2 model pilot overlaps Provider; verify save/fail/keyboard |
| Workbench chrome | History filters, PermissionMode, resourceCatalog dialogs | SearchSelect | Confirm each dialog open state + empty/fail |
| Code / Org / onboarding | CurrentConfigPanel, OrgDataScope, Onboarding, Import/Export, MigrationApp | SearchSelect | Auxiliary window entry via main query params |
| Segmented / tabs | PlanEditorWindow, WorkScheduledTasks, AccountCenterMenu ScopeTab, CapabilitiesPage chips, TaskDiffView | SegmentedControl / SelectionHighlight (~420ms) | Reduced-motion + aria-checked matrix |
| Search fields | ResourceCatalogSearchInput, MiniAppsPage, PluginStorePage, GlobalSearch | 30px borderless `#f5f5f5` / SearchInput sm | Catalog empty + focus ring |
| Floating chrome | Sidebar (floating), UpdateFab, scroll FAB | In-place opacity/scale; elevated shadow; 8–12px radius FABs | Platform window chrome out of scope |
| Scale feedback | Launchpad, WorkbenchApplets, ProviderLogoPicker | Hover scale capped ~0.97 | Reduced-motion |
| Shared UI hygiene | `packages/ui` Select*, Arc dialog, motionTokens, legacy font stack | Select deprecated; Arc Dialog/Select not on product barrel; Ubuntu sans; dialog 160/200 | Calendar/CompositeInput internal pickers still open |
| Office / Analysis | Office primitives, form/flow outer chrome | Not migrated in B0–B4 | A6 decision per control; keep engines |
| Out of product denominator | guest nativeStyleInspector, third-party plugin DOM, CLI/TUI, system dialogs | Excluded / N/A | Record as 有证据的范围外 when A0 freezes |

## Component family map (seed)

| Family | Product default | Legacy / gallery | Delete condition |
| --- | --- | --- | --- |
| Picker | `SearchSelect` / `Combobox` | Radix `Select*` (deprecated, debt empty in Studio renderer) | No product import of SelectTrigger |
| Segmented | `SegmentedControl` + `SelectionHighlight` | Hand-rolled white-plate tabs | No instantaneous background swap as sole selected affordance |
| Dialog / popover | `styles/dialog-motion.css`, `popup-motion.css` (160/200, opacity+scale) | Arc gallery Dialog adapted; not barrel-exported for product | No travel/`filter` blur on product owners |
| Search | `SearchInput` size sm / catalog equivalent | Ad-hoc bordered inputs | Match DESIGN Search |
| Type | Ubuntu / system UI stack | Inter default removed from legacy-compat `--font-sans` | No Studio-visible Inter default leak |

## Explicit gaps for A0 (do not claim covered)

- Full route × state matrix (loading / empty / fail / offline / readonly).
- Performance baseline before/after Arc.
- Every auxiliary window (Wiki, applet, mail compose, visual feedback) control inventory.
- Office toolbar / selection / portal controls.
- Cross-platform (Windows / Linux) native interaction proofs.
