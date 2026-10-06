//! Which shape handles a selected primitive shows, where, and the
//! parameter arithmetic each one's drag performs
//! (`specs/0003-primitive-shapes/adrs.md`: "handle layout lives here, not in
//! `document-core`, because which handles exist is interaction design").
//! Pure functions of a [`PrimitiveSnapshot`] (and, for the arithmetic,
//! a drag delta) — no Loro, no rendering, no hit-testing policy; see
//! [`crate::shape_hit_test`] for the latter.

use vecmanf_document_core::{
    Angle, EllipseFrame, InnerRatio, Length, Point, PointCount, PrimitiveSnapshot, RectBounds,
    Shape, StarFrame, Vec2, effective_corner_radius, shape_center,
};

/// One of a rectangle/ellipse's eight bounding-box resize handles, or a
/// polygon/star's four outer-radius ones (cardinal only there — "the
/// outer bounding circle's N/E/S/W-most points",
/// `specs/0003-primitive-shapes/specification.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    /// North (top edge midpoint).
    N,
    /// North-east (top-right corner).
    Ne,
    /// East (right edge midpoint).
    E,
    /// South-east (bottom-right corner).
    Se,
    /// South (bottom edge midpoint).
    S,
    /// South-west (bottom-left corner).
    Sw,
    /// West (left edge midpoint).
    W,
    /// North-west (top-left corner).
    Nw,
}

impl ResizeDirection {
    /// Every direction a rectangle/ellipse bounding box shows.
    pub const ALL_EIGHT: [Self; 8] = [
        Self::N,
        Self::Ne,
        Self::E,
        Self::Se,
        Self::S,
        Self::Sw,
        Self::W,
        Self::Nw,
    ];
    /// The four cardinal directions a polygon/star's outer handles use.
    pub const CARDINAL_FOUR: [Self; 4] = [Self::N, Self::E, Self::S, Self::W];

    /// The unit vector from a shape's center toward this direction, in
    /// document (Y-down) space.
    #[must_use]
    pub const fn unit_vector(self) -> Vec2 {
        let (x, y) = match self {
            Self::N => (0.0, -1.0),
            Self::Ne => (1.0, -1.0),
            Self::E => (1.0, 0.0),
            Self::Se => (1.0, 1.0),
            Self::S => (0.0, 1.0),
            Self::Sw => (-1.0, 1.0),
            Self::W => (-1.0, 0.0),
            Self::Nw => (-1.0, -1.0),
        };
        Vec2::new(x, y)
    }
}

/// A shape handle's kind (`docs/design-system.md`'s "shape handle"
/// glyph: one vocabulary for resize, corner-radius and inner-radius).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    /// Scales/resizes the shape (acceptance criteria 3, 9, 13).
    Resize(ResizeDirection),
    /// The rectangle's one draggable corner-radius handle (acceptance
    /// criteria 4, 5, 6).
    CornerRadius,
    /// A rectangle's other three corners, once rounded — display-only,
    /// confirming "all four corners round together"
    /// (`specification.md`).
    CornerRadiusEcho,
    /// A star's inner-radius handle (acceptance criterion 14). Never
    /// shown on a plain polygon.
    InnerRadius,
}

/// One shape handle: where it is, what it does, and whether it can be
/// dragged at all (the three `CornerRadiusEcho` glyphs are display-only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeHandle {
    /// What this handle does.
    pub kind: HandleKind,
    /// Where it is, in document space.
    pub position: Point,
    /// Whether the maker can drag it — `false` for a `CornerRadiusEcho`.
    pub draggable: bool,
}

fn rect_bbox(bounds: RectBounds) -> (f64, f64, f64, f64) {
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.origin.x + bounds.width.as_mm(),
        bounds.origin.y + bounds.height.as_mm(),
    )
}

fn bbox_handle_position(direction: ResizeDirection, x0: f64, y0: f64, x1: f64, y1: f64) -> Point {
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

/// The diagonal-toward-center unit vector for each rectangle corner —
/// used to place the corner-radius handle(s) inset from their own
/// corner (`specification.md`'s "Placement": "one shape handle inset
/// along the diagonal from the rectangle's top-right corner").
fn corner_inward_diagonal(direction: ResizeDirection) -> Vec2 {
    direction.unit_vector().negated().normalized_to(1.0)
}

/// The handles a selected rectangle shows: a draggable corner-radius
/// handle at the top-right corner (present even at zero radius) and,
/// once the effective radius is positive, three matching display-only
/// echoes at the other corners, plus 8 resize handles.
///
/// The corner-radius handle is listed *before* the resize handles
/// deliberately: at exactly zero radius it sits at the same point as
/// the NE resize handle, and [`crate::hit_test_handle`]'s nearest-within-
/// tolerance search keeps the first handle it finds on an exact
/// distance tie — so a click on that shared corner always reads as
/// "discover rounding" (`specification.md`'s own framing for why the
/// handle exists at zero radius at all), not an ordinary resize. Once
/// the radius is positive the two handles separate and the tie no
/// longer applies.
#[must_use]
pub fn rect_handles(bounds: RectBounds, corner_radius: Length) -> Vec<ShapeHandle> {
    let (x0, y0, x1, y1) = rect_bbox(bounds);
    let effective = effective_corner_radius(bounds, corner_radius).as_mm();
    let corners = [
        (ResizeDirection::Ne, HandleKind::CornerRadius, true),
        (ResizeDirection::Nw, HandleKind::CornerRadiusEcho, false),
        (ResizeDirection::Sw, HandleKind::CornerRadiusEcho, false),
        (ResizeDirection::Se, HandleKind::CornerRadiusEcho, false),
    ];
    let mut handles = Vec::with_capacity(12);
    for (direction, kind, draggable) in corners {
        if kind == HandleKind::CornerRadiusEcho && effective <= 0.0 {
            continue;
        }
        let corner = bbox_handle_position(direction, x0, y0, x1, y1);
        let position = corner.translated(corner_inward_diagonal(direction).scaled(effective));
        handles.push(ShapeHandle {
            kind,
            position,
            draggable,
        });
    }
    handles.extend(
        ResizeDirection::ALL_EIGHT
            .into_iter()
            .map(|direction| ShapeHandle {
                kind: HandleKind::Resize(direction),
                position: bbox_handle_position(direction, x0, y0, x1, y1),
                draggable: true,
            }),
    );
    handles
}

/// The handles a selected ellipse shows: 8 resize handles on its
/// bounding box (acceptance criterion 9).
#[must_use]
pub fn ellipse_handles(frame: EllipseFrame) -> Vec<ShapeHandle> {
    let bounds = RectBounds {
        origin: Point::new(
            frame.center.x - frame.rx.as_mm(),
            frame.center.y - frame.ry.as_mm(),
        ),
        width: Length::from_mm(frame.rx.as_mm() * 2.0),
        height: Length::from_mm(frame.ry.as_mm() * 2.0),
    };
    let (x0, y0, x1, y1) = rect_bbox(bounds);
    ResizeDirection::ALL_EIGHT
        .into_iter()
        .map(|direction| ShapeHandle {
            kind: HandleKind::Resize(direction),
            position: bbox_handle_position(direction, x0, y0, x1, y1),
            draggable: true,
        })
        .collect()
}

/// The handles a selected polygon or star shows: 4 cardinal resize
/// handles on the outer bounding circle (acceptance criterion 13), plus
/// — star only — one inner-radius handle at the first inner vertex
/// (acceptance criterion 14).
#[must_use]
pub fn polygon_or_star_handles(
    frame: StarFrame,
    point_count: PointCount,
    inner_ratio: Option<InnerRatio>,
) -> Vec<ShapeHandle> {
    let mut handles: Vec<ShapeHandle> = ResizeDirection::CARDINAL_FOUR
        .into_iter()
        .map(|direction| ShapeHandle {
            kind: HandleKind::Resize(direction),
            position: frame
                .center
                .translated(direction.unit_vector().scaled(frame.radius.as_mm())),
            draggable: true,
        })
        .collect();
    if let Some(ratio) = inner_ratio {
        handles.push(ShapeHandle {
            kind: HandleKind::InnerRadius,
            position: inner_radius_handle_position(frame, point_count, ratio),
            draggable: true,
        });
    }
    handles
}

/// The star's first inner vertex position — "clockwise from the topmost
/// outer vertex" is exactly [`vecmanf_document_core::star_outline`]'s
/// own first inner vertex, at `θ + π/N` (the midpoint between the first
/// and second outer vertices).
#[must_use]
pub fn inner_radius_handle_position(
    frame: StarFrame,
    point_count: PointCount,
    inner_ratio: InnerRatio,
) -> Point {
    let step = std::f64::consts::TAU / f64::from(point_count.get());
    let theta = frame.angle.as_radians() + step / 2.0;
    let radius = frame.radius.as_mm() * inner_ratio.get();
    frame
        .center
        .translated(Vec2::new(radius * theta.cos(), radius * theta.sin()))
}

/// Every handle a selected primitive currently shows, at the positions
/// its own `rotation` implies (`specs/0005-object-transform/
/// specification.md` acceptance criterion 25: "handles follow the
/// object's local frame"): each shape's handles are laid out in its own
/// unrotated local frame by the functions above, then turned about the
/// shape's frame center — the same pivot [`vecmanf_document_core::
/// outline_of_rotated`] turns the outline about, so a handle never
/// disagrees with the corner it belongs to.
#[must_use]
pub fn handles_for(snapshot: &PrimitiveSnapshot) -> Vec<ShapeHandle> {
    let local = match snapshot.shape {
        Shape::Rect {
            bounds,
            corner_radius,
        } => rect_handles(bounds, corner_radius),
        Shape::Ellipse { frame } => ellipse_handles(frame),
        Shape::Polygon { frame, point_count } => polygon_or_star_handles(frame, point_count, None),
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => polygon_or_star_handles(frame, point_count, Some(inner_ratio)),
    };
    let center = shape_center(&snapshot.shape);
    local
        .into_iter()
        .map(|handle| ShapeHandle {
            position: handle.position.rotated_around(center, snapshot.rotation),
            ..handle
        })
        .collect()
}

/// A pointer drag's displacement from `from` to `to`, expressed along a
/// primitive's own local axes (turned by `-rotation`) — what every
/// shape-tool handle drag's arithmetic expects, since that arithmetic
/// works on the primitive's unrotated stored frame (acceptance criterion
/// 25). Identical to a plain `from.vector_to(to)` at zero rotation.
#[must_use]
pub(crate) fn local_delta(rotation: Angle, from: Point, to: Point) -> Vec2 {
    from.vector_to(to)
        .rotated(Angle::from_radians(-rotation.as_radians()))
}

/// Resizes a rectangle's bounding box by dragging the handle at
/// `direction` by `delta` (acceptance criterion 3): the opposite
/// corner/edge stays fixed, the dragged one moves by `delta`'s relevant
/// component(s) — a cardinal (N/E/S/W) handle only ever changes one
/// dimension.
#[must_use]
pub fn resize_rect_bounds(
    bounds: RectBounds,
    direction: ResizeDirection,
    delta: Vec2,
) -> RectBounds {
    let (mut x0, mut y0, mut x1, mut y1) = rect_bbox(bounds);
    match direction {
        ResizeDirection::N => y0 += delta.y,
        ResizeDirection::Ne => {
            x1 += delta.x;
            y0 += delta.y;
        }
        ResizeDirection::E => x1 += delta.x,
        ResizeDirection::Se => {
            x1 += delta.x;
            y1 += delta.y;
        }
        ResizeDirection::S => y1 += delta.y,
        ResizeDirection::Sw => {
            x0 += delta.x;
            y1 += delta.y;
        }
        ResizeDirection::W => x0 += delta.x,
        ResizeDirection::Nw => {
            x0 += delta.x;
            y0 += delta.y;
        }
    }
    RectBounds::from_corners(Point::new(x0, y0), Point::new(x1, y1))
}

/// Resizes an ellipse's frame the same way (acceptance criterion 9),
/// via its own bounding box.
#[must_use]
pub fn resize_ellipse_frame(
    frame: EllipseFrame,
    direction: ResizeDirection,
    delta: Vec2,
) -> EllipseFrame {
    let bounds = RectBounds {
        origin: Point::new(
            frame.center.x - frame.rx.as_mm(),
            frame.center.y - frame.ry.as_mm(),
        ),
        width: Length::from_mm(frame.rx.as_mm() * 2.0),
        height: Length::from_mm(frame.ry.as_mm() * 2.0),
    };
    let resized = resize_rect_bounds(bounds, direction, delta);
    let (x0, y0, x1, y1) = rect_bbox(resized);
    EllipseFrame::from_corners(Point::new(x0, y0), Point::new(x1, y1))
}

/// Uniformly scales a polygon/star frame by dragging one of its four
/// cardinal resize handles (acceptance criterion 13): the outer radius
/// changes by `delta`'s projection onto that handle's own direction;
/// point count and orientation are untouched, and (since a star stores
/// `inner_ratio`, not an inner length) the ratio stays unchanged too,
/// automatically.
#[must_use]
pub fn scale_star_frame(frame: StarFrame, direction: ResizeDirection, delta: Vec2) -> StarFrame {
    let unit = direction.unit_vector();
    let projected = delta.x * unit.x + delta.y * unit.y;
    StarFrame {
        radius: Length::from_mm((frame.radius.as_mm() + projected).max(0.0)),
        ..frame
    }
}

/// The corner-radius handle's drag arithmetic (acceptance criteria 4,
/// 5, 6): the new radius is `start_radius` plus the press→release
/// displacement projected onto the inward diagonal — the same "value at
/// press time, moved by the displacement" rule
/// [`crate::NodeTool`]'s own handle drag uses, not the
/// absolute pointer position (a press within hit tolerance but off the
/// handle's exact tip must not relocate it). Clamped to `[0, half the
/// shorter side]` at the moment of the drag (`adrs.md`: "the radius-
/// handle drag writes a value already clamped against the bounds at
/// that moment"). Dragging far enough back toward the corner gives
/// exactly `0.0` (acceptance criterion 6).
#[must_use]
pub fn corner_radius_from_drag(start_radius: Length, bounds: RectBounds, delta: Vec2) -> Length {
    let diagonal = corner_inward_diagonal(ResizeDirection::Ne);
    let projected = delta.x * diagonal.x + delta.y * diagonal.y;
    let half_shorter_side = bounds.width.as_mm().min(bounds.height.as_mm()) / 2.0;
    Length::from_mm((start_radius.as_mm() + projected).clamp(0.0, half_shorter_side.max(0.0)))
}

/// The star inner-radius handle's drag arithmetic (acceptance criterion
/// 14): the new ratio is the handle's new distance from the center
/// (projected along its own direction), divided by the fixed outer
/// radius, clamped to the tool-options bar's own `0.01..=0.99` bounds
/// (`specification.md`'s "Polygon/star point-count and ratio" UX note)
/// so it always lands inside [`InnerRatio`]'s open interval.
#[must_use]
pub fn inner_ratio_from_drag(
    frame: StarFrame,
    point_count: PointCount,
    start_ratio: InnerRatio,
    delta: Vec2,
) -> InnerRatio {
    let step = std::f64::consts::TAU / f64::from(point_count.get());
    let theta = frame.angle.as_radians() + step / 2.0;
    let direction = Vec2::new(theta.cos(), theta.sin());
    let projected = delta.x * direction.x + delta.y * direction.y;
    let start_distance = frame.radius.as_mm() * start_ratio.get();
    let new_distance = start_distance + projected;
    let outer = frame.radius.as_mm().max(f64::EPSILON);
    let ratio = (new_distance / outer).clamp(0.01, 0.99);
    // invariant: `clamp(0.01, 0.99)` always lands inside `(0, 1)`.
    #[allow(clippy::unwrap_used)]
    InnerRatio::new(ratio).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::Angle;

    fn bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
        RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        }
    }

    #[test]
    fn rect_handles_include_corner_radius_handle_at_zero_radius() {
        let handles = rect_handles(bounds(0.0, 0.0, 10.0, 10.0), Length::from_mm(0.0));
        let corner_radius = handles
            .iter()
            .find(|h| h.kind == HandleKind::CornerRadius)
            .expect("present even at zero radius");
        assert_eq!(corner_radius.position, Point::new(10.0, 0.0));
        assert!(
            !handles
                .iter()
                .any(|h| h.kind == HandleKind::CornerRadiusEcho),
            "no echoes at zero radius"
        );
    }

    #[test]
    fn rect_handles_show_echoes_once_rounded() {
        let handles = rect_handles(bounds(0.0, 0.0, 10.0, 10.0), Length::from_mm(2.0));
        let echoes = handles
            .iter()
            .filter(|h| h.kind == HandleKind::CornerRadiusEcho)
            .count();
        assert_eq!(echoes, 3);
    }

    /// `object-transform` acceptance criterion 25: the corner-radius
    /// handle of a rectangle rotated 30° sits inset along the *rotated*
    /// corner — the unrotated position turned 30° about the center — and
    /// a drag of it is measured along the rectangle's own local axes.
    #[test]
    fn handles_for_a_rotated_rect_follow_its_rotation() {
        use vecmanf_document_core::Document;
        let document = Document::new(1);
        let id = document.create_rect(bounds(0.0, 0.0, 10.0, 10.0));
        document
            .set_corner_radius(&[id], Length::from_mm(2.0))
            .expect("radius");
        let angle = Angle::from_radians(30.0_f64.to_radians());
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(5.0, 5.0), angle),
            )
            .expect("rotate");
        let snapshot = document.primitive(id).expect("exists");

        let unrotated = rect_handles(bounds(0.0, 0.0, 10.0, 10.0), Length::from_mm(2.0));
        let local = unrotated
            .iter()
            .find(|h| h.kind == HandleKind::CornerRadius)
            .expect("corner-radius handle")
            .position;
        let rotated = handles_for(&snapshot)
            .into_iter()
            .find(|h| h.kind == HandleKind::CornerRadius)
            .expect("corner-radius handle")
            .position;
        let expected = local.rotated_around(Point::new(5.0, 5.0), angle);
        assert!((rotated.x - expected.x).abs() < 1e-9);
        assert!((rotated.y - expected.y).abs() < 1e-9);
        assert!(
            (rotated.x - local.x).abs() > 0.5 || (rotated.y - local.y).abs() > 0.5,
            "30° off where it sits on an unrotated rectangle of the same size"
        );
    }

    /// A pointer displacement along the object's own axes is the plain
    /// displacement turned back by the rotation.
    #[test]
    fn local_delta_undoes_the_rotation() {
        let angle = Angle::from_radians(std::f64::consts::FRAC_PI_2);
        let delta = local_delta(angle, Point::new(0.0, 0.0), Point::new(0.0, 5.0));
        // Document +Y on a 90°-rotated object is its own local +X.
        assert!((delta.x - 5.0).abs() < 1e-9);
        assert!(delta.y.abs() < 1e-9);
    }

    /// AC3: dragging the E handle changes only width.
    #[test]
    fn resize_rect_bounds_east_only_changes_width() {
        let resized = resize_rect_bounds(
            bounds(0.0, 0.0, 10.0, 10.0),
            ResizeDirection::E,
            Vec2::new(5.0, 5.0),
        );
        assert_eq!(resized.origin, Point::new(0.0, 0.0));
        assert!((resized.width.as_mm() - 15.0).abs() < 1e-9);
        assert!((resized.height.as_mm() - 10.0).abs() < 1e-9);
    }

    /// AC6: dragging the corner-radius handle back onto the corner
    /// gives exactly zero.
    #[test]
    fn corner_radius_from_drag_back_to_the_corner_is_zero() {
        let b = bounds(0.0, 0.0, 10.0, 10.0);
        let diagonal = corner_inward_diagonal(ResizeDirection::Ne).scaled(3.0);
        let out_radius = corner_radius_from_drag(Length::from_mm(0.0), b, diagonal);
        assert!((out_radius.as_mm() - 3.0).abs() < 1e-9);
        let back_radius = corner_radius_from_drag(out_radius, b, diagonal.scaled(-1.0));
        assert!((back_radius.as_mm()).abs() < 1e-9);
    }

    /// AC5: the drag cannot exceed half the shorter side.
    #[test]
    fn corner_radius_from_drag_clamps_to_half_the_shorter_side() {
        let b = bounds(0.0, 0.0, 10.0, 20.0);
        let diagonal = corner_inward_diagonal(ResizeDirection::Ne).scaled(100.0);
        let radius = corner_radius_from_drag(Length::from_mm(0.0), b, diagonal);
        assert!((radius.as_mm() - 5.0).abs() < 1e-9);
    }

    /// AC13: scaling via a cardinal handle changes only the outer
    /// radius; ratio/point-count/orientation are the caller's business
    /// (unaffected by this pure function, which only ever returns a new
    /// radius).
    #[test]
    fn scale_star_frame_changes_only_radius() {
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.3),
        };
        let scaled = scale_star_frame(frame, ResizeDirection::E, Vec2::new(5.0, 0.0));
        assert!((scaled.radius.as_mm() - 15.0).abs() < 1e-9);
        assert_eq!(scaled.center, frame.center);
        assert_eq!(scaled.angle, frame.angle);
    }
}
