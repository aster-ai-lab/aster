---
description: Create a feature specification from the template, gated by the constitution.
---

## User Input

```text
$ARGUMENTS
```

## Procedure

1. Read `.specify/memory/constitution.md` and `Agents.md`.
2. Determine the next feature number from `specs/` (NNN, zero-padded).
3. Create the feature branch `git checkout -b <NNN-slug>` only when
   the user asked for a branch.
4. Copy `.specify/templates/spec-template.md` to
   `specs/<NNN-slug>/spec.md` and fill it from the input.
5. Fill every mandatory section. The House Checklist is a checklist.
   Every unchecked box is addressed by the plan or explicitly deferred.
6. Fail-closed phrasing: every requirement states what the system
   MUST refuse as well as what it MUST do.
7. Do not commit unless the user asked. When they do, stage files
   individually.

## Output

Report the spec path, the user stories with priorities, the gate
context, and any [NEEDS CLARIFICATION] markers.
