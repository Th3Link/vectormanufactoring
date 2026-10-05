//! The primitive-shape schema's pure data types (`specs/0003-primitive-shapes/
//! adrs.md`, "the primitive schema"): a shape's parameters, the two
//! validated newtypes the specification pins (point count, inner ratio),
//! and the object-level read model that lets a caller tell a path from a
//! primitive without this crate ever handing back an empty
//! [`crate::path_model::PathSnapshot`] for a node that is not a path at
//! all. No Loro type appears here — the CRDT wiring lives in
//! [`crate::shape_codec`] — so this is what `vecmanf-ui-core` and
//! `vecmanf-render-core` actually depend on, the same split
//! [`crate::path_model`] already draws for paths.

use serde::{Deserialize, Serialize};

use crate::path_model::{Color, NodeId, PathSnapshot};
use crate::units::{Angle, Length, Point, Vec2};

/// A rectangle's bounding box, normalized so `origin` is always the
/// top-left (minimum) corner and `width`/`height` are always
/// non-negative (`specs/0003-primitive-shapes/adrs.md`: "rect bounds are
/// normalized on write" — a drag from A to B in any direction stores the
/// same value, acceptance criterion 1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectBounds {
    /// The top-left corner.
    pub origin: Point,
    /// Always ≥ 0.
    pub width: Length,
    /// Always ≥ 0.
    pub height: Length,
}

impl RectBounds {
    /// Builds a [`RectBounds`] from two opposite corners of a drag, in
    /// either direction (acceptance criterion 1).
    #[must_use]
    pub fn from_corners(a: Point, b: Point) -> Self {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        let width = (b.x - a.x).abs();
        let height = (b.y - a.y).abs();
        Self {
            origin: Point::new(x, y),
            width: Length::from_mm(width),
            height: Length::from_mm(height),
        }
    }

    /// Whether `a` and `b` describe a zero-size drag (acceptance
    /// criterion 1: "a drag where A equals B... creates nothing").
    #[must_use]
    pub fn is_degenerate(a: Point, b: Point) -> bool {
        a == b
    }
}

/// A circle/ellipse's frame: center plus the two radii (acceptance
/// criteria 7, 8, 9). A circle is simply a frame with `rx == ry` — "it
/// has no kind of its own and no flag" (`adrs.md`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EllipseFrame {
    /// The ellipse's center.
    pub center: Point,
    /// The horizontal radius.
    pub rx: Length,
    /// The vertical radius.
    pub ry: Length,
}

impl EllipseFrame {
    /// Builds an [`EllipseFrame`] from two opposite corners of its
    /// bounding box's drag (acceptance criterion 7).
    #[must_use]
    pub fn from_corners(a: Point, b: Point) -> Self {
        let cx = f64::midpoint(a.x, b.x);
        let cy = f64::midpoint(a.y, b.y);
        let rx = (b.x - a.x).abs() / 2.0;
        let ry = (b.y - a.y).abs() / 2.0;
        Self {
            center: Point::new(cx, cy),
            rx: Length::from_mm(rx),
            ry: Length::from_mm(ry),
        }
    }
}

/// A polygon or star's frame: center, outer radius, and the angle of its
/// first outer vertex (`adrs.md`: "θ is the angle of the first outer
/// vertex, `atan2(dy, dx)` from the centre in Y-down document space").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StarFrame {
    /// The shape's center.
    pub center: Point,
    /// The outer radius (acceptance criteria 11, 12).
    pub radius: Length,
    /// The first outer vertex's angle from the center.
    pub angle: Angle,
}

impl StarFrame {
    /// Builds a [`StarFrame`] from a center and the point one outer
    /// vertex lands on (acceptance criteria 11, 12): `radius = |AB|`,
    /// `angle = atan2(dy, dx)`.
    #[must_use]
    pub fn from_center_and_vertex(center: Point, vertex: Point) -> Self {
        let v = center.vector_to(vertex);
        Self {
            center,
            radius: Length::from_mm(v.length()),
            angle: Angle::from_radians(v.y.atan2(v.x)),
        }
    }
}

/// Why building a validated primitive parameter failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShapeParamError {
    /// Acceptance criterion 10: `point_count` must be `3..=1024`
    /// (Inkscape's own limit, adopted as a crash-safety bound).
    #[error(
        "point count must be between {} and {}",
        PointCount::MIN,
        PointCount::MAX
    )]
    PointCountOutOfRange,
    /// Acceptance criterion 12: the inner/outer ratio must be strictly
    /// between 0 and 1.
    #[error("inner ratio must be strictly between 0 and 1")]
    InnerRatioOutOfRange,
}

/// A polygon or star's point count, validated to `3..=1024` at
/// construction (acceptance criterion 10) so no public signature in this
/// crate ever accepts a bare, unchecked `u32` for it (`adrs.md`: "a
/// validated newtype... not a bare u32/f64 on a public signature").
///
/// `Serialize` only, deliberately not `Deserialize`: a derived
/// `Deserialize` would skip [`PointCount::new`]'s own range check
/// entirely (architect review) — nothing in this crate reads one back
/// this way in any case; `crate::shape_codec` reads the raw stored
/// integer and calls `new` itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PointCount(u32);

impl PointCount {
    /// Inkscape's own lower bound — three points is the smallest
    /// polygon.
    pub const MIN: u32 = 3;
    /// Inkscape's own upper bound, adopted here as a crash-safety cap
    /// (acceptance criterion 10): an unbounded point count risks the
    /// resource-exhaustion crash `project-file-foundation` AC 7 already
    /// rules out for a damaged file.
    pub const MAX: u32 = 1024;

    /// Validates `count` against `3..=1024`.
    ///
    /// # Errors
    /// [`ShapeParamError::PointCountOutOfRange`] if `count` is outside
    /// that range.
    pub fn new(count: u32) -> Result<Self, ShapeParamError> {
        if (Self::MIN..=Self::MAX).contains(&count) {
            Ok(Self(count))
        } else {
            Err(ShapeParamError::PointCountOutOfRange)
        }
    }

    /// The validated point count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A star's inner/outer radius ratio, validated to the open interval
/// `(0, 1)` at construction (acceptance criterion 12: "0 < R < 1 taken
/// literally — the field never allows exactly 0 or 1, which would
/// collapse the star to a point or a plain polygon").
///
/// `Serialize` only, deliberately not `Deserialize` — same reasoning as
/// [`PointCount`]'s own doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct InnerRatio(f64);

impl InnerRatio {
    /// Validates `ratio` against the open interval `(0, 1)`, finite.
    ///
    /// # Errors
    /// [`ShapeParamError::InnerRatioOutOfRange`] if `ratio` is not
    /// finite, or is outside that open interval.
    pub fn new(ratio: f64) -> Result<Self, ShapeParamError> {
        if ratio.is_finite() && ratio > 0.0 && ratio < 1.0 {
            Ok(Self(ratio))
        } else {
            Err(ShapeParamError::InnerRatioOutOfRange)
        }
    }

    /// The validated ratio.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Which kind of primitive a node holds, and its own parameters
/// (`specs/0003-primitive-shapes/adrs.md`, "the primitive schema"). A closed
/// `enum` rather than a trait: "the set is closed, and `CLAUDE.md` §5
/// applies" (`adrs.md`). `Serialize` only, not `Deserialize`: it holds
/// [`PointCount`]/[`InnerRatio`], and neither of those derives
/// `Deserialize` either — see their own doc comments.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Shape {
    /// Acceptance criteria 1-6.
    Rect {
        /// The rectangle's bounding box.
        bounds: RectBounds,
        /// Stored as entered, clamped only where it is evaluated
        /// (`adrs.md`'s "the corner radius is stored raw" decision) —
        /// see [`crate::primitive_outline::effective_corner_radius`].
        corner_radius: Length,
    },
    /// Acceptance criteria 7-9. A circle is an `Ellipse` with
    /// `rx == ry`, not a kind of its own.
    Ellipse {
        /// The ellipse's frame.
        frame: EllipseFrame,
    },
    /// Acceptance criteria 10, 11, 13, 15.
    Polygon {
        /// Center, outer radius and orientation.
        frame: StarFrame,
        /// Always `3..=1024`.
        point_count: PointCount,
    },
    /// Acceptance criteria 10, 12, 13, 14, 15.
    Star {
        /// Center, outer radius and orientation.
        frame: StarFrame,
        /// Always `3..=1024`.
        point_count: PointCount,
        /// Always strictly between 0 and 1.
        inner_ratio: InnerRatio,
    },
}

/// A primitive's full data as read from the document — the primitive
/// counterpart to [`PathSnapshot`]. `Serialize` only, not
/// `Deserialize`: it holds a [`Shape`], which does not derive
/// `Deserialize` either.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PrimitiveSnapshot {
    /// This primitive's identity — the same [`NodeId`] a path or any
    /// other object would carry (ADR 0002 §5: one shared tree).
    pub id: NodeId,
    /// This primitive's kind and parameters.
    pub shape: Shape,
    /// Stroke width — this slice's one placeholder default, same as a
    /// path's (acceptance criterion 16).
    pub stroke_width: Length,
    /// Stroke color — same placeholder default as a path's.
    pub stroke: Color,
    /// Fill — always `None` in this slice, same as a path's.
    pub fill: Option<Color>,
}

/// One tree node's data, read generically without first knowing whether
/// it is a path or a primitive (`adrs.md`: "`Document::path(id)` returns
/// `None` for a primitive node... reading goes through an object-level
/// snapshot"). `Serialize` only, not `Deserialize`: it holds a
/// [`PrimitiveSnapshot`], which does not derive `Deserialize` either.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ObjectSnapshot {
    /// A path node (no `shape` tag).
    Path(PathSnapshot),
    /// A primitive node.
    Primitive(PrimitiveSnapshot),
}

impl ObjectSnapshot {
    /// This object's identity, whichever kind it is.
    #[must_use]
    pub const fn id(&self) -> NodeId {
        match self {
            Self::Path(snapshot) => snapshot.id,
            Self::Primitive(snapshot) => snapshot.id,
        }
    }

    /// This object translated by `offset` — a path's anchor `point`s (its
    /// handles are relative to their own anchor and so need no write,
    /// `specs/0002-path-node-editing/adrs.md` decision 2) or a primitive's
    /// frame origin/center (`translate_shape`). Elementary arithmetic, no
    /// document access — the Select tool's live move preview
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "a move
    /// rewrites geometry... Preview and commit therefore share one
    /// implementation") renders this directly, and
    /// [`crate::Document::translate_objects`] commits the same rule.
    #[must_use]
    pub fn translated(&self, offset: Vec2) -> Self {
        match self {
            Self::Path(path) => {
                let mut path = path.clone();
                for anchor in &mut path.anchors {
                    anchor.point = anchor.point.translated(offset);
                }
                Self::Path(path)
            }
            Self::Primitive(primitive) => {
                let mut primitive = *primitive;
                primitive.shape = translate_shape(primitive.shape, offset);
                Self::Primitive(primitive)
            }
        }
    }
}

/// Translates a primitive's own frame by `offset`: a rectangle's `origin`,
/// an ellipse's `center`, or a polygon/star's `center` — point count,
/// orientation, inner ratio and (for a rectangle) corner radius are all
/// untouched. The one rule [`ObjectSnapshot::translated`]'s primitive arm
/// and [`crate::Document::translate_objects`]'s primitive writes both call,
/// so a moved primitive's live preview and its committed result can never
/// independently drift apart.
#[must_use]
pub(crate) fn translate_shape(shape: Shape, offset: Vec2) -> Shape {
    match shape {
        Shape::Rect {
            bounds,
            corner_radius,
        } => Shape::Rect {
            bounds: RectBounds {
                origin: bounds.origin.translated(offset),
                ..bounds
            },
            corner_radius,
        },
        Shape::Ellipse { frame } => Shape::Ellipse {
            frame: EllipseFrame {
                center: frame.center.translated(offset),
                ..frame
            },
        },
        Shape::Polygon { frame, point_count } => Shape::Polygon {
            frame: StarFrame {
                center: frame.center.translated(offset),
                ..frame
            },
            point_count,
        },
        Shape::Star {
            frame,
            point_count,
            inner_ratio,
        } => Shape::Star {
            frame: StarFrame {
                center: frame.center.translated(offset),
                ..frame
            },
            point_count,
            inner_ratio,
        },
    }
}

/// The axis-aligned frame bounds of any primitive shape — what the
/// Select-tool (and shape-tool) bounding-box decoration draws around
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "the primitive box
/// that `render-core/src/shape_preview.rs` computes privately... moves to
/// `document-core` as plain arithmetic on its own types"). A star keeps its
/// circumscribed-frame box, matching `primitive-shapes`' own convention.
#[must_use]
pub fn shape_frame_bounds(shape: &Shape) -> (Point, Point) {
    match *shape {
        Shape::Rect { bounds, .. } => (
            bounds.origin,
            bounds
                .origin
                .translated(Vec2::new(bounds.width.as_mm(), bounds.height.as_mm())),
        ),
        Shape::Ellipse { frame } => (
            Point::new(
                frame.center.x - frame.rx.as_mm(),
                frame.center.y - frame.ry.as_mm(),
            ),
            Point::new(
                frame.center.x + frame.rx.as_mm(),
                frame.center.y + frame.ry.as_mm(),
            ),
        ),
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => (
            Point::new(
                frame.center.x - frame.radius.as_mm(),
                frame.center.y - frame.radius.as_mm(),
            ),
            Point::new(
                frame.center.x + frame.radius.as_mm(),
                frame.center.y + frame.radius.as_mm(),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_bounds_from_corners_normalizes_any_drag_direction() {
        let a = RectBounds::from_corners(Point::new(10.0, 10.0), Point::new(0.0, 0.0));
        let b = RectBounds::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        assert_eq!(a, b);
        assert_eq!(a.origin, Point::new(0.0, 0.0));
        assert!((a.width.as_mm() - 10.0).abs() < f64::EPSILON);
        assert!((a.height.as_mm() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rect_bounds_is_degenerate_only_when_corners_coincide() {
        assert!(RectBounds::is_degenerate(
            Point::new(1.0, 1.0),
            Point::new(1.0, 1.0)
        ));
        assert!(!RectBounds::is_degenerate(
            Point::new(1.0, 1.0),
            Point::new(1.0, 2.0)
        ));
    }

    #[test]
    fn ellipse_frame_from_corners_centers_and_halves() {
        let frame = EllipseFrame::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        assert_eq!(frame.center, Point::new(5.0, 10.0));
        assert!((frame.rx.as_mm() - 5.0).abs() < f64::EPSILON);
        assert!((frame.ry.as_mm() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn star_frame_from_center_and_vertex() {
        let frame = StarFrame::from_center_and_vertex(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        assert!((frame.radius.as_mm() - 10.0).abs() < f64::EPSILON);
        assert!(frame.angle.as_radians().abs() < 1e-9);
    }

    #[test]
    fn point_count_accepts_the_inclusive_range() {
        assert!(PointCount::new(3).is_ok());
        assert!(PointCount::new(1024).is_ok());
        assert!(PointCount::new(2).is_err());
        assert!(PointCount::new(1025).is_err());
    }

    #[test]
    fn inner_ratio_rejects_the_closed_endpoints() {
        assert!(InnerRatio::new(0.5).is_ok());
        assert!(InnerRatio::new(0.0).is_err());
        assert!(InnerRatio::new(1.0).is_err());
        assert!(InnerRatio::new(f64::NAN).is_err());
    }

    #[test]
    fn translated_primitive_moves_the_frame_only() {
        let shape = Shape::Rect {
            bounds: RectBounds {
                origin: Point::new(0.0, 0.0),
                width: Length::from_mm(10.0),
                height: Length::from_mm(5.0),
            },
            corner_radius: Length::from_mm(2.0),
        };
        let snapshot = PrimitiveSnapshot {
            id: NodeId::from_parts(1, 1),
            shape,
            stroke_width: Length::from_mm(0.25),
            stroke: Color::BLACK,
            fill: None,
        };
        let moved = ObjectSnapshot::Primitive(snapshot).translated(Vec2::new(3.0, 4.0));
        let ObjectSnapshot::Primitive(moved) = moved else {
            panic!("expected a primitive");
        };
        let Shape::Rect {
            bounds,
            corner_radius,
        } = moved.shape
        else {
            panic!("expected a rect");
        };
        assert_eq!(bounds.origin, Point::new(3.0, 4.0));
        assert!((bounds.width.as_mm() - 10.0).abs() < f64::EPSILON);
        assert!((corner_radius.as_mm() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn translated_path_moves_every_anchor_point_but_not_its_handles() {
        use crate::path_model::{AnchorId, AnchorKind, NewAnchor};

        let anchor = NewAnchor {
            id: AnchorId::new(1, 1),
            point: Point::new(0.0, 0.0),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::new(5.0, 0.0),
            kind: AnchorKind::Symmetric,
        };
        let path = PathSnapshot {
            id: NodeId::from_parts(1, 2),
            closed: false,
            stroke_width: Length::from_mm(0.25),
            stroke: Color::BLACK,
            fill: None,
            anchors: vec![anchor],
        };
        let moved = ObjectSnapshot::Path(path).translated(Vec2::new(1.0, 2.0));
        let ObjectSnapshot::Path(moved) = moved else {
            panic!("expected a path");
        };
        assert_eq!(moved.anchors[0].point, Point::new(1.0, 2.0));
        assert_eq!(moved.anchors[0].handle_out, Vec2::new(5.0, 0.0));
    }

    #[test]
    fn shape_frame_bounds_of_a_rect_is_its_own_box() {
        let shape = Shape::Rect {
            bounds: RectBounds {
                origin: Point::new(1.0, 2.0),
                width: Length::from_mm(10.0),
                height: Length::from_mm(5.0),
            },
            corner_radius: Length::from_mm(0.0),
        };
        let (min, max) = shape_frame_bounds(&shape);
        assert_eq!(min, Point::new(1.0, 2.0));
        assert_eq!(max, Point::new(11.0, 7.0));
    }

    #[test]
    fn shape_frame_bounds_of_a_star_is_its_circumscribed_frame() {
        let shape = Shape::Star {
            frame: StarFrame {
                center: Point::new(0.0, 0.0),
                radius: Length::from_mm(10.0),
                angle: Angle::from_radians(0.0),
            },
            point_count: PointCount::new(5).unwrap(),
            inner_ratio: InnerRatio::new(0.5).unwrap(),
        };
        let (min, max) = shape_frame_bounds(&shape);
        assert_eq!(min, Point::new(-10.0, -10.0));
        assert_eq!(max, Point::new(10.0, 10.0));
    }
}
