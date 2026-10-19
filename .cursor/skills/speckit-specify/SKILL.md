---
name: speckit-specify
description: >
  Create a bounded-epic feature specification from the house spec template, gated by the constitution. Use when the user runs /speckit-specify.
disable-model-invocation: true
---

# Specify

Resolve repository paths from the Git repository root.

Read `docs/spec-driven-development.md` and apply its **Cursor surface**
rules. Read `.claude/commands/speckit.specify.md` and follow that
shared procedure. Treat the user's text as its `$ARGUMENTS`; do not
treat the invocation token as input.
