---
name: speckit-tasks
description: >
  Break the plan into ordered tasks carrying hermetic test obligations. Use when the user runs /speckit-tasks.
disable-model-invocation: true
---

# Tasks

Resolve repository paths from the Git repository root.

Read `docs/spec-driven-development.md` and apply its **Cursor surface**
rules. Read `.claude/commands/speckit.tasks.md` and follow that
shared procedure. Treat the user's text as its `$ARGUMENTS`; do not
treat the invocation token as input.
