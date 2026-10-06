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
//! box reaches here as four document-space corners (oriented to the
//! object's own rotation, `object-transform` acceptance criterion 18),
//! computed by `vecmanf_ui_core::oriented_bounds` and passed through by
//! `vecmanf-editor-wasm` — the same way `primitive-shapes` already passes
//! shape-handle positions.

use vecmanf_document_core::{Angle, NodeId, Point, Vec2, ViewTransform};

use crate::color::RgbaColor;
use crate::glyphs::{self, DrawList, quad_outline};
use crate::shape_preview;
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
    for &(_, corners) in &input.selected {
        list.extend(quad_outline(corners, width_mm, theme::ACCENT));
    }
    if let Some((_, corners)) = input.hovered {
        list.extend(quad_outline(corners, width_mm, theme::ACCENT_HOVER));
    }
    list
}

/// Which glyph a transform handle draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformGlyphKind {
    /// A hollow rounded square ("squircle"): resize.
    Resize,
    /// The circular-arrow icon, never turned: rotate (corner or side).
    Rotate,
    /// Two opposed parallel arrows along `direction` (a unit vector in
    /// document space, the side's own axis): skew.
    Skew {
        /// The unit vector the arrows point along.
        direction: Vec2,
    },
    /// The centre move handle: a rounded square with a four-way arrow.
    Move,
}

/// One of the Select tool's transform handles — this crate's own minimal
/// shape (ADR 0011 §3: it cannot read `vecmanf-ui-core`'s
/// `TransformHandle` directly), carrying only what drawing needs: where
/// it is, which glyph vocabulary it uses, and whether it is the one
/// currently being dragged (solid `--accent` fill instead of the idle
/// hollow/transparent state, `docs/design-system.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformHandleGlyph {
    /// Document-space position.
    pub position: Point,
    /// Which glyph to draw.
    pub kind: TransformGlyphKind,
    /// Whether this exact handle is the one currently being dragged (or
    /// whose numeric entry is open).
    pub dragging: bool,
    /// Whether the pointer is over this handle (and no drag is in
    /// flight) — the rotate, skew and move glyphs have a hover look
    /// (`--accent-hover`, `docs/design-system.md`); the resize glyph has
    /// none.
    pub hovered: bool,
}

/// What the Select tool's transform-handle overlay decorates this frame
/// (acceptance criteria 1, 14-17, 22 of `specs/0005-object-transform/
/// specification.md`; 1, 5, 6, 37, 55, 56 of `object-transform-
/// refinements`) — every handle of the single selected object currently
/// showing them, the pivot marker, and the skew fixed-line guide.
#[derive(Debug, Clone, Default)]
pub struct TransformDecorationInput {
    /// Every transform handle currently shown (empty when zero or two-
    /// plus objects are selected, acceptance criterion 2).
    pub handles: Vec<TransformHandleGlyph>,
    /// The active scale/rotate/skew pivot (during a drag or while an entry
    /// is open), or the preview of the pivot the hovered handle would use
    /// (`docs/design-system.md`'s "Transform pivot marker").
    pub pivot_marker: Option<Point>,
    /// The two end points of the dashed guide along the skew's fixed line,
    /// during a skew drag only.
    pub skew_guide: Option<(Point, Point)>,
}

/// A hollow resize-handle glyph: `--accent` outline on a rounded square
/// ("squircle", `docs/design-system.md`: 2px corner radius), white idle
/// fill or solid `--accent` fill while dragging — the same nested
/// construction `shape_preview.rs`'s own shape-handle glyph uses
/// (`docs/design-system.md`'s "one fill-state rule for every handle in
/// the product"), with the rounded corner as the one deliberate
/// silhouette difference.
fn resize_handle_glyph(view: ViewTransform, center: Point, dragging: bool) -> DrawList {
    let size = screen_px_to_mm(view, theme::TRANSFORM_RESIZE_HANDLE_SIZE_PX);
    let outline = screen_px_to_mm(view, theme::TRANSFORM_RESIZE_HANDLE_OUTLINE_PX);
    let radius = screen_px_to_mm(view, theme::TRANSFORM_RESIZE_HANDLE_CORNER_RADIUS_PX);
    let mut list = glyphs::rounded_square(center, size, radius, theme::ACCENT);
    let fill = if dragging {
        theme::ACCENT
    } else {
        RgbaColor::WHITE
    };
    list.extend(glyphs::rounded_square(
        center,
        (size - 2.0 * outline).max(0.0),
        (radius - outline).max(0.0),
        fill,
    ));
    list
}

/// The rotate handle's glyph: a circular-arrow icon — `--accent` stroke
/// on a transparent ground when idle, `--accent-hover` fill behind it on
/// hover, solid `--accent` fill with a white glyph while dragging
/// (`docs/design-system.md`).
fn rotate_handle_glyph(
    view: ViewTransform,
    center: Point,
    dragging: bool,
    hovered: bool,
) -> DrawList {
    let size = screen_px_to_mm(view, theme::TRANSFORM_ROTATE_HANDLE_SIZE_PX);
    let thickness = screen_px_to_mm(view, theme::TRANSFORM_ROTATE_HANDLE_STROKE_PX);
    let mut list = DrawList::default();
    let glyph_color = if dragging {
        list.extend(glyphs::circle(center, size, theme::ACCENT));
        RgbaColor::WHITE
    } else {
        if hovered {
            list.extend(glyphs::circle(center, size, theme::ACCENT_HOVER));
        }
        theme::ACCENT
    };
    // The arc fills 90% of the 12px footprint; the arrowhead flares to
    // about the edge of it.
    list.extend(glyphs::arc_arrow(
        center,
        size * 0.75,
        thickness,
        glyph_color,
    ));
    list
}

/// The centre move handle's glyph: a white rounded square with an
/// `--accent` outline and a four-way arrow; `--accent-hover` ground on
/// hover, solid `--accent` with a white arrow while its own move runs
/// (`docs/design-system.md`, "Transform center move handle"). Does not turn
/// with the box: a move runs along the screen axes.
fn move_handle_glyph(
    view: ViewTransform,
    center: Point,
    dragging: bool,
    hovered: bool,
) -> DrawList {
    let size = screen_px_to_mm(view, theme::TRANSFORM_MOVE_HANDLE_SIZE_PX);
    let outline = screen_px_to_mm(view, theme::TRANSFORM_MOVE_HANDLE_OUTLINE_PX);
    let radius = screen_px_to_mm(view, theme::TRANSFORM_MOVE_HANDLE_CORNER_RADIUS_PX);
    let (ground, arrow_color) = if dragging {
        (theme::ACCENT, RgbaColor::WHITE)
    } else if hovered {
        (theme::ACCENT_HOVER, theme::ACCENT)
    } else {
        (RgbaColor::WHITE, theme::ACCENT)
    };
    let mut list = glyphs::rounded_square(center, size, radius, theme::ACCENT);
    list.extend(glyphs::rounded_square(
        center,
        (size - 2.0 * outline).max(0.0),
        (radius - outline).max(0.0),
        RgbaColor::WHITE,
    ));
    if ground != RgbaColor::WHITE {
        list.extend(glyphs::rounded_square(
            center,
            (size - 2.0 * outline).max(0.0),
            (radius - outline).max(0.0),
            ground,
        ));
    }
    let reach = screen_px_to_mm(view, theme::TRANSFORM_MOVE_ARROW_SIZE_PX) / 2.0;
    let stroke = screen_px_to_mm(view, theme::TRANSFORM_ARROW_STROKE_PX);
    let head = screen_px_to_mm(view, theme::TRANSFORM_ARROW_HEAD_PX);
    for direction in [
        Vec2::new(1.0, 0.0),
        Vec2::new(-1.0, 0.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(0.0, -1.0),
    ] {
        list.extend(glyphs::arrow(
            center,
            center.translated(direction.scaled(reach)),
            stroke,
            head,
            arrow_color,
        ));
    }
    list
}

/// The skew handle's glyph: two opposed parallel arrows along `direction`
/// (the "shear" picture, never a single double arrow, so it cannot read as
/// a resize handle) — `--accent` strokes on a transparent ground idle,
/// `--accent-hover` rounded-rect ground on hover, solid `--accent` ground
/// with white arrows while dragging (`docs/design-system.md`, "Transform
/// skew handle").
fn skew_handle_glyph(
    view: ViewTransform,
    center: Point,
    direction: Vec2,
    dragging: bool,
    hovered: bool,
) -> DrawList {
    let length = screen_px_to_mm(view, theme::TRANSFORM_SKEW_HANDLE_LENGTH_PX);
    let width = screen_px_to_mm(view, theme::TRANSFORM_SKEW_HANDLE_WIDTH_PX);
    let stroke = screen_px_to_mm(view, theme::TRANSFORM_ARROW_STROKE_PX);
    let head = screen_px_to_mm(view, theme::TRANSFORM_ARROW_HEAD_PX);
    let radius = screen_px_to_mm(view, theme::TRANSFORM_SKEW_HANDLE_CORNER_RADIUS_PX);
    let unit = direction.normalized_to(1.0);
    let mut list = DrawList::default();
    let arrow_color = if dragging {
        list.extend(glyphs::rounded_rect(
            center,
            (length, width),
            radius,
            Angle::from_radians(unit.y.atan2(unit.x)),
            theme::ACCENT,
        ));
        RgbaColor::WHITE
    } else {
        if hovered {
            list.extend(glyphs::rounded_rect(
                center,
                (length, width),
                radius,
                Angle::from_radians(unit.y.atan2(unit.x)),
                theme::ACCENT_HOVER,
            ));
        }
        theme::ACCENT
    };
    // Two arrows, half the head's width off the axis on either side, one
    // pointing each way: the footprint is `length` by `width`.
    let across = Vec2::new(-unit.y, unit.x).scaled(width / 2.0 - head);
    let half = unit.scaled(length / 2.0);
    for (offset, from, to) in [
        (across, half.negated(), half),
        (across.negated(), half, half.negated()),
    ] {
        list.extend(glyphs::arrow(
            center.translated(offset).translated(from),
            center.translated(offset).translated(to),
            stroke,
            head,
            arrow_color,
        ));
    }
    list
}

/// Builds the Select tool's transform-handle overlay for this frame.
#[must_use]
pub fn build_transform_handles(view: ViewTransform, input: &TransformDecorationInput) -> DrawList {
    let mut list = DrawList::default();
    // A handle exactly at the active pivot (Shift put the pivot on it, or
    // a resize anchors on the opposite corner) is not drawn: the pivot
    // dot replaces it, so it never reads as a pressed handle with a dot
    // on top. "At" is within one screen pixel.
    let at_pivot_mm = screen_px_to_mm(view, 1.0);
    for handle in &input.handles {
        if let Some(pivot) = input.pivot_marker
            && handle.position.vector_to(pivot).length() <= at_pivot_mm
        {
            continue;
        }
        list.extend(match handle.kind {
            TransformGlyphKind::Resize => {
                resize_handle_glyph(view, handle.position, handle.dragging)
            }
            TransformGlyphKind::Rotate => {
                rotate_handle_glyph(view, handle.position, handle.dragging, handle.hovered)
            }
            TransformGlyphKind::Skew { direction } => skew_handle_glyph(
                view,
                handle.position,
                direction,
                handle.dragging,
                handle.hovered,
            ),
            TransformGlyphKind::Move => {
                move_handle_glyph(view, handle.position, handle.dragging, handle.hovered)
            }
        });
    }
    if let Some((from, to)) = input.skew_guide {
        list.extend(shape_preview::dashed_guide(
            from,
            to,
            screen_px_to_mm(view, theme::TRANSFORM_SKEW_GUIDE_WIDTH_PX),
            theme::TRANSFORM_SKEW_GUIDE_COLOR,
            screen_px_to_mm(view, theme::GUIDE_DASH_PX),
            screen_px_to_mm(view, theme::GUIDE_GAP_PX),
        ));
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
                    kind: TransformGlyphKind::Resize,
                    dragging: false,
                    hovered: false,
                },
                TransformHandleGlyph {
                    position: Point::new(5.0, -10.0),
                    kind: TransformGlyphKind::Rotate,
                    dragging: false,
                    hovered: false,
                },
            ],
            pivot_marker: None,
            skew_guide: None,
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }

    /// Design-system fill states for the rotate handle: idle is just the
    /// arc icon; hover adds the `--accent-hover` ground; dragging adds
    /// the solid `--accent` ground.
    #[test]
    fn rotate_handle_has_distinct_idle_hover_and_dragging_looks() {
        let count = |dragging: bool, hovered: bool| {
            let input = TransformDecorationInput {
                handles: vec![TransformHandleGlyph {
                    position: Point::new(0.0, 0.0),
                    kind: TransformGlyphKind::Rotate,
                    dragging,
                    hovered,
                }],
                pivot_marker: None,
                skew_guide: None,
            };
            build_transform_handles(ViewTransform::identity(), &input)
        };
        let idle = count(false, false);
        let hover = count(false, true);
        let drag = count(true, false);
        assert!(hover.triangle_count() > idle.triangle_count());
        assert!(drag.triangle_count() > idle.triangle_count());
        assert!(
            drag.triangles.iter().any(|v| v.color == RgbaColor::WHITE),
            "the dragging glyph is white on a solid ground"
        );
    }

    /// UX review item 4: a handle sitting exactly on the pivot is not
    /// drawn, so the pivot dot is not painted over a "pressed" handle.
    #[test]
    fn a_handle_at_the_pivot_is_replaced_by_the_pivot_dot() {
        let handle = |x: f64, y: f64| TransformHandleGlyph {
            position: Point::new(x, y),
            kind: TransformGlyphKind::Resize,
            dragging: false,
            hovered: false,
        };
        let with_pivot_elsewhere = TransformDecorationInput {
            handles: vec![handle(0.0, 0.0), handle(10.0, 10.0)],
            pivot_marker: Some(Point::new(50.0, 50.0)),
            skew_guide: None,
        };
        let at_handle = TransformDecorationInput {
            handles: vec![handle(0.0, 0.0), handle(10.0, 10.0)],
            pivot_marker: Some(Point::new(0.0, 0.0)),
            skew_guide: None,
        };
        let view = ViewTransform::identity();
        let elsewhere = build_transform_handles(view, &with_pivot_elsewhere);
        let on_handle = build_transform_handles(view, &at_handle);
        assert!(
            on_handle.triangle_count() < elsewhere.triangle_count(),
            "one handle glyph fewer when the pivot sits on it"
        );
        let handle_glyph = build_transform_handles(
            view,
            &TransformDecorationInput {
                handles: vec![handle(0.0, 0.0)],
                pivot_marker: None,
                skew_guide: None,
            },
        );
        assert_eq!(
            elsewhere.triangle_count() - on_handle.triangle_count(),
            handle_glyph.triangle_count()
        );
    }

    /// The pivot marker draws only while present.
    #[test]
    fn pivot_marker_draws_when_present() {
        let input = TransformDecorationInput {
            handles: vec![],
            pivot_marker: Some(Point::new(5.0, 5.0)),
            skew_guide: None,
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        assert_ne!(list.triangles.len(), 0);
    }
    fn glyph(kind: TransformGlyphKind, dragging: bool, hovered: bool) -> TransformHandleGlyph {
        TransformHandleGlyph {
            position: Point::new(0.0, 0.0),
            kind,
            dragging,
            hovered,
        }
    }

    fn build_one(g: TransformHandleGlyph) -> DrawList {
        build_transform_handles(
            ViewTransform::identity(),
            &TransformDecorationInput {
                handles: vec![g],
                pivot_marker: None,
                skew_guide: None,
            },
        )
    }

    /// The centre move handle fits its 16 px footprint and has idle, hover
    /// and dragging looks; dragging turns the arrow white on a solid ground.
    #[test]
    fn the_centre_move_handle_has_its_footprint_and_three_looks() {
        let idle = build_one(glyph(TransformGlyphKind::Move, false, false));
        let hover = build_one(glyph(TransformGlyphKind::Move, false, true));
        let drag = build_one(glyph(TransformGlyphKind::Move, true, false));
        assert_ne!(idle.triangle_count(), 0);
        for v in &idle.triangles {
            assert!(v.position.x.abs() <= 8.0 + 1e-9 && v.position.y.abs() <= 8.0 + 1e-9);
        }
        assert!(
            hover
                .triangles
                .iter()
                .any(|v| v.color == theme::ACCENT_HOVER)
        );
        assert!(
            !idle
                .triangles
                .iter()
                .any(|v| v.color == theme::ACCENT_HOVER)
        );
        assert!(drag.triangles.iter().any(|v| v.color == RgbaColor::WHITE));
        assert!(drag.triangles.iter().any(|v| v.color == theme::ACCENT));
    }

    /// The skew glyph stays inside its 18 x 12 footprint, follows its
    /// direction, and has idle, hover and dragging looks.
    #[test]
    fn the_skew_handle_follows_its_axis_and_keeps_its_footprint() {
        let along_x = build_one(glyph(
            TransformGlyphKind::Skew {
                direction: Vec2::new(1.0, 0.0),
            },
            false,
            false,
        ));
        let along_y = build_one(glyph(
            TransformGlyphKind::Skew {
                direction: Vec2::new(0.0, 1.0),
            },
            false,
            false,
        ));
        let extent = |list: &DrawList| {
            let xs = list.triangles.iter().map(|v| v.position.x);
            let ys = list.triangles.iter().map(|v| v.position.y);
            (
                xs.clone().fold(f64::MIN, f64::max) - xs.fold(f64::MAX, f64::min),
                ys.clone().fold(f64::MIN, f64::max) - ys.fold(f64::MAX, f64::min),
            )
        };
        let (w, h) = extent(&along_x);
        assert!(w <= 18.0 + 1e-9 && w > 15.0, "{w}");
        assert!(h <= 12.0 + 1e-9 && h > 6.0, "{h}");
        let (w, h) = extent(&along_y);
        assert!(h <= 18.0 + 1e-9 && h > 15.0, "turned: {h}");
        assert!(w <= 12.0 + 1e-9);
        let direction = Vec2::new(1.0, 0.0);
        let hover = build_one(glyph(TransformGlyphKind::Skew { direction }, false, true));
        let drag = build_one(glyph(TransformGlyphKind::Skew { direction }, true, false));
        assert!(hover.triangle_count() > along_x.triangle_count());
        assert!(drag.triangles.iter().any(|v| v.color == RgbaColor::WHITE));
    }

    /// The fixed-line guide is dashed: several separate pieces over its length.
    #[test]
    fn the_skew_guide_draws_as_dashes() {
        let input = TransformDecorationInput {
            handles: vec![],
            pivot_marker: None,
            skew_guide: Some((Point::new(0.0, 0.0), Point::new(70.0, 0.0))),
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        // 70 px at 4 on / 3 off: ten dashes of two triangles each.
        assert_eq!(list.triangle_count(), 20);
    }
}
