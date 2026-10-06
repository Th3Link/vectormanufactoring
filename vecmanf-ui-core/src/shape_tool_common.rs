//! Shared types and pure helpers of the three shape-creation tools
//! (`specs/0003-primitive-shapes/specification.md`, acceptance criteria 1, 2,
//! 7, 8, 11, 12; `specs/unified-object-editing/`, criteria 25 and 26):
//! [`crate::RectangleTool`], [`crate::EllipseTool`] and
//! [`crate::PolygonStarTool`] each `use` these rather than duplicating them.
//! The shape tools only create; every edit of an existing shape is the
//! Select tool's.

use vecmanf_document_core::{NodeId, Point, Shape};

/// A create-drag's live, uncommitted preview (`specification.md`'s "Live
/// creation feedback": "a maker dragging out a rectangle sees a rectangle
/// updating live, not a placeholder box that snaps to shape on release").
/// Never touches the [`vecmanf_document_core::Document`] (ADR 0009 §2:
/// ephemeral state); read each frame by the host for the on-canvas outline
/// and the numeric readout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreatePreview {
    /// The shape as it would commit right now.
    pub shape: Shape,
    /// The live pointer position (point B) the numeric readout is anchored
    /// near.
    pub anchor: Point,
}

/// What a creation tool's `pointer_up` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateOutcome {
    /// No drag was in flight, or it moved nowhere (acceptance criterion 1's
    /// "A = B creates nothing"; criterion 26 of `unified-object-editing`).
    NoOp,
    /// A new shape was created and committed.
    Created(NodeId),
}

/// Point A equals point B (acceptance criteria 1, 7, 11, 12: a drag
/// with no movement creates nothing, and "shows no preview at all, not
/// a zero-size one" per the UX notes).
pub(crate) fn is_degenerate(a: Point, b: Point) -> bool {
    a == b
}

/// The Ctrl-constrain square/circle endpoint (acceptance criteria 2, 8):
/// `b`, moved so both axes have the same extent from `a` — the larger
/// of the drag's own horizontal/vertical extents, keeping each axis's
/// original sign (or defaulting positive if that axis had no movement
/// at all).
pub(crate) fn constrained_endpoint(a: Point, b: Point) -> Point {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let extent = dx.abs().max(dy.abs());
    let sign = |d: f64| if d == 0.0 { 1.0 } else { d.signum() };
    Point::new(a.x + extent * sign(dx), a.y + extent * sign(dy))
}
