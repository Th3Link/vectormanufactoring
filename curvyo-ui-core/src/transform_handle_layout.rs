//! The Select tool's own transform-handle vocabulary
//! (`specs/0005-object-transform/specification.md`, acceptance criteria
//! 1, 4-17): 8 resize handles (corner + edge-midpoint) plus one rotate
//! handle, laid out in an [`OrientedBox`]'s local frame and mapped to
//! document space through it (`adrs.md`: "every handle position... is
//! computed in the object's own local, rotated coordinate frame").
//! This is the Select tool's one, kind-independent vocabulary of
//! transform handles on top of any object's [`OrientedBox`]; the parameter
//! (knob) handles are laid out in [`crate::param_handles`]. The resize and
//! rotate arithmetic lives next door in [`crate::transform_math`].

use vecmanf_document_core::{Angle, Point, Tolerance};

use crate::ResizeDirection;
use crate::oriented_box::OrientedBox;
use crate::param_handles::{
    CENTRE_YIELD_PX, KNOB_INSET_PX, KNOB_PITCH_PX, PARAM_HIT_PX, PARAM_MIN_SIDE_PX, ParamHandle,
};

/// The 8 resize handles a rectangle, ellipse or path shows (acceptance
/// criterion 1).
pub const ALL_EIGHT: [ResizeDirection; 8] = ResizeDirection::ALL_EIGHT;

/// The 4 corner-only handles a polygon or star shows (acceptance
/// criterion 11: "corner handles only — no edge handles").
pub const CORNERS_FOUR: [ResizeDirection; 4] = [
    ResizeDirection::Ne,
    ResizeDirection::Se,
    ResizeDirection::Sw,
    ResizeDirection::Nw,
];

/// One side of an oriented box: the four skew handles sit one per side.
/// Its own type (not [`ResizeDirection`]) so an invalid `Skew(Ne)` cannot be
/// written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The top edge (local minimum y); an x skew.
    Top,
    /// The right edge (local maximum x); a y skew.
    Right,
    /// The bottom edge (local maximum y); an x skew.
    Bottom,
    /// The left edge (local minimum x); a y skew.
    Left,
}

impl Side {
    /// Every side, in a fixed order.
    pub const ALL: [Self; 4] = [Self::Top, Self::Right, Self::Bottom, Self::Left];

    /// Whether a skew from this side shears along the box's `u` axis (top
    /// and bottom: an x skew) rather than `v` (left and right: a y skew).
    #[must_use]
    pub const fn skews_along_u(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }

    /// The resize direction of the same side's midpoint.
    #[must_use]
    pub const fn direction(self) -> ResizeDirection {
        match self {
            Self::Top => ResizeDirection::N,
            Self::Right => ResizeDirection::E,
            Self::Bottom => ResizeDirection::S,
            Self::Left => ResizeDirection::W,
        }
    }

    /// The side opposite this one: the fixed edge of a skew from it.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}

/// One of the Select tool's own transform handles
/// (`specs/object-transform-refinements/adrs.md`, "handle set, hit test and
/// the Shift reveal").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditHandle {
    /// A resize handle (corner = free/proportional resize, edge
    /// midpoint = single-axis resize).
    Resize(ResizeDirection),
    /// A rotate handle outside a corner (always shown) or outside a side
    /// midpoint (only while Shift is held). The direction names the
    /// position; the pivot is derived from it (`rotate_pivot`).
    Rotate(ResizeDirection),
    /// A skew handle at a side's midpoint (paths only).
    Skew(Side),
    /// The centre move handle: hover feedback only, never a press target
    /// of its own (a press on it is a body press).
    Move,
    /// A parameter handle of a primitive: the corner radius of a rectangle
    /// or a star's inner radius (`specs/unified-object-editing/`).
    Param(ParamHandle),
}

/// Which handle kinds an object's box shows: derived from the object's
/// kind and the live modifier state by the caller.
#[derive(Debug, Clone, Copy)]
pub struct HandleSpec {
    /// The resize handles (corners only for a polygon or star).
    pub resize_directions: &'static [ResizeDirection],
    /// Whether skew handles may be shown (paths only, criteria 37, 50).
    pub skew: bool,
    /// Whether the four side rotate handles are revealed (Shift held, no
    /// drag running, criterion 6).
    pub side_rotate: bool,
}

/// The screen-pixel constants of the transform handles
/// (`docs/design-system.md`, "Transform handle layout"), already converted
/// to document millimetres at the current zoom by the caller, the same
/// convention every other hit-test tolerance uses.
#[derive(Debug, Clone, Copy)]
pub struct TransformHandleTolerances {
    /// Bounds a hit against a resize handle (16 px; shrinks on a small box).
    pub resize: Tolerance,
    /// Bounds a hit against a rotate handle (16 px).
    pub rotate: Tolerance,
    /// Bounds a hit against a skew handle (12 px).
    pub skew: Tolerance,
    /// The centre handle's hover radius cap (12 px, further capped at a
    /// quarter of the box's shorter side).
    pub center_hover: Tolerance,
    /// A rotate handle's distance from its corner (along the diagonal) or
    /// its side midpoint (along the normal): 32 px.
    pub rotate_offset_mm: f64,
    /// A skew handle's distance from its side midpoint: 16 px.
    pub skew_offset_mm: f64,
    /// A box's shorter side below which only corner resize handles are
    /// drawn (24 px).
    pub edge_handle_min_side_mm: f64,
    /// The box dimension across a skew axis below which that axis's skew
    /// handles are hidden (24 px, criterion 37).
    pub skew_min_side_mm: f64,
    /// A box's shorter side below which the centre handle is not drawn
    /// (48 px, criterion 4).
    pub center_min_side_mm: f64,
    /// The dead zone radius of every drag on the selected object (3 px).
    pub drag_threshold_mm: f64,
    /// Bounds a hit against a parameter handle (12 px, criterion 5).
    pub param_hit: Tolerance,
    /// A radius handle's distance from its corner at radius 0 (15 px).
    pub param_inset_mm: f64,
    /// The centre distance of two radius handles at the largest radius
    /// (14 px: the knob diameter plus 4).
    pub param_pitch_mm: f64,
    /// A box's shorter side from which the parameter handles are drawn
    /// (72 px, criterion 7).
    pub param_min_side_mm: f64,
    /// The centre handle is not drawn while a parameter handle is within
    /// this distance of the box centre (20 px, criterion 8).
    pub param_centre_yield_mm: f64,
}

/// The dead zone radius of every Select-tool drag on the selected object, and
/// of the Pen tool's click-versus-drag test, in screen pixels.
pub const DRAG_THRESHOLD_PX: f64 = 3.0;

impl TransformHandleTolerances {
    /// The design-system pixel constants at `px_per_mm` screen pixels per
    /// document millimetre.
    #[must_use]
    pub fn at_scale(px_per_mm: f64) -> Self {
        let mm = |px: f64| px / px_per_mm;
        Self {
            resize: Tolerance::from_mm(mm(16.0)),
            rotate: Tolerance::from_mm(mm(16.0)),
            skew: Tolerance::from_mm(mm(12.0)),
            center_hover: Tolerance::from_mm(mm(12.0)),
            rotate_offset_mm: mm(32.0),
            skew_offset_mm: mm(16.0),
            edge_handle_min_side_mm: mm(24.0),
            skew_min_side_mm: mm(24.0),
            center_min_side_mm: mm(48.0),
            drag_threshold_mm: mm(DRAG_THRESHOLD_PX),
            param_hit: Tolerance::from_mm(mm(PARAM_HIT_PX)),
            param_inset_mm: mm(KNOB_INSET_PX),
            param_pitch_mm: mm(KNOB_PITCH_PX),
            param_min_side_mm: mm(PARAM_MIN_SIDE_PX),
            param_centre_yield_mm: mm(CENTRE_YIELD_PX),
        }
    }
}

/// How far under a pixel threshold a size may fall and still count as
/// reaching it: a box of exactly 48 px must keep its centre handle although
/// `px / scale * scale` rounds a hair under 48 (about 0.05 px at the 48 px
/// threshold).
const THRESHOLD_SLACK: f64 = 1e-3;

/// Whether `value` reaches `threshold` (both millimetres), within
/// [`THRESHOLD_SLACK`].
pub(crate) fn at_least(value: f64, threshold: f64) -> bool {
    value >= threshold * (1.0 - THRESHOLD_SLACK)
}

/// A box dimension at or below this (millimetres) is "zero" for the skew
/// handles: a skew has no distance to scale by (criterion 37).
const ZERO_EXTENT_MM: f64 = 1e-6;

/// Whether `direction` is one of the four diagonal corners (free or
/// Ctrl-proportional resize) as opposed to an edge midpoint
/// (single-axis only).
#[must_use]
pub const fn is_corner(direction: ResizeDirection) -> bool {
    matches!(
        direction,
        ResizeDirection::Ne | ResizeDirection::Se | ResizeDirection::Sw | ResizeDirection::Nw
    )
}

/// `direction`'s handle position in `box_`'s own local frame.
#[must_use]
pub fn resize_handle_local_position(box_: &OrientedBox, direction: ResizeDirection) -> Point {
    let (x0, y0, x1, y1) = (box_.min.x, box_.min.y, box_.max.x, box_.max.y);
    let (mx, my) = (f64::midpoint(x0, x1), f64::midpoint(y0, y1));
    match direction {
        ResizeDirection::N => Point::new(mx, y0),
        ResizeDirection::Ne => Point::new(x1, y0),
        ResizeDirection::E => Point::new(x1, my),
        ResizeDirection::Se => Point::new(x1, y1),
        ResizeDirection::S => Point::new(mx, y1),
        ResizeDirection::Sw => Point::new(x0, y1),
        ResizeDirection::W => Point::new(x0, my),
        ResizeDirection::Nw => Point::new(x0, y0),
    }
}

/// The rotate handle at `direction`'s local position (criteria 5, 6, 8): a
/// corner handle sits `offset_mm` away along the outward diagonal (so
/// `offset_mm / √2` on each local axis), a side handle `offset_mm` along the
/// outward normal of the side's midpoint.
#[must_use]
pub fn rotate_handle_local_position(
    box_: &OrientedBox,
    direction: ResizeDirection,
    offset_mm: f64,
) -> Point {
    let anchor = resize_handle_local_position(box_, direction);
    let outward = direction.unit_vector();
    let outward = if is_corner(direction) {
        outward.scaled(offset_mm / std::f64::consts::SQRT_2)
    } else {
        outward.scaled(offset_mm)
    };
    Point::new(anchor.x + outward.x, anchor.y + outward.y)
}

/// The skew handle on `side`: `offset_mm` outward of the side's midpoint.
#[must_use]
pub fn skew_handle_local_position(box_: &OrientedBox, side: Side, offset_mm: f64) -> Point {
    let anchor = resize_handle_local_position(box_, side.direction());
    let outward = side.direction().unit_vector().scaled(offset_mm);
    Point::new(anchor.x + outward.x, anchor.y + outward.y)
}

/// Whether the box has room for `side`'s skew handle: its lever (the box
/// dimension across the skew axis) is not zero and at least
/// `min_side_mm` (criterion 37).
fn skew_side_visible(box_: &OrientedBox, side: Side, min_side_mm: f64) -> bool {
    let lever = if side.skews_along_u() {
        box_.height()
    } else {
        box_.width()
    };
    lever > ZERO_EXTENT_MM && at_least(lever, min_side_mm)
}

/// Every handle the box currently has, in document space, paired with its
/// own kind (criteria 1, 5-8, 37, 4): the resize handles, the four corner
/// rotate handles, the four side rotate handles when revealed, the skew
/// handles per axis, and the centre handle on a box whose shorter side is
/// at least `center_min_side_mm`. Edge resize handles on a very small box
/// are in the set (they stay hit-testable, slice 5) but not drawn, see
/// [`is_drawn_handle`].
#[must_use]
pub fn transform_handles(
    box_: &OrientedBox,
    spec: HandleSpec,
    tolerances: &TransformHandleTolerances,
) -> Vec<(EditHandle, Point)> {
    let at = |local: Point| box_.to_document(local);
    let mut handles: Vec<(EditHandle, Point)> = spec
        .resize_directions
        .iter()
        .map(|&direction| {
            (
                EditHandle::Resize(direction),
                at(resize_handle_local_position(box_, direction)),
            )
        })
        .collect();
    for direction in ALL_EIGHT {
        if is_corner(direction) || spec.side_rotate {
            handles.push((
                EditHandle::Rotate(direction),
                at(rotate_handle_local_position(
                    box_,
                    direction,
                    tolerances.rotate_offset_mm,
                )),
            ));
        }
    }
    if spec.skew {
        for side in Side::ALL {
            if skew_side_visible(box_, side, tolerances.skew_min_side_mm) {
                handles.push((
                    EditHandle::Skew(side),
                    at(skew_handle_local_position(
                        box_,
                        side,
                        tolerances.skew_offset_mm,
                    )),
                ));
            }
        }
    }
    if at_least(
        box_.width().min(box_.height()),
        tolerances.center_min_side_mm,
    ) {
        handles.push((EditHandle::Move, at(box_.local_center())));
    }
    handles
}

/// Whether `handle` is drawn: every handle in [`transform_handles`] is,
/// except an edge resize handle on a box whose shorter side is under
/// `edge_handle_min_side_mm` (the glyphs would merge; slice 5's rule).
#[must_use]
pub fn is_drawn_handle(
    handle: EditHandle,
    box_: &OrientedBox,
    tolerances: &TransformHandleTolerances,
) -> bool {
    match handle {
        EditHandle::Resize(direction) => {
            is_corner(direction)
                || at_least(
                    box_.width().min(box_.height()),
                    tolerances.edge_handle_min_side_mm,
                )
        }
        // A parameter handle is only ever in the set when it is drawn
        // (`param_handles`): below 72 px there is none.
        EditHandle::Rotate(_) | EditHandle::Skew(_) | EditHandle::Move | EditHandle::Param(_) => {
            true
        }
    }
}

/// How far inside the box's edge, as a fraction of the resize hit radius
/// (6 of 16 px), a resize handle still wins a press over the body.
const INNER_HIT_BAND: f64 = 6.0 / 16.0;

/// Two handle centres this much (millimetres) apart in distance from the
/// pointer are equidistant: the tie order decides.
const TIE_EPSILON_MM: f64 = 1e-9;

/// The shrunken resize hit radius on a small box: a third of the box's
/// smaller side, never below a quarter of the full radius (slice 5).
fn resize_radius(box_: &OrientedBox, tolerances: &TransformHandleTolerances) -> f64 {
    let full = tolerances.resize.as_mm();
    (box_.width().min(box_.height()) / 3.0).clamp(full / 4.0, full)
}

/// Whether `point` is strictly inside the box, and how deep.
fn depth_inside(box_: &OrientedBox, point: Point) -> Option<f64> {
    let local = box_.to_local(point);
    let depth = (local.x - box_.min.x)
        .min(box_.max.x - local.x)
        .min(local.y - box_.min.y)
        .min(box_.max.y - local.y);
    (depth > 0.0).then_some(depth)
}

/// The one hit rule for press, hover, cursor and hint (criterion 9): every
/// handle whose centre is within its own radius of `point` is a candidate;
/// the nearest centre wins, and on an exact tie a parameter handle beats
/// resize beats skew beats rotate. The centre handle is not a candidate; with `include_move` it is
/// the fallback within its own hover radius (`min(12 px, s/4)`), for hover
/// feedback only.
///
/// A resize handle keeps slice 5's rules so a small object's body stays
/// reachable: its radius shrinks to a third of the box's smaller side
/// (never below a quarter of the full radius), and a point *inside* the box
/// only grabs one within the inner hit band (6 of 16 px of the resize radius) from the edge.
/// Outside the box a handle always wins. A parameter handle has its own
/// radius (12 px) and no inner band: it is the one handle family that sits
/// inside the box, and `handles` only holds the handles that are drawn.
#[must_use]
pub fn hit_transform_handle(
    handles: &[(EditHandle, Point)],
    box_: &OrientedBox,
    point: Point,
    tolerances: &TransformHandleTolerances,
    include_move: bool,
) -> Option<EditHandle> {
    let resize_radius = resize_radius(box_, tolerances);
    let inside_depth = depth_inside(box_, point);
    // (distance, rank, handle): rank 0 parameter, 1 resize, 2 skew, 3 rotate.
    let mut best: Option<(f64, u8, EditHandle)> = None;
    for &(handle, position) in handles {
        let (radius, rank) = match handle {
            EditHandle::Param(_) => (tolerances.param_hit.as_mm(), 0),
            EditHandle::Resize(_) => (resize_radius, 1),
            EditHandle::Skew(_) => (tolerances.skew.as_mm(), 2),
            EditHandle::Rotate(_) => (tolerances.rotate.as_mm(), 3),
            EditHandle::Move => continue,
        };
        let distance = position.vector_to(point).length();
        if distance > radius {
            continue;
        }
        if matches!(handle, EditHandle::Resize(_))
            && inside_depth.is_some_and(|depth| depth > resize_radius * INNER_HIT_BAND)
        {
            continue;
        }
        let better = best.is_none_or(|(best_distance, best_rank, _)| {
            distance < best_distance - TIE_EPSILON_MM
                || ((distance - best_distance).abs() <= TIE_EPSILON_MM && rank < best_rank)
        });
        if better {
            best = Some((distance, rank, handle));
        }
    }
    if let Some((_, _, handle)) = best {
        return Some(handle);
    }
    if !include_move {
        return None;
    }
    let (_, center) = handles
        .iter()
        .find(|(handle, _)| *handle == EditHandle::Move)?;
    let radius = tolerances
        .center_hover
        .as_mm()
        .min(box_.width().min(box_.height()) / 4.0);
    (center.vector_to(point).length() <= radius).then_some(EditHandle::Move)
}

/// The on-screen direction, in degrees clockwise from the horizontal, a
/// resize handle's double-headed-arrow cursor must point
/// (`specs/0005-object-transform/specification.md`'s UX notes, "Cursor
/// feedback": "object rotation + handle's own base angle"): `0` for the
/// E/W handles, `90` for N/S, `45` for the Se/Nw diagonal, `-45` for
/// Ne/Sw — all in Y-down screen space — plus `rotation`. Reduced to
/// `[0, 180)` since a double-headed arrow looks the same turned half a
/// circle.
#[must_use]
pub fn resize_cursor_angle_degrees(direction: ResizeDirection, rotation: Angle) -> f64 {
    let base = match direction {
        ResizeDirection::E | ResizeDirection::W => 0.0,
        ResizeDirection::Se | ResizeDirection::Nw => 45.0,
        ResizeDirection::S | ResizeDirection::N => 90.0,
        ResizeDirection::Sw | ResizeDirection::Ne => 135.0,
    };
    (base + rotation.as_radians().to_degrees()).rem_euclid(180.0)
}

/// The on-screen angle (degrees clockwise from horizontal, `[0, 180)`) of a
/// skew handle's cursor: the box rotation for the top and bottom handles
/// (arrows along `u`), plus 90° for the left and right ones (along `v`).
#[must_use]
pub fn skew_cursor_angle_degrees(side: Side, rotation: Angle) -> f64 {
    let base = if side.skews_along_u() { 0.0 } else { 90.0 };
    (base + rotation.as_radians().to_degrees()).rem_euclid(180.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unrotated_box(min: Point, max: Point) -> OrientedBox {
        OrientedBox {
            min,
            max,
            angle: Angle::from_radians(0.0),
            pivot: Point::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y)),
        }
    }

    /// One pixel is one millimetre in these tests.
    fn tol() -> TransformHandleTolerances {
        TransformHandleTolerances::at_scale(1.0)
    }

    fn big_box() -> OrientedBox {
        unrotated_box(Point::new(0.0, 0.0), Point::new(100.0, 100.0))
    }

    const RECT: HandleSpec = HandleSpec {
        resize_directions: &ALL_EIGHT,
        skew: false,
        side_rotate: false,
    };

    fn count(handles: &[(EditHandle, Point)], f: impl Fn(&EditHandle) -> bool) -> usize {
        handles.iter().filter(|(h, _)| f(h)).count()
    }

    fn position_of(handles: &[(EditHandle, Point)], wanted: EditHandle) -> Point {
        handles
            .iter()
            .find(|(h, _)| *h == wanted)
            .map(|(_, p)| *p)
            .expect("handle exists")
    }

    fn assert_near(actual: Point, x: f64, y: f64) {
        assert!(
            (actual.x - x).abs() < 1e-9 && (actual.y - y).abs() < 1e-9,
            "expected ({x}, {y}), got {actual:?}"
        );
    }

    /// Criteria 1, 5: eight resize, four corner rotate and the centre move
    /// handle on a rectangle; no side rotate, no skew.
    #[test]
    fn a_rectangle_shows_resize_corner_rotate_and_centre_handles() {
        let handles = transform_handles(&big_box(), RECT, &tol());
        assert_eq!(handles.len(), 13);
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Resize(_))), 8);
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Rotate(_))), 4);
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Skew(_))), 0);
        assert_eq!(count(&handles, |h| *h == EditHandle::Move), 1);
    }

    /// Criteria 6, 7: Shift reveals four side rotate handles; a polygon
    /// (corner resize only) has all eight rotate positions.
    #[test]
    fn shift_reveals_four_side_rotate_handles_for_every_kind() {
        let polygon = HandleSpec {
            resize_directions: &CORNERS_FOUR,
            skew: false,
            side_rotate: true,
        };
        let handles = transform_handles(&big_box(), polygon, &tol());
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Resize(_))), 4);
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Rotate(_))), 8);
        let without = HandleSpec {
            side_rotate: false,
            ..polygon
        };
        let handles = transform_handles(&big_box(), without, &tol());
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Rotate(_))), 4);
    }

    /// Criterion 5: a corner rotate handle sits 32 px out on the diagonal
    /// (22.6 on each local axis); criterion 6: a side one 32 px on the
    /// normal.
    #[test]
    fn rotate_handles_sit_at_the_rotate_offset_outside_corners_and_sides() {
        let spec = HandleSpec {
            side_rotate: true,
            ..RECT
        };
        let handles = transform_handles(&big_box(), spec, &tol());
        let d = 32.0 / std::f64::consts::SQRT_2;
        assert_near(
            position_of(&handles, EditHandle::Rotate(ResizeDirection::Ne)),
            100.0 + d,
            -d,
        );
        assert_near(
            position_of(&handles, EditHandle::Rotate(ResizeDirection::Sw)),
            -d,
            100.0 + d,
        );
        assert_near(
            position_of(&handles, EditHandle::Rotate(ResizeDirection::N)),
            50.0,
            -32.0,
        );
        assert_near(
            position_of(&handles, EditHandle::Rotate(ResizeDirection::E)),
            132.0,
            50.0,
        );
    }

    /// Criterion 8: every handle follows the oriented box, not the screen
    /// axes: a box turned a quarter turn about its centre puts the top
    /// rotate handle at the screen's right.
    #[test]
    fn handles_follow_a_rotated_box() {
        let rotated = OrientedBox {
            angle: Angle::from_radians(std::f64::consts::FRAC_PI_2),
            ..big_box()
        };
        let spec = HandleSpec {
            side_rotate: true,
            ..RECT
        };
        let handles = transform_handles(&rotated, spec, &tol());
        // Local top (50, -32) turned a quarter turn about (50, 50): the
        // offset (0, -82) becomes (82, 0).
        assert_near(
            position_of(&handles, EditHandle::Rotate(ResizeDirection::N)),
            132.0,
            50.0,
        );
    }

    /// Criterion 37: skew handles 16 px outward of each side midpoint, on
    /// a path.
    #[test]
    fn a_path_shows_four_skew_handles_between_resize_and_side_rotate() {
        let spec = HandleSpec {
            skew: true,
            side_rotate: true,
            ..RECT
        };
        let handles = transform_handles(&big_box(), spec, &tol());
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Skew(_))), 4);
        assert_near(
            position_of(&handles, EditHandle::Skew(Side::Top)),
            50.0,
            -16.0,
        );
        assert_near(
            position_of(&handles, EditHandle::Skew(Side::Right)),
            116.0,
            50.0,
        );
        assert_near(
            position_of(&handles, EditHandle::Skew(Side::Bottom)),
            50.0,
            116.0,
        );
        assert_near(
            position_of(&handles, EditHandle::Skew(Side::Left)),
            -16.0,
            50.0,
        );
    }

    /// Criterion 37: top and bottom skew need a box at least 24 px high,
    /// left and right at least 24 px wide, and a non-zero extent.
    #[test]
    fn skew_handles_are_hidden_per_axis_on_a_small_or_zero_extent() {
        let spec = HandleSpec { skew: true, ..RECT };
        let flat = unrotated_box(Point::new(0.0, 0.0), Point::new(100.0, 20.0));
        let handles = transform_handles(&flat, spec, &tol());
        assert!(
            handles
                .iter()
                .all(|(h, _)| !matches!(h, EditHandle::Skew(Side::Top | Side::Bottom)))
        );
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Skew(_))), 2);

        let line = unrotated_box(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        let tiny_tiers = TransformHandleTolerances {
            skew_min_side_mm: 0.0,
            ..tol()
        };
        let handles = transform_handles(&line, spec, &tiny_tiers);
        assert_eq!(
            count(&handles, |h| matches!(
                h,
                EditHandle::Skew(Side::Top | Side::Bottom)
            )),
            0,
            "zero height removes the x-skew handles whatever the pixel tier"
        );
        assert_eq!(count(&handles, |h| matches!(h, EditHandle::Skew(_))), 2);
    }

    /// Criterion 4: the centre handle needs a shorter side of 48 px.
    #[test]
    fn the_centre_handle_needs_a_forty_eight_pixel_box() {
        let small = unrotated_box(Point::new(0.0, 0.0), Point::new(47.0, 200.0));
        let large = unrotated_box(Point::new(0.0, 0.0), Point::new(48.0, 200.0));
        assert_eq!(
            count(&transform_handles(&small, RECT, &tol()), |h| *h
                == EditHandle::Move),
            0
        );
        assert_eq!(
            count(&transform_handles(&large, RECT, &tol()), |h| *h
                == EditHandle::Move),
            1
        );
    }

    /// Slice 5: below a 24 px shorter side only corner resize handles are
    /// drawn, but every handle is still in the hit set.
    #[test]
    fn small_boxes_draw_only_corner_resize_handles() {
        let small = unrotated_box(Point::new(0.0, 0.0), Point::new(20.0, 20.0));
        let handles = transform_handles(&small, RECT, &tol());
        let drawn = handles
            .iter()
            .filter(|(h, _)| is_drawn_handle(*h, &small, &tol()))
            .count();
        assert_eq!(handles.len() - drawn, 4, "four edge handles not drawn");
        assert!(is_drawn_handle(
            EditHandle::Rotate(ResizeDirection::Ne),
            &small,
            &tol()
        ));
    }

    fn hit(
        handles: &[(EditHandle, Point)],
        box_: &OrientedBox,
        x: f64,
        y: f64,
    ) -> Option<EditHandle> {
        hit_transform_handle(handles, box_, Point::new(x, y), &tol(), false)
    }

    /// Criterion 9: nearest centre wins; the boundary between the edge
    /// resize handle and the skew arrow is 8 px outward of the edge, the
    /// one between skew and side rotate is 24 px.
    #[test]
    fn nearest_centre_splits_the_resize_skew_and_rotate_overlap() {
        let spec = HandleSpec {
            skew: true,
            side_rotate: true,
            ..RECT
        };
        let box_ = big_box();
        let handles = transform_handles(&box_, spec, &tol());
        let n = EditHandle::Resize(ResizeDirection::N);
        let skew = EditHandle::Skew(Side::Top);
        let rotate = EditHandle::Rotate(ResizeDirection::N);
        assert_eq!(hit(&handles, &box_, 50.0, -7.0), Some(n));
        assert_eq!(hit(&handles, &box_, 50.0, -9.0), Some(skew));
        assert_eq!(hit(&handles, &box_, 50.0, -23.0), Some(skew));
        assert_eq!(hit(&handles, &box_, 50.0, -25.0), Some(rotate));
        assert_eq!(hit(&handles, &box_, 50.0, -47.0), Some(rotate));
        assert_eq!(hit(&handles, &box_, 50.0, -49.0), None);
    }

    /// Criterion 9: on an exact tie the order is resize, skew, rotate.
    #[test]
    fn an_exact_tie_goes_to_resize_then_skew_then_rotate() {
        let box_ = big_box();
        let pointer = Point::new(-5.0, 50.0);
        let rotate = (
            EditHandle::Rotate(ResizeDirection::N),
            Point::new(-5.0, 55.0),
        );
        let skew = (EditHandle::Skew(Side::Top), Point::new(-5.0, 45.0));
        let resize = (
            EditHandle::Resize(ResizeDirection::W),
            Point::new(-10.0, 50.0),
        );
        let winner = |handles: &[(EditHandle, Point)]| {
            hit_transform_handle(handles, &box_, pointer, &tol(), false)
        };
        assert_eq!(winner(&[rotate, skew, resize]), Some(resize.0));
        assert_eq!(winner(&[resize, skew, rotate]), Some(resize.0));
        assert_eq!(winner(&[rotate, skew]), Some(skew.0));
        assert_eq!(winner(&[rotate]), Some(rotate.0));
    }

    /// Slice 5's rules stay: inside the box only the 6 px edge band grabs a
    /// resize handle; the body is a move.
    #[test]
    fn inside_the_box_only_the_edge_band_grabs_a_resize_handle() {
        let box_ = big_box();
        let handles = transform_handles(&box_, RECT, &tol());
        assert_eq!(
            hit(&handles, &box_, 50.0, 5.0),
            Some(EditHandle::Resize(ResizeDirection::N))
        );
        assert_eq!(hit(&handles, &box_, 50.0, 7.0), None);
    }

    /// Criterion 9 and 3: a corner resize and its rotate handle never
    /// overlap, and the centre handle is hover-only.
    #[test]
    fn the_centre_handle_is_hover_only_and_never_a_press_target() {
        let box_ = big_box();
        let handles = transform_handles(&box_, RECT, &tol());
        assert_eq!(hit(&handles, &box_, 50.0, 50.0), None);
        assert_eq!(
            hit_transform_handle(&handles, &box_, Point::new(50.0, 50.0), &tol(), true),
            Some(EditHandle::Move)
        );
        assert_eq!(
            hit_transform_handle(&handles, &box_, Point::new(50.0, 63.0), &tol(), true),
            None,
            "outside the 12 px hover radius"
        );
    }

    /// UX notes: the skew cursor turns with the box; left and right are 90°
    /// off top and bottom.
    #[test]
    fn skew_cursor_angle_follows_the_box_rotation() {
        let zero = Angle::from_radians(0.0);
        assert!((skew_cursor_angle_degrees(Side::Top, zero) - 0.0).abs() < 1e-9);
        assert!((skew_cursor_angle_degrees(Side::Left, zero) - 90.0).abs() < 1e-9);
        let rotated = Angle::from_radians(30.0_f64.to_radians());
        assert!((skew_cursor_angle_degrees(Side::Bottom, rotated) - 30.0).abs() < 1e-9);
        assert!((skew_cursor_angle_degrees(Side::Right, rotated) - 120.0).abs() < 1e-9);
    }

    /// UX notes' rotated resize cursors: base angle plus the object's
    /// own rotation, never one of the four fixed browser cursors once
    /// rotated.
    #[test]
    fn resize_cursor_angle_adds_the_objects_rotation_to_the_handles_base_angle() {
        let zero = Angle::from_radians(0.0);
        assert!((resize_cursor_angle_degrees(ResizeDirection::E, zero) - 0.0).abs() < 1e-9);
        assert!((resize_cursor_angle_degrees(ResizeDirection::N, zero) - 90.0).abs() < 1e-9);
        assert!((resize_cursor_angle_degrees(ResizeDirection::Se, zero) - 45.0).abs() < 1e-9);
        assert!((resize_cursor_angle_degrees(ResizeDirection::Ne, zero) - 135.0).abs() < 1e-9);
        let rotated = Angle::from_radians(45.0_f64.to_radians());
        assert!((resize_cursor_angle_degrees(ResizeDirection::N, rotated) - 135.0).abs() < 1e-9);
        assert!(
            (resize_cursor_angle_degrees(ResizeDirection::N, rotated)
                - resize_cursor_angle_degrees(ResizeDirection::S, rotated))
            .abs()
                < 1e-9
        );
    }

    /// A rotated box's resize handle still reports the same *local*
    /// position; only its mapped document position changes.
    #[test]
    fn resize_handle_local_position_is_independent_of_the_boxs_own_rotation() {
        let unrotated = big_box();
        let rotated = OrientedBox {
            angle: Angle::from_radians(0.7),
            ..unrotated
        };
        assert_eq!(
            resize_handle_local_position(&unrotated, ResizeDirection::Ne),
            resize_handle_local_position(&rotated, ResizeDirection::Ne)
        );
    }
    /// Criterion 9 and the UX notes' clearance rule: every glyph keeps a
    /// clear gap of at least 4 px to every other glyph, at every box size
    /// (24 px is the smallest box that shows edge, skew and centre-free
    /// handles; the centre handle needs 48).
    #[test]
    fn every_glyph_keeps_four_pixels_clear_of_every_other_glyph() {
        let spec = HandleSpec {
            skew: true,
            side_rotate: true,
            ..RECT
        };
        let half_extent = |handle: EditHandle| match handle {
            EditHandle::Resize(_) => (4.0, 4.0),
            EditHandle::Rotate(_) => (6.0, 6.0),
            EditHandle::Skew(side) if side.skews_along_u() => (9.0, 6.0),
            EditHandle::Skew(_) => (6.0, 9.0),
            EditHandle::Move => (8.0, 8.0),
            EditHandle::Param(_) => (5.0, 5.0),
        };
        for (w, h) in [
            (24.0, 24.0),
            (30.0, 80.0),
            (48.0, 48.0),
            (100.0, 200.0),
            (400.0, 30.0),
        ] {
            let box_ = unrotated_box(Point::new(0.0, 0.0), Point::new(w, h));
            let handles = transform_handles(&box_, spec, &tol());
            for (i, (a, pa)) in handles.iter().enumerate() {
                for (b, pb) in &handles[i + 1..] {
                    let (ha, hb) = (half_extent(*a), half_extent(*b));
                    let dx = (pa.x - pb.x).abs() - (ha.0 + hb.0);
                    let dy = (pa.y - pb.y).abs() - (ha.1 + hb.1);
                    assert!(
                        dx.max(dy) >= 4.0 - 1e-9,
                        "{w}x{h}: {a:?} and {b:?} are {} px apart",
                        dx.max(dy)
                    );
                }
            }
        }
    }

    /// A box of exactly 48 px keeps its centre handle and one of exactly 24 px
    /// its edge and skew handles, whatever the zoom rounds to.
    #[test]
    fn the_pixel_thresholds_hold_at_exactly_48_and_24_px() {
        for scale in [3.779_527_559, 1.0, 0.37, 7.131, 25.0] {
            let tolerances = TransformHandleTolerances::at_scale(scale);
            let box_ =
                |px: f64| unrotated_box(Point::new(0.0, 0.0), Point::new(px / scale, px / scale));
            let spec = HandleSpec { skew: true, ..RECT };
            let handles = transform_handles(&box_(48.0), spec, &tolerances);
            assert!(
                handles.iter().any(|(h, _)| *h == EditHandle::Move),
                "{scale}"
            );
            let small = box_(24.0);
            let handles = transform_handles(&small, spec, &tolerances);
            assert_eq!(
                count(&handles, |h| matches!(h, EditHandle::Skew(_))),
                4,
                "{scale}"
            );
            assert!(is_drawn_handle(
                EditHandle::Resize(ResizeDirection::N),
                &small,
                &tolerances
            ));
        }
    }
}
