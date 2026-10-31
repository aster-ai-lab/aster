# Aster Code UI fork notes

This tree is the Aster Code TUI surface used by `aster-tui`.

Product direction:
- Keep the pager UI (scrollback, prompt, chrome) as the coding surface.
- Drive sessions with `aster-core` 9.1.1 over ACP (`aster-acp`).
- Branding and slash catalog are Aster Code only (`aster-*` / `aster_*`).

Binary entry: `aster-tui` (from `crates/codegen/aster-pager-bin`).

Upstream license attribution stays in this tree's Apache-2.0 `LICENSE` files.

Upstream snapshot: https://github.com/xai-org/grok-build commit
`f0e3be1100ef5252488e3be8bb0e91cf68d8c305` (2026-09-23).
