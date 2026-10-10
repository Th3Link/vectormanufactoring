//! `Document::transform_objects`: the one commit that writes the result of a
//! gesture on several objects at once (`specs/0019-multi-object-transform/`
//! criterion 31, `adrs.md` decision 1). Split out of [`crate::objects`], which
//! holds the commands whose arithmetic is the same for every object.

use loro::TreeID;

use crate::document::{Document, OBJECTS_TREE};
use crate::objects::ObjectEditError;
use crate::path_codec::{
    self, KEY_HANDLE_IN, KEY_HANDLE_OUT, KEY_POINT, anchor_map_at, node_exists, read_point,
    read_vec2, write_point, write_vec2,
};
use crate::path_model::AnchorSnapshot;
use crate::primitive_model::{ObjectSnapshot, Shape};
use crate::shape_codec;
use crate::shape_radii::checked;
use crate::shapes::write_stroke_width_if_changed;
use crate::style_codec::stroke_width_is_writable;
use crate::subpath_codec::anchor_positions;
use crate::units::Point;

/// One object of the call, resolved against the document before any write.
enum Planned<'a> {
    Primitive {
        meta: loro::LoroMap,
        current: Shape,
        result: &'a ObjectSnapshot,
    },
    Path {
        meta: loro::LoroMap,
        anchors: Vec<(loro::LoroMovableList, usize, &'a AnchorSnapshot)>,
        result: &'a ObjectSnapshot,
    },
}

/// Whether `a` and `b` are the same kind of shape.
fn same_kind(a: &Shape, b: &Shape) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Whether the frame register of `a` and of `b` hold the same numbers.
fn same_frame(a: &Shape, b: &Shape) -> bool {
    match (a, b) {
        (Shape::Rect { bounds: x, .. }, Shape::Rect { bounds: y, .. }) => x == y,
        (Shape::Ellipse { frame: x }, Shape::Ellipse { frame: y }) => x == y,
        (Shape::Polygon { frame: x, .. }, Shape::Polygon { frame: y, .. })
        | (Shape::Star { frame: x, .. }, Shape::Star { frame: y, .. }) => x == y,
        _ => false,
    }
}

impl Document {
    /// Writes the resolved results of a gesture on several objects in **one
    /// commit**: `results` are the objects as the gesture leaves them (the same
    /// snapshots the live preview drew). Per object only what differs from the
    /// stored value is written, so an unchanged register is never rewritten
    /// (a rewrite could beat a concurrent edit, ADR 0009 section 3): a primitive's
    /// frame, a rectangle's corner radii, `rotation`, a path's anchor points and
    /// handle vectors, and, with `write_stroke_width`, the stroke width. Nothing
    /// else is written, and nothing is committed when nothing differs.
    ///
    /// Every object, its kind and every anchor are resolved before the first
    /// write, so a stale id, a changed kind, a missing anchor or an invalid
    /// value refuses the whole call and writes nothing: no reader, saved file or
    /// peer sees some objects transformed and others not.
    ///
    /// # Errors
    /// [`ObjectEditError::NoSuchObject`] if an object or an anchor no longer
    /// exists, or an object is no longer of the kind its result says;
    /// [`ObjectEditError::InvalidStrokeWidth`] for a width that is not finite and
    /// above zero (only when `write_stroke_width`);
    /// [`ObjectEditError::InvalidRadius`] for a corner radius that is not finite;
    /// [`ObjectEditError::NonFiniteGeometry`] for any other number of a result (a
    /// position, size, handle or rotation) that is not finite: a NaN or an
    /// infinity would be saved as `null`. The gesture layer already refuses such
    /// results; this is the second guard (`docs/technical-debt.md`: the older
    /// single-object writers have none).
    pub fn transform_objects(
        &self,
        results: &[ObjectSnapshot],
        write_stroke_width: bool,
    ) -> Result<(), ObjectEditError> {
        if write_stroke_width
            && !results
                .iter()
                .all(|r| stroke_width_is_writable(Some(r.style().stroke.width)))
        {
            return Err(ObjectEditError::InvalidStrokeWidth);
        }
        if !results.iter().all(is_finite) {
            return Err(ObjectEditError::NonFiniteGeometry);
        }
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let planned = results
            .iter()
            .map(|result| {
                let tree_id = TreeID::new(result.id().peer, result.id().counter);
                if !node_exists(&tree, tree_id) {
                    return Err(ObjectEditError::NoSuchObject);
                }
                let meta = tree
                    .get_meta(tree_id)
                    .map_err(|_| ObjectEditError::NoSuchObject)?;
                match (result, shape_codec::read_shape_tag(&meta)) {
                    (ObjectSnapshot::Primitive(primitive), Some(tag)) => {
                        let current = shape_codec::read_shape(&meta, &tag)
                            .filter(|current| same_kind(current, &primitive.shape))
                            .ok_or(ObjectEditError::NoSuchObject)?;
                        if let Shape::Rect { corner_radii, .. } = primitive.shape {
                            checked(corner_radii).map_err(|_| ObjectEditError::InvalidRadius)?;
                        }
                        Ok(Planned::Primitive {
                            meta,
                            current,
                            result,
                        })
                    }
                    (ObjectSnapshot::Path(path), None) => {
                        let positions = anchor_positions(&meta);
                        let anchors = path
                            .all_anchors()
                            .map(|anchor| {
                                positions
                                    .get(anchor.id)
                                    .map(|(list, index)| (list, index, anchor))
                                    .ok_or(ObjectEditError::NoSuchObject)
                            })
                            .collect::<Result<_, _>>()?;
                        Ok(Planned::Path {
                            meta,
                            anchors,
                            result,
                        })
                    }
                    _ => Err(ObjectEditError::NoSuchObject),
                }
            })
            .collect::<Result<Vec<_>, ObjectEditError>>()?;
        for plan in &planned {
            let (meta, result) = match plan {
                Planned::Primitive {
                    meta,
                    current,
                    result,
                } => {
                    write_primitive(meta, current, result);
                    (meta, result)
                }
                Planned::Path {
                    meta,
                    anchors,
                    result,
                } => {
                    write_anchors(anchors);
                    (meta, result)
                }
            };
            if path_codec::read_rotation(meta) != result.rotation().normalized() {
                path_codec::write_rotation(meta, result.rotation());
            }
            if write_stroke_width {
                write_stroke_width_if_changed(meta, Some(result.style().stroke.width));
            }
        }
        self.commit_with_label("transform_objects");
        Ok(())
    }
}

/// Whether every number of `object`'s geometry (shape, rotation, anchors and
/// handles; not the corner radii) is finite.
fn is_finite(object: &ObjectSnapshot) -> bool {
    let point = |p: Point| p.x.is_finite() && p.y.is_finite();
    let rotation = object.rotation().as_radians().is_finite();
    match object {
        ObjectSnapshot::Primitive(primitive) => {
            rotation
                && match &primitive.shape {
                    // The corner radii have their own refusal (`InvalidRadius`).
                    Shape::Rect { bounds, .. } => {
                        point(bounds.origin)
                            && bounds.width.as_mm().is_finite()
                            && bounds.height.as_mm().is_finite()
                    }
                    Shape::Ellipse { frame } => {
                        point(frame.center)
                            && frame.rx.as_mm().is_finite()
                            && frame.ry.as_mm().is_finite()
                    }
                    Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => {
                        point(frame.center)
                            && frame.radius.as_mm().is_finite()
                            && frame.angle.as_radians().is_finite()
                    }
                }
        }
        ObjectSnapshot::Path(path) => {
            rotation
                && path.all_anchors().all(|a| {
                    point(a.point)
                        && a.handle_in.x.is_finite()
                        && a.handle_in.y.is_finite()
                        && a.handle_out.x.is_finite()
                        && a.handle_out.y.is_finite()
                })
        }
    }
}

/// Writes a primitive's frame and corner radii where they differ.
fn write_primitive(meta: &loro::LoroMap, current: &Shape, result: &ObjectSnapshot) {
    let ObjectSnapshot::Primitive(primitive) = result else {
        return;
    };
    if !same_frame(current, &primitive.shape) {
        shape_codec::write_shape_frame(meta, &primitive.shape);
    }
    if let Shape::Rect { corner_radii, .. } = primitive.shape
        && let Ok(radii) = checked(corner_radii)
    {
        crate::corner_radii_codec::write_corner_radii_if_changed(meta, radii);
    }
}

/// Writes every anchor point and handle vector that differs.
fn write_anchors(anchors: &[(loro::LoroMovableList, usize, &AnchorSnapshot)]) {
    for (list, index, anchor) in anchors {
        let map = anchor_map_at(list, *index);
        if read_point(&map, KEY_POINT) != anchor.point {
            write_point(&map, KEY_POINT, anchor.point);
        }
        if read_vec2(&map, KEY_HANDLE_IN) != anchor.handle_in {
            write_vec2(&map, KEY_HANDLE_IN, anchor.handle_in);
        }
        if read_vec2(&map, KEY_HANDLE_OUT) != anchor.handle_out {
            write_vec2(&map, KEY_HANDLE_OUT, anchor.handle_out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::{AnchorId, NewAnchor};
    use crate::primitive_model::{PrimitiveSnapshot, RectBounds};
    use crate::units::{Angle, Length, Point, Vec2};

    fn rect(document: &Document) -> crate::path_model::NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        })
    }

    fn ops(document: &Document) -> usize {
        document.loro().len_ops()
    }

    /// ADR 0009 section 3: only the registers that differ are written. A move
    /// writes the frame alone; a turn about the centre writes `rotation` alone;
    /// the stroke width and the radii stay out of both.
    #[test]
    fn only_the_registers_that_differ_are_written() {
        let document = Document::new(1);
        let id = rect(&document);
        let object = document.object(id).expect("exists");
        let before = ops(&document);
        document
            .transform_objects(&[object.translated(Vec2::new(4.0, 0.0))], true)
            .expect("moves");
        assert_eq!(ops(&document) - before, 1, "the frame register only");
        let object = document.object(id).expect("exists");
        let centre = Point::new(9.0, 5.0);
        let before = ops(&document);
        document
            .transform_objects(&[object.rotated(centre, Angle::from_radians(0.5))], true)
            .expect("turns");
        assert_eq!(ops(&document) - before, 1, "the rotation register only");
        let before = ops(&document);
        let same = document.object(id).expect("exists");
        document
            .transform_objects(&[same], true)
            .expect("no change");
        assert_eq!(ops(&document), before, "an equal result writes nothing");
    }

    /// The same for a path: an anchor whose point does not change is not
    /// rewritten, and handle vectors that do not change stay.
    #[test]
    fn a_path_writes_only_the_anchors_that_changed() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
            ],
            false,
        );
        let ObjectSnapshot::Path(mut path) = document.object(id).expect("exists") else {
            panic!("a path");
        };
        path.anchors[1].point = Point::new(12.0, 0.0);
        let before = ops(&document);
        document
            .transform_objects(&[ObjectSnapshot::Path(path)], false)
            .expect("writes");
        assert_eq!(ops(&document) - before, 1, "one anchor point");
    }

    /// A non-finite corner radius refuses the call before any write.
    #[test]
    fn a_non_finite_radius_is_refused() {
        let document = Document::new(1);
        let id = rect(&document);
        let ObjectSnapshot::Primitive(mut primitive) = document.object(id).expect("exists") else {
            panic!("a primitive");
        };
        if let Shape::Rect { corner_radii, .. } = &mut primitive.shape {
            corner_radii.tl = Length::from_mm(f64::NAN);
        }
        let before = ops(&document);
        assert_eq!(
            document.transform_objects(
                &[ObjectSnapshot::Primitive(PrimitiveSnapshot { ..primitive })],
                false
            ),
            Err(ObjectEditError::InvalidRadius)
        );
        assert_eq!(ops(&document), before);
    }
}
