//! The in-tree pure-Rust light-tier embedding engine (specs/013 Phase 2).
//!
//! One provider behind the core `EmbeddingProvider` contract: a Model2Vec
//! static model loaded from verified asset bytes — no ONNX, no model server,
//! no network. Bytes come from `<exe>/embedding/` (or `ASTER_EMBEDDING_DIR`),
//! or from the optional compile-time `bundled-assets` feature. Digests are
//! pinned so drift refuses at load (fail-closed; the `use.bge.*` manifest
//! error family cannot fire here by construction).
//!
//! ## AGENTS.md (copied verbatim from crates/bge)
//!
//! - Engines stay local. Do not add Python, torch, or a model server process.
//! - No implicit download source; every byte is in this repository or the
//!   matching files shipped next to the host binary.

pub mod adapter;
pub mod assets;
pub mod hygiene;
pub mod provider;
pub mod vector;

pub use adapter::LightTierProvider;
pub use provider::StaticEmbeddingProvider;
