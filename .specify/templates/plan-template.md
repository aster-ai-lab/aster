# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Spec**: [`specs/[###-feature-name]/spec.md`] | **Input**: [`specs/[###-feature-name]/spec.md`]

**Gate context**: [Owning app or crate and slice]

## Summary

[One paragraph: what lands, and which existing module it extends.]

## Constitution Check

*GATE: Re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE
violation stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | [PASS/VIOLATES] | [where] |
| II. Evidence retained | [PASS/VIOLATES] | [evidence target] |
| III. Hermetic tests | [PASS/VIOLATES] | [test target] |
| IV. Closed sets | [PASS/VIOLATES] | [enums added] |
| V. One workspace / published APIs | [PASS/VIOLATES] | [crates touched] |
| VI. Local schema changes | [PASS/VIOLATES/N-A] | [owner] |
| VII. Shrink-only ratchets | [PASS/VIOLATES/N-A] | [allowlists touched] |
| VIII. Replaceable transports | [PASS/VIOLATES/N-A] | [transport surface] |
| IX. One configuration language | [PASS/VIOLATES/N-A] | [ACL changes] |
| XIII. One repository | [PASS/VIOLATES] | [no new repo] |
| XIV. Names are not guessed | [PASS/VIOLATES/N-A] | [name source] |
| XV. Meaning is not matched | [PASS/VIOLATES/N-A] | [decision source] |
| XVI. Pickers are drawn | [PASS/VIOLATES/N-A] | [picker] |
| XVII. Dialogs stay in place | [PASS/VIOLATES/N-A] | [motion] |

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature-name]/
├── plan.md
├── research.md
├── data-model.md
└── contracts/
```

### Source Code

| Layer | Path | New files |
| --- | --- | --- |
| CLI | `apps/aster/` or `crates/code/` | [files] |
| AsterDream | `apps/work/` | [files] |
| Studio | `apps/studio/` | [files] |
| Library | `crates/<name>/` | [files] |
| UI package | `packages/<name>/` | [files] |

## Research

For each decision, name the in-repo prior art: the file that already
does this, and why a new pattern is required if one is.

## Verification Plan

- Hermetic test: [package and filter]
- Ratchet that must stay green, if this feature touches one
- What is explicitly not run (no live service, no full workspace suite)
