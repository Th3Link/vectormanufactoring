//! The Select tool's own decoration geometry
//! (`specs/0004-canvas-navigation-and-selection/specification.md`'s "a new,
//! unified 'selected' indicator": a plain bounding-box outline on every
//! object type, selected or hovered, with no shape handles and no path
//! nodes — acceptance criteria 14, 15, 20).
//!
//! [`SelectDecorationInput`] mirrors [`crate::DecorationInput`]/
//! [`crate::ShapeDecorationInput`]'s own reason for existing: this crate
//! cannot read `vecmanf-ui-core`'s `ObjectSelection` or `object_bounds`
//! directly (ADR 0011 §3), so each selected/hovered object's own bounding
//! box reaches here as a plain rectangle, computed by
//! `vecmanf_ui_core::object_bounds` and passed through by
//! `vecmanf-editor-wasm` — the same way `primitive-shapes` already passes
//! shape-handle positions.

use vecmanf_document_core::{NodeId, Point, ViewTransform};

use crate::glyphs::{self, DrawList};
use crate::theme;

/// One object's axis-aligned selection-box bounds, `(min, max)` corners —
/// `vecmanf_ui_core::object_bounds`'s own return shape.
pub type SelectionBox = (Point, Point);

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

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// Draws a plain rectangle outline around `(min, max)` — the Select
/// tool's one shared indicator shape, independent of the object's own
/// kind (`specification.md`: "no shape handles... no path nodes, only
/// the bounding box").
fn bounding_box_outline(
    (min, max): SelectionBox,
    width_mm: f64,
    color: crate::RgbaColor,
) -> DrawList {
    let corners = [
        Point::new(min.x, min.y),
        Point::new(max.x, min.y),
        Point::new(max.x, max.y),
        Point::new(min.x, max.y),
    ];
    let mut list = DrawList::default();
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        list.extend(glyphs::thick_line(a, b, width_mm, color));
    }
    list
}

/// Builds the Select tool's decoration geometry for this frame: one
/// `--accent` box per selected object, plus a `--accent-hover` box for a
/// hovered-but-unselected one (`docs/design-system.md`'s "Bounding-box
/// selection outline").
#[must_use]
pub fn build(view: ViewTransform, input: &SelectDecorationInput) -> DrawList {
    let width_mm = screen_px_to_mm(view, theme::BOUNDING_BOX_OUTLINE_PX);
    let mut list = DrawList::default();
    for &(_, selection_box) in &input.selected {
        list.extend(bounding_box_outline(selection_box, width_mm, theme::ACCENT));
    }
    if let Some((_, selection_box)) = input.hovered {
        list.extend(bounding_box_outline(
            selection_box,
            width_mm,
            theme::ACCENT_HOVER,
        ));
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::NodeId;

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

    #[test]
    fn no_selection_and_no_hover_draws_nothing() {
        let list = build(ViewTransform::identity(), &SelectDecorationInput::default());
        assert_eq!(list.triangles.len(), 0);
    }

    #[test]
    fn a_selected_object_draws_a_box() {
        let input = SelectDecorationInput {
            selected: vec![(fixture_id(), (Point::new(0.0, 0.0), Point::new(10.0, 10.0)))],
            hovered: None,
        };
        let list = build(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    #[test]
    fn a_hovered_object_draws_a_box_too() {
        let input = SelectDecorationInput {
            selected: vec![],
            hovered: Some((fixture_id(), (Point::new(0.0, 0.0), Point::new(10.0, 10.0)))),
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
            selected: vec![(fixture_id(), (Point::new(0.0, 0.0), Point::new(10.0, 10.0)))],
            hovered: None,
        };
        let two = SelectDecorationInput {
            selected: vec![
                (fixture_id(), (Point::new(0.0, 0.0), Point::new(10.0, 10.0))),
                (
                    fixture_id(),
                    (Point::new(50.0, 50.0), Point::new(60.0, 60.0)),
                ),
            ],
            hovered: None,
        };
        let one_list = build(ViewTransform::identity(), &one);
        let two_list = build(ViewTransform::identity(), &two);
        assert!(two_list.triangle_count() > one_list.triangle_count());
    }
}
