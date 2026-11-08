# AGENTS.md

## Repository

This crate owns the in-tree pure-Rust light-tier embedding engine: the
Model2Vec potion-code-16M-v2 static model, its embedded (compile-time)
assets, and the provider implementation behind the core
`EmbeddingProvider` contract.

## Boundaries

- The assets are checked in with their sha256 pinned in `src/assets.rs`.
  A drift refuses at load with the named digest mismatch; there is no
  download and no network access at any point. CLI release builds may omit
  the `bundled-assets` feature and load the same files from
  `<prefix>/embedding/` (or `ASTER_EMBEDDING_DIR`) instead.
- Engines stay local and pure Rust. Do not add Python, torch, ONNX, or a
  model server process. The `crates/bge` discipline applies verbatim.
- The public load surface is `StaticEmbeddingProvider` (`from_embedded`,
  `from_dir`, dimension, id, encode_batch). Do not widen it without a spec.
