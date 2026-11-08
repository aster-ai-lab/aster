# Core / ACP 9.2.0

## User-visible changes

- Process command isolation uses the locked ksandbox dependency in Core. CLI
  pair archives no longer ship sandbox companions or a kbox MicroVM; hosts that
  still need those companions use Studio / desktop packaging.
- ACP can load offline Model2Vec weights from an adjacent `embedding/` directory
  (or `ASTER_EMBEDDING_DIR`) when built without `bundled-embedding`. Desktop and
  default ACP builds keep bundled assets. CLI release builds ship the sidecar.
- ACP depends on `aster-core` with an explicit `local-code` feature set
  (`default-features = false`) so release and Studio feature matrices do not
  silently drift.
- Unused Core surfaces were removed from the default coding harness: S3 / remote
  workspace adapters, dynamic-workflow / Flow projection, evaluation and
  research chicken-rib modules, and related examples. Thin builds no longer link
  those optional paths.
- Image and tool payload handling was hardened for the shared Core/ACP boundary
  used by CLI and desktop hosts. Typed `tool.result` facts keep the envelope in
  `receipt.output` and leave top-level `output` empty (no dual-write); Effect
  overlap oracles assert that contract.

## Qualification and delivery

Core remains a private/local Rust SDK. Source and ACP platform archives use
`aster-acp-v9.2.0` with `--latest=false`. Aster CLI picks up this Core/ACP
revision in `aster-cli-v0.3.0`.

Before publication: clean-commit branch qualification via `qualify-code.yml`,
matching Core and ACP Cargo versions, and `crates/code/manual/RELEASE_9.2.0.md`.
Platform ACP archives are produced by `release-code.yml`.

Actual packaging and size measurements for the sidecar / LTO path are retained in
[CLI size optimisation](../../../docs/evidence/code/2026-11-08-cli-size-optimisation/OUTCOME.md)
and the aster-cli 0.2.1 publish evidence.

## Known limits

This release does not claim a new frozen LLM qualification cohort beyond the
branch Core/ACP CI gates. Windows/Linux native host behaviour beyond those gates
is unchanged in claim scope. Studio / desktop embedding remains bundled unless a
host opts into the sidecar build.
