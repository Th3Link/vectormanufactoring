//! Shared types and pure helpers for the three primitive-shape tools
//! (`specs/primitive-shapes/specification.md`, acceptance criteria
//! 1-15): [`crate::RectangleTool`], [`crate::EllipseTool`] and
//! [`crate::PolygonStarTool`] each `use` these rather than duplicating
//! them (ux/architect review: `shape_tools.rs` grew to cover three
//! unrelated tools in one module; this crate now gives each its own
//! module — `rectangle_tool.rs`, `ellipse_tool.rs`, `poly_star_tool.rs`
//! — sharing only what is genuinely common, here).

use vecmanf_document_core::{NodeId, Point, PrimitiveSnapshot, Shape, Tolerance};

use crate::PrimitiveSelection;

/// The two hit-test tolerances every shape tool needs — mirrors
/// [`crate::HitTolerances`]'s split for the node tool. `outline` bounds
/// a selection click against a primitive's own outline; `handle` bounds
/// a click against one of its shape handles.
#[derive(Debug, Clone, Copy)]
pub struct ShapeHitTolerances {
    /// Bounds a selection hit against a primitive's outline.
    pub outline: Tolerance,
    /// Bounds a hit against a shape handle.
    pub handle: Tolerance,
}

/// A shape tool's live, uncommitted preview (`specification.md`'s "Live
/// creation feedback": "a maker dragging out a rectangle sees a
/// rectangle updating live, not a placeholder box that snaps to shape
/// on release"). Never touches the [`vecmanf_document_core::Document`]
/// — this is `ADR 0009 §2` ephemeral state, read each frame by the host
/// for both the on-canvas outline preview and (`Creating` only) the
/// numeric readout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiveShape {
    /// A create-drag in progress: the shape as it would commit right
    /// now, plus the live pointer position (point B) the numeric
    /// readout is anchored near (`specification.md`'s "Live creation
    /// feedback" UX notes — the readout only applies to a create-drag,
    /// never to a resize/radius/ratio adjustment below).
    Creating(Shape, Point),
    /// A resize, corner-radius or inner-radius drag on an existing
    /// primitive, in progress: the shape as it would commit right now.
    /// No numeric readout for this case (`specification.md` only
    /// specifies one for the create-drag).
    Adjusting(Shape),
}

impl LiveShape {
    /// The shape to preview, whichever variant this is.
    #[must_use]
    pub const fn shape(&self) -> &Shape {
        match self {
            Self::Creating(shape, _) | Self::Adjusting(shape) => shape,
        }
    }
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

pub(crate) fn rects_only(primitives: &[PrimitiveSnapshot]) -> Vec<PrimitiveSnapshot> {
    primitives
        .iter()
        .copied()
        .filter(|p| matches!(p.shape, Shape::Rect { .. }))
        .collect()
}

pub(crate) fn ellipses_only(primitives: &[PrimitiveSnapshot]) -> Vec<PrimitiveSnapshot> {
    primitives
        .iter()
        .copied()
        .filter(|p| matches!(p.shape, Shape::Ellipse { .. }))
        .collect()
}

pub(crate) fn polygons_and_stars_only(primitives: &[PrimitiveSnapshot]) -> Vec<PrimitiveSnapshot> {
    primitives
        .iter()
        .copied()
        .filter(|p| matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. }))
        .collect()
}

/// Selects (or shift-toggles) `hit`, matching the node tool's own
/// selecting-click convention. A plain click that hits nothing clears
/// the selection unless shift is held.
pub(crate) fn apply_selection_click(
    selection: &mut PrimitiveSelection,
    hit: Option<NodeId>,
    shift: bool,
) {
    match hit {
        Some(id) if shift => selection.toggle(id),
        Some(id) => selection.select_single(id),
        None if !shift => selection.clear(),
        None => {}
    }
}
