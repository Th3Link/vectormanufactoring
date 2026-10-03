//! The vecmanf wasm editor facade (ADR 0001 §3): binds
//! `vecmanf-document-core`, `vecmanf-ui-core` and `vecmanf-render-core`
//! and owns the `wgpu` device/surface and GPU submission. No editing
//! logic of its own — [`session`] is a thin orchestration layer over the
//! three crates above, and [`wasm_api`]/[`gpu`] (wasm32-only) are a thin
//! `wasm-bindgen`/`wgpu` shell over [`session`].
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
mod wasm_api;

pub use session::{Session, Tool};

#[cfg(target_arch = "wasm32")]
pub use wasm_api::{WasmSession, init_panic_hook};
