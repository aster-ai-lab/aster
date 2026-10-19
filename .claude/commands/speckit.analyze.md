---
description: Cross-check spec, plan, and tasks for consistency and constitution violations.
---

## User Input

```text
$ARGUMENTS
```

## Procedure

Read `specs/<NNN-slug>/spec.md`, `plan.md`, and `tasks.md`, then
report. Do not edit the artifacts.

1. **Constitution violations**: a NON-NEGOTIABLE breach, including a
   production `unwrap()` without an infallibility comment, a missing
   hermetic test, an allowlist edit, a new repository, or non-ACL
   product configuration.
2. **Coverage gaps**: a user story or requirement with no task, or a
   Final Verification Gate item with no producing task.
3. **Ambiguity**: a task that cannot be verified as written.
4. **Scope drift**: a task outside the spec's Explicitly not claimed
   boundary.

Output a table: finding, severity (blocker, warning, or note), file
and task ID, and the suggested resolution.

## Output

Report the findings table and a verdict: ready for implement, or
needs resolution.
