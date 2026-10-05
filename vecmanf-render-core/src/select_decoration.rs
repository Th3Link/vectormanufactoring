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

use crate::color::RgbaColor;
use crate::glyphs::{self, DrawList, box_outline};
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

/// Builds the Select tool's decoration geometry for this frame: one
/// `--accent` box per selected object, plus a `--accent-hover` box for a
/// hovered-but-unselected one (`docs/design-system.md`'s "Bounding-box
/// selection outline").
#[must_use]
pub fn build(view: ViewTransform, input: &SelectDecorationInput) -> DrawList {
    let width_mm = screen_px_to_mm(view, theme::BOUNDING_BOX_OUTLINE_PX);
    let mut list = DrawList::default();
    for &(_, (min, max)) in &input.selected {
        list.extend(box_outline(min, max, width_mm, theme::ACCENT));
    }
    if let Some((_, (min, max))) = input.hovered {
        list.extend(box_outline(min, max, width_mm, theme::ACCENT_HOVER));
    }
    list
}

/// One of `object-transform`'s own transform handles — this crate's own
/// minimal shape (ADR 0011 §3: it cannot read `vecmanf-ui-core`'s
/// `TransformHandle` directly), carrying only what drawing needs: where
/// it is, which of the two glyph vocabularies it uses, and whether it is
/// the one currently being dragged (solid `--accent` fill instead of the
/// idle hollow/transparent state, `docs/design-system.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformHandleGlyph {
    /// Document-space position.
    pub position: Point,
    /// A resize handle (squircle-ish hollow square) or the rotate
    /// handle (circular-arrow icon, drawn as a plain circle — see
    /// `theme::TRANSFORM_ROTATE_HANDLE_SIZE_PX`'s own doc comment).
    pub is_rotate: bool,
    /// Whether this exact handle is the one currently being dragged.
    pub dragging: bool,
}

/// What the Select tool's transform-handle overlay decorates this frame
/// (acceptance criteria 1, 14-17, 22 of `specs/0005-object-transform/
/// specification.md`) — every handle of the single selected object
/// currently showing them, plus the pivot marker shown for the duration
/// of a scale/rotate drag only.
#[derive(Debug, Clone, Default)]
pub struct TransformDecorationInput {
    /// Every transform handle currently shown (empty when zero or two-
    /// plus objects are selected, acceptance criterion 2).
    pub handles: Vec<TransformHandleGlyph>,
    /// The active scale/rotate pivot, shown only while a drag is in
    /// flight (`docs/design-system.md`'s "Transform pivot marker").
    pub pivot_marker: Option<Point>,
}

/// A hollow resize-handle glyph: `--accent` outline, white idle fill or
/// solid `--accent` fill while dragging — the same nested-square
/// construction `shape_preview.rs`'s own shape-handle glyph uses
/// (`docs/design-system.md`'s "one fill-state rule for every handle in
/// the product").
fn resize_handle_glyph(view: ViewTransform, center: Point, dragging: bool) -> DrawList {
    let size = screen_px_to_mm(view, theme::TRANSFORM_RESIZE_HANDLE_SIZE_PX);
    let outline = screen_px_to_mm(view, theme::TRANSFORM_RESIZE_HANDLE_OUTLINE_PX);
    let mut list = glyphs::square(center, size, theme::ACCENT);
    let fill = if dragging {
        theme::ACCENT
    } else {
        RgbaColor::WHITE
    };
    list.extend(glyphs::square(
        center,
        (size - 2.0 * outline).max(0.0),
        fill,
    ));
    list
}

/// The rotate handle's glyph — `--accent` stroke / transparent idle,
/// solid `--accent` fill while dragging (`docs/design-system.md`). Drawn
/// as a plain ring/disc in this crate; the "circular-arrow icon" detail
/// is frontend/ux-engineer follow-up (`theme::TRANSFORM_ROTATE_HANDLE_
/// SIZE_PX`'s own doc comment).
fn rotate_handle_glyph(view: ViewTransform, center: Point, dragging: bool) -> DrawList {
    let size = screen_px_to_mm(view, theme::TRANSFORM_ROTATE_HANDLE_SIZE_PX);
    if dragging {
        glyphs::circle(center, size, theme::ACCENT)
    } else {
        glyphs::ring(center, size, screen_px_to_mm(view, 1.5), theme::ACCENT)
    }
}

/// Builds the Select tool's transform-handle overlay for this frame.
#[must_use]
pub fn build_transform_handles(view: ViewTransform, input: &TransformDecorationInput) -> DrawList {
    let mut list = DrawList::default();
    for handle in &input.handles {
        if handle.is_rotate {
            list.extend(rotate_handle_glyph(view, handle.position, handle.dragging));
        } else {
            list.extend(resize_handle_glyph(view, handle.position, handle.dragging));
        }
    }
    if let Some(pivot) = input.pivot_marker {
        list.extend(glyphs::circle(
            pivot,
            screen_px_to_mm(view, theme::TRANSFORM_PIVOT_MARKER_SIZE_PX),
            theme::TRANSFORM_PIVOT_MARKER_COLOR,
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

    /// No handles and no pivot marker draws nothing (acceptance
    /// criterion 2: multi/no selection shows no transform handles).
    #[test]
    fn no_transform_handles_and_no_pivot_draws_nothing() {
        let list = build_transform_handles(
            ViewTransform::identity(),
            &TransformDecorationInput::default(),
        );
        assert_eq!(list.triangles.len(), 0);
    }

    /// Acceptance criterion 1: a resize handle and the rotate handle
    /// both draw geometry.
    #[test]
    fn resize_and_rotate_handles_each_draw_geometry() {
        let input = TransformDecorationInput {
            handles: vec![
                TransformHandleGlyph {
                    position: Point::new(10.0, 10.0),
                    is_rotate: false,
                    dragging: false,
                },
                TransformHandleGlyph {
                    position: Point::new(5.0, -10.0),
                    is_rotate: true,
                    dragging: false,
                },
            ],
            pivot_marker: None,
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    /// The pivot marker draws only while present.
    #[test]
    fn pivot_marker_draws_when_present() {
        let input = TransformDecorationInput {
            handles: vec![],
            pivot_marker: Some(Point::new(5.0, 5.0)),
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }
}
