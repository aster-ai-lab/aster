# Implementation Plan: Default AsterDream tenant

Use the existing AsterDream tenant pipeline and shared ACP. Copy only canonical Work brand assets; do not copy its renderer, product alias table, runtime or user state. Keep Legacy's legacy brand and current bundle/profile identity. Aster gets `dev.a3s.desktop.aster` and a fresh profile namespace.

## Constitution check

| Principle | Result | Implementation constraint |
| --- | --- | --- |
| I/IV fail closed, closed sets | PASS | Explicit tenant identity; unknown/mismatched identities refuse |
| II/III/XII evidence, tests, review | PASS | Owning RED/green, two-tenant build/GUI and independent adversarial review |
| V/VI workspace, schemas | PASS | One root workspace; existing stores remain owned; no implicit migration |
| VII/XVI/XVII UI ratchets | PASS | Existing controls/primitives; no native select or allowlist expansion |
| VIII/IX configuration | PASS | No new provider/policy config format; tenant branding remains definition data |
| XIII/XIV/XV repository, names, meaning | PASS | Same repository, explicit product contract, typed behavior |

## Ownership and caller closure

- Brand/UI owner: tenant assets and loader/stamper, renderer brand surfaces and theme, explicit tenant capability callers and tests.
- Native owner: tenant/profile/bundle/WebView/lock/runtime discovery, all store callers and sibling isolation tests; model-admission investigation.
- Delivery owner: root entry points, Tauri selected build identity, development launcher, package sealer and Kylin pipeline/tests.
- Root: bounded spec, native updater identity/feed refusal, integration builds, real CUA, governance/provenance and completion audit.

Tests use newly created synthetic profiles and model/update services. HOME is not replaced. Legacy Work state is never opened. No changes to live release metadata or API/control-plane persistence are required for an unpublished Aster feed.

## Analyze result

No unresolved requirement/ownership conflict. User clarification resolves the migration question. The original shared bundle ID and profile collision are confirmed defects and addressed by FR-004/005. Aster's publication is not inferred from an existing legacy feed. Spec 003 remains a dependency; its unchecked interactive requirements stay incomplete until observed.
