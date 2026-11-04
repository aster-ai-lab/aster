# Console residual classification (2026-11-04)

Rule: anything that duplicates a general chrome control must wrap primitives. Domain composites may keep APIs.

## Bridged / shrink-only (no second chrome owner)

| Console module | Owner skin / motion |
| --- | --- |
| Dialog / FormDialog / ConfirmDialog / ExportDialog | primitives Dialog |
| Drawer | primitives Drawer |
| DropdownMenu | primitives DropdownMenu |
| Textarea | primitives Textarea (`Textarea.Input`); console fill wrapper not barrel-exported |
| FormField | primitives Field |
| Toast / InlineBanner / ErrorBanner | primitives toast + Alert |
| PopConfirm | Button + usePopupPresence |
| EmptyState | Button + layout (shrink-only) |
| CommandPalette | primitives CommandDialog / cmdk |
| RingProgress | primitives CircularProgress |
| Pagination / CursorPager | Barrel name `OffsetPagination` + SelectionHighlight plate; page math stays here |
| FilterChip | optional SelectionHighlight |
| Table (ConsoleTable) | primitives Table* + Card |
| IdentityStatusBadge / RoleChipList | primitives Badge |
| SearchSelect | product picker owner (DESIGN); not Radix Select twin |

## Domain composites (intentional — not general chrome twins)

| Group | Modules |
| --- | --- |
| Tables / trees | DataTable, VirtualizedTable, Tree, TreeSearchPanel, PermissionMatrix |
| Wizards / layout | InviteWizard, WizardShell, PageHeader, PageLoading, ListToolbar, SectionHeader, DirectorySplit, NavLinkList, ResourceMetaHeader, DetailSection, DescriptionList, MetaSummary |
| Pickers / calendar | Cascader, DateRangePicker, calendar (dayKey grid for range; ≠ DayPicker) |
| Cards / identity | Card, Stat, RoleCard, PersonCell, Logo, OrgMark, DangerZone |
| Misc domain | FileDropzone, CodeBlock, Timeline, SecretReveal, Reveal, ConfettiBurst |

## Explicitly deferred (Office only)

- **Office Popover → Radix**: exit presence already shared (`usePopupPresence` 200ms). Full Radix rewrite blocked on preserving layered dismiss (`openPopoverLayers` / `hasOpenOfficePopover`) + floating portal property inheritance. Tracked in `office-remaining.md`.
- **Office Button visual swap**: press motion shared; `ds-button` document chrome retained. Tracked in `office-remaining.md`.

## Non-Office closed this evidence day

- Colliding general-chrome `Console*` barrel aliases removed (`OffsetPagination` / `FormDialog` / owner primitives remain).
- Arc React prune complete: **zero** `components/arc/**/*.tsx`; button press/morph in `primitives/arc-button.tsx`; CSS + `motionTokens` stay under `arc/`.
- `motionTokens` locked to DESIGN.md durations/springs via `motion-tokens.test.ts`.
