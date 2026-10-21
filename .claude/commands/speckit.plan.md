---
description: Produce the implementation plan with the constitution check table.
---

## User Input

```text
$ARGUMENTS
```

## Procedure

1. Read `.specify/memory/constitution.md`, `Agents.md`, and
   `specs/<NNN-slug>/spec.md`.
2. Copy `.specify/templates/plan-template.md` to
   `specs/<NNN-slug>/plan.md`.
3. Fill the Constitution Check honestly. A NON-NEGOTIABLE violation
   stops the plan.
4. Fill the source table with paths in this workspace: `apps/aster`,
   `apps/work`, `apps/desktop`, `crates/`, or `packages/`. Do not plan
   an `apps/cloud` module.
5. Name the in-repo file that already does each chosen pattern.
6. Do not commit unless the user asked.

## Output

Report the constitution table, the file plan, and any deviations.
