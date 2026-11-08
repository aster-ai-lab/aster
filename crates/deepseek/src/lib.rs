//! Model-owned native DeepSeek execution components.
//!
//! Power owns shared execution, admission, devices and residency. This crate
//! owns architecture, tensor layout and model state. Initial integration does
//! not advertise a complete DeepSeek model or CUDA backend.

#[cfg(feature = "server")]
pub mod backend;
pub mod config;
pub mod cuda;
pub mod engram;
pub mod execution;
#[cfg(feature = "server")]
pub mod hosting;
pub mod layers;
pub mod model;
pub mod parallel;
pub mod prompt;
pub mod reference;
pub mod replicas;
pub mod state;
pub mod tokenizer;
pub mod weights;
