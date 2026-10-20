Unattended goals can keep admitting read-only work until the deadline, headless
authentication disagrees with the TUI, and several advertised ACP/program
features are silently disconnected. This change gives stalled goals a terminal
result, shares local-config authentication and durable-memory ownership, and
wires validated append rules and named program templates through existing
session and tool governance.

Gateway's internal A3S dependencies resolve locally, and an unconfirmed
workspace feature gate reports its actual boundary. Explicit ACL settings,
permission ceilings, old fact replay, direct memory-store exclusion and
published Rust APIs retain their contracts. Full system-prompt replacement
refuses explicitly rather than being silently ignored.

Isolated regression results: Core 205, ACP 50 and TUI 36 passed, zero failed.
Gateway with wire, locked offline metadata (161 members), workspace formatting
and patch applicability checks pass. The four workspace-gate cases passed in the prior repair run; the gate implementation
and sibling tests are byte-identical to the validated source. That evidence
is reused explicitly; a new isolated binary retry did not complete because of
shared build-lock and local-cache I/O contention. The complete
Python workspace guard is 5/6 because the existing Web/tenant fixture is absent.

The seven issue-specific acceptance cases, exact candidate identity, commands
and limits are recorded in
`docs/evidence/code/2026-10-21-issue-resolution/OUTCOME.md`.

No new release or installer is published by this source fix. The default
branch's existing Web/tenant fixture and CI failures are recorded separately;
all six observed CI failure classes match the base diagnostics, and both
workflows are unchanged. Later CI stages did not run.

Fixes #1
Fixes #2
Fixes #3
Fixes #4
Fixes #5
Fixes #6
Fixes #7
