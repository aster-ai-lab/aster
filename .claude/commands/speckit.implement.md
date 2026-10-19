---
description: Execute the tasks per Agents.md, landing with evidence.
---

## User Input

```text
$ARGUMENTS
```

## Procedure

1. Read `Agents.md`, `specs/<NNN-slug>/tasks.md`, and `plan.md`.
2. Run the analyze procedure first unless it has passed in this
   session. Blockers stop implementation.
3. Execute the tasks in order. A task that changes behavior includes
   its hermetic test.
4. Follow `Agents.md`: one workspace, path dependencies for `a3s-*`
   crates, no production `unwrap()` without an infallibility comment,
   closed sets with `parse` / `as_str`, ACL for product configuration.
5. After the last task, run the Final Verification Gate in `tasks.md`.
   When the landing is recorded, write
   `docs/evidence/<domain>/<date-slug>/OUTCOME.md` with Result, What
   landed, and Explicitly not claimed.
6. Do not commit unless the user asked. Stage only this task's files
   and inspect `git diff --cached --name-only` first.

## Output

Report the tasks completed, the tests and counts, and the evidence
path.
