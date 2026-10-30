#![cfg(all(target_os = "macos", not(feature = "ui")))]

pub mod agent_context;
pub mod aster;
pub mod audio;
pub mod autocrop;
pub mod autocrop_lab;
pub mod ax;
pub mod capture;
pub mod command_line;
pub mod command_transcriber;
pub mod delivery;
pub mod export;
pub mod handoff;
pub mod helper;
pub mod hex;
pub mod inbox;
pub mod ingest_http;
pub mod keyframes;
pub mod live_transcribe;
pub mod mic;
pub mod model;
pub mod onboarding;
pub mod page;
pub mod phone;
#[cfg(not(feature = "ui"))]
#[path = "main.rs"]
pub mod pipeline;
#[cfg_attr(not(target_os = "macos"), path = "win/playback.rs")]
pub mod playback;
pub mod recover;
pub mod render;
pub mod retention;
pub mod routing;
pub mod screens;
pub mod settings;
pub mod shortcut;
pub mod sketch;
pub mod speech_model;
pub mod speech_model_missing;
pub mod transcribe;
#[path = "core_ui.rs"]
pub mod ui;
pub mod web;
pub mod windows;
pub mod zoom;
#[cfg(not(feature = "ui"))]
pub use pipeline::{
    FullFrames, PreparedVisuals, SourceExtras, StageTimings, build_bundle_from_parts,
    finalize_capture_bundle, generate_demo_bundle, make_bundle_id, movie_frames_at,
    movie_keyframes, now_epoch_ms, prepare_visuals, rebuild_screen_bundle,
    rebuild_screen_bundle_with,
};
