---
name: speckit-implement
description: >
  Execute the current tasks per Agents.md and land them with evidence. Use when the user runs /speckit-implement.
disable-model-invocation: true
---

# Implement

Resolve repository paths from the Git repository root.

Read `docs/spec-driven-development.md` and apply its **Cursor surface**
rules. Read `.claude/commands/speckit.implement.md` and follow that
shared procedure. Treat the user's text as its `$ARGUMENTS`; do not
treat the invocation token as input.
