//! Shared types and pure helpers of the three shape-creation tools
//! (`specs/0003-primitive-shapes/specification.md`, acceptance criteria 1, 2,
//! 7, 8, 11, 12; `specs/unified-object-editing/`, criteria 25 and 26):
//! [`crate::RectangleTool`], [`crate::EllipseTool`] and
//! [`crate::PolygonStarTool`] each `use` these rather than duplicating them.
//! The shape tools only create; every edit of an existing shape is the
//! Select tool's.

use vecmanf_document_core::{NodeId, Point, Shape};

use crate::modifiers::Modifiers;

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
    /// The press point when Shift makes it the centre of a rectangle or
    /// ellipse: the host marks it with the pivot marker
    /// (`docs/design-system.md`, "Modifiers in a rectangle or ellipse
    /// create-drag"). `None` otherwise, and always for a polygon or star.
    pub centre: Option<Point>,
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

/// The box a rectangle or ellipse create-drag makes: two opposite corners,
/// and where the numeric readout sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CreateDragBox {
    /// One corner of the box.
    pub(crate) corner_a: Point,
    /// The opposite corner.
    pub(crate) corner_b: Point,
    /// The effective endpoint E, the readout's anchor: the pointer, or the
    /// constrained corner under Ctrl.
    pub(crate) anchor: Point,
    /// `a` when Shift makes it the centre.
    pub(crate) centre: Option<Point>,
}

/// The one computation of a rectangle or ellipse create-drag from press point
/// `a` and pointer `b` (`specs/shape-creation-from-center/`, "The rule"):
/// Ctrl moves the endpoint to E so both extents are equal
/// ([`constrained_endpoint`]); without Shift the box spans `a` and E, with
/// Shift it spans E and its mirror `2a - E`, so `a` is the centre. `None` when
/// E equals `a` (criterion 12: nothing to preview or create).
pub(crate) fn create_drag_box(a: Point, b: Point, modifiers: Modifiers) -> Option<CreateDragBox> {
    let end = if modifiers.ctrl {
        constrained_endpoint(a, b)
    } else {
        b
    };
    if is_degenerate(a, end) {
        return None;
    }
    let corner_a = if modifiers.shift {
        Point::new(2.0 * a.x - end.x, 2.0 * a.y - end.y)
    } else {
        a
    };
    Some(CreateDragBox {
        corner_a,
        corner_b: end,
        anchor: end,
        centre: modifiers.shift.then_some(a),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Point = Point::new(100.0, 50.0);
    const TOL: f64 = 1e-9;

    fn close(p: Point, x: f64, y: f64) -> bool {
        (p.x - x).abs() < TOL && (p.y - y).abs() < TOL
    }

    /// The box as (min corner, max corner), whichever way round the two
    /// corners come.
    fn spans(b: &CreateDragBox) -> (Point, Point) {
        (
            Point::new(
                b.corner_a.x.min(b.corner_b.x),
                b.corner_a.y.min(b.corner_b.y),
            ),
            Point::new(
                b.corner_a.x.max(b.corner_b.x),
                b.corner_a.y.max(b.corner_b.y),
            ),
        )
    }

    fn assert_box(m: Modifiers, to: Point, min: (f64, f64), max: (f64, f64), anchor: (f64, f64)) {
        let b = create_drag_box(A, to, m).expect("a box");
        let (lo, hi) = spans(&b);
        assert!(close(lo, min.0, min.1), "{m:?} to {to:?}: min {lo:?}");
        assert!(close(hi, max.0, max.1), "{m:?} to {to:?}: max {hi:?}");
        assert!(
            close(b.anchor, anchor.0, anchor.1),
            "{m:?} to {to:?}: anchor {:?}",
            b.anchor
        );
    }

    /// AC 1, 2, 5: Shift draws around A; the side of A the pointer is on does
    /// not matter.
    #[test]
    fn shift_makes_a_the_centre_whichever_side_the_pointer_is_on() {
        let m = Modifiers::new(true, false);
        for (to, anchor) in [
            (Point::new(130.0, 40.0), (130.0, 40.0)),
            (Point::new(70.0, 60.0), (70.0, 60.0)),
            (Point::new(70.0, 40.0), (70.0, 40.0)),
            (Point::new(130.0, 60.0), (130.0, 60.0)),
        ] {
            assert_box(m, to, (70.0, 40.0), (130.0, 60.0), anchor);
        }
    }

    /// AC 3, 6: Shift+Ctrl is a square around A; the larger extent wins; the
    /// readout anchor is the constrained corner.
    #[test]
    fn shift_ctrl_makes_a_square_around_a_from_the_larger_extent() {
        let m = Modifiers::new(true, true);
        assert_box(
            m,
            Point::new(130.0, 40.0),
            (70.0, 20.0),
            (130.0, 80.0),
            (130.0, 20.0),
        );
        assert_box(
            m,
            Point::new(110.0, 20.0),
            (70.0, 20.0),
            (130.0, 80.0),
            (130.0, 20.0),
        );
    }

    /// AC 4, 7: Ctrl alone and no modifier are the shipped boxes.
    #[test]
    fn ctrl_alone_and_no_modifier_are_unchanged() {
        assert_box(
            Modifiers::new(false, true),
            Point::new(130.0, 40.0),
            (100.0, 20.0),
            (130.0, 50.0),
            (130.0, 20.0),
        );
        assert_box(
            Modifiers::NONE,
            Point::new(130.0, 40.0),
            (100.0, 40.0),
            (130.0, 50.0),
            (130.0, 40.0),
        );
    }

    /// AC 12: a pointer at A makes no box under any modifier state.
    #[test]
    fn a_pointer_at_a_makes_no_box_in_any_modifier_state() {
        for shift in [false, true] {
            for ctrl in [false, true] {
                assert_eq!(create_drag_box(A, A, Modifiers::new(shift, ctrl)), None);
            }
        }
    }

    /// AC 12: a one-axis drag is not refused; under Shift alone the other
    /// dimension is zero and the first twice as long.
    #[test]
    fn a_one_axis_drag_is_kept_and_has_a_zero_other_extent() {
        let b = create_drag_box(A, Point::new(130.0, 50.0), Modifiers::new(true, false))
            .expect("a box");
        let (lo, hi) = spans(&b);
        assert!(close(lo, 70.0, 50.0) && close(hi, 130.0, 50.0));
        assert_eq!(
            create_drag_box(A, Point::new(130.0, 50.0), Modifiers::NONE).map(|b| spans(&b)),
            Some((A, Point::new(130.0, 50.0)))
        );
    }
}
