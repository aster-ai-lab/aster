# Feature Specification: Aster/ToWork shell parity to the no-accounts boundary (phase B)

**Feature Branch**: none (not requested; work happens in the working tree)

**Created**: 2026-10-22

**Status**: Draft

**Input**: Owner directives 2026-10-22, in order: ① both tenants keep the
ActivityBar (icon-rail) shell — the in-flight "remove the rail for aster"
work was reverted the same day; ② Aster (`just dev`) and ToWork/toyou
(`just dev:toyou`) share identical UI/UX and functionality; ③ differences
are limited to name, logo, and per-tenant default theme color (specs/004
FR-002: Aster blue-silver + star-trail mark, org product original); ④
functional parity stops at the no-accounts boundary (this spec, phase B);
full parity including login/org/IM is phase A, a separate future epic that
must amend specs/004 FR-007/FR-013/FR-014 first.

**Gate context**: Owning app is `apps/studio` — private submodule, product
regime (hermetic Vitest tests). Continues the `specs/004-desktop-default-aster`
lineage without reopening its account contract. No studio ROADMAP row names
it; it exists by owner directive 2026-10-22. The phase-B baseline already
landed in the working tree (uncommitted): the dead session's asterShell lane
was reverted — `AppShell.tsx`, `AppShell.test.tsx`, `WorkbenchHome.tsx`,
`WorkbenchHome.test.tsx` restored to HEAD, `AsterShellNav.tsx` deleted,
orphan i18n key `workbench.nav.section.use` removed from both locales;
references verified zero; AppShell + WorkbenchHome suites 124/124 green.
That revert commits with this epic's evidence, not before.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One shell, two brands (Priority: P1)

A user launches `just dev` (aster) and `just dev:toyou` side by side and
sees the same product: same §11 icon-rail frame, same sidebar, same pages,
same interactions. The only differences are the name, the logo, and the
default theme accent.

**Why this priority**: This is the owner's directive; everything else serves
it.

**Independent Test**: apps/studio renderer Vitest — render the shell in both
tenant modes; assert identical chrome elements (rail present in both, same
`data-ui` shell parts) and assert brand values differ only through
`tenantBrand.ts` outputs (product name, mark).

### User Story 2 - Divergence inventory is the law (Priority: P1)

Every `PUBLIC_STUDIO_TENANT === 'aster'` gate in the renderer, plus the
native `tenant-aster` / `tenant-toyou` feature split surface, is classified
in a table (this spec's tasks) as either **account-bound** (kept: the
feature requires managed identity, org membership, or the OS catalog) or
**unify** (removed: the divergence has no account reason). Unify gates are
deleted with parity tests; account-bound gates stay and are listed.

**Why this priority**: "对齐到无账户边界" is only enforceable as an explicit
classified set; an unlisted gate is an unreviewed divergence.

**Independent Test**: for each gate flipped to unify, a hermetic test asserts
the two tenant modes render the same element; the inventory table itself is
reviewed in analyze/implement phases against a fresh gate scan.

### User Story 3 - Account-bound entries hide fail-closed (Priority: P1)

On aster, destinations that depend on managed identity or org scope
(消息, 团队/组织, 平台, and any org-catalog surface so classified) do not
render — the row does not exist, it is never a disabled dead row, and no
org/login entry leaks into the local build. specs/004 FR-007/FR-013/FR-014
stay binding.

**Why this priority**: Parity must not smuggle the org product into the
local build; that is the phase-A decision, not this one.

**Independent Test**: existing + tightened renderer tests — aster mode
asserts absence (not disabled state) of classified destinations;
`LOCAL_HIDDEN_APP_IDS` membership matches the classified set.

### User Story 4 - Per-tenant default theme accent (Priority: P2)

Aster defaults to the blue-silver accent; the org product keeps its
original default. A user's saved theme choice overrides the default on both.
Theme changes color tokens only; it never alters layout, spacing, or chrome
structure.

**Why this priority**: The visible brand difference the owner asked to keep;
P2 because the mechanism is expected to exist from specs/004 (status
verified in plan).

**Independent Test**: renderer Vitest on the theme-default resolution —
aster/toyou defaults resolve to their accents, a saved preference wins on
both, and the resolved theme carries no layout-affecting keys.

### User Story 5 - The design law stays single (Priority: P2)

`DESIGN.md` keeps describing ONE chrome. If any wording implies aster
diverges structurally, it is corrected; the per-tenant default accent gets
at most one sentence beside the existing tenant-brand statement. No second
`DESIGN.md`, no dual-state §11.

**Why this priority**: The reverted lane would have forced a dual-state
spec; phase B must not reintroduce that complexity by the back door.

**Independent Test**: docs review; `pnpm docs:check` / `pnpm lint` gates on
touched files.

### Edge Cases

- A gate whose account-dependence is ambiguous (e.g. the plugin catalog is
  OS-org-sourced while capabilities install locally): classified by what its
  data source requires, not by guesswork; ambiguous cases park as
  account-bound until phase A.
- A unify flip would surface a feature whose backend is org-only on aster:
  the flip is refused and reclassified account-bound; the refusal is named
  in the inventory.
- User-saved theme from before an accent change: the saved choice wins; no
  silent re-defaulting.
- Tenant mismatch at dev time: `devServerTenant.mjs` refusal unchanged.
- specs/004's native mutual exclusion (`tenant-aster` / `tenant-toyou`)
  stays; phase B does not merge cargo features.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Both tenants MUST render the identical DESIGN.md §11 chrome.
  The shell components MUST NOT carry tenant-conditional layout branches
  outside the classified inventory; any such branch is a defect.
- **FR-002**: Per-tenant differences MUST be limited to: product name,
  wordmark/logo, default theme accent, and inventory-classified
  account-bound gates. Anything else that diverges between the tenants is a
  defect and MUST be unified in this epic.
- **FR-003**: The inventory MUST cover at least these known gate sites
  (verified in plan): `utils/sidebar.ts` (`LOCAL_HIDDEN_APP_IDS`,
  temporarily-hidden set), `im/useIm.ts`, `hooks/useAvatar.ts`,
  `hooks/useUserName.ts`, `windows/main/MainApp.tsx` (onboarding),
  `chat/messages/frame/MessageHeader.tsx` +
  `useMessageHeaderCapabilities.ts`, `hooks/useAssistantCatalogPresets.ts`,
  `components/layout/ShellTabBarActions.tsx`, `components/app/Sidebar.tsx`,
  `components/settingsMenu.ts` (additive local-model pages are aster-only
  BYOK surfaces — classify, do not delete), `tenantBrand.ts`, and the
  native `tenant-aster`/`tenant-toyou` feature surface list.
- **FR-004**: Account-bound destinations MUST NOT render on aster
  (row-not-exists, never disabled); aster MUST NOT gain login, org, or
  managed-identity entries (specs/004 FR-007/FR-013/FR-014 binding).
- **FR-005**: Default theme accent per tenant (Aster blue-silver, org
  original); a saved user theme overrides on both; theme MUST NOT change
  layout tokens. A tenant-keyed layout difference is refused.
- **FR-006**: No locale divergence beyond product-name strings; all new
  strings follow the i18n gate (`en-us.json` source, `i18n:sync`).
- **FR-007**: Data isolation is unchanged: bundle/app IDs
  (`dev.aster.studio.aster` / `dev.aster.studio`), stores, locks, WebView and
  runtime namespaces stay separate. **NON-NEGOTIABLE** even under future
  phase A.
- **FR-008**: The 2026-10-22 revert (shell files to HEAD, `AsterShellNav`
  deleted, orphan key removed) is part of this epic and lands together with
  its evidence when the user asks to commit; it is not committed separately
  or swept into another lane's commit.
- **FR-009**: No native dropdown appears in any flipped surface (ratchet
  VII); any new chooser is the drawn picker.

### Key Entities

- **Tenant stamp**: `PUBLIC_STUDIO_TENANT` (renderer, compile-time) and
  `ASTER_TENANT_SLUG` (dev/build); `scripts/studio_product.py` resolves
  `tenant-aster` / `tenant-toyou` cargo features.
- **`tenantBrand.ts`**: the only sanctioned brand surface (name, marks,
  company strings).
- **Divergence inventory**: the classified table produced by this epic's
  plan/tasks; the review contract for every future tenant gate.

## House Checklist *(mandatory — from Agents.md)*

- [x] New behavior has a small hermetic test in the owning package — per-gate
      parity/absence tests named in the stories (apps/studio renderer suites)
- [ ] Closed sets: the divergence inventory is the closed set; verified in
      plan's constitution check
- [x] No production `unwrap()` / `expect()` — TypeScript surface; lint gates
- [x] Internal `aster-*` dependencies stay on local paths — no new deps
- [x] Product configuration, if any, is ACL — none introduced
- [x] Evidence target:
      `docs/evidence/desktop/2026-10-22-tenant-parity-phase-b/OUTCOME.md`
      when the landing is recorded (includes the side-by-side captures)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Renderer suites for every touched component pass
  (`./node_modules/.bin/vitest run --project renderer <files>`), zero
  failures, without weakened assertions.
- **SC-002**: A fresh `PUBLIC_STUDIO_TENANT` gate scan finds no site outside
  the inventory (recorded in the evidence OUTCOME).
- **SC-003**: Side-by-side `just dev` / `just dev:toyou` captures in the
  evidence directory show identical chrome, differing name/logo/accent
  only.

## Assumptions

- Phase A (login/org/IM on aster) is out of scope and requires a specs/004
  amendment first; nothing in this epic pre-implements it.
- The per-tenant default accent mechanism exists from specs/004 work or its
  completion becomes a task here (decided in plan).
- The dead session's other uncommitted lanes (CUA, knowledge compile,
  documents, taskConversation, dev server tenant script, e2e) remain
  untouched and are triaged separately.

## Explicitly Not Claimed

- No login, org membership, managed identity, or IM on aster (phase A).
- No toyou feature removal; parity fixes may touch shared components but
  never delete org-product behavior.
- No native cargo-feature merge; `tenant-aster`/`tenant-toyou` stay
  mutually exclusive.
- No theme editor, no new theme tokens, no dark-mode work.
- No triage of the dead session's other lanes.
