//! The oriented selection box of a single selected object
//! (`specs/0005-object-transform/adrs.md`, "the oriented box lives in
//! `vecmanf-ui-core`"): a rectangle in the object's own local, unrotated
//! frame plus the angle that maps that frame into document space —
//! [`crate::object_bounds::object_bounds`]'s own axis-aligned two-point
//! box is this type's angle-0 case, kept separately for multi-selection
//! (acceptance criterion 2 of `specs/0005-object-transform/
//! specification.md`, which still shows every object's plain,
//! non-oriented box regardless of its own rotation).
//!
//! A primitive's local frame coincides with its stored
//! `RectBounds`/`EllipseFrame`/`StarFrame` (already unrotated by
//! construction, `adrs.md`'s "the frame registers... are the shape in
//! its LOCAL frame"), so its own frame center is both the local and the
//! document-space pivot the rotation turns about. A path has no local
//! frame stored at all — its anchors are baked, absolute document-space
//! points — so this module derives one by rotating every anchor by
//! `-rotation` about a fixed reference point (document origin) and
//! taking [`vecmanf_geometry_core::segment_bounds`]'s tight union in
//! that frame, exactly mirroring [`crate::object_bounds::object_bounds`]'s
//! own curve-tight (not control-hull) rule.

use vecmanf_document_core::{
    Angle, ObjectSnapshot, PathSnapshot, Point, shape_center, shape_frame_bounds,
};
use vecmanf_geometry_core::segment_bounds;

use crate::hit_test::segment_pairs;

/// A fixed reference point for de-rotating a path's baked anchors into
/// its own local frame — any point works as long as the same one is used
/// to de-rotate and to map back out via [`OrientedBox::to_document`], so
/// the document origin is as good as any other and needs no extra state.
const PATH_DEROTATION_PIVOT: Point = Point::new(0.0, 0.0);

/// An object's selection box in its own local, unrotated frame, plus the
/// angle and pivot that maps a local-frame point into document space
/// (acceptance criteria 1, 18 of `specs/0005-object-transform/
/// specification.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrientedBox {
    /// The box's minimum corner, in the local (unrotated) frame.
    pub min: Point,
    /// The box's maximum corner, in the local (unrotated) frame.
    pub max: Point,
    /// The object's own rotation — how far the local frame is turned
    /// from document axes.
    pub angle: Angle,
    /// The point [`OrientedBox::to_document`] rotates about.
    pub pivot: Point,
}

impl OrientedBox {
    /// The box's center, in the local frame.
    #[must_use]
    pub fn local_center(&self) -> Point {
        Point::new(
            f64::midpoint(self.min.x, self.max.x),
            f64::midpoint(self.min.y, self.max.y),
        )
    }

    /// The box's width in the local frame (document millimetres).
    #[must_use]
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// The box's height in the local frame (document millimetres).
    #[must_use]
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Maps a point from this box's local frame into document space —
    /// every handle position and the rotate handle's offset go through
    /// this one call (`adrs.md`'s "oriented bounding box" rule: "every
    /// handle position... is computed in the object's own local, rotated
    /// coordinate frame and then mapped to screen space by the current
    /// rotation").
    #[must_use]
    pub fn to_document(&self, local: Point) -> Point {
        local.rotated_around(self.pivot, self.angle)
    }

    /// The box's four corners in document space, in order around its
    /// perimeter (top-left, top-right, bottom-right, bottom-left of the
    /// *local* frame) — what the selection/hover outline draws, so the
    /// outline turns with the object instead of re-squaring to the
    /// screen axes (acceptance criterion 18).
    #[must_use]
    pub fn document_corners(&self) -> [Point; 4] {
        [
            Point::new(self.min.x, self.min.y),
            Point::new(self.max.x, self.min.y),
            Point::new(self.max.x, self.max.y),
            Point::new(self.min.x, self.max.y),
        ]
        .map(|corner| self.to_document(corner))
    }

    /// Maps a point from document space into this box's local frame —
    /// the inverse of [`OrientedBox::to_document`], used to map a
    /// pointer position into local axes before resize/rotate drag
    /// arithmetic that assumes an unrotated frame (`adrs.md`: "the
    /// local-frame mapping for the shape tools", extended here to the
    /// Select tool's own transform handles).
    #[must_use]
    pub fn to_local(&self, document: Point) -> Point {
        document.rotated_around(self.pivot, Angle::from_radians(-self.angle.as_radians()))
    }
}

/// `object`'s own [`OrientedBox`] — a primitive's local frame
/// ([`shape_frame_bounds`]) with its own rotation, or a path's
/// derotated-anchor bounds (`specs/0005-object-transform/adrs.md`).
#[must_use]
pub fn oriented_bounds(object: &ObjectSnapshot) -> OrientedBox {
    match object {
        ObjectSnapshot::Primitive(primitive) => {
            let (min, max) = shape_frame_bounds(&primitive.shape);
            OrientedBox {
                min,
                max,
                angle: primitive.rotation,
                pivot: shape_center(&primitive.shape),
            }
        }
        ObjectSnapshot::Path(path) => path_oriented_bounds(path),
    }
}

fn path_oriented_bounds(path: &PathSnapshot) -> OrientedBox {
    let pivot = PATH_DEROTATION_PIVOT;
    let into_local = Angle::from_radians(-path.rotation.as_radians());
    let derotated: Vec<(
        Point,
        vecmanf_document_core::Vec2,
        vecmanf_document_core::Vec2,
    )> = path
        .anchors
        .iter()
        .map(|anchor| {
            (
                anchor.point.rotated_around(pivot, into_local),
                anchor.handle_in.rotated(into_local),
                anchor.handle_out.rotated(into_local),
            )
        })
        .collect();

    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (i, j) in segment_pairs(derotated.len(), path.closed) {
        let (start, _, start_out) = derotated[i];
        let (end, end_in, _) = derotated[j];
        let (seg_min, seg_max) = segment_bounds(start, start_out, end_in, end);
        min.x = min.x.min(seg_min.x);
        min.y = min.y.min(seg_min.y);
        max.x = max.x.max(seg_max.x);
        max.y = max.y.max(seg_max.y);
    }
    if min.x.is_infinite() {
        // A degenerate single-anchor path — see `object_bounds::path_bounds`'s
        // own doc comment for why this is defensive, not expected.
        let only = derotated.first().map_or(pivot, |(p, _, _)| *p);
        min = only;
        max = only;
    }
    OrientedBox {
        min,
        max,
        angle: path.rotation,
        pivot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Document, Length, RectBounds};

    #[test]
    fn oriented_bounds_of_an_unrotated_rect_matches_its_frame() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(5.0),
        });
        let object = document.object(id).expect("exists");
        let b = oriented_bounds(&object);
        assert_eq!(b.min, Point::new(0.0, 0.0));
        assert_eq!(b.max, Point::new(10.0, 5.0));
        assert!(b.angle.as_radians().abs() < 1e-9);
    }

    /// Acceptance criterion 18: a rotated primitive's oriented box keeps
    /// the local-frame rectangle (unchanged) and reports the rotation
    /// angle, rather than re-squaring to the screen axes.
    #[test]
    fn oriented_bounds_of_a_rotated_rect_keeps_the_local_frame_and_angle() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .rotate_object(
                id,
                Point::new(5.0, 5.0),
                Angle::from_radians(std::f64::consts::FRAC_PI_4),
            )
            .expect("rotate");
        let object = document.object(id).expect("exists");
        let b = oriented_bounds(&object);
        // The local frame itself is unchanged by rotation.
        assert_eq!(b.min, Point::new(0.0, 0.0));
        assert_eq!(b.max, Point::new(10.0, 10.0));
        assert!((b.angle.as_radians() - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
    }

    /// `to_document` maps the local top-left corner to the rotated
    /// document-space position.
    #[test]
    fn to_document_maps_a_local_corner_through_the_rotation() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .rotate_object(
                id,
                Point::new(5.0, 5.0),
                Angle::from_radians(std::f64::consts::FRAC_PI_2),
            )
            .expect("rotate");
        let object = document.object(id).expect("exists");
        let b = oriented_bounds(&object);
        // Top-left (0,0) rotated 90 degrees about (5,5) -> (10, 0).
        let mapped = b.to_document(b.min);
        assert!((mapped.x - 10.0).abs() < 1e-9);
        assert!((mapped.y - 0.0).abs() < 1e-9);
    }

    /// Acceptance criterion 18: a rotated object's outline corners are
    /// the local box's corners turned by its rotation, not the
    /// axis-aligned bounds of the turned shape.
    #[test]
    fn document_corners_follow_the_objects_rotation() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(4.0),
        });
        document
            .rotate_object(
                id,
                Point::new(5.0, 2.0),
                Angle::from_radians(std::f64::consts::FRAC_PI_2),
            )
            .expect("rotate");
        let b = oriented_bounds(&document.object(id).expect("exists"));
        let corners = b.document_corners();
        // Turned 90 degrees about (5, 2): the 10x4 box is now 4 wide, 10 tall.
        let xs: Vec<f64> = corners.iter().map(|c| c.x).collect();
        let ys: Vec<f64> = corners.iter().map(|c| c.y).collect();
        let span = |v: &[f64]| {
            v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min)
        };
        assert!((span(&xs) - 4.0).abs() < 1e-9);
        assert!((span(&ys) - 10.0).abs() < 1e-9);
        // Consecutive corners are still a rectangle's edges (right angles).
        let e1 = corners[0].vector_to(corners[1]);
        let e2 = corners[1].vector_to(corners[2]);
        assert!((e1.x * e2.x + e1.y * e2.y).abs() < 1e-9);
    }

    /// `to_local` is the exact inverse of `to_document`.
    #[test]
    fn to_local_is_the_inverse_of_to_document() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .rotate_object(id, Point::new(5.0, 5.0), Angle::from_radians(0.7))
            .expect("rotate");
        let object = document.object(id).expect("exists");
        let b = oriented_bounds(&object);
        let local = Point::new(3.0, 8.0);
        let round_tripped = b.to_local(b.to_document(local));
        assert!((round_tripped.x - local.x).abs() < 1e-9);
        assert!((round_tripped.y - local.y).abs() < 1e-9);
    }

    /// A rotated path's oriented box is tight in its own local (de-
    /// rotated) frame, and reports the path's `rotation` register.
    #[test]
    fn oriented_bounds_of_a_rotated_path() {
        use vecmanf_document_core::{AnchorId, NewAnchor};

        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
            ],
            false,
        );
        document
            .rotate_object(
                id,
                Point::new(0.0, 0.0),
                Angle::from_radians(std::f64::consts::FRAC_PI_2),
            )
            .expect("rotate");
        let object = document.object(id).expect("exists");
        let b = oriented_bounds(&object);
        assert!((b.angle.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        // In the derotated (local) frame, the path is tight to its own
        // original 10mm extent along one axis.
        assert!((b.width() - 10.0).abs() < 1e-6 || (b.height() - 10.0).abs() < 1e-6);
    }
}
