//! Draws the Select tool's transform handles, pivot marker and guides.
//!
//! The selection and hover boxes live in [`crate::select_box`]. The handles
//! reach here as this crate's own minimal [`TransformHandleGlyph`] values,
//! because it cannot read `vecmanf-ui-core`'s `EditHandle` (ADR 0011 §3).

use vecmanf_document_core::{Angle, Point, Vec2, ViewTransform};

use crate::color::RgbaColor;
use crate::glyphs::{self, DrawList};
use crate::screen_px_to_mm;
use crate::select_box;
use crate::shape_preview;
use crate::theme;

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
    /// A parameter handle's knob: a circle with a ring and a centre dot
    /// (the corner radius of a rectangle, a star's inner radius).
    Parameter,
}

/// One of the Select tool's transform handles — this crate's own minimal
/// shape (ADR 0011 §3: it cannot read `vecmanf-ui-core`'s
/// `EditHandle` directly), carrying only what drawing needs: where
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
    /// Dashed guides from a rectangle corner to its hovered or dragged
    /// radius handle (`docs/design-system.md`, "Parameter handle guide"):
    /// corner first, handle second.
    pub param_guides: Vec<(Point, Point)>,
    /// `window.devicePixelRatio`: the skew guide snaps to whole device pixels
    /// with it, as the selection box does. Not a positive finite number (and
    /// the default 0) reads as 1.
    pub device_pixel_ratio: f64,
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

/// The parameter handle's knob: a `--accent` ring around a white ground, an
/// `--accent` centre dot; `--accent-hover` ground on hover (and on the
/// followers of a radius drag), solid `--accent` ground with a white dot
/// while dragging (`docs/design-system.md`, "Parameter handle"). Round with a
/// centre dot is the one silhouette no other canvas glyph has.
fn parameter_handle_glyph(
    view: ViewTransform,
    center: Point,
    dragging: bool,
    hovered: bool,
) -> DrawList {
    let size = screen_px_to_mm(view, theme::PARAM_KNOB_DIAMETER_PX);
    let ring = screen_px_to_mm(view, theme::PARAM_KNOB_RING_PX);
    let dot = screen_px_to_mm(view, theme::PARAM_KNOB_DOT_PX);
    let ground_size = (size - 2.0 * ring).max(0.0);
    let mut list = glyphs::circle(center, size, theme::SHAPE_HANDLE_STROKE);
    list.extend(glyphs::circle(center, ground_size, RgbaColor::WHITE));
    let dot_color = if dragging {
        list.extend(glyphs::circle(center, ground_size, theme::ACCENT));
        RgbaColor::WHITE
    } else {
        if hovered {
            list.extend(glyphs::circle(center, ground_size, theme::ACCENT_HOVER));
        }
        theme::ACCENT
    };
    list.extend(glyphs::circle(center, dot, dot_color));
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
    // Parameter handles sit above every other handle (`docs/design-system.md`):
    // their guides and knobs are drawn after the loop below.
    let transform_handles = input
        .handles
        .iter()
        .filter(|handle| handle.kind != TransformGlyphKind::Parameter);
    for handle in transform_handles {
        if let Some(pivot) = input.pivot_marker
            && handle.position.vector_to(pivot).length() <= at_pivot_mm
        {
            continue;
        }
        list.extend(match handle.kind {
            TransformGlyphKind::Parameter => continue,
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
    for &(corner, handle) in &input.param_guides {
        list.extend(shape_preview::dashed_guide(
            corner,
            handle,
            screen_px_to_mm(view, 1.0),
            theme::SHAPE_HANDLE_GUIDE,
            screen_px_to_mm(view, theme::GUIDE_DASH_PX),
            screen_px_to_mm(view, theme::GUIDE_GAP_PX),
        ));
    }
    for handle in input
        .handles
        .iter()
        .filter(|handle| handle.kind == TransformGlyphKind::Parameter)
    {
        list.extend(parameter_handle_glyph(
            view,
            handle.position,
            handle.dragging,
            handle.hovered,
        ));
    }
    if let Some((from, to)) = input.skew_guide {
        let (from, to, width_px) =
            select_box::snap_guide_line(view, from, to, input.device_pixel_ratio);
        list.extend(shape_preview::dashed_guide(
            from,
            to,
            screen_px_to_mm(view, width_px),
            theme::TRANSFORM_SKEW_GUIDE_COLOR,
            screen_px_to_mm(view, theme::SKEW_GUIDE_DASH_PX),
            screen_px_to_mm(view, theme::SKEW_GUIDE_GAP_PX),
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
            param_guides: Vec::new(),
            ..TransformDecorationInput::default()
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
                param_guides: Vec::new(),
                ..TransformDecorationInput::default()
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
            param_guides: Vec::new(),
            ..TransformDecorationInput::default()
        };
        let at_handle = TransformDecorationInput {
            handles: vec![handle(0.0, 0.0), handle(10.0, 10.0)],
            pivot_marker: Some(Point::new(0.0, 0.0)),
            skew_guide: None,
            param_guides: Vec::new(),
            ..TransformDecorationInput::default()
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
                param_guides: Vec::new(),
                ..TransformDecorationInput::default()
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
            param_guides: Vec::new(),
            ..TransformDecorationInput::default()
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
                param_guides: Vec::new(),
                ..TransformDecorationInput::default()
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
            param_guides: Vec::new(),
            ..TransformDecorationInput::default()
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        // 70 px at 2 on / 2 off (criterion 68): eighteen dashes of two triangles each.
        assert_eq!(list.triangle_count(), 36);
    }

    fn glyph_list(kind: TransformGlyphKind, dragging: bool, hovered: bool) -> DrawList {
        build_transform_handles(
            ViewTransform::identity(),
            &TransformDecorationInput {
                handles: vec![TransformHandleGlyph {
                    position: Point::new(0.0, 0.0),
                    kind,
                    dragging,
                    hovered,
                }],
                ..TransformDecorationInput::default()
            },
        )
    }

    fn extent(list: &DrawList) -> f64 {
        list.triangles
            .iter()
            .map(|v| v.position.vector_to(Point::new(0.0, 0.0)).length())
            .fold(0.0, f64::max)
    }

    /// `vecmanf-ui-core`'s 4 px clearance property is derived from these
    /// numbers (`param_handles.rs`: 8 px resize squircle, 12 px rotate glyph,
    /// 16 px centre glyph, 10 px knob). The two crates share no code, so the
    /// duplicated numbers are guarded here too: no glyph is drawn larger than
    /// its circumscribed radius there.
    #[test]
    fn no_glyph_is_drawn_larger_than_the_size_the_clearance_property_assumes() {
        let half_diagonal = |side: f64| side * std::f64::consts::FRAC_1_SQRT_2;
        // The farthest point of a rounded square: its corner arc's centre
        // plus the radius.
        let rounded_square_extent =
            |side: f64, radius: f64| (side / 2.0 - radius) * std::f64::consts::SQRT_2 + radius;
        let cases = [
            (TransformGlyphKind::Resize, half_diagonal(8.0)),
            (TransformGlyphKind::Rotate, 6.0),
            (TransformGlyphKind::Move, rounded_square_extent(16.0, 3.0)),
            (TransformGlyphKind::Parameter, 5.0),
        ];
        for (kind, bound) in cases {
            for (dragging, hovered) in [(false, false), (false, true), (true, false)] {
                let list = glyph_list(kind, dragging, hovered);
                assert_ne!(list.triangle_count(), 0, "{kind:?}");
                assert!(
                    extent(&list) <= bound + 1e-9,
                    "{kind:?} reaches {} px, over {bound}",
                    extent(&list)
                );
            }
        }
    }

    /// Criterion 8: the knob is round with a centre dot, in both the idle
    /// and the dragging state; no other glyph is.
    #[test]
    fn the_parameter_knob_is_a_ring_with_a_centre_dot_whose_states_differ_by_ground() {
        let idle = glyph_list(TransformGlyphKind::Parameter, false, false);
        let hover = glyph_list(TransformGlyphKind::Parameter, false, true);
        let drag = glyph_list(TransformGlyphKind::Parameter, true, false);
        let has =
            |list: &DrawList, color: RgbaColor| list.triangles.iter().any(|v| v.color == color);
        assert!(has(&idle, RgbaColor::WHITE) && has(&idle, theme::ACCENT));
        assert!(!has(&idle, theme::ACCENT_HOVER));
        assert!(has(&hover, theme::ACCENT_HOVER), "hover ground");
        assert!(!has(&drag, RgbaColor::WHITE) || has(&drag, theme::ACCENT));
        // A centre dot: a vertex at the 2 px dot radius exists in every state.
        for list in [&idle, &hover, &drag] {
            assert!(list.triangles.iter().any(|v| {
                let r = v.position.vector_to(Point::new(0.0, 0.0)).length();
                (r - theme::PARAM_KNOB_DOT_PX / 2.0).abs() < 1e-6
            }));
        }
        // The dragging dot is white on a solid accent ground.
        let dot_color = drag
            .triangles
            .iter()
            .rev()
            .find(|v| {
                let r = v.position.vector_to(Point::new(0.0, 0.0)).length();
                (r - theme::PARAM_KNOB_DOT_PX / 2.0).abs() < 1e-6
            })
            .map(|v| v.color);
        assert_eq!(dot_color, Some(RgbaColor::WHITE));
    }

    /// `docs/design-system.md`: the parameter handle is drawn above every
    /// other handle, so its triangles come last in the list.
    #[test]
    fn parameter_handles_are_drawn_after_every_other_handle() {
        let input = TransformDecorationInput {
            handles: vec![
                TransformHandleGlyph {
                    position: Point::new(0.0, 0.0),
                    kind: TransformGlyphKind::Parameter,
                    dragging: false,
                    hovered: false,
                },
                TransformHandleGlyph {
                    position: Point::new(50.0, 0.0),
                    kind: TransformGlyphKind::Resize,
                    dragging: false,
                    hovered: false,
                },
            ],
            ..TransformDecorationInput::default()
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        let final_vertex = list.triangles.last().expect("geometry").position;
        assert!(
            final_vertex.vector_to(Point::new(0.0, 0.0)).length() < 6.0,
            "the knob, listed first, is drawn last"
        );
    }

    #[test]
    fn a_guide_joins_the_corner_to_a_radius_handle() {
        let input = TransformDecorationInput {
            param_guides: vec![(Point::new(0.0, 0.0), Point::new(30.0, 30.0))],
            ..TransformDecorationInput::default()
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        assert_ne!(list.triangle_count(), 0);
        assert!(
            list.triangles
                .iter()
                .all(|v| v.color == theme::SHAPE_HANDLE_GUIDE)
        );
    }

    /// Criterion 68: the axis-aligned guide is pixel-snapped like the box (one
    /// whole device row or column, full accent), so it no longer renders as
    /// pale blocks; the ends sit on whole device pixels.
    #[test]
    fn the_skew_guide_sits_on_whole_device_pixels() {
        for ratio in [1.0, 1.5, 2.0, 3.0] {
            let input = TransformDecorationInput {
                skew_guide: Some((Point::new(-5.3, 40.7), Point::new(126.2, 40.7))),
                device_pixel_ratio: ratio,
                ..TransformDecorationInput::default()
            };
            let list = build_transform_handles(ViewTransform::identity(), &input);
            assert_ne!(list.triangle_count(), 0);
            let width = crate::select_box::device_line_width(ratio);
            for v in &list.triangles {
                assert_eq!(v.color, theme::TRANSFORM_SKEW_GUIDE_COLOR);
                let y = v.position.y * ratio;
                // Across the line the edges are whole device pixel boundaries.
                assert!((y - y.round()).abs() < 1e-6, "ratio {ratio}: y {y}");
            }
            let ys: Vec<f64> = list
                .triangles
                .iter()
                .map(|v| v.position.y * ratio)
                .collect();
            let span = ys.iter().copied().fold(f64::MIN, f64::max)
                - ys.iter().copied().fold(f64::MAX, f64::min);
            assert!(
                (span - width).abs() < 1e-6,
                "ratio {ratio}: {span} device px thick"
            );
        }
    }

    /// At ratio 1 every dash of the guide is two whole pixels with two whole
    /// pixels between, so none renders as a half-covered block.
    #[test]
    fn the_snapped_guide_dashes_are_whole_pixels_at_ratio_one() {
        let input = TransformDecorationInput {
            skew_guide: Some((Point::new(-5.3, 40.7), Point::new(126.2, 40.7))),
            device_pixel_ratio: 1.0,
            ..TransformDecorationInput::default()
        };
        let list = build_transform_handles(ViewTransform::identity(), &input);
        for v in &list.triangles {
            assert!(
                (v.position.x - v.position.x.round()).abs() < 1e-6,
                "{}",
                v.position.x
            );
        }
    }

    /// A rotated guide is not snapped: the same dashes the guide drew before.
    #[test]
    fn a_rotated_skew_guide_keeps_its_true_line() {
        let (from, to) = (Point::new(3.3, 4.4), Point::new(90.1, 52.7));
        let input = TransformDecorationInput {
            skew_guide: Some((from, to)),
            device_pixel_ratio: 2.0,
            ..TransformDecorationInput::default()
        };
        let view = ViewTransform::identity();
        let got = build_transform_handles(view, &input);
        let want = shape_preview::dashed_guide(
            from,
            to,
            1.0,
            theme::TRANSFORM_SKEW_GUIDE_COLOR,
            theme::SKEW_GUIDE_DASH_PX,
            theme::SKEW_GUIDE_GAP_PX,
        );
        assert_eq!(got.triangles, want.triangles);
    }
}
