---
name: speckit-clarify
description: >
  Resolve clarification markers in the current feature spec. Use when the user runs /speckit-clarify.
disable-model-invocation: true
---

# Clarify

Resolve repository paths from the Git repository root.

Read `docs/spec-driven-development.md` and apply its **Cursor surface**
rules. Read `.claude/commands/speckit.clarify.md` and follow that
shared procedure. Treat the user's text as its `$ARGUMENTS`; do not
treat the invocation token as input.
