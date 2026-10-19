---
name: speckit-plan
description: >
  Produce the implementation plan with the constitution check table. Use when the user runs /speckit-plan.
disable-model-invocation: true
---

# Plan

Resolve repository paths from the Git repository root.

Read `docs/spec-driven-development.md` and apply its **Cursor surface**
rules. Read `.claude/commands/speckit.plan.md` and follow that
shared procedure. Treat the user's text as its `$ARGUMENTS`; do not
treat the invocation token as input.
