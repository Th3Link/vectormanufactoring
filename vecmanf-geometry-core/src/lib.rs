//! The vecmanf geometry kernel (ADR 0003 §1): the narrow slice of
//! operations that need to know what a cubic Bézier is, for
//! `path-node-editing`'s hit-testing — nearest-point-on-segment and de
//! Casteljau subdivision (`specs/path-node-editing/adrs.md`, "the
//! path/node crate boundary").
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads or UI. `vecmanf-document-core`'s newtypes
//! ([`vecmanf_document_core::Point`], [`vecmanf_document_core::Vec2`],
//! [`vecmanf_document_core::Length`], [`vecmanf_document_core::Tolerance`])
//! are this crate's only input/output types besides [`Subdivision`] — no
//! `kurbo` type crosses this crate's public API. `Tolerance` itself lives
//! in `document-core` (ADR 0002 §3 names it there, alongside `Length`), so
//! `vecmanf-ui-core`'s hit-testing and this crate's own nearest/subdivide
//! share the exact same type without this crate inventing one.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod segment;

pub use segment::{Subdivision, nearest_point_on_segment, subdivide_at_parameter};
pub use vecmanf_document_core::Tolerance;
