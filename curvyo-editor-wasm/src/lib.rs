//! The Curvyo wasm editor facade (ADR 0001 §3): binds
//! `curvyo-document-core`, `curvyo-ui-core` and `curvyo-render-core`
//! and owns the `wgpu` device/surface and GPU submission. No editing
//! logic of its own — the private `session` module (its public surface
//! is [`Session`] and [`Tool`]) is a thin orchestration layer over the
//! three crates above, and the private `wasm_api`/`gpu` modules
//! (wasm32-only, public surface `WasmSession`) are a thin
//! `wasm-bindgen`/`wgpu` shell over it.
//!
//! `session` is plain Rust and exercised by ordinary `cargo test` on the
//! host; `wasm_api` and `gpu` only compile for `wasm32` (`Cargo.toml`
//! makes `wgpu`/`wasm-bindgen`/`web-sys` wasm32-only dependencies), so
//! this crate's gate-required `cargo build --target wasm32-unknown-
//! unknown` is what actually type-checks them — there is no browser in
//! this environment to run them in.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod session;

#[cfg(target_arch = "wasm32")]
mod gpu;
#[cfg(target_arch = "wasm32")]
mod gpu_paint;
#[cfg(target_arch = "wasm32")]
mod gpu_pipeline;
#[cfg(target_arch = "wasm32")]
mod wasm_api;
#[cfg(target_arch = "wasm32")]
mod wasm_keys;
#[cfg(target_arch = "wasm32")]
mod wasm_move;
#[cfg(target_arch = "wasm32")]
mod wasm_move_entry;
#[cfg(target_arch = "wasm32")]
mod wasm_navigation;
#[cfg(target_arch = "wasm32")]
mod wasm_node_tool;
#[cfg(target_arch = "wasm32")]
mod wasm_properties_panel;
#[cfg(target_arch = "wasm32")]
mod wasm_render;
#[cfg(target_arch = "wasm32")]
mod wasm_ruler;
#[cfg(target_arch = "wasm32")]
mod wasm_select_bar;
#[cfg(target_arch = "wasm32")]
mod wasm_select_tool;
#[cfg(target_arch = "wasm32")]
mod wasm_shape_tools;

pub use session::{EscapeStep, KeyHint, KeyInput, KeyOutcome, MoveIndicators, Session, Tool};

#[cfg(target_arch = "wasm32")]
pub use wasm_api::{WasmSession, init_panic_hook};
