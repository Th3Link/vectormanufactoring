//! Draws the selection and hover boxes of the Select tool.
//!
//! Each selected or hovered object's own bounding box reaches here as four
//! document-space corners (oriented to the object's own rotation,
//! `object-transform` acceptance criterion 18), computed by
//! `vecmanf_ui_core::oriented_bounds` and passed through by
//! `vecmanf-editor-wasm`: this crate cannot read `vecmanf-ui-core`'s
//! selection or `object_bounds` directly (ADR 0011 §3).

use vecmanf_document_core::{NodeId, Point, ViewTransform};

use crate::glyphs::{DrawList, quad_outline};
use crate::screen_px_to_mm;
use crate::theme;

/// One object's selection box: its four corners in document space, in
/// order around the perimeter — oriented to the object's own rotation
/// (`vecmanf_ui_core::OrientedBox::document_corners`), which for an
/// unrotated object is the plain axis-aligned box slice 4 shipped.
pub type SelectionBox = [Point; 4];

/// What the Select tool decorates this frame: every currently selected
/// object's own box (plural — a heterogeneous multi-select shows each
/// object's own real box simultaneously, never one merged box,
/// `docs/design-system.md`'s "Mixed-state display on multi-select"
/// extension), plus a hovered-but-not-yet-selected object's box.
#[derive(Debug, Clone, Default)]
pub struct SelectDecorationInput {
    /// Selected objects, each with its own id and box.
    pub selected: Vec<(NodeId, SelectionBox)>,
    /// A hovered, not-yet-selected object's id and box, if any.
    pub hovered: Option<(NodeId, SelectionBox)>,
}

/// Builds the Select tool's decoration geometry for this frame: one
/// `--accent` box per selected object, plus a `--accent-hover` box for a
/// hovered-but-unselected one (`docs/design-system.md`'s "Bounding-box
/// selection outline").
#[must_use]
pub fn build(view: ViewTransform, input: &SelectDecorationInput) -> DrawList {
    let width_mm = screen_px_to_mm(view, theme::BOUNDING_BOX_OUTLINE_PX);
    let mut list = DrawList::default();
    for &(_, corners) in &input.selected {
        list.extend(quad_outline(corners, width_mm, theme::ACCENT));
    }
    if let Some((_, corners)) = input.hovered {
        list.extend(quad_outline(corners, width_mm, theme::ACCENT_HOVER));
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_id() -> NodeId {
        // `NodeId` has no public constructor outside `document-core`; any
        // real one round-tripped through a `Document` is fine here, since
        // these tests never read the id back, only the geometry it keys.
        let document = vecmanf_document_core::Document::new(1);
        document.create_rect(vecmanf_document_core::RectBounds {
            origin: Point::new(0.0, 0.0),
            width: vecmanf_document_core::Length::from_mm(1.0),
            height: vecmanf_document_core::Length::from_mm(1.0),
        })
    }

    fn axis_box(x0: f64, y0: f64, x1: f64, y1: f64) -> SelectionBox {
        [
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ]
    }

    /// Acceptance criterion 18 / UX notes: a rotated object's outline is
    /// drawn through its own four (turned) corners, not their axis-
    /// aligned bounds — a 45° diamond's outline reaches its apex at
    /// `(0, -r)` and never the bounding square's corner `(r, -r)`.
    #[test]
    fn a_rotated_selection_box_draws_through_its_own_corners_not_its_bounds() {
        let r = 10.0;
        let diamond: SelectionBox = [
            Point::new(0.0, -r),
            Point::new(r, 0.0),
            Point::new(0.0, r),
            Point::new(-r, 0.0),
        ];
        let input = SelectDecorationInput {
            selected: vec![(fixture_id(), diamond)],
            hovered: None,
        };
        let list = build(ViewTransform::identity(), &input);
        let reaches = |target: Point| {
            list.triangles
                .iter()
                .any(|v| v.position.vector_to(target).length() < 1.0)
        };
        assert!(reaches(Point::new(0.0, -r)), "apex drawn");
        assert!(
            !reaches(Point::new(r, -r)),
            "bounding-square corner not drawn"
        );
    }

    #[test]
    fn no_selection_and_no_hover_draws_nothing() {
        let list = build(ViewTransform::identity(), &SelectDecorationInput::default());
        assert_eq!(list.triangles.len(), 0);
    }

    #[test]
    fn a_selected_object_draws_a_box() {
        let input = SelectDecorationInput {
            selected: vec![(fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))],
            hovered: None,
        };
        let list = build(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    #[test]
    fn a_hovered_object_draws_a_box_too() {
        let input = SelectDecorationInput {
            selected: vec![],
            hovered: Some((fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))),
        };
        let list = build(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    /// Acceptance criterion 17's multi-select UX note: two selected
    /// objects draw two independent boxes, not one merged box — strictly
    /// more geometry than either alone.
    #[test]
    fn two_selected_objects_each_draw_their_own_box() {
        let one = SelectDecorationInput {
            selected: vec![(fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0))],
            hovered: None,
        };
        let two = SelectDecorationInput {
            selected: vec![
                (fixture_id(), axis_box(0.0, 0.0, 10.0, 10.0)),
                (fixture_id(), axis_box(50.0, 50.0, 60.0, 60.0)),
            ],
            hovered: None,
        };
        let one_list = build(ViewTransform::identity(), &one);
        let two_list = build(ViewTransform::identity(), &two);
        assert!(two_list.triangle_count() > one_list.triangle_count());
    }
}
