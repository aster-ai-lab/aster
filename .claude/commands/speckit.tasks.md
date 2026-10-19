---
description: Break the plan into ordered tasks carrying the house test obligations.
---

## User Input

```text
$ARGUMENTS
```

## Procedure

1. Read `specs/<NNN-slug>/plan.md` and `spec.md`.
2. Copy `.specify/templates/tasks-template.md` to
   `specs/<NNN-slug>/tasks.md`.
3. Order tasks: behavior and its hermetic test, then callers and
   permissions, then evidence.
4. Every behavior-changing task carries its test in the same task.
5. Mark [P] only for tasks on different files with no shared state.
6. End with the Final Verification Gate from the template.
7. Do not commit unless the user asked.

## Output

Report the task count per phase and any phase that is blocked.
