# Core / ACP 9.3.3 and Aster CLI 0.4.3

## Core and ACP

- **Binary & footprint reduction**:
  - Added a lightweight `tokio-tungstenite` CDP backend (`quark-cdp`) in
    `crates/browser` so `aster-core` and `aster-acp` no longer link `chromiumoxide`
    for Quark local CDP sessions.
  - Replaced `html2text` (`html5ever`/`markup5ever`) in `web_fetch` with existing
    `htmd` + `pulldown-cmark`, replaced `sha256` with `sha2` (`sha256_hex`),
    disabled `aster-lane` `telemetry` (`opentelemetry 0.21`) while keeping
    `metrics`/`monitoring`/`distributed`, and unified duplicated `base64`, `toml`,
    `thiserror`, and `zip` versions across the workspace.
  - Switched `aster-embedding` to `aster-core` (`default-features = false`) and
    `model2vec-rs` `fancy-regex` (dropping `onig_sys`, `hf-hub`, and `ureq`), and
    gated `aster-office` `collaboration` (`yrs`) and `install` (`reqwest`) behind
    default features so `aster-acp` (`default-features = false`) links neither.
- **Long-horizon resilience & crash recovery**:
  - `aster-effect` adds emergency overflow compaction, `ReplaySafe` tool
    recovery alongside reconciled/unreconciled effect ledgers, and refined
    duplicate/revalidation tracking across novel observations vs. same-byte or
    incomplete mutations.
  - `aster-core` adds `inspect_interrupted_turn` (`Idle`, `Resumable`,
    `UnresolvedTool`), `PriorVerificationPending`, systemic refusal-cycle
    classification (`bash`/`read`/`background`/`systemic` refusal cycles), and
    deferred tool catalog + memory/grep projection improvements.
  - Added Quark browser account & search support and JSON-render `aster-form-core`
    updates.

## Aster CLI

- Ships the matched `aster 0.4.3` + `aster-acp 9.3.3` pair integrating
  `aster-core 9.3.3`, sharing `BUILD_ID`, `SHA256SUMS`, and offline Model2Vec
  `embedding/` assets.
- Decoupled `aster-pager` runtime dependencies from `aster-core` via ACL
  scalar/document helpers (`aster` CLI drops `aster-core` and `chromiumoxide`
  completely), removed unused `gcloud-storage` / `tonic`, gated `cloud-storage`,
  `pdf-read`, and `otlp-grpc` behind optional features, trimmed `aster-use` in
  `apps/aster` to `extensions`, and switched `aster-markdown` embedded
  syntaxes to zlib-compressed `dump_to_file`/`from_binary`.
- Tuned `[profile.release-dist]` (`opt-level = "z"`, `lto = "fat"`,
  `codegen-units = 1`, `strip = true`, `panic = "abort"`), shrinking macOS
  `release-dist` binaries by ~62% (`aster`: 141.4 MiB -> 53.7 MiB) and ~56%
  (`aster-acp`: 46.3 MiB -> 20.3 MiB) and cutting the macOS archive from
  109.8 MiB to 70.4 MiB.
- `aster-pager` adds ACP crash self-healing/resume flows and typed worktree/error
  handling.

## Delivery

Core remains a local Rust SDK. Core and ACP share version `9.3.3` and source tag
`aster-acp-v9.3.3`. Aster CLI uses `0.4.3` and source tag `aster-cli-v0.4.3`,
publishing the matched `aster` + `aster-acp` pair across macOS, Linux x86_64
and arm64 (glibc 2.31 plus static `linux-musl` and `linux-musl-arm64`), optional
`kylin-x86_64` alias, and Windows.

Both Core/ACP and CLI releases use `--latest=false` (desktop retains repository
Latest). `aster-cli-stable` and the compatibility `latest.json` on the desktop
Latest release are updated after publication.
[Release governance](../../../docs/release-governance.md)
