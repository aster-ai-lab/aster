# AGENTS.md

## Repository

This crate owns the local BGE model engines for Aster Use: the `bge-m3`
embedding encoder and the `bge-reranker-v2-m3` cross-encoder, their
manifest-driven installer, and the `aster-use-bge` binary.

## Boundaries

- The installer is manifest-driven: every file arrives with an explicit
  URL, byte length, and sha256. There is no pinned-bundle constant and
  no implicit download source; the platform model supply (aster-api +
  RustFS) provides the manifest.
- The gateway never auto-downloads. A missing or unreadable model is a
  named `use.bge.*` error carrying an install suggestion.
- Keep one ONNX Runtime version (`=2.0.0-rc.10`) aligned with
  `crates/ocr`; two ORT dylibs in one process is a defect.
- Engines stay local through ONNX Runtime. Do not add Python, torch, or
  a model server process.
- Model dimensions, token counts, and output shapes are read at runtime
  from the loaded artifacts; do not hard-code them.
- Requests carry closed-set limits (item counts and byte bounds) and
  name the refused limit in every rejection.
- `aster-use-core` owns shared machine contracts; depend on its released
  crate.

## Engineering

- Use Tokio for I/O; run inference on the blocking pool behind a
  lazily loaded, mutex-guarded engine that reloads when the model
  directory changes.
- Keep public types `Send + Sync`; return typed contextual errors; no
  production panics (`unwrap`/`expect` require a construction-level
  infallibility comment).
- Tests live in sibling files (`install/tests.rs`, `tests.rs`).
- Keep all code and documentation in English.
- Run `cargo fmt`, focused tests, and Clippy before completion.
