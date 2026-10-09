//! The Curvyo geometry kernel (ADR 0003 §1): the narrow slice of
//! operations that need to know what a cubic Bézier is, for
//! `path-node-editing`'s hit-testing — nearest-point-on-segment and de
//! Casteljau subdivision (`specs/0002-path-node-editing/adrs.md`, "the
//! path/node crate boundary").
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads or UI. `curvyo-document-core`'s newtypes
//! ([`curvyo_document_core::Point`], [`curvyo_document_core::Vec2`],
//! [`curvyo_document_core::Length`], [`curvyo_document_core::Tolerance`])
//! are this crate's only input/output types besides [`Subdivision`] — no
//! `kurbo` type crosses this crate's public API. `Tolerance` itself lives
//! in `document-core` (ADR 0002 §3 names it there, alongside `Length`), so
//! `curvyo-ui-core`'s hit-testing and this crate's own nearest/subdivide
//! share the exact same type without this crate inventing one.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod boolean;
mod boolean_cleanup;
mod boolean_grid;
mod flatten;
mod interior;
mod segment;

pub use boolean::{BooleanError, BooleanOp, BooleanResult, Outline, boolean, signed_area_mm2};
pub use curvyo_document_core::Tolerance;
pub use interior::{OutlineTriple, contains_point_in_outlines};
pub use segment::{Subdivision, nearest_point_on_segment, segment_bounds, subdivide_at_parameter};
