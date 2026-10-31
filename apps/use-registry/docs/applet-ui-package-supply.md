# Applet UI Package Supply (Registry)

Status: admissions + package source + committed `registry/` republish delivered (P5)

Canonical multi-project plan (aster monorepo):
[`docs/work-applet-plugin-path.md`](../../docs/work-applet-plugin-path.md).

## Ownership

The Use Registry and signed packages supply **Applet** UI surfaces
(`ui` / activity-bar metadata: title, icon, order) with declared Tool/MCP
dependencies. They do not project into Code or render pages.

## Delivered

1. Admitted package source: `packages/applet-demo` (`aster/applet-demo@0.1.0`)
   with Tool `echo`, MCP `context`, Skill `applet-demo`, UI `panel`.
2. Admission record in `admissions.acl` (+ `permission-ceiling.json`).
3. Authoring rules below.
4. First-principles gate Track A in `scripts/test_just_registry.sh`
   (assemble admissions → planning tool/mcp → plan-install catalog ui/panel →
   apply-plan → capability `activity_bar` digest equals signed archive HTML)
   plus Track J6 (committed `registry/` just transport → plan-install
   `aster/applet-demo` catalog ui/panel + archive HTML bytes) and Track S1
   (package source ACL bytes == committed archive ACL — fail closed on
   custody lag / `bind_tool` drift).

## Authoring rules

1. Put package sources under `packages/<name>/` with `aster-use-extension.acl`.
2. UI assets are package-relative paths (`entry` / `styles` / `scripts`). Digests
   are computed at assemble time — do not hand-edit digests in the manifest.
3. Declare `title`, `description`, `icon`, and `order` on `ui` blocks so hosts
   can project ActivityBar entries without Host-local metadata.
4. `bind_tool` / `bind_mcp` (and Skill `requires_*`) must reference ids that
   exist in the same package. Incomplete edges fail closed at Use publish.
   Hosts project UI `activity_bar` dependencies from UI binds (`bind_*` +
   `skill`). `applet-demo` binds Tool `echo`; Track A A3/A4 (fresh admissions
   assemble) and Track J6 (committed `registry/` archive) both assert
   `bind_tool=["echo"]` plus MCP/Skill deps — package source and signed
   archive must not drift.
5. Executable Tool/MCP surfaces require a reviewed `permission_ceiling` JSON on
   the admission.
6. Add an `admission` block in `admissions.acl`; do not invent a second publish
   path.
7. Operators republish the signed `registry/` tree with custody keys
   (`docs/operators.md`). CI verifies both the committed tree and a fresh
   assemble from admissions.
8. Keep Host projection, WebView, and renderer code out of this repository.

## Exit evidence

- [x] Example package with `ui` + Tool/MCP dependencies exists and assembles
- [x] Gate proves planning + plan-install catalog surfaces (Track A A1–A3)
- [x] Gate proves Plugin Manager apply publishes `activity_bar` whose entry
      digest matches signed package HTML bytes (Track A A4) — not host-built UI
- [x] Committed `registry/` targets include `aster/applet-demo` (dev-preview
      republish; bootstrap root pin refreshed with metadata expiry resign)
- [x] Track J6: just-served committed tree plan-installs `aster/applet-demo`
      with ui/panel surfaces and UI requires Tool `echo` + MCP `context` +
      Skill `applet-demo`; archive manifest `bind_tool=["echo"]`;
      `targets.json` sha256 matches archive bytes; apply-plan `activity_bar`
      entry digest equals committed HTML with the same Tool/MCP/Skill deps
- [x] Track S0: git HEAD `targets.json` advertises `aster/applet-demo` archive
      (clean-clone durable P5 tip)
- [x] Track S1: `packages/applet-demo/aster-use-extension.acl` byte-equals the
      ACL inside the committed archive (fail closed on source↔signed drift)
- Monorepo verify: `just test::applet-non-work` (includes Track A/J6/S0/S1
  via `scripts/test_just_registry.sh`)

## Non-goals

- Work UiHost implementation
- Cloud-rendered package UI as source of truth
- Second package mutation path outside Use
