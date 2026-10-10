//! A primitive as a path (`specs/0019-multi-object-transform/` criterion 53 and
//! `adrs.md` decision 8) and "Object to path" (acceptance criteria 17, 21, 22):
//! which shapes a stretch turns into paths, the one function that builds the
//! path, and the resolved anchor geometry a
//! [`curvyo_document_core::Document::convert_to_paths`] call needs,
//! for a whole primitive selection at once. Lives in this crate, not
//! `curvyo-editor-wasm`'s facade (architect review: ADR 0001 §1's
//! "the facade has no editing logic of its own" rule) — `Session` just
//! calls [`build_primitive_conversions`] and passes the result straight
//! to `Document::convert_to_paths`.

use curvyo_document_core::{
    AnchorSnapshot, Angle, Document, EllipseFrame, NewAnchor, NodeId, ObjectSnapshot, PathSnapshot,
    PrimitiveSnapshot, Shape, outline_of_rotated,
};

use crate::AnchorIdMinter;

/// A rotation within this (radians) of a multiple of 90 degrees is one.
const QUARTER_TURN_TOLERANCE_RAD: f64 = 1e-9;

/// Two radii closer than this (millimetres) make a circle.
const CIRCLE_TOLERANCE_MM: f64 = 1e-6;

/// The kinds of shape a stretch turns into paths, in the order the maker's texts
/// name them (`specs/0019-multi-object-transform/` criteria 53 and 55).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertingKind {
    /// A regular polygon.
    Polygon,
    /// A star.
    Star,
    /// A rectangle that is not turned by a multiple of 90 degrees (against the
    /// stretch axes).
    RotatedRectangle,
    /// An ellipse, not a circle, that is not turned by a multiple of 90 degrees.
    RotatedEllipse,
}

/// How many shapes of each [`ConvertingKind`] a selection holds or a commit
/// converted: what the texts of a stretch name ("Stretching turns 2 shapes into
/// paths"). The wording is the frontend's; this only counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConversionCounts {
    /// Polygons.
    pub polygons: u32,
    /// Stars.
    pub stars: u32,
    /// Rectangles turned off the axes.
    pub rotated_rectangles: u32,
    /// Ellipses turned off the axes.
    pub rotated_ellipses: u32,
}

impl ConversionCounts {
    /// All shapes counted.
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.polygons + self.stars + self.rotated_rectangles + self.rotated_ellipses
    }

    /// Whether nothing is counted.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// The counts as `[polygons, stars, rotated rectangles, rotated ellipses]`,
    /// the order of [`ConvertingKind`].
    #[must_use]
    pub const fn as_array(&self) -> [u32; 4] {
        [
            self.polygons,
            self.stars,
            self.rotated_rectangles,
            self.rotated_ellipses,
        ]
    }

    /// Counts one more shape of `kind`.
    pub fn add(&mut self, kind: ConvertingKind) {
        match kind {
            ConvertingKind::Polygon => self.polygons += 1,
            ConvertingKind::Star => self.stars += 1,
            ConvertingKind::RotatedRectangle => self.rotated_rectangles += 1,
            ConvertingKind::RotatedEllipse => self.rotated_ellipses += 1,
        }
    }
}

/// Whether `rotation` is a multiple of 90 degrees.
fn is_quarter_turn(rotation: Angle) -> bool {
    let quarters = rotation.as_radians() / std::f64::consts::FRAC_PI_2;
    (quarters - quarters.round()).abs() * std::f64::consts::FRAC_PI_2 <= QUARTER_TURN_TOLERANCE_RAD
}

/// Whether an ellipse frame is a circle: the one test the handles and the scale of
/// a group share, so that a circle never converts yet stretches in its own turned
/// frame.
pub(crate) fn is_circle(frame: &EllipseFrame) -> bool {
    (frame.rx.as_mm() - frame.ry.as_mm()).abs() < CIRCLE_TOLERANCE_MM
}

/// The kind of `object` if a stretch along axes at `axes` would turn it into a
/// path, `None` if it keeps its kind (`adrs.md` decision 8: the one predicate that
/// decides). A path never converts; a polygon and a star always do (they store one
/// radius); a rectangle or ellipse does unless its `rotation` minus `axes` is a
/// quarter turn; a circle never does (it becomes an ellipse). The group passes
/// `axes` 0; a single object passes its own box angle, which a rectangle or
/// ellipse shares.
#[must_use]
pub(crate) fn converting_kind(object: &ObjectSnapshot, axes: Angle) -> Option<ConvertingKind> {
    let ObjectSnapshot::Primitive(primitive) = object else {
        return None;
    };
    let relative = Angle::from_radians(primitive.rotation.as_radians() - axes.as_radians());
    match &primitive.shape {
        Shape::Polygon { .. } => Some(ConvertingKind::Polygon),
        Shape::Star { .. } => Some(ConvertingKind::Star),
        Shape::Rect { .. } => {
            (!is_quarter_turn(relative)).then_some(ConvertingKind::RotatedRectangle)
        }
        Shape::Ellipse { frame } => (!is_quarter_turn(relative) && !is_circle(frame))
            .then_some(ConvertingKind::RotatedEllipse),
    }
}

impl ConversionCounts {
    /// What an edge stretch of the one object `object` turns into a path (criterion
    /// 56): one polygon or one star, nothing for any other object.
    #[must_use]
    pub fn of_single(object: &ObjectSnapshot) -> Self {
        let mut counts = Self::default();
        if let Some(kind @ (ConvertingKind::Polygon | ConvertingKind::Star)) =
            converting_kind(object, Angle::from_radians(0.0))
        {
            counts.add(kind);
        }
        counts
    }
}

/// The path a primitive becomes: the outline "Object to path" gives it, under the
/// same id and with the same style, its anchors minted by `minter`, one closed
/// outline, and the shown angle (a polygon's or star's frame angle plus its
/// register) as `rotation`. The one function behind "Object to path" and the
/// conversion of a stretch, so the two cannot differ (`adrs.md` decision 8).
#[must_use]
pub fn primitive_as_path(
    primitive: &PrimitiveSnapshot,
    minter: &mut AnchorIdMinter,
) -> PathSnapshot {
    let anchors = outline_of_rotated(&primitive.shape, primitive.rotation)
        .into_iter()
        .map(|anchor| AnchorSnapshot {
            id: minter.mint(),
            point: anchor.point,
            handle_in: anchor.handle_in,
            handle_out: anchor.handle_out,
            kind: anchor.kind,
        })
        .collect();
    PathSnapshot {
        id: primitive.id,
        closed: true,
        style: primitive.style.clone(),
        anchors,
        extra_subpaths: Vec::new(),
        rotation: ObjectSnapshot::Primitive(primitive.clone()).orientation(),
    }
}

/// `path` with every anchor id replaced by a fresh one from `minter`: the ids a
/// conversion carries from the press (built before a minter was at hand) become the
/// session's own before anything is previewed or written.
pub(crate) fn with_fresh_anchor_ids(path: &mut PathSnapshot, minter: &mut AnchorIdMinter) {
    for anchor in &mut path.anchors {
        anchor.id = minter.mint();
    }
}

/// The path a single polygon or star becomes if an edge drag or a typed size
/// stretches it (`specs/0019-multi-object-transform/` criterion 56), with the ids of
/// a local counter (see [`converted_paths`]); `None` for any other object.
#[must_use]
pub(crate) fn converted_polygon_or_star(object: &ObjectSnapshot) -> Option<PathSnapshot> {
    match object {
        ObjectSnapshot::Primitive(primitive)
            if matches!(primitive.shape, Shape::Polygon { .. } | Shape::Star { .. }) =>
        {
            Some(primitive_as_path(primitive, &mut AnchorIdMinter::new(0)))
        }
        _ => None,
    }
}

/// The paths the converting shapes of `starts` become if a stretch ends in a
/// commit, one entry per start, `None` for an object that keeps its kind. The
/// anchors carry the ids of a local counter, unique within the call and replaced by
/// the session's own before the first preview (`SelectTool::mint_conversion_ids`).
#[must_use]
pub(crate) fn converted_paths(starts: &[ObjectSnapshot]) -> Vec<Option<PathSnapshot>> {
    let mut placeholder = AnchorIdMinter::new(0);
    starts
        .iter()
        .map(|object| match object {
            ObjectSnapshot::Primitive(primitive)
                if converting_kind(object, Angle::from_radians(0.0)).is_some() =>
            {
                Some(primitive_as_path(primitive, &mut placeholder))
            }
            _ => None,
        })
        .collect()
}

/// The shapes a commit turned into paths: every start that is a primitive whose
/// result is a path, counted by the kind it had.
#[must_use]
pub(crate) fn converted_counts(
    starts: &[ObjectSnapshot],
    results: &[ObjectSnapshot],
) -> ConversionCounts {
    let mut counts = ConversionCounts::default();
    for (start, result) in starts.iter().zip(results) {
        if matches!(result, ObjectSnapshot::Path(_))
            && let Some(kind) = converting_kind(start, Angle::from_radians(0.0))
        {
            counts.add(kind);
        }
    }
    counts
}

/// For every id in `ids` that still names a live primitive, the outline
/// "Object to path" gives it ([`primitive_as_path`]) with that id, exactly the
/// shape `Document::convert_to_paths` takes. An id that no longer resolves to a
/// primitive (already deleted, or already converted) is silently skipped, the same
/// lazy-resolution stance `curvyo-ui-core`'s other multi-id operations take (ADR
/// 0009 §2); `convert_to_paths` itself still refuses the whole call on any id it
/// cannot resolve, so a genuinely stale id surviving to that point is reported
/// there, not swallowed twice. The path's `rotation` is written by
/// `convert_to_paths` itself, from the stored primitive.
#[must_use]
pub fn build_primitive_conversions(
    document: &Document,
    minter: &mut AnchorIdMinter,
    ids: &[NodeId],
) -> Vec<(NodeId, Vec<NewAnchor>)> {
    ids.iter()
        .filter_map(|&id| {
            let primitive = document.primitive(id)?;
            let path = primitive_as_path(&primitive, minter);
            let anchors = path
                .anchors
                .into_iter()
                .map(|anchor| NewAnchor {
                    id: anchor.id,
                    point: anchor.point,
                    handle_in: anchor.handle_in,
                    handle_out: anchor.handle_out,
                    kind: anchor.kind,
                })
                .collect();
            Some((id, anchors))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{
        AnchorKind, EllipseFrame, InnerRatio, Length, Point, PointCount, RectBounds, StarFrame,
    };

    fn turned(document: &Document, id: NodeId, degrees: f64) -> ObjectSnapshot {
        let rotated = document.object(id).expect("exists").rotated(
            Point::new(0.0, 0.0),
            Angle::from_radians(degrees.to_radians()),
        );
        document.rotate_object(&rotated).expect("rotates");
        document.object(id).expect("exists")
    }

    /// Decision 8: a path and a circle never convert; an aligned rectangle or ellipse
    /// keeps its kind; a polygon, a star and an off-axis rectangle or ellipse do.
    #[test]
    fn the_converting_kinds_follow_the_rule() {
        let document = Document::new(1);
        let rect = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(6.0),
        });
        let ellipse = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(3.0),
        });
        let circle = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(4.0),
            ry: Length::from_mm(4.0),
        });
        let star = document.create_star(
            StarFrame {
                center: Point::new(0.0, 0.0),
                radius: Length::from_mm(5.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).expect("count"),
            InnerRatio::new(0.4).expect("ratio"),
        );
        let zero = Angle::from_radians(0.0);
        let kind = |object: &ObjectSnapshot| converting_kind(object, zero);
        assert_eq!(kind(&document.object(rect).unwrap()), None);
        assert_eq!(
            kind(&turned(&document, rect, 90.0)),
            None,
            "a quarter turn is aligned"
        );
        assert_eq!(
            kind(&turned(&document, rect, 30.0)),
            Some(ConvertingKind::RotatedRectangle)
        );
        assert_eq!(kind(&document.object(ellipse).unwrap()), None);
        assert_eq!(
            kind(&turned(&document, ellipse, 30.0)),
            Some(ConvertingKind::RotatedEllipse)
        );
        assert_eq!(
            kind(&turned(&document, circle, 30.0)),
            None,
            "a circle never converts"
        );
        assert_eq!(
            kind(&document.object(star).unwrap()),
            Some(ConvertingKind::Star)
        );
        // Against its own box angle a turned rectangle keeps its kind (one object).
        let own = turned(&document, rect, 30.0);
        assert_eq!(converting_kind(&own, own.rotation()), None);
    }

    /// Criterion 53.1/53.2 and the debt item: the converted path has the id, style,
    /// closed outline of "Object to path" and the shown angle as its rotation.
    #[test]
    fn a_star_becomes_the_path_object_to_path_gives_it() {
        let document = Document::new(1);
        let id = document.create_star(
            StarFrame {
                center: Point::new(10.0, 10.0),
                radius: Length::from_mm(5.0),
                angle: Angle::from_radians(0.5),
            },
            PointCount::new(5).expect("count"),
            InnerRatio::new(0.4).expect("ratio"),
        );
        let ObjectSnapshot::Primitive(star) = document.object(id).expect("exists") else {
            panic!("a star");
        };
        let mut minter = AnchorIdMinter::new(3);
        let path = primitive_as_path(&star, &mut minter);
        assert_eq!(path.id, id);
        assert_eq!(path.style, star.style);
        assert!(path.closed && path.extra_subpaths.is_empty());
        assert_eq!(path.anchors.len(), 10);
        assert!((path.rotation.as_radians() - 0.5).abs() < 1e-12);
        // The same anchors "Object to path" writes.
        let conversions = build_primitive_conversions(&document, &mut minter, &[id]);
        for (a, b) in path.anchors.iter().zip(&conversions[0].1) {
            assert_eq!(
                (a.point, a.handle_in, a.handle_out),
                (b.point, b.handle_in, b.handle_out)
            );
        }
        let mut again = path;
        let first = again.anchors[0].id;
        with_fresh_anchor_ids(&mut again, &mut minter);
        assert_ne!(again.anchors[0].id, first);
    }

    /// AC17: one primitive's conversion carries its outline's exact
    /// anchor count/kind, with freshly minted, distinct ids.
    #[test]
    fn build_primitive_conversions_mints_fresh_ids_for_one_rect() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let mut minter = AnchorIdMinter::new(1);
        let conversions = build_primitive_conversions(&document, &mut minter, &[id]);
        assert_eq!(conversions.len(), 1);
        let (converted_id, anchors) = &conversions[0];
        assert_eq!(*converted_id, id);
        assert_eq!(anchors.len(), 4);
        assert!(anchors.iter().all(|a| a.kind == AnchorKind::Corner));
        let mut ids: Vec<_> = anchors.iter().map(|a| a.id).collect();
        ids.sort_by_key(|id| id.to_hex());
        ids.dedup();
        assert_eq!(ids.len(), 4, "every minted anchor id is distinct");
    }

    /// AC22: multiple primitives convert independently in the returned
    /// list, each with its own outline.
    #[test]
    fn build_primitive_conversions_handles_a_multi_object_selection() {
        let document = Document::new(1);
        let rect_id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let ellipse_id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let mut minter = AnchorIdMinter::new(1);
        let conversions =
            build_primitive_conversions(&document, &mut minter, &[rect_id, ellipse_id]);
        assert_eq!(conversions.len(), 2);
        assert_eq!(conversions[0].1.len(), 4, "rect: 4 corners");
        assert_eq!(conversions[1].1.len(), 4, "ellipse: 4 smooth nodes");
    }

    /// Acceptance criterion 17/21: converting a rotated primitive bakes
    /// the rotated outline into the new path's anchors, and the
    /// converted path keeps the original's `rotation` register.
    #[test]
    fn build_primitive_conversions_bakes_rotation_and_keeps_the_register() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(-5.0, -5.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .rotate_object(&document.object(id).expect("object exists").rotated(
                Point::new(0.0, 0.0),
                curvyo_document_core::Angle::from_radians(std::f64::consts::FRAC_PI_2),
            ))
            .expect("rotate");
        let mut minter = AnchorIdMinter::new(1);
        let conversions = build_primitive_conversions(&document, &mut minter, &[id]);
        document.convert_to_paths(&conversions).expect("convert");

        let path = document.path(id).expect("now a path, same id");
        assert!(
            (path.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "rotation register carried over"
        );
        // The unrotated top-left corner was (-5, -5); after a 90-degree
        // rotation about the origin it is now (5, -5).
        assert!(
            path.anchors
                .iter()
                .any(|a| (a.point.x - 5.0).abs() < 1e-6 && (a.point.y - (-5.0)).abs() < 1e-6),
            "anchors are baked into the rotated, absolute positions: {:?}",
            path.anchors
        );
    }

    /// A stale id (no longer a primitive) is skipped, not included.
    #[test]
    fn build_primitive_conversions_skips_a_stale_id() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(1.0),
            height: Length::from_mm(1.0),
        });
        let mut minter = AnchorIdMinter::new(1);
        document
            .convert_to_paths(&build_primitive_conversions(&document, &mut minter, &[id]))
            .expect("convert once");

        let conversions = build_primitive_conversions(&document, &mut minter, &[id]);
        assert!(conversions.is_empty(), "already a path, not a primitive");
    }
}
