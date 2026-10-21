<a id="feature-specification-asterdream-as-desktops-default-tenant"></a>

# Feature Specification: Default AsterDream desktop product

**Created**: 2026-10-21
**Status**: Specified; implementation in progress
**Input**: Merge AsterDream into `apps/desktop` as the default tenant, retain its name/logo/theme, and retain the original desktop product under `legacy`.
**Clarification**: The user explicitly chose functionality and branding only. Old AsterDream data remains unchanged; no history, credentials, preferences or sessions are imported.

## Product contract

AsterDream is the shared desktop host. Its default tenant is `aster`, product **AsterDream**, with the existing Work blue/silver theme, transparent star/orbit logo and desktop icon. Explicit `legacy` remains **legacy**, retaining its current identity and appearance. Product names do not decide runtime capabilities.

| Tenant | Product | Host binary | Desktop bundle/app ID | Data ownership |
| --- | --- | --- | --- | --- |
| aster | AsterDream | `Desktop` | dev.a3s.desktop.aster | New AsterDream-owned profile; never the legacy Work profile |
| legacy | 通友租户 | `Desktop` | dev.a3s.desktop | Existing legacy profile and instance compatibility |

The shared host package version remains 2.1.3. Legacy AsterDream 0.1.x is not silently upgraded into this new product generation. Installed identity and an explicitly supplied launch tenant must agree. Development entry points select `aster`; invalid/empty explicit tenants refuse rather than falling back.

## Scenarios and requirements

- **FR-001**: Default `just dev`, `just dev`, and `just work` start AsterDream. Explicit `just dev` retains the original product. No launcher kills legacy AsterDream processes.
- **FR-002**: Loader, home, login, conversation/avatar, settings/about, window, Dock and packaged icons display the selected brand. Aster blue/silver light/dark defaults preserve user theme overrides; legacy remains visually unchanged.
- **FR-003**: Both products retain AsterDream's own workspace, focus/timer, composer and runtime behavior, including spec 003 planning, goals, approvals, model persistence and IME repairs. Exact tenant capabilities replace display-name-dependent behavior without changing existing declared fixture behavior.
- **FR-004**: Every native store, WebView, lock and runtime-discovery path isolates Aster's new profile from legacy Work and from legacy. Synthetic sentinels prove old credentials and preferences are not read or rewritten. No user profile is inspected during development.
- **FR-005**: Build, sealing, signature checks, archive validation and Linux packaging use the same declared tenant/product/ID contract. Missing resources, wrong tenant, wrong ID and mixed generations refuse. ACP and Office remain root-workspace companions.
- **FR-006**: Update detection/download/install must not cross products. Legacy's current server contract remains compatible; Aster must not consume that unscoped release as an Aster release. An unpublished product feed reports unavailable/not published truthfully and never offers or installs the other product. Live publishing/feed mutation is outside this request.
- **FR-007**: Product configuration remains ACL, trust boundaries remain in force, and no name/intent is guessed. Organization eligibility must not be weakened to retain a branding change. The `aster` build retains local/BYOK models with optional organization login; the `legacy` build retains managed organization admission. Local identity never grants organization access.
- **FR-008**: Hermetic before/after tests, actual renderer/host builds, two-tenant native GUI verification and independent review support the final outcome. Old fixture/package results do not qualify a new tenant or changed bytes.

- **FR-009**: Cargo/frontend build selection enables the selected product logic. Features are mutually exclusive; a running product cannot switch to another tenant or enable its unavailable provider logic. Build and installed identities must agree.
- **FR-010**: Task statuses use distinct semantic foreground colors with text/icons in light/dark themes; the planning switch has a visible thumb and correct pointer, keyboard, disabled and IME behavior.
- **FR-012**: Within one assistant turn, accepted plan updates replace a single stable progress card. Failed outputs do not replace accepted progress; completed plans default to a compact expandable summary; text/tool history and distinct plan revisions remain available.

- **FR-011**: Current product documentation and internal design prose use AsterDream/tenant names. Technical identifiers, licensing and historical evidence retain their original meaning.

## Scope relationship

This request explicitly replaces spec 003's assumption that Work and AsterDream remain separate desktop applications. Spec 003's behavior and verification obligations remain active inside the shared host. `apps/work` is retained as legacy source/provenance and for the Office kernel/vendor build dependency; removing it or migrating its data is not part of this change.

## Success criteria

1. Both selected tenant builds have correct names, assets, identities and isolated data; the default is AsterDream.
2. All new owning tests pass and the existing alignment regressions remain covered.
3. Real native GUI evidence covers both brands plus send/reply and the spec 003 planning/approval/goal flows on the shared host.
4. Evidence records the exact source/package generations, all commands and material platform/feed limitations. No commit, push, release publication or user-data migration is implied.
