# Core / ACP 9.2.2 and Aster CLI 0.3.4

## User-visible changes

- The TUI offers `/resume` through its session picker and restores the selected
  session's effective model configuration. Session controls have shipped-pair
  regression coverage for mouse stop, Ctrl+C and send-now.
- Model discovery updates reasoning options and context limits asynchronously.
  The request contract distinguishes a reset to the model default from an
  explicit effort choice, including `none`. Discovery avoids duplicated version
  segments when joining provider model endpoints.
- Product-authored permission labels use YOLO consistently. Built-in themes
  use Aster Night and Aster Day names.
- Verification and tool refusals retain actionable diagnostics and current-run
  evidence instead of treating a narrative response as verified completion.
- The current-source TUI test plan maps commands, settings, shortcuts, modals,
  forms and tools to explicit planned cases. Its catalogue is not a claim that
  a full platform/provider campaign has run.

## Qualification and delivery

Core remains a private/local Rust SDK. Core and ACP share version `9.2.2` and
source tag `aster-acp-v9.2.2`; ACP platform archives are produced by
`release-code.yml`. Aster CLI uses the independent version `0.3.4` and tag
`aster-cli-v0.3.4`, with `aster` and `aster-acp` built from one clean source
commit and distributed as a pair with shared BUILD_ID, SHA256SUMS and offline
embedding assets.

Required qualification follows `docs/release-governance.md` and the exact-source
branch runs of `qualify-code.yml` and `qualify-aster-cli.yml`. Platform archives,
member allowlists, versions and hashes must be checked before publication.
Both product releases use `--latest=false`; the desktop release retains GitHub
Latest. CLI stable and the current desktop Latest compatibility manifest must
point to the immutable CLI versioned release.

## Known limits

The source tree's existing visual and live-model evidence describes its recorded
candidate and environment. It does not establish a complete execution of the
new TUI catalogue, every provider reasoning mode, or native terminal behavior
on an untested platform. Native process execution remains subject to its real
host qualification and sandbox capabilities.
