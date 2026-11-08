# Core / ACP 9.2.1

## User-visible changes

- Interactive TUI spawn forwards CLI `--model` into `aster-acp`, so an explicit
  launch pin outranks host/workspace ACL `default_model` (ASTER_DEFAULT_MODEL
  remains the highest merge layer).
- Esc on a bare `/command` draft (for example `/effort`) clears the composer so
  the next prose turn cannot concatenate into `/effortReply` / Unknown TUI
  command.
- Formal Layer C dialog answering treats `ask_user` / `1/3:select` like the
  visual harness (prefer the option that runs the checker).
- Hermetic A1/B1 PTY gates and inventory document the authority and mode
  exclusivity invariants; CI samples hermetic A1.

## Qualification and delivery

Core remains a private/local Rust SDK. Source and ACP platform archives use
`aster-acp-v9.2.1` with `--latest=false`. Aster CLI picks up this Core/ACP
revision in `aster-cli-v0.3.1`.

Before publication: clean-commit branch qualification via `qualify-code.yml`,
matching Core and ACP Cargo versions, and `crates/code/manual/RELEASE_9.2.1.md`.
Platform ACP archives are produced by `release-code.yml`.

## Known limits

This patch does not claim a full ignored Layer C re-matrix, non-macOS visual
companion runs, or product policy that suppresses model `ask_user` for checker
commands.
