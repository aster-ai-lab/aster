# CLI size optimisation (release-dist) — phases 0–3

2026-11-08. Local `aarch64-apple-darwin` verification. No remote publish.

## Delivered

### Phase 0 — compiler / allocator
- `[profile.release-dist]` inherits thin LTO + `codegen-units = 1` + strip.
- Embedding asset tables are `static` (no CGU duplication).
- `jemalloc-profiling` is opt-in; release-dist does not enable it.

### Phase 1 — embedding sidecar
- `aster-embedding`: `bundled-assets` feature (default on); `load_verified` /
  `from_dir` / `ASTER_EMBEDDING_DIR` / `<exe>/embedding/`.
- CLI packaging ships `embedding/potion-code-16M-v2.{safetensors,tokenizer.json,config.json}`.
- `install.sh` / `install.ps1` / `cli_pair_update` accept and install the directory.
- CLI release builds: `aster-acp --no-default-features --features svg-png,account-import`
  (no `bundled-embedding`). Studio / default ACP retain bundled assets.

### Phase 2 — explicit Core features
- `aster-acp` depends on `aster-core` with `default-features = false, features = ["local-code"]`
  (same surface as Core’s former default; no silent default drift).

### Phase 3 — CLI vs Studio matrix
- Documented in `docs/release-governance.md` and `release-aster-cli.yml`.

## Commands

```bash
cargo test -p aster-embedding --lib --locked
cargo test -p aster --bin aster --locked cli_pair_update
cargo test -p aster-acp --lib --locked local_embedding
cargo test -p aster-acp --lib --locked retrieval_seam
env -u GROK_VERSION cargo build --locked --profile release-dist -p aster
env -u GROK_VERSION cargo build --locked --profile release-dist -p aster-acp \
  --no-default-features --features svg-png,account-import
bash scripts/package-aster-cli.sh macos target/release-dist /tmp/aster-cli-opt2-macos.tar.gz
```

## Measured sizes (macOS)

| Artifact | aster-cli-v0.2.0 | After phase 0 | After phase 1 (unbundled ACP) |
|---|---:|---:|---:|
| `aster` | 167.4 MiB | 141.1 MiB | 141.1 MiB |
| `aster-acp` | 111.1 MiB | 72.6 MiB | **39.8 MiB** |
| model copies in `aster-acp` | 2 | 1 | **0** |
| gzip archive | 145.8 MiB | 107.3 MiB | **106.9 MiB** (+ `embedding/` members) |

Pair stamp (final smoke):
`aster-cli-pair-v1:455ccb74aa5650b7b3c8f64908b0cefb4f361bf8:aarch64-apple-darwin`

Archive members: `aster`, `aster-acp`, `BUILD_ID`, `SHA256SUMS`, three `embedding/*` files.

## Tests observed

| Gate | Result |
|------|--------|
| `aster-embedding` lib | **22 passed** |
| `aster` `cli_pair_update` | **10 passed** |
| `aster-acp` `local_embedding` | **1 passed** |
| `aster-acp` `retrieval_seam` | **5 passed** |
| package + pair `--build-id` + embedding dir present | **ok** |

Also fixed a pre-existing `agent_tests` `Arc<[u8]>` assert that blocked `--lib` compiles.

## Explicitly not claimed

- Linux / Windows archive rebuilds.
- Full `aster-core --lib` suite (not re-run end-to-end here).
- Studio headed e2e / DMG rebuild.
- Fat LTO, TUI feature stripping, or shared dylib.
- That gzip download size drops further once assets are already on disk (floats compress poorly; sidecar mainly shrinks the ACP **binary** and avoids re-embedding weights).
