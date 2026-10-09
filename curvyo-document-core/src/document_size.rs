//! `Document`'s size commands: resize with the centre fixed, and fit to
//! content (`specs/0015-document-size-and-rulers/`, criteria 17-27a).
//!
//! Each command is one commit that writes the size registers and moves every
//! object together, so no state exists in which only one of the two has
//! happened. Nothing here scales, crops, restyles or rotates an object.

use crate::document::{Document, KEY_HEIGHT_MM, KEY_WIDTH_MM, OBJECTS_TREE, ROOT_MAP};
use crate::objects::translate_meta;
use crate::paths::tree_id_of;
use crate::units::{DocumentSize, Length, Point, Vec2};

/// The smallest document side: 1 mm.
pub const MIN_DOCUMENT_MM: f64 = 1.0;

/// The largest document side: 100 000 mm (100 m), so a field refuses nonsense
/// without refusing a large CNC bed or a banner.
pub const MAX_DOCUMENT_MM: f64 = 100_000.0;

/// Two sizes or shifts closer than this are equal (1e-9 mm, the tolerance of
/// the typed move, `MOVE_EQUAL_EPSILON_MM`). It is also how far outside a
/// limit a size may be and still be accepted and clamped to it.
const EQUAL_EPSILON_MM: f64 = 1e-9;

/// Why a size command refused to apply. A refused command writes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DocumentSizeError {
    /// A size or a bound is infinite or not a number.
    #[error("the size is not a finite number")]
    NotFinite,
    /// A side is outside [`MIN_DOCUMENT_MM`] to [`MAX_DOCUMENT_MM`].
    #[error("the size is outside the allowed range")]
    OutOfRange,
    /// The content box's maximum corner lies before its minimum corner.
    #[error("the content bounds are reversed")]
    InvalidBounds,
}

/// Checks one side against the limits (with 1e-9 mm of tolerance) and clamps a
/// value within that tolerance onto the limit. The one place a typed size is
/// validated: [`Document::resize`] calls it, and so does the parser of the
/// Width and Height fields, so a field never accepts what the document
/// refuses (criterion 16).
///
/// # Errors
/// [`DocumentSizeError::NotFinite`] for an infinite or NaN length,
/// [`DocumentSizeError::OutOfRange`] for one outside
/// [`MIN_DOCUMENT_MM`]`..=`[`MAX_DOCUMENT_MM`] by more than 1e-9 mm.
pub fn validated_document_side(side: Length) -> Result<Length, DocumentSizeError> {
    let mm = side.as_mm();
    if !mm.is_finite() {
        return Err(DocumentSizeError::NotFinite);
    }
    if mm < MIN_DOCUMENT_MM - EQUAL_EPSILON_MM || mm > MAX_DOCUMENT_MM + EQUAL_EPSILON_MM {
        return Err(DocumentSizeError::OutOfRange);
    }
    Ok(Length::from_mm(mm.clamp(MIN_DOCUMENT_MM, MAX_DOCUMENT_MM)))
}

impl Document {
    /// Sets the document size and moves every object by half the change, so
    /// the centre of the document and every object's place relative to it
    /// stay as they were (criterion 17; the shift is
    /// `((w1 - w0) / 2, (h1 - h0) / 2)`). Nothing is scaled, cropped or
    /// rotated, also when the new size is smaller than the content
    /// (criterion 18).
    ///
    /// One commit, labelled `resize_document`, writes both size registers and
    /// moves all objects (criterion 19). A size equal to the current one
    /// within 1e-9 mm writes nothing and returns `Ok(false)`. A side within
    /// 1e-9 mm of a limit is accepted and clamped onto it.
    ///
    /// # Errors
    /// [`DocumentSizeError::NotFinite`] for an infinite or NaN side,
    /// [`DocumentSizeError::OutOfRange`] for a side outside
    /// [`MIN_DOCUMENT_MM`]`..=`[`MAX_DOCUMENT_MM`].
    pub fn resize(&self, size: DocumentSize) -> Result<bool, DocumentSizeError> {
        let size = DocumentSize::new(
            validated_document_side(size.width)?,
            validated_document_side(size.height)?,
        );
        let current = self.size();
        let shift = Vec2::new(
            (size.width.as_mm() - current.width.as_mm()) / 2.0,
            (size.height.as_mm() - current.height.as_mm()) / 2.0,
        );
        Ok(self.apply_size(size, shift, "resize_document"))
    }

    /// Sets the document to the extent of `content` (the axis-aligned box of
    /// all objects' outline geometry, computed by the caller because curve
    /// extremes need `curvyo-geometry-core`) with a margin of 0 mm, and moves
    /// every object so the box's top-left corner lies at (0, 0)
    /// (criteria 23 and 24). Unlike [`Document::resize`] the centre of the
    /// old document does not matter.
    ///
    /// A side of the box narrower than [`MIN_DOCUMENT_MM`] becomes 1 mm and
    /// the content is centred on it (criterion 25). One commit, labelled
    /// `fit_document_to_content` (criterion 27). If the size and every
    /// position would stay the same within 1e-9 mm, nothing is written and
    /// the result is `Ok(false)` (criterion 26). A document without objects
    /// is left as it is and also returns `Ok(false)` (criterion 22).
    ///
    /// # Errors
    /// [`DocumentSizeError::NotFinite`] for a non-finite bound,
    /// [`DocumentSizeError::InvalidBounds`] if `content.1` lies before
    /// `content.0` on an axis, [`DocumentSizeError::OutOfRange`] if the box is
    /// wider or taller than [`MAX_DOCUMENT_MM`] (criterion 27a).
    pub fn fit_to_content(&self, content: (Point, Point)) -> Result<bool, DocumentSizeError> {
        let (min, max) = content;
        if ![min.x, min.y, max.x, max.y].iter().all(|v| v.is_finite()) {
            return Err(DocumentSizeError::NotFinite);
        }
        let (extent_x, extent_y) = (max.x - min.x, max.y - min.y);
        if extent_x < 0.0 || extent_y < 0.0 {
            return Err(DocumentSizeError::InvalidBounds);
        }
        let size = DocumentSize::new(
            validated_document_side(Length::from_mm(extent_x.max(MIN_DOCUMENT_MM)))?,
            validated_document_side(Length::from_mm(extent_y.max(MIN_DOCUMENT_MM)))?,
        );
        if self.object_ids().is_empty() {
            return Ok(false);
        }
        let shift = Vec2::new(
            -min.x + (size.width.as_mm() - extent_x) / 2.0,
            -min.y + (size.height.as_mm() - extent_y) / 2.0,
        );
        Ok(self.apply_size(size, shift, "fit_document_to_content"))
    }

    /// Writes the size registers and shifts all objects by `shift` in one
    /// commit named `label`; writes nothing and returns `false` if neither
    /// the size nor any position would change.
    fn apply_size(&self, size: DocumentSize, shift: Vec2, label: &str) -> bool {
        let current = self.size();
        let size_same = (size.width.as_mm() - current.width.as_mm()).abs() <= EQUAL_EPSILON_MM
            && (size.height.as_mm() - current.height.as_mm()).abs() <= EQUAL_EPSILON_MM;
        let shift_zero = shift.x.abs() <= EQUAL_EPSILON_MM && shift.y.abs() <= EQUAL_EPSILON_MM;
        if size_same && shift_zero {
            return false;
        }
        // Both registers are set. Loro records an operation only for a value
        // that differs from the stored one, and a damaged stored side always
        // differs from the valid new one, so a damaged size is repaired by the
        // next resize or fit. Across peers each register merges on its own
        // (docs/technical-debt.md, "Resize and fit merge per field").
        let root = self.loro().get_map(ROOT_MAP);
        // invariant: the root map is attached and the values are plain f64.
        #[allow(clippy::unwrap_used)]
        {
            root.insert(KEY_WIDTH_MM, size.width.as_mm()).unwrap();
            root.insert(KEY_HEIGHT_MM, size.height.as_mm()).unwrap();
        }
        if !shift_zero {
            self.translate_all_objects(shift);
        }
        self.commit_with_label(label);
        true
    }

    /// Moves every object by `offset` without committing; the caller commits.
    fn translate_all_objects(&self, offset: Vec2) {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        for id in self.object_ids() {
            if let Ok(meta) = tree.get_meta(tree_id_of(id)) {
                translate_meta(&meta, offset);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::{AnchorId, AnchorKind, NewAnchor, NodeId};
    use crate::primitive_model::{
        EllipseFrame, InnerRatio, ObjectSnapshot, PointCount, RectBounds, StarFrame,
    };
    use crate::units::Angle;

    fn mm(value: f64) -> Length {
        Length::from_mm(value)
    }

    fn rect(document: &Document, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, y),
            width: mm(w),
            height: mm(h),
        })
    }

    fn line(document: &Document, from: (f64, f64), to: (f64, f64)) -> NodeId {
        document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(from.0, from.1)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(to.0, to.1)),
            ],
            false,
        )
    }

    fn changes(document: &Document) -> usize {
        document.loro().len_changes()
    }

    /// The commit message of the newest change.
    fn last_label(document: &Document) -> String {
        let vv = document.loro().oplog_vv();
        let peer = document.loro().peer_id();
        let end = vv.get(&peer).copied().expect("the peer has written");
        let change = document
            .loro()
            .get_change(loro::ID::new(peer, end - 1))
            .expect("the newest change exists");
        change.message().to_string()
    }

    /// Criterion 17's example: a 50 x 50 rectangle at (10, 10) in the A4
    /// document is at (55, 61.5) after 300 x 400, and still 50 x 50.
    #[test]
    fn resize_keeps_the_centre_fixed_for_the_spec_example() {
        let document = Document::new(1);
        let id = rect(&document, 10.0, 10.0, 50.0, 50.0);

        assert_eq!(
            document.resize(DocumentSize::from_mm(300.0, 400.0)),
            Ok(true)
        );

        assert_eq!(document.size(), DocumentSize::from_mm(300.0, 400.0));
        let Some(ObjectSnapshot::Primitive(primitive)) = document.object(id) else {
            panic!("a primitive");
        };
        let crate::primitive_model::Shape::Rect { bounds, .. } = primitive.shape else {
            panic!("a rect");
        };
        assert!((bounds.origin.x - 55.0).abs() < 1e-9);
        assert!((bounds.origin.y - 61.5).abs() < 1e-9);
        assert!((bounds.width.as_mm() - 50.0).abs() < 1e-9);
        assert!((bounds.height.as_mm() - 50.0).abs() < 1e-9);
    }

    /// Criteria 17 and 18 for every kind, rotated or not: each object is its
    /// old self translated by the half-change and nothing else.
    #[test]
    fn resize_moves_every_kind_rotated_or_not_by_half_the_change() {
        let document = Document::new(1);
        let curved = document.create_path(
            &[
                NewAnchor {
                    id: AnchorId::new(1, 1),
                    point: Point::new(5.0, 5.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(0.0, 30.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: Point::new(15.0, 5.0),
                    handle_in: Vec2::new(0.0, 30.0),
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Symmetric,
                },
            ],
            true,
        );
        let plain_rect = rect(&document, 10.0, 10.0, 50.0, 50.0);
        let turned_rect = rect(&document, 100.0, 20.0, 30.0, 10.0);
        let ellipse = document.create_ellipse(EllipseFrame {
            center: Point::new(70.0, 80.0),
            rx: mm(12.0),
            ry: mm(6.0),
        });
        let frame = StarFrame {
            center: Point::new(150.0, 150.0),
            radius: mm(20.0),
            angle: Angle::from_radians(0.3),
        };
        let polygon = document.create_polygon(frame, PointCount::new(6).unwrap());
        let star = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.4).unwrap(),
        );
        for id in [turned_rect, star] {
            let object = document.object(id).expect("exists");
            let pivot = Point::new(100.0, 100.0);
            document
                .rotate_object(&object.rotated(pivot, Angle::from_radians(0.7)))
                .expect("rotate");
        }
        let ids = [curved, plain_rect, turned_rect, ellipse, polygon, star];
        let before: Vec<_> = ids.iter().map(|id| document.object(*id).unwrap()).collect();

        document
            .resize(DocumentSize::from_mm(100.0, 50.0))
            .expect("valid");

        // (100 - 210) / 2 and (50 - 297) / 2
        let shift = Vec2::new(-55.0, -123.5);
        for (id, old) in ids.iter().zip(&before) {
            assert_eq!(
                document.object(*id).as_ref(),
                Some(&old.translated(shift)),
                "{id:?}"
            );
        }
        assert_eq!(document.size(), DocumentSize::from_mm(100.0, 50.0));
    }

    /// Criterion 19: one commit, named, holding both the size and the moves.
    #[test]
    fn resize_is_one_commit_named_resize_document() {
        let document = Document::new(1);
        rect(&document, 10.0, 10.0, 5.0, 5.0);
        line(&document, (0.0, 0.0), (1.0, 1.0));
        let before = changes(&document);

        document
            .resize(DocumentSize::from_mm(300.0, 400.0))
            .unwrap();

        assert_eq!(changes(&document), before + 1);
        assert_eq!(last_label(&document), "resize_document");
    }

    #[test]
    fn resizing_to_the_current_size_writes_nothing() {
        let document = Document::new(1);
        rect(&document, 10.0, 10.0, 5.0, 5.0);
        let before = changes(&document);

        assert_eq!(
            document.resize(DocumentSize::from_mm(210.0, 297.0)),
            Ok(false)
        );
        assert_eq!(
            document.resize(DocumentSize::from_mm(210.0 + 1e-10, 297.0 - 1e-10)),
            Ok(false)
        );

        assert_eq!(changes(&document), before);
    }

    /// Criterion 16 at the model: nothing is written for a bad size, however
    /// many objects exist.
    #[test]
    fn resize_refuses_bad_sizes_and_writes_nothing() {
        let document = Document::new(1);
        let id = rect(&document, 10.0, 10.0, 5.0, 5.0);
        let object = document.object(id);
        let before = changes(&document);

        for (w, h, error) in [
            (f64::NAN, 100.0, DocumentSizeError::NotFinite),
            (100.0, f64::INFINITY, DocumentSizeError::NotFinite),
            (0.999, 100.0, DocumentSizeError::OutOfRange),
            (100.0, 0.0, DocumentSizeError::OutOfRange),
            (-5.0, 100.0, DocumentSizeError::OutOfRange),
            (100.0, 100_000.001, DocumentSizeError::OutOfRange),
        ] {
            assert_eq!(
                document.resize(DocumentSize::from_mm(w, h)),
                Err(error),
                "{w} x {h}"
            );
        }

        assert_eq!(changes(&document), before);
        assert_eq!(document.size(), DocumentSize::default());
        assert_eq!(document.object(id), object);
    }

    /// The limits are inclusive, and a value within 1e-9 mm of one is
    /// accepted and clamped onto it (adrs.md decision 6).
    #[test]
    fn resize_accepts_the_limits_and_clamps_rounding_noise_onto_them() {
        let document = Document::new(1);
        assert_eq!(
            document.resize(DocumentSize::from_mm(1.0, 100_000.0)),
            Ok(true)
        );
        assert_eq!(document.size(), DocumentSize::from_mm(1.0, 100_000.0));

        assert_eq!(
            document.resize(DocumentSize::from_mm(1.0 - 5e-10, 100_000.0 + 5e-10)),
            Ok(false),
            "clamped onto the stored limits, so equal"
        );
        assert_eq!(
            document.resize(DocumentSize::from_mm(-1.0, 100_000.0)),
            Err(DocumentSizeError::OutOfRange)
        );
        document.resize(DocumentSize::from_mm(50.0, 50.0)).unwrap();
        document
            .resize(DocumentSize::from_mm(100_000.0 + 9e-10, 1.0 - 9e-10))
            .unwrap();
        assert_eq!(document.size(), DocumentSize::from_mm(100_000.0, 1.0));
    }

    /// Criterion 18: a size smaller than the content moves, never crops.
    #[test]
    fn resize_below_the_content_leaves_objects_whole_on_the_pasteboard() {
        let document = Document::new(1);
        let id = rect(&document, 0.0, 0.0, 200.0, 200.0);

        document.resize(DocumentSize::from_mm(10.0, 10.0)).unwrap();

        let Some(ObjectSnapshot::Primitive(primitive)) = document.object(id) else {
            panic!("a primitive");
        };
        let crate::primitive_model::Shape::Rect { bounds, .. } = primitive.shape else {
            panic!("a rect");
        };
        assert!((bounds.origin.x - -100.0).abs() < 1e-9);
        assert!((bounds.width.as_mm() - 200.0).abs() < 1e-9);
    }

    /// Criteria 23, 24: the box's top-left corner lands at (0, 0) and the
    /// size is the box's extent.
    #[test]
    fn fit_moves_the_box_to_the_origin_and_takes_its_extent() {
        let document = Document::new(1);
        let a = rect(&document, 30.0, 40.0, 20.0, 10.0);
        let b = line(&document, (-10.0, 5.0), (25.0, 80.0));
        let rect_before = document.object(a).unwrap();
        let before = changes(&document);

        let applied = document.fit_to_content((Point::new(-10.0, 5.0), Point::new(50.0, 80.0)));

        assert_eq!(applied, Ok(true));
        assert_eq!(changes(&document), before + 1);
        assert_eq!(last_label(&document), "fit_document_to_content");
        assert_eq!(document.size(), DocumentSize::from_mm(60.0, 75.0));
        let shift = Vec2::new(10.0, -5.0);
        assert_eq!(
            document.object(a).as_ref(),
            Some(&rect_before.translated(shift))
        );
        let Some(ObjectSnapshot::Path(path)) = document.object(b) else {
            panic!("a path");
        };
        assert_eq!(
            path.anchors[0].point,
            Point::new(-10.0, 5.0).translated(shift)
        );
        assert_eq!(
            path.anchors[1].point,
            Point::new(25.0, 80.0).translated(shift)
        );
    }

    /// Criterion 25: a line 100 mm wide and 0 mm tall fits to 100 x 1 mm and
    /// lies at y = 0.5.
    #[test]
    fn fit_gives_a_one_mm_minimum_and_centres_the_content_on_that_axis() {
        let document = Document::new(1);
        let id = line(&document, (0.0, 0.0), (100.0, 0.0));

        let applied = document.fit_to_content((Point::new(0.0, 0.0), Point::new(100.0, 0.0)));

        assert_eq!(applied, Ok(true));
        assert_eq!(document.size(), DocumentSize::from_mm(100.0, 1.0));
        let Some(ObjectSnapshot::Path(path)) = document.object(id) else {
            panic!("a path");
        };
        assert_eq!(path.anchors[0].point, Point::new(0.0, 0.5));
        assert_eq!(path.anchors[1].point, Point::new(100.0, 0.5));
    }

    /// Criterion 26: fitting a fitted document changes and commits nothing,
    /// also in the minimum-size case.
    #[test]
    fn fit_is_idempotent_and_writes_nothing_the_second_time() {
        let document = Document::new(1);
        line(&document, (3.0, 7.0), (103.0, 7.0));
        document
            .fit_to_content((Point::new(3.0, 7.0), Point::new(103.0, 7.0)))
            .unwrap();
        let before = changes(&document);
        let size = document.size();

        let again = document.fit_to_content((Point::new(0.0, 0.5), Point::new(100.0, 0.5)));

        assert_eq!(again, Ok(false));
        assert_eq!(changes(&document), before);
        assert_eq!(document.size(), size);
    }

    /// Criterion 27a: content over the maximum is refused with nothing
    /// changed; content exactly at it is accepted.
    #[test]
    fn fit_refuses_content_larger_than_the_maximum() {
        let document = Document::new(1);
        let id = line(&document, (0.0, 0.0), (100_001.0, 5.0));
        let object = document.object(id);
        let before = changes(&document);

        let refused = document.fit_to_content((Point::new(0.0, 0.0), Point::new(100_001.0, 5.0)));

        assert_eq!(refused, Err(DocumentSizeError::OutOfRange));
        assert_eq!(changes(&document), before);
        assert_eq!(document.size(), DocumentSize::default());
        assert_eq!(document.object(id), object);

        let exact = document.fit_to_content((Point::new(0.0, 0.0), Point::new(100_000.0, 5.0)));
        assert_eq!(exact, Ok(true));
        assert_eq!(document.size(), DocumentSize::from_mm(100_000.0, 5.0));
    }

    #[test]
    fn fit_refuses_non_finite_and_reversed_bounds() {
        let document = Document::new(1);
        line(&document, (0.0, 0.0), (1.0, 1.0));
        assert_eq!(
            document.fit_to_content((Point::new(f64::NAN, 0.0), Point::new(1.0, 1.0))),
            Err(DocumentSizeError::NotFinite)
        );
        assert_eq!(
            document.fit_to_content((Point::new(5.0, 0.0), Point::new(1.0, 1.0))),
            Err(DocumentSizeError::InvalidBounds)
        );
    }

    /// Criterion 22: with no objects the size cannot change by a fit.
    #[test]
    fn fit_on_a_document_without_objects_changes_nothing() {
        let document = Document::new(1);
        let before = changes(&document);

        let applied = document.fit_to_content((Point::new(0.0, 0.0), Point::new(5.0, 5.0)));

        assert_eq!(applied, Ok(false));
        assert_eq!(changes(&document), before);
        assert_eq!(document.size(), DocumentSize::default());
    }

    /// A resize and a fit leave a path's handles alone (they are relative).
    #[test]
    fn resize_and_fit_keep_path_handles_unchanged() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor {
                    id: AnchorId::new(1, 1),
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(3.0, 4.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 10.0)),
            ],
            false,
        );
        document.resize(DocumentSize::from_mm(50.0, 60.0)).unwrap();
        let Some(ObjectSnapshot::Path(path)) = document.object(id) else {
            panic!("a path");
        };
        assert_eq!(path.anchors[0].handle_out, Vec2::new(3.0, 4.0));
        assert_eq!(path.anchors[0].point, Point::new(-80.0, -118.5));
    }

    /// Criterion 13: missing, non-finite or out-of-range sides open as A4 on
    /// both axes, and reading writes nothing.
    #[test]
    fn damaged_sizes_read_as_a4_on_both_axes_without_writing() {
        for (width, height) in [
            (Some(f64::NAN), Some(100.0)),
            (Some(f64::INFINITY), Some(100.0)),
            (Some(0.5), Some(100.0)),
            (Some(100.0), Some(100_000.5)),
            (Some(-3.0), Some(100.0)),
            (None, Some(100.0)),
            (Some(100.0), None),
        ] {
            let document = Document::new(1);
            let root = document.loro().get_map(ROOT_MAP);
            match width {
                Some(v) => root.insert(KEY_WIDTH_MM, v).unwrap(),
                None => root.delete(KEY_WIDTH_MM).unwrap(),
            }
            match height {
                Some(v) => root.insert(KEY_HEIGHT_MM, v).unwrap(),
                None => root.delete(KEY_HEIGHT_MM).unwrap(),
            }
            document.loro().commit();
            let before = changes(&document);

            assert_eq!(
                document.size(),
                DocumentSize::default(),
                "{width:?} x {height:?}"
            );
            assert_eq!(changes(&document), before);
        }
    }

    #[test]
    fn a_text_side_reads_as_a4() {
        let document = Document::new(1);
        document
            .loro()
            .get_map(ROOT_MAP)
            .insert(KEY_WIDTH_MM, "wide")
            .unwrap();
        assert_eq!(document.size(), DocumentSize::default());
    }

    /// A resize repairs a damaged stored side: both registers are written.
    #[test]
    fn resize_after_a_damaged_side_stores_a_valid_size() {
        let document = Document::new(1);
        document
            .loro()
            .get_map(ROOT_MAP)
            .insert(KEY_WIDTH_MM, f64::NAN)
            .unwrap();
        document
            .resize(DocumentSize::from_mm(210.0, 400.0))
            .unwrap();
        assert_eq!(document.size(), DocumentSize::from_mm(210.0, 400.0));
    }

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    /// The document `size_damaged_v7.curvyo` is made from: a 50 x 50 mm
    /// rectangle at (10, 10) and a stored height of 100000.5 mm, just over
    /// the 100 000 mm limit, so the file is damaged in one axis only.
    fn damaged_size_document() -> Document {
        let document = Document::new(1);
        let _ = rect(&document, 10.0, 10.0, 50.0, 50.0);
        let root = document.loro().get_map(ROOT_MAP);
        root.insert(KEY_WIDTH_MM, 120.0).unwrap();
        root.insert(KEY_HEIGHT_MM, 100_000.5).unwrap();
        document.loro().commit();
        document
    }

    /// Run deliberately to regenerate `tests/fixtures/size_damaged_v7.curvyo`
    /// and `display_unit_in_v7.curvyo` (`cargo test -p curvyo-document-core
    /// generate_size_fixtures -- --ignored`). The committed bytes are what
    /// `tests/document_size.rs` pins.
    #[test]
    #[ignore = "run deliberately to regenerate tests/fixtures/*.curvyo, not on every `cargo test`"]
    fn generate_size_fixtures() {
        let bytes = crate::container::pack(&damaged_size_document(), "0.1.0").unwrap();
        std::fs::write(fixture_path("size_damaged_v7.curvyo"), bytes).unwrap();

        // A 50 x 50 mm rectangle at (10, 10), resized from A4 to US Letter
        // (215.9 x 279.4 mm, 8.5 x 11 in), which moves it by (2.95, -8.8), to
        // (12.95, 1.2); display unit inches.
        let document = Document::new(1);
        let _ = rect(&document, 10.0, 10.0, 50.0, 50.0);
        document
            .resize(DocumentSize::from_mm(215.9, 279.4))
            .unwrap();
        assert!(document.set_display_unit(crate::display_unit::DisplayUnit::In));
        let bytes = crate::container::pack(&document, "0.1.0").unwrap();
        std::fs::write(fixture_path("display_unit_in_v7.curvyo"), bytes).unwrap();
    }

    /// The golden really is damaged: the raw stored height is out of range
    /// (and the width is valid), so `size()` falling back to A4 is the damaged
    /// path and not a coincidence of a valid A4 file.
    #[test]
    fn the_damaged_golden_stores_a_height_over_the_limit() {
        let bytes = std::fs::read(fixture_path("size_damaged_v7.curvyo")).unwrap();
        let document = crate::container::unpack(2, &bytes).unwrap();
        let root = document.loro().get_map(ROOT_MAP);
        let stored = |key: &str| match root.get(key).unwrap().get_deep_value() {
            loro::LoroValue::Double(value) => value,
            other => panic!("not a double: {other:?}"),
        };
        assert!((stored(KEY_WIDTH_MM) - 120.0).abs() < f64::EPSILON);
        assert!(stored(KEY_HEIGHT_MM) > MAX_DOCUMENT_MM);
        assert_eq!(document.size(), DocumentSize::default());
    }
}
