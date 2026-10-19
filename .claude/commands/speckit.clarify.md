---
description: Resolve [NEEDS CLARIFICATION] markers in the current feature spec.
---

## User Input

```text
$ARGUMENTS
```

## Procedure

1. Read the current `specs/<NNN-slug>/spec.md`.
2. Collect every `[NEEDS CLARIFICATION: ...]` marker. If there are
   none, report that the spec is already clear and stop.
3. For each marker, ask the user at most one question with 2-3
   candidate answers grounded in this repository. Prefer fail closed,
   closed sets, and retained evidence.
4. Apply the chosen resolution and remove the marker.
5. If a marker needs a decision outside this spec, record it under
   Explicitly not claimed and move on.
6. Do not commit unless the user asked.

## Output

Report each marker, its resolution, and the remaining open count.
