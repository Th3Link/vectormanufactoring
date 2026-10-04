//! `Document`'s primitive-shape command methods
//! (`specs/primitive-shapes/adrs.md`, "the primitive schema" and "'object
//! to path' keeps the `NodeId`"). Mirrors [`crate::paths`]'s shape for
//! paths: each mutating method here ends in exactly one Loro commit
//! (ADR 0002 §9), and every id is resolved before the first write so a
//! refused multi-object call (`convert_to_paths`) never leaves a partial
//! edit behind.

use loro::TreeParentId;

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::node_exists;
use crate::path_model::{NewAnchor, NodeId};
use crate::primitive_model::{
    EllipseFrame, InnerRatio, ObjectSnapshot, PointCount, PrimitiveSnapshot, RectBounds, StarFrame,
};
use crate::shape_codec::{self, SHAPE_ELLIPSE, SHAPE_POLYGON, SHAPE_RECT, SHAPE_STAR};
use crate::units::Length;

/// Why a shape-editing [`Document`] method refused to apply.
///
/// Mirrors [`crate::path_model::PathEditError`]'s own framing: every
/// variant describes a caller error against a snapshot that is already
/// stale (ADR 0009 §2), not a defect in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShapeEditError {
    /// No object with this [`NodeId`] exists (deleted locally or by a
    /// collaborator, or already converted to a path, since the caller
    /// last read a snapshot).
    #[error("no such object")]
    NoSuchObject,
    /// The object exists but is a path, not a primitive — or (for
    /// `convert_to_paths`) is already a path.
    #[error("this object is not a primitive shape")]
    NotAPrimitive,
    /// The object exists and is a primitive, but not the shape this
    /// operation applies to (acceptance criterion 14: a polygon has no
    /// inner ratio to set).
    #[error("this operation does not apply to this primitive's shape")]
    WrongShape,
}

impl Document {
    /// Creates a new rectangle primitive with zero corner radius
    /// (acceptance criterion 1), becoming the selected object is the
    /// caller's job (ADR 0009 §2: selection is ephemeral `vecmanf-ui-
    /// core` state).
    ///
    /// # Panics
    /// Does not panic in practice: see [`crate::paths::Document::
    /// create_path`]'s own doc comment — the same reasoning applies to
    /// every `create_*` method below.
    #[must_use]
    pub fn create_rect(&self, bounds: RectBounds) -> NodeId {
        let id = self.create_primitive_node(SHAPE_RECT);
        let tree = self.loro().get_tree(OBJECTS_TREE);
        // invariant: this node was just created by `create_primitive_node`.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(tree_id_of(id)).unwrap();
        shape_codec::write_rect_bounds(&meta, bounds);
        shape_codec::write_corner_radius(&meta, Length::from_mm(0.0));
        self.commit_with_label("create_rect");
        id
    }

    /// Creates a new ellipse/circle primitive (acceptance criteria 7, 8).
    ///
    /// # Panics
    /// Does not panic in practice: see [`Document::create_rect`]'s own
    /// doc comment.
    #[must_use]
    pub fn create_ellipse(&self, frame: EllipseFrame) -> NodeId {
        let id = self.create_primitive_node(SHAPE_ELLIPSE);
        let tree = self.loro().get_tree(OBJECTS_TREE);
        // invariant: see `create_rect`.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(tree_id_of(id)).unwrap();
        shape_codec::write_ellipse_frame(&meta, frame);
        self.commit_with_label("create_ellipse");
        id
    }

    /// Creates a new regular polygon primitive (acceptance criterion 11).
    ///
    /// # Panics
    /// Does not panic in practice: see [`Document::create_rect`]'s own
    /// doc comment.
    #[must_use]
    pub fn create_polygon(&self, frame: StarFrame, point_count: PointCount) -> NodeId {
        let id = self.create_primitive_node(SHAPE_POLYGON);
        let tree = self.loro().get_tree(OBJECTS_TREE);
        // invariant: see `create_rect`.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(tree_id_of(id)).unwrap();
        shape_codec::write_star_frame(&meta, frame);
        shape_codec::write_point_count(&meta, point_count);
        self.commit_with_label("create_polygon");
        id
    }

    /// Creates a new star primitive (acceptance criterion 12).
    ///
    /// # Panics
    /// Does not panic in practice: see [`Document::create_rect`]'s own
    /// doc comment.
    #[must_use]
    pub fn create_star(
        &self,
        frame: StarFrame,
        point_count: PointCount,
        inner_ratio: InnerRatio,
    ) -> NodeId {
        let id = self.create_primitive_node(SHAPE_STAR);
        let tree = self.loro().get_tree(OBJECTS_TREE);
        // invariant: see `create_rect`.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(tree_id_of(id)).unwrap();
        shape_codec::write_star_frame(&meta, frame);
        shape_codec::write_point_count(&meta, point_count);
        shape_codec::write_inner_ratio(&meta, inner_ratio);
        self.commit_with_label("create_star");
        id
    }

    /// Creates a root tree node tagged `shape` and carrying this slice's
    /// shared placeholder style (acceptance criterion 16), common to
    /// every `create_*` method above. Leaves the commit to its caller,
    /// which still has shape-specific fields left to write.
    ///
    /// # Panics
    /// Does not panic in practice: only creates a root-level node and
    /// inserts known-valid keys into its freshly created meta map,
    /// neither of which Loro's API can reject.
    fn create_primitive_node(&self, shape: &str) -> NodeId {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        // invariant: creating a root-level node on a freshly obtained
        // tree handle cannot fail.
        #[allow(clippy::unwrap_used)]
        let tree_id = tree.create(TreeParentId::Root).unwrap();
        // invariant: reading the meta map of a node this call just
        // created cannot fail.
        #[allow(clippy::unwrap_used)]
        let meta = tree.get_meta(tree_id).unwrap();
        shape_codec::write_shape_tag(&meta, shape);
        shape_codec::write_primitive_style_fields(&meta);
        NodeId::from_parts(tree_id.peer, tree_id.counter)
    }

    /// Reads one object's full current data generically, or `None` if it
    /// no longer exists — the dispatch point `specs/primitive-shapes/
    /// adrs.md` names ("`Document::path(id)` returns `None` for a
    /// primitive node... reading goes through an object-level snapshot").
    #[must_use]
    pub fn object(&self, id: NodeId) -> Option<ObjectSnapshot> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let tree_id = tree_id_of(id);
        if !node_exists(&tree, tree_id) {
            return None;
        }
        let meta = tree.get_meta(tree_id).ok()?;
        match shape_codec::read_shape_tag(&meta) {
            Some(shape) => Some(ObjectSnapshot::Primitive(
                shape_codec::read_primitive_snapshot(id, &meta, &shape),
            )),
            None => Some(ObjectSnapshot::Path(crate::path_codec::read_path_snapshot(
                id, &meta,
            ))),
        }
    }

    /// Reads one primitive's full current data, or `None` if it no
    /// longer exists or is a path instead.
    #[must_use]
    pub fn primitive(&self, id: NodeId) -> Option<PrimitiveSnapshot> {
        match self.object(id)? {
            ObjectSnapshot::Primitive(snapshot) => Some(snapshot),
            ObjectSnapshot::Path(_) => None,
        }
    }

    /// Sets a rectangle's bounding box as one commit — a resize drag
    /// (acceptance criterion 3). Leaves `corner_radius` untouched; its
    /// *effective* value is reclamped on read, never rewritten here
    /// (`adrs.md`'s "clamped where it is evaluated" decision).
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] if `id` no longer exists;
    /// [`ShapeEditError::NotAPrimitive`] / [`ShapeEditError::WrongShape`]
    /// if it exists but is not a rectangle.
    pub fn set_rect_bounds(&self, id: NodeId, bounds: RectBounds) -> Result<(), ShapeEditError> {
        let meta = self.require_shape(id, SHAPE_RECT)?;
        shape_codec::write_rect_bounds(&meta, bounds);
        self.commit_with_label("set_rect_bounds");
        Ok(())
    }

    /// Sets a corner radius as entered — clamped only where it is later
    /// evaluated, never here (acceptance criteria 4, 5, 6) — on every
    /// named rectangle, as **one commit for the whole batch** (architect
    /// review: a resize-handle drag always names exactly one id, but
    /// "remove rounding" can name an entire multi-rectangle selection,
    /// and that must not cost one commit per object). Every id is
    /// resolved and confirmed to be a rectangle before any of them is
    /// written, so one unknown or non-rectangle id anywhere in `ids`
    /// refuses the whole call — the same contract
    /// [`Document::convert_to_paths`] already uses. A negative value is
    /// floored to zero defensively; nothing in this slice's UI can
    /// produce one.
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer
    /// exists; [`ShapeEditError::NotAPrimitive`] /
    /// [`ShapeEditError::WrongShape`] if any named id is not a
    /// rectangle.
    pub fn set_corner_radius(&self, ids: &[NodeId], radius: Length) -> Result<(), ShapeEditError> {
        let radius = Length::from_mm(radius.as_mm().max(0.0));
        let metas: Vec<_> = ids
            .iter()
            .map(|&id| self.require_shape(id, SHAPE_RECT))
            .collect::<Result<_, _>>()?;
        for meta in metas {
            shape_codec::write_corner_radius(&meta, radius);
        }
        self.commit_with_label("set_corner_radius");
        Ok(())
    }

    /// Sets an ellipse/circle's frame as one commit (acceptance
    /// criterion 9).
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] / [`ShapeEditError::NotAPrimitive`]
    /// / [`ShapeEditError::WrongShape`] if `id` is not an ellipse.
    pub fn set_ellipse_frame(&self, id: NodeId, frame: EllipseFrame) -> Result<(), ShapeEditError> {
        let meta = self.require_shape(id, SHAPE_ELLIPSE)?;
        shape_codec::write_ellipse_frame(&meta, frame);
        self.commit_with_label("set_ellipse_frame");
        Ok(())
    }

    /// Sets a polygon or star's frame (center, outer radius,
    /// orientation) as one commit — a uniform-scale resize (acceptance
    /// criterion 13).
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] / [`ShapeEditError::NotAPrimitive`]
    /// if `id` is neither a polygon nor a star.
    pub fn set_star_frame(&self, id: NodeId, frame: StarFrame) -> Result<(), ShapeEditError> {
        let meta = self.require_polygon_or_star(id)?;
        shape_codec::write_star_frame(&meta, frame);
        self.commit_with_label("set_star_frame");
        Ok(())
    }

    /// Sets a point count as one commit for the **whole batch**
    /// (architect review, same reasoning as [`Document::set_corner_
    /// radius`]: the tool-options bar's point-count stepper can apply
    /// to an entire multi-selection, acceptance criteria 10, 15),
    /// independent of `star_frame`/`inner_ratio` (`adrs.md` decision 2:
    /// separate registers). Every id is resolved and confirmed to be a
    /// polygon or a star before any of them is written.
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer
    /// exists; [`ShapeEditError::NotAPrimitive`] if any named id is
    /// neither a polygon nor a star.
    pub fn set_point_count(
        &self,
        ids: &[NodeId],
        point_count: PointCount,
    ) -> Result<(), ShapeEditError> {
        let metas: Vec<_> = ids
            .iter()
            .map(|&id| self.require_polygon_or_star(id))
            .collect::<Result<_, _>>()?;
        for meta in metas {
            shape_codec::write_point_count(&meta, point_count);
        }
        self.commit_with_label("set_point_count");
        Ok(())
    }

    /// Sets a star's inner/outer ratio as one commit for the **whole
    /// batch** (architect review, same reasoning as
    /// [`Document::set_corner_radius`], acceptance criteria 12, 14),
    /// independent of `star_frame` — the outer radius stays exactly as
    /// stored. Every id is resolved and confirmed to be a star before
    /// any of them is written.
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer
    /// exists; [`ShapeEditError::WrongShape`] if any named id is a
    /// plain polygon (acceptance criterion 14: "a polygon has no inner
    /// radius... distinct from its outer one") or anything else that
    /// is not a star.
    pub fn set_inner_ratio(&self, ids: &[NodeId], ratio: InnerRatio) -> Result<(), ShapeEditError> {
        let metas: Vec<_> = ids
            .iter()
            .map(|&id| self.require_shape(id, SHAPE_STAR))
            .collect::<Result<_, _>>()?;
        for meta in metas {
            shape_codec::write_inner_ratio(&meta, ratio);
        }
        self.commit_with_label("set_inner_ratio");
        Ok(())
    }

    /// Converts every named primitive to a path, in one commit for the
    /// whole call (acceptance criteria 17, 22) — a rewrite of each node
    /// in place, keeping its [`NodeId`] (`adrs.md`'s "'object to path'
    /// keeps the `NodeId`" decision): `shape` and every primitive
    /// parameter key are deleted, `closed = true` and `anchors` are
    /// written, and the style keys are left exactly as they are.
    ///
    /// Every id is resolved — and confirmed to currently name a
    /// primitive — before any of them is written, so one unknown or
    /// already-a-path id anywhere in `conversions` refuses the whole
    /// call rather than converting a prefix of it. `vecmanf-ui-core` is
    /// expected to have already filtered a mixed selection down to its
    /// primitives before calling this (`adrs.md`).
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer exists;
    /// [`ShapeEditError::NotAPrimitive`] if any named id is already a
    /// path.
    pub fn convert_to_paths(
        &self,
        conversions: &[(NodeId, Vec<NewAnchor>)],
    ) -> Result<(), ShapeEditError> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let metas: Vec<_> = conversions
            .iter()
            .map(|(id, _)| {
                let tree_id = tree_id_of(*id);
                if !node_exists(&tree, tree_id) {
                    return Err(ShapeEditError::NoSuchObject);
                }
                let meta = tree
                    .get_meta(tree_id)
                    .map_err(|_| ShapeEditError::NoSuchObject)?;
                if shape_codec::read_shape_tag(&meta).is_none() {
                    return Err(ShapeEditError::NotAPrimitive);
                }
                Ok(meta)
            })
            .collect::<Result<_, _>>()?;

        for (meta, (_, anchors)) in metas.into_iter().zip(conversions.iter()) {
            shape_codec::strip_primitive_keys(&meta);
            shape_codec::write_converted_path_fields(&meta, anchors);
        }
        self.commit_with_label("convert_to_paths");
        Ok(())
    }

    /// Looks up a primitive's meta map, refusing unless it both exists
    /// and currently has `shape == expected`.
    fn require_shape(&self, id: NodeId, expected: &str) -> Result<loro::LoroMap, ShapeEditError> {
        let meta = self.require_primitive(id)?;
        match shape_codec::read_shape_tag(&meta) {
            Some(shape) if shape == expected => Ok(meta),
            Some(_) => Err(ShapeEditError::WrongShape),
            None => Err(ShapeEditError::NotAPrimitive),
        }
    }

    /// Same as [`Document::require_shape`], but accepts either
    /// `"polygon"` or `"star"` — the operations whose parameter
    /// (`star_frame`, `point_count`) both shapes share.
    fn require_polygon_or_star(&self, id: NodeId) -> Result<loro::LoroMap, ShapeEditError> {
        let meta = self.require_primitive(id)?;
        match shape_codec::read_shape_tag(&meta) {
            Some(shape) if shape == SHAPE_POLYGON || shape == SHAPE_STAR => Ok(meta),
            Some(_) => Err(ShapeEditError::WrongShape),
            None => Err(ShapeEditError::NotAPrimitive),
        }
    }

    fn require_primitive(&self, id: NodeId) -> Result<loro::LoroMap, ShapeEditError> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let tree_id = tree_id_of(id);
        if !node_exists(&tree, tree_id) {
            return Err(ShapeEditError::NoSuchObject);
        }
        tree.get_meta(tree_id)
            .map_err(|_| ShapeEditError::NoSuchObject)
    }
}

fn tree_id_of(id: NodeId) -> loro::TreeID {
    loro::TreeID::new(id.peer, id.counter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::{AnchorId, AnchorKind};
    use crate::primitive_model::Shape;
    use crate::units::{Angle, Point};

    fn rect_bounds(x: f64, y: f64, w: f64, h: f64) -> RectBounds {
        RectBounds {
            origin: Point::new(x, y),
            width: Length::from_mm(w),
            height: Length::from_mm(h),
        }
    }

    #[test]
    fn create_rect_round_trips_zero_radius_bounds() {
        let document = Document::new(1);
        let bounds = rect_bounds(0.0, 0.0, 10.0, 20.0);
        let id = document.create_rect(bounds);
        let snapshot = document.primitive(id).expect("exists");
        assert_eq!(
            snapshot.shape,
            Shape::Rect {
                bounds,
                corner_radius: Length::from_mm(0.0)
            }
        );
        assert_eq!(snapshot.stroke, crate::path_model::Color::BLACK);
    }

    #[test]
    fn path_returns_none_for_a_primitive_and_object_returns_the_primitive_variant() {
        let document = Document::new(1);
        let id = document.create_rect(rect_bounds(0.0, 0.0, 10.0, 10.0));
        assert_eq!(document.path(id), None);
        assert!(matches!(
            document.object(id),
            Some(ObjectSnapshot::Primitive(_))
        ));
    }

    #[test]
    fn set_rect_bounds_changes_bounds_but_not_corner_radius() {
        let document = Document::new(1);
        let id = document.create_rect(rect_bounds(0.0, 0.0, 10.0, 10.0));
        document
            .set_corner_radius(&[id], Length::from_mm(3.0))
            .expect("set radius");
        document
            .set_rect_bounds(id, rect_bounds(5.0, 5.0, 20.0, 20.0))
            .expect("resize");
        let snapshot = document.primitive(id).expect("exists");
        let Shape::Rect {
            bounds,
            corner_radius,
        } = snapshot.shape
        else {
            panic!("expected a rect");
        };
        assert_eq!(bounds, rect_bounds(5.0, 5.0, 20.0, 20.0));
        assert!((corner_radius.as_mm() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_corner_radius_on_an_ellipse_is_refused() {
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let result = document.set_corner_radius(&[id], Length::from_mm(1.0));
        assert_eq!(result, Err(ShapeEditError::WrongShape));
    }

    #[test]
    fn set_inner_ratio_on_a_polygon_is_refused() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        };
        let id = document.create_polygon(frame, PointCount::new(5).unwrap());
        let result = document.set_inner_ratio(&[id], InnerRatio::new(0.5).unwrap());
        assert_eq!(result, Err(ShapeEditError::WrongShape));
    }

    #[test]
    fn set_point_count_keeps_frame_and_ratio_on_a_star() {
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(1.0, 2.0),
            radius: Length::from_mm(10.0),
            angle: Angle::from_radians(0.0),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        document
            .set_point_count(&[id], PointCount::new(7).unwrap())
            .expect("set count");
        let snapshot = document.primitive(id).expect("exists");
        let Shape::Star {
            frame: new_frame,
            point_count,
            inner_ratio,
        } = snapshot.shape
        else {
            panic!("expected a star");
        };
        assert_eq!(new_frame, frame, "AC15: size/orientation unchanged");
        assert_eq!(point_count.get(), 7);
        assert!(
            (inner_ratio.get() - 0.5).abs() < f64::EPSILON,
            "AC15: ratio unchanged"
        );
    }

    #[test]
    fn operations_on_an_unknown_id_are_refused() {
        let document = Document::new(1);
        let id = document.create_rect(rect_bounds(0.0, 0.0, 1.0, 1.0));
        document
            .convert_to_paths(&[(
                id,
                vec![
                    NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(1, 2), Point::new(1.0, 0.0)),
                ],
            )])
            .expect("convert");
        let result = document.set_rect_bounds(id, rect_bounds(0.0, 0.0, 2.0, 2.0));
        assert_eq!(result, Err(ShapeEditError::NotAPrimitive));
    }

    /// AC17: converting replaces the primitive with a path keeping the
    /// same id; the shape's own parameters no longer exist anywhere.
    #[test]
    fn convert_to_paths_keeps_the_node_id_and_removes_shape_params() {
        let document = Document::new(1);
        let id = document.create_rect(rect_bounds(0.0, 0.0, 10.0, 10.0));
        let anchors = vec![
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 3), Point::new(10.0, 10.0)),
            NewAnchor::corner(AnchorId::new(1, 4), Point::new(0.0, 10.0)),
        ];
        document
            .convert_to_paths(&[(id, anchors.clone())])
            .expect("convert");

        assert_eq!(document.primitive(id), None, "no shape params remain");
        let path = document.path(id).expect("now a path, same id");
        assert!(path.closed);
        assert_eq!(path.anchors.len(), 4);
        assert!(path.anchors.iter().all(|a| a.kind == AnchorKind::Corner));
    }

    /// AC22: two primitives selected together convert independently in
    /// one call, each ending up a path with its own anchors.
    #[test]
    fn convert_to_paths_converts_a_multi_selection_independently() {
        let document = Document::new(1);
        let rect_id = document.create_rect(rect_bounds(0.0, 0.0, 10.0, 10.0));
        let ellipse_id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let rect_anchors = vec![
            NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
        ];
        let ellipse_anchors = vec![
            NewAnchor::corner(AnchorId::new(1, 3), Point::new(5.0, 0.0)),
            NewAnchor::corner(AnchorId::new(1, 4), Point::new(-5.0, 0.0)),
        ];
        document
            .convert_to_paths(&[(rect_id, rect_anchors), (ellipse_id, ellipse_anchors)])
            .expect("convert both");
        assert_eq!(document.path(rect_id).expect("exists").anchors.len(), 2);
        assert_eq!(document.path(ellipse_id).expect("exists").anchors.len(), 2);
    }

    /// `convert_to_paths` refuses the whole batch, writing nothing, when
    /// one id in it is stale.
    #[test]
    fn convert_to_paths_refuses_the_whole_batch_on_one_stale_id() {
        let document = Document::new(1);
        let id = document.create_rect(rect_bounds(0.0, 0.0, 10.0, 10.0));
        let unknown = {
            let other = document.create_rect(rect_bounds(0.0, 0.0, 1.0, 1.0));
            document
                .convert_to_paths(&[(
                    other,
                    vec![
                        NewAnchor::corner(AnchorId::new(1, 9), Point::new(0.0, 0.0)),
                        NewAnchor::corner(AnchorId::new(1, 10), Point::new(1.0, 1.0)),
                    ],
                )])
                .expect("convert the throwaway");
            other
        };
        let result = document.convert_to_paths(&[
            (
                id,
                vec![
                    NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(1, 2), Point::new(1.0, 1.0)),
                ],
            ),
            (unknown, vec![]),
        ]);
        assert_eq!(result, Err(ShapeEditError::NotAPrimitive));
        assert!(
            document.primitive(id).is_some(),
            "untouched by the refused batch"
        );
    }
}
