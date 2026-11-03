# Implementation Plan: Consistent Arc foundations

**Branch**: `main` | **Spec**: [spec.md](spec.md)
**Gate context**: UI package and Studio product paths; shared foundation consistency.

## Summary
Normalize the existing Arc foundation and adapters, lift composer's measured hover plate into the UI owner, wire shared choice surfaces, and remove duplicate Studio hover code. Review every primitive; change only a confirmed inconsistency. Preserve concurrent edits.

## Constitution Check
| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Disabled and unknown choices retain their owner guards |
| II. Evidence retained | PASS | Desktop evidence directory from the spec |
| III. Hermetic tests | PASS | UI package and browser suites |
| IV. Closed sets | PASS | No new business enum |
| V. Workspace / APIs | PASS | Existing public props and local dependencies |
| VI. Schema changes | N-A | Presentation-only |
| VII. Ratchets | PASS | No allowlist changes |
| VIII. Transports | N-A | No transport change |
| IX. Configuration | N-A | No configuration added |
| XIII. One repository | PASS | Current checkout and existing Studio submodule |
| XIV. Names | PASS | AST declarations/imports and actual DOM roles |
| XV. Meaning | PASS | Owner callbacks retain business decisions |
| XVI. Drawn pickers | PASS | Existing Radix/Command pickers |
| XVII. In-place dialogs | PASS | Existing modal scale/opacity contract |

## Project Structure
- `packages/ui/src/components/arc`: semantic material, sizes and existing control recipes.
- `packages/ui/src/components/primitives`: common hover implementation and choice-surface wiring.
- `packages/ui/src/components/console`: SearchSelect reuses the same owner.
- `apps/studio/src/renderer`: shared helper adoption; delete redundant composer code.
- `packages/ui/stories` and existing Studio E2E harness conventions: actual component review and browser coverage.

## Research
[research.md](research.md) records the existing implementation and official behavioral references. `contracts/components.json` freezes definitions, exports and static callers; no runtime registry is introduced.

## Verification Plan
UI focused and package tests; package typecheck; scoped source checks; existing theme contract; Chromium/WebKit component and workbench tests. Record preexisting failures separately. Native windows are checked only with a verified exact-window binding. Windows/Linux native behavior is not inferred from browser runs.
