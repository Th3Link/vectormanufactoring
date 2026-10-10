//! The affine transforms of a path snapshot: rotate, scale and shear about a pivot, baked into
//! the anchors and handles (`specs/0005-object-transform`, `specs/0008-object-transform-refinements`,
//! `specs/0019-multi-object-transform`). Split out of `path_model.rs`.

use crate::path_model::PathSnapshot;
use crate::units::{Angle, Point, Vec2};

impl PathSnapshot {
    /// This path rotated by `angle` about `pivot`: every anchor's `point`
    /// rotates about `pivot`; every `handle_in`/`handle_out` rotates by
    /// `angle` alone (already relative to its own anchor, so it needs no
    /// pivot) — acceptance criterion 20 of `specs/0005-object-transform/
    /// specification.md`. `rotation` advances by `angle`, normalized, so
    /// a later reselect still knows this path's own orientation
    /// (criterion 18) even though the anchors themselves are now the
    /// sole source of truth for its actual geometry.
    #[must_use]
    pub fn rotated(&self, pivot: Point, angle: Angle) -> Self {
        let mut rotated = self.clone();
        for anchor in rotated.all_anchors_mut() {
            anchor.point = anchor.point.rotated_around(pivot, angle);
            anchor.handle_in = anchor.handle_in.rotated(angle);
            anchor.handle_out = anchor.handle_out.rotated(angle);
        }
        rotated.rotation =
            Angle::from_radians(self.rotation.as_radians() + angle.as_radians()).normalized();
        rotated
    }

    /// This path resized by `(sx, sy)` about `pivot`, measured along the
    /// path's own local axes (acceptance criterion 12): every anchor's
    /// point and handle vectors are mapped into the path's local frame
    /// (rotated by `-rotation` about `pivot`), scaled per axis, then
    /// mapped back out (rotated by `+rotation`) — "into the local frame,
    /// per-axis scale, back out" (`adrs.md`). Identical to a plain
    /// anisotropic scale when `rotation` is zero. `rotation` itself is
    /// untouched: a resize never changes an object's orientation
    /// (`specification.md` acceptance criterion 23's "a move is always a
    /// pure translation", extended here to "a resize never rotates").
    #[must_use]
    pub fn scaled(&self, pivot: Point, sx: f64, sy: f64) -> Self {
        self.scaled_along(pivot, sx, sy, self.rotation)
    }

    /// This path scaled by `(sx, sy)` about `pivot` along axes turned by
    /// `axes`: [`PathSnapshot::scaled`] with the frame given instead of the
    /// path's own `rotation`. A selection of several objects scales along the
    /// document axes (`axes` zero, `specs/0019-multi-object-transform/` criterion
    /// 20). `rotation` is untouched.
    #[must_use]
    pub fn scaled_along(&self, pivot: Point, sx: f64, sy: f64, axes: Angle) -> Self {
        let into_local = Angle::from_radians(-axes.as_radians());
        let out_of_local = axes;
        let mut scaled = self.clone();
        for anchor in scaled.all_anchors_mut() {
            anchor.point =
                scale_point_in_local_frame(anchor.point, pivot, into_local, out_of_local, sx, sy);
            anchor.handle_in =
                scale_vec_in_local_frame(anchor.handle_in, into_local, out_of_local, sx, sy);
            anchor.handle_out =
                scale_vec_in_local_frame(anchor.handle_out, into_local, out_of_local, sx, sy);
        }
        scaled
    }

    /// This path sheared about the line through `pivot` along the path's own
    /// local axes (`specs/0008-object-transform-refinements/adrs.md`, "skew
    /// (Part B) writes anchors only"): in local coordinates the linear part
    /// is `L = [[1, ku], [kv, 1]]` (an x skew sets `ku`, a y skew `kv`; the
    /// caller passes one of them as zero), applied to every anchor's offset
    /// from `pivot`; in document space `M = R(θ) · L · R(−θ)` with θ the
    /// path's `rotation`. Handle vectors are relative to their anchor, so
    /// they take the linear part only, without a translation. An affine map
    /// sends a cubic's control points to the control points of its image, so
    /// every segment stays exact. `rotation` is untouched: a shear is baked
    /// into the anchors and handles, the register keeps naming the box's
    /// orientation.
    #[must_use]
    pub fn sheared(&self, pivot: Point, ku: f64, kv: f64) -> Self {
        self.sheared_along(pivot, ku, kv, self.rotation)
    }

    /// This path sheared along axes turned by `axes`: [`PathSnapshot::sheared`]
    /// with the frame given instead of the path's own `rotation`. A selection
    /// of several paths shears along the document axes (`axes` zero,
    /// `specs/0019-multi-object-transform/` criterion 41). `rotation` is untouched.
    #[must_use]
    pub fn sheared_along(&self, pivot: Point, ku: f64, kv: f64, axes: Angle) -> Self {
        let into_local = Angle::from_radians(-axes.as_radians());
        let out_of_local = axes;
        let shear = |x: f64, y: f64| (x + ku * y, y + kv * x);
        let mut sheared = self.clone();
        for anchor in sheared.all_anchors_mut() {
            let local = anchor.point.rotated_around(pivot, into_local);
            let (dx, dy) = shear(local.x - pivot.x, local.y - pivot.y);
            anchor.point =
                Point::new(pivot.x + dx, pivot.y + dy).rotated_around(pivot, out_of_local);
            for handle in [&mut anchor.handle_in, &mut anchor.handle_out] {
                let local = handle.rotated(into_local);
                let (hx, hy) = shear(local.x, local.y);
                *handle = Vec2::new(hx, hy).rotated(out_of_local);
            }
        }
        sheared
    }
}

/// `point`, mapped into the local frame about `pivot` (rotate by
/// `into_local`), scaled per axis relative to `pivot`, then mapped back
/// out (rotate by `out_of_local`) — [`PathSnapshot::scaled`]'s one rule
/// for an anchor's absolute position.
fn scale_point_in_local_frame(
    point: Point,
    pivot: Point,
    into_local: Angle,
    out_of_local: Angle,
    sx: f64,
    sy: f64,
) -> Point {
    let local = point.rotated_around(pivot, into_local);
    let scaled_local = Point::new(
        pivot.x + (local.x - pivot.x) * sx,
        pivot.y + (local.y - pivot.y) * sy,
    );
    scaled_local.rotated_around(pivot, out_of_local)
}

/// A handle vector (relative to its own anchor, so no pivot is involved)
/// mapped into the local frame, scaled per axis, then mapped back out —
/// [`PathSnapshot::scaled`]'s one rule for a handle.
fn scale_vec_in_local_frame(
    v: Vec2,
    into_local: Angle,
    out_of_local: Angle,
    sx: f64,
    sy: f64,
) -> Vec2 {
    let local = v.rotated(into_local);
    let scaled_local = Vec2::new(local.x * sx, local.y * sy);
    scaled_local.rotated(out_of_local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::{AnchorId, AnchorKind, NewAnchor, NodeId};
    use crate::style_model::Style;

    fn simple_path(a: Point, b: Point) -> PathSnapshot {
        PathSnapshot {
            id: NodeId::from_parts(1, 1),
            closed: false,
            style: Style::default(),
            anchors: vec![
                NewAnchor::corner(AnchorId::new(1, 1), a),
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: b,
                    handle_in: Vec2::new(-2.0, 0.0),
                    handle_out: Vec2::new(2.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
            ],
            extra_subpaths: Vec::new(),
            rotation: Angle::from_radians(0.0),
        }
    }

    /// Acceptance criterion 20: every anchor point rotates about the
    /// pivot; handle vectors rotate by the angle alone (no pivot); the
    /// `rotation` register advances by the same angle.
    #[test]
    fn path_rotated_bakes_points_and_handles_and_advances_rotation() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let pivot = Point::new(0.0, 0.0);
        let rotated = path.rotated(pivot, Angle::from_radians(std::f64::consts::FRAC_PI_2));
        // (10, 0) rotated 90 degrees about the origin -> (0, 10).
        assert!((rotated.anchors[1].point.x - 0.0).abs() < 1e-9);
        assert!((rotated.anchors[1].point.y - 10.0).abs() < 1e-9);
        // handle_out (2, 0) rotates to (0, 2), no pivot involved.
        assert!((rotated.anchors[1].handle_out.x - 0.0).abs() < 1e-9);
        assert!((rotated.anchors[1].handle_out.y - 2.0).abs() < 1e-9);
        assert!(
            (rotated.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "rotation register advances"
        );
    }

    /// A rotate about a point other than the origin still only rotates
    /// the points, never the (already-relative) handle vectors.
    #[test]
    fn path_rotated_about_an_off_origin_pivot() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let pivot = Point::new(5.0, 0.0);
        let rotated = path.rotated(pivot, Angle::from_radians(std::f64::consts::PI));
        // (0,0) rotated 180 degrees about (5,0) -> (10, 0).
        assert!((rotated.anchors[0].point.x - 10.0).abs() < 1e-9);
        assert!(rotated.anchors[0].point.y.abs() < 1e-9);
        // (10,0) rotated 180 degrees about (5,0) -> (0, 0).
        assert!(rotated.anchors[1].point.x.abs() < 1e-9);
        assert!(rotated.anchors[1].point.y.abs() < 1e-9);
    }

    /// Acceptance criterion 12: a plain (unrotated) scale is the
    /// ordinary per-axis scale about the pivot.
    #[test]
    fn path_scaled_with_zero_rotation_scales_plainly_about_the_pivot() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let scaled = path.scaled(Point::new(0.0, 0.0), 2.0, 3.0);
        assert!((scaled.anchors[1].point.x - 20.0).abs() < 1e-9);
        assert!(scaled.anchors[1].point.y.abs() < 1e-9);
        // handle_out (2,0) scales by sx=2 -> (4, 0).
        assert!((scaled.anchors[1].handle_out.x - 4.0).abs() < 1e-9);
        assert!(scaled.anchors[1].handle_out.y.abs() < 1e-9);
        // rotation register is untouched by a resize.
        assert!(scaled.rotation.as_radians().abs() < 1e-9);
    }

    /// A rotated path's resize happens along its own local axes: scaling
    /// a path that is already rotated 90 degrees by sx along what is now
    /// the document's Y axis (its own local X) stretches it along Y, not
    /// X — proving the "into local frame, scale, back out" round trip.
    #[test]
    fn path_scaled_respects_its_own_rotation_local_axes() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0)).rotated(
            Point::new(0.0, 0.0),
            Angle::from_radians(std::f64::consts::FRAC_PI_2),
        );
        // After the 90-degree rotation, anchor[1] sits at (0, 10) (its
        // local +X axis now points along document +Y).
        assert!((path.anchors[1].point.x - 0.0).abs() < 1e-6);
        assert!((path.anchors[1].point.y - 10.0).abs() < 1e-6);

        let scaled = path.scaled(Point::new(0.0, 0.0), 2.0, 1.0);
        // Scaling by sx=2 along the path's own local X axis (now
        // document Y) doubles the Y extent, leaving X untouched.
        assert!(scaled.anchors[1].point.x.abs() < 1e-6);
        assert!((scaled.anchors[1].point.y - 20.0).abs() < 1e-6);
    }
    /// Skew: an x shear about the line `y = 0` moves each point along x in
    /// proportion to its distance from that line; the line itself stays.
    #[test]
    fn path_sheared_moves_points_in_proportion_to_their_distance_from_the_line() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 4.0));
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.5, 0.0);
        assert!(
            sheared.anchors[0].point.x.abs() < 1e-12,
            "on the fixed line"
        );
        assert!((sheared.anchors[1].point.x - 12.0).abs() < 1e-12);
        assert!((sheared.anchors[1].point.y - 4.0).abs() < 1e-12);
        // Handle vector (2, 0) has no y component: unchanged by an x shear
        // (linear part only, no translation).
        assert!((sheared.anchors[1].handle_out.x - 2.0).abs() < 1e-12);
        assert!(sheared.anchors[1].handle_out.y.abs() < 1e-12);
    }

    /// A y shear moves along y in proportion to the distance from the
    /// vertical line, and a handle with an x component picks up a y part.
    #[test]
    fn path_sheared_in_y_shears_handles_by_the_linear_part_only() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 4.0));
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.0, 0.25);
        assert!((sheared.anchors[1].point.x - 10.0).abs() < 1e-12);
        assert!((sheared.anchors[1].point.y - 6.5).abs() < 1e-12);
        assert!((sheared.anchors[1].handle_out.x - 2.0).abs() < 1e-12);
        assert!((sheared.anchors[1].handle_out.y - 0.5).abs() < 1e-12);
        assert!((sheared.anchors[1].handle_in.y + 0.5).abs() < 1e-12);
    }

    /// Criterion 44: the shear acts along the path's local axes, and the
    /// `rotation` register is not touched.
    #[test]
    fn path_sheared_acts_along_local_axes_and_keeps_rotation() {
        let theta = Angle::from_radians(std::f64::consts::FRAC_PI_2);
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0))
            .rotated(Point::new(0.0, 0.0), theta);
        // The path now runs along document +y; its local x axis is +y. A
        // local y shear (kv) therefore moves points along local y, which is
        // document -x. Anchor 1 at local (10, 0): local y' = 0 + kv * 10.
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.0, 0.3);
        assert!((sheared.anchors[1].point.x + 3.0).abs() < 1e-9);
        assert!((sheared.anchors[1].point.y - 10.0).abs() < 1e-9);
        assert!((sheared.rotation.as_radians() - theta.as_radians()).abs() < 1e-15);
    }

    /// Criterion 43: a skew by k followed by one by −k restores every
    /// anchor and handle (the fixed line is unchanged by the first skew).
    #[test]
    fn path_sheared_by_k_then_minus_k_is_the_identity() {
        let original = simple_path(Point::new(3.0, -2.0), Point::new(10.0, 4.0))
            .rotated(Point::new(1.0, 1.0), Angle::from_radians(0.7));
        let pivot = Point::new(5.0, 5.0);
        for (ku, kv) in [(0.8, 0.0), (0.0, -1.3)] {
            let round_trip = original.sheared(pivot, ku, kv).sheared(pivot, -ku, -kv);
            for (a, b) in round_trip.anchors.iter().zip(&original.anchors) {
                assert!((a.point.x - b.point.x).abs() < 1e-9);
                assert!((a.point.y - b.point.y).abs() < 1e-9);
                assert!((a.handle_in.x - b.handle_in.x).abs() < 1e-9);
                assert!((a.handle_out.y - b.handle_out.y).abs() < 1e-9);
            }
        }
    }

    /// Node kinds, node count and everything but anchors are unchanged
    /// (criterion 42).
    #[test]
    fn path_sheared_changes_only_points_and_handle_vectors() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 4.0));
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.5, 0.0);
        assert_eq!(sheared.anchors.len(), path.anchors.len());
        for (a, b) in sheared.anchors.iter().zip(&path.anchors) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.kind, b.kind);
        }
        assert_eq!(sheared.style, path.style);
        assert_eq!(sheared.closed, path.closed);
    }
}
