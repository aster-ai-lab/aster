---
description: Amend the project constitution (.specify/memory/constitution.md).
---

## User Input

```text
$ARGUMENTS
```

You MUST consider the user input before proceeding (if not empty).

## Scope Guard

This command only updates `.specify/memory/constitution.md` and, when
the rationale changes, `docs/development-standards.md`. Do not start
feature work here. If the input includes feature work, record it as
a deferred intent.

## Procedure

1. Read `.specify/memory/constitution.md`,
   `docs/development-standards.md`, and the recent evidence they govern.
2. Classify the input as a principle, constraint, or workflow change,
   or as a separate intent.
3. Amend the constitution. NON-NEGOTIABLE marks only where this
   repository already refuses the violation.
4. A principle that contradicts current behavior requires the behavior
   change to land first, with its evidence.
5. Do not commit unless the user asked.

## Output

Report the principles added or amended, any behavior change that had
to land first, and the deferred intents.
