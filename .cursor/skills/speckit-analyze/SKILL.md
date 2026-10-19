---
name: speckit-analyze
description: >
  Cross-check spec, plan, and tasks without editing them. Use when the user runs /speckit-analyze.
disable-model-invocation: true
---

# Analyze

Resolve repository paths from the Git repository root.

Read `docs/spec-driven-development.md` and apply its **Cursor surface**
rules. Read `.claude/commands/speckit.analyze.md` and follow that
shared procedure. Treat the user's text as its `$ARGUMENTS`; do not
treat the invocation token as input.
