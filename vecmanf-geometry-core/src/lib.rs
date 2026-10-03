//! The vecmanf geometry kernel (ADR 0003 §1): the narrow slice of
//! operations that need to know what a cubic Bézier is, for
//! `path-node-editing`'s hit-testing — flatten-for-hit-test,
//! nearest-point-on-segment, and de Casteljau subdivision
//! (`specs/path-node-editing/adrs.md`, "the path/node crate boundary").
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads or UI. `vecmanf-document-core`'s newtypes
//! ([`vecmanf_document_core::Point`], [`vecmanf_document_core::Vec2`],
//! [`vecmanf_document_core::Length`]) are this crate's only input/output
//! types besides [`Tolerance`] and [`Subdivision`] — no `kurbo` type
//! crosses this crate's public API.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod segment;
mod tolerance;

pub use segment::{Subdivision, flatten_segment, nearest_point_on_segment, subdivide_at_parameter};
pub use tolerance::Tolerance;
