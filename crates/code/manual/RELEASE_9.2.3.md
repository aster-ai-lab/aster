# Core / ACP 9.2.3 and Aster CLI 0.3.5

## User-visible changes

- The TUI adds `/init`. It injects a structured prompt that asks the model to
  inspect the working tree and write a short `AGENTS.md` project overview for
  future agents. When `AGENTS.md` already exists the command refuses before
  dispatch and renders a message; it never overwrites human-authored content.
- The repository root now carries `CLAUDE.md`, a navigation-level project
  overview that points to `AGENTS.md` as the authoritative rule file. It is
  documentation only and does not change runtime behaviour.
- Core and ACP have no independent feature change in this revision. Their
  versions move with the CLI so the paired build identity, versioned tag and
  platform archives stay consistent under `docs/release-governance.md`.

## Qualification and delivery

Core remains a private/local Rust SDK. Core and ACP share version `9.2.3` and
source tag `aster-acp-v9.2.3`; ACP platform archives are produced by
`release-code.yml`. Aster CLI uses the independent version `0.3.5` and tag
`aster-cli-v0.3.5`, with `aster` and `aster-acp` built from one clean source
commit and distributed as a pair with shared BUILD_ID, SHA256SUMS and offline
embedding assets.

Required qualification follows `docs/release-governance.md` and the exact-source
branch runs of `qualify-code.yml` and `qualify-aster-cli.yml`. Platform archives,
member allowlists, versions and hashes must be checked before publication.
Both product releases use `--latest=false`; the desktop release retains GitHub
Latest. CLI stable and the current desktop Latest compatibility manifest must
point to the immutable CLI versioned release.

## Known limits

- The 508-case shipped-TUI campaign is not complete: 7 PASS, 203 PARTIAL,
  298 NOT_RUN. PTY, static and Rust suite results are suite-level evidence and
  do not by themselves flip a campaign case to PASS.
- The agent-desktop visual E2E lane was blocked in the recorded environment
  because `/bin/ps` was denied, so no native-window case was executed there.
  `list-windows`, `launch` and `snapshot` were reachable; key delivery was not.
- `PTY-INPUT-005` remains an intermittent failure under full-suite runs. The
  harness now records screen, composer and relay state on recurrence; no
  assertion was weakened and no retry was added.
- The 12-column composer rendering has no product layout test below 40 columns
  and is an undefined contract, not a proven defect or a harness false negative.
- The source tree's existing visual and live-model evidence describes its
  recorded candidate and environment. It does not establish a complete
  execution of the TUI catalogue, every provider reasoning mode, or native
  terminal behavior on an untested platform. Native process execution remains
  subject to its real host qualification and sandbox capabilities.
