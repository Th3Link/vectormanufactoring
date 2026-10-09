//! `Document`'s commands that write a rectangle's corner radii, split out of
//! [`crate::shapes`] so that module stays under the size limit
//! (`specs/0013-rectangle-corner-radii/adrs.md`, decisions 1, 4 and 7).
//!
//! Every command writes each corner register only if its value changes, so an
//! unchanged register is never rewritten (a rewrite is a new operation that
//! could beat a concurrent edit, ADR 0009 §3), and commits nothing when no
//! register changes.

use crate::corner_radii::{Corner, CornerRadii};
use crate::corner_radii_codec::write_corner_radii_if_changed;
use crate::document::Document;
use crate::path_model::NodeId;
use crate::primitive_model::RectBounds;
use crate::shape_codec::{self, SHAPE_RECT};
use crate::shapes::{ShapeEditError, write_stroke_width_if_changed};
use crate::units::Length;

/// The one input rule of every radius command: a radius that is not finite is
/// refused ([`ShapeEditError::InvalidRadius`], nothing written, so a saved
/// file can never hold a value that reopens as damaged), a negative one is
/// floored to zero (defence; nothing in the UI produces one).
fn checked(radii: CornerRadii) -> Result<CornerRadii, ShapeEditError> {
    let floor = |radius: Length| Length::from_mm(radius.as_mm().max(0.0));
    if !Corner::ALL
        .iter()
        .all(|&corner| radii.get(corner).as_mm().is_finite())
    {
        return Err(ShapeEditError::InvalidRadius);
    }
    Ok(CornerRadii {
        tl: floor(radii.tl),
        tr: floor(radii.tr),
        br: floor(radii.br),
        bl: floor(radii.bl),
    })
}

impl Document {
    /// Sets all four corner radii of every named rectangle to `radius`, as
    /// **one commit for the whole batch** (a resize-handle drag always names
    /// exactly one id, but "remove rounding" can name an entire
    /// multi-rectangle selection, and that must not cost one commit per
    /// object). The radius is stored as entered — clamped only where it is
    /// later evaluated, never here (acceptance criteria 4, 5, 6, 10). Every id
    /// is resolved and confirmed to be a rectangle before any of them is
    /// written, so one unknown or non-rectangle id anywhere in `ids` refuses
    /// the whole call — the same contract [`Document::convert_to_paths`]
    /// already uses. A negative value is floored to zero and a non-finite
    /// one refuses the whole call ([`ShapeEditError::InvalidRadius`]). A corner that
    /// already holds the value is not rewritten, and nothing is committed when
    /// no register changes.
    ///
    /// # Errors
    /// [`ShapeEditError::InvalidRadius`] for a NaN or infinite radius;
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer
    /// exists; [`ShapeEditError::NotAPrimitive`] /
    /// [`ShapeEditError::WrongShape`] if any named id is not a
    /// rectangle.
    pub fn set_corner_radius(&self, ids: &[NodeId], radius: Length) -> Result<(), ShapeEditError> {
        let radii = checked(CornerRadii::uniform(radius))?;
        let metas: Vec<_> = ids
            .iter()
            .map(|&id| self.require_shape(id, SHAPE_RECT))
            .collect::<Result<_, _>>()?;
        let mut changed = false;
        for meta in metas {
            changed |= write_corner_radii_if_changed(&meta, radii);
        }
        if changed {
            self.commit_with_label("set_corner_radius");
        }
        Ok(())
    }

    /// Sets each named rectangle's four corner radii to its own values as
    /// **one commit for the whole batch**: a drag, a typed entry or the Select
    /// bar's "Radius" field over rectangles of different sizes
    /// (`specs/0009-unified-object-editing/` criterion 21a). A corner whose stored
    /// radius already equals its value is not rewritten (an LWW rewrite of an
    /// unchanged value could beat a concurrent radius edit), so two peers
    /// editing different corners both survive, and nothing is committed when
    /// no register changes. Every id is resolved and confirmed to be a
    /// rectangle before any is written. A negative value is floored to zero, a non-finite one
    /// refuses the whole call.
    ///
    /// # Errors
    /// [`ShapeEditError::InvalidRadius`] for a NaN or infinite radius;
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer exists;
    /// [`ShapeEditError::NotAPrimitive`] / [`ShapeEditError::WrongShape`] if
    /// any named id is not a rectangle.
    pub fn set_corner_radii(&self, radii: &[(NodeId, CornerRadii)]) -> Result<(), ShapeEditError> {
        let metas: Vec<_> = radii
            .iter()
            .map(|&(id, radii)| {
                let radii = checked(radii)?;
                self.require_shape(id, SHAPE_RECT).map(|meta| (meta, radii))
            })
            .collect::<Result<_, _>>()?;
        let mut changed = false;
        for (meta, radii) in metas {
            changed |= write_corner_radii_if_changed(&meta, radii);
        }
        if changed {
            self.commit_with_label("set_corner_radii");
        }
        Ok(())
    }

    /// Resizes a rectangle's bounding box, corner radii and (optionally)
    /// stroke width together as **one commit** (`specs/0005-object-transform/
    /// adrs.md`'s resize-writes table: "frame, `corner_radius` (rect),
    /// `stroke_width`" — a Select-tool resize-handle drag, acceptance
    /// criteria 8, 9). `curvyo-ui-core` computes all values (including the
    /// local-frame mapping for a rotated object and the √(sx·sy)
    /// stroke/radius factor) before calling this — `stroke_width: None` leaves
    /// the stored stroke width untouched. This method is purely "write what was
    /// computed", the same split [`Document::set_rect_bounds`] already follows
    /// for a plain resize. With "Scale corner radius" off the caller hands the
    /// stored radii back and no corner register is written
    /// (`specs/0013-rectangle-corner-radii/adrs.md`, decision 7).
    ///
    /// # Errors
    /// [`ShapeEditError::InvalidRadius`] for a NaN or infinite radius (nothing
    /// is written; a negative one is floored to zero);
    /// [`ShapeEditError::NoSuchObject`] / [`ShapeEditError::NotAPrimitive`]
    /// / [`ShapeEditError::WrongShape`] if `id` is not a rectangle;
    /// [`ShapeEditError::InvalidStrokeWidth`] for a width that is not above
    /// zero.
    pub fn resize_rect(
        &self,
        id: NodeId,
        bounds: RectBounds,
        corner_radii: CornerRadii,
        stroke_width: Option<Length>,
    ) -> Result<(), ShapeEditError> {
        let corner_radii = checked(corner_radii)?;
        crate::shapes::check_stroke_width(stroke_width)?;
        let meta = self.require_shape(id, SHAPE_RECT)?;
        shape_codec::write_rect_bounds(&meta, bounds);
        write_corner_radii_if_changed(&meta, corner_radii);
        write_stroke_width_if_changed(&meta, stroke_width);
        self.commit_with_label("resize_rect");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::OBJECTS_TREE;
    use crate::primitive_model::Shape;
    use crate::units::Point;

    fn bounds(width: f64, height: f64) -> RectBounds {
        RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(width),
            height: Length::from_mm(height),
        }
    }

    fn radii(tl: f64, tr: f64, br: f64, bl: f64) -> CornerRadii {
        CornerRadii {
            tl: Length::from_mm(tl),
            tr: Length::from_mm(tr),
            br: Length::from_mm(br),
            bl: Length::from_mm(bl),
        }
    }

    fn stored(document: &Document, id: NodeId) -> CornerRadii {
        let Shape::Rect { corner_radii, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected a rectangle");
        };
        corner_radii
    }

    /// Two peers that opened the same document holding one rectangle.
    fn two_peers() -> (Document, Document, NodeId) {
        let a = Document::new(1);
        let id = a.create_rect(bounds(100.0, 100.0));
        let b = Document::from_loro_snapshot(2, &a.export_loro_snapshot().expect("snapshot"))
            .expect("peer B opens it");
        (a, b, id)
    }

    fn merge(a: &Document, b: &Document) {
        let from_b = b.loro().export(loro::ExportMode::all_updates()).expect("B");
        let from_a = a.loro().export(loro::ExportMode::all_updates()).expect("A");
        a.loro().import(&from_b).expect("A merges B");
        b.loro().import(&from_a).expect("B merges A");
    }

    fn ops(document: &Document) -> usize {
        document.loro().len_ops()
    }

    #[test]
    fn two_peers_editing_different_corners_both_survive_the_merge() {
        let (a, b, id) = two_peers();
        a.set_corner_radii(&[(id, radii(10.0, 0.0, 0.0, 0.0))])
            .expect("A edits TL");
        b.set_corner_radii(&[(id, radii(0.0, 0.0, 20.0, 0.0))])
            .expect("B edits BR");
        merge(&a, &b);
        for peer in [&a, &b] {
            assert_eq!(stored(peer, id), radii(10.0, 0.0, 20.0, 0.0));
        }
    }

    #[test]
    fn the_same_corner_is_last_writer_wins_and_both_peers_converge() {
        let (a, b, id) = two_peers();
        a.set_corner_radii(&[(id, radii(10.0, 0.0, 0.0, 0.0))])
            .expect("A");
        b.set_corner_radii(&[(id, radii(30.0, 0.0, 0.0, 0.0))])
            .expect("B");
        merge(&a, &b);
        let merged = stored(&a, id);
        assert_eq!(merged, stored(&b, id));
        assert!(
            merged.tl == Length::from_mm(10.0) || merged.tl == Length::from_mm(30.0),
            "one of the two writes wins"
        );
    }

    /// A node holding only the legacy `corner_radius` (a version-5 file),
    /// edited at two corners by two peers: the untouched corners fall back to
    /// the legacy value on both sides.
    #[test]
    fn a_legacy_node_edited_at_two_corners_by_two_peers_merges_cleanly() {
        let a = Document::new(1);
        let id = a.create_rect(bounds(100.0, 100.0));
        let tree = a.loro().get_tree(OBJECTS_TREE);
        let meta = tree
            .get_meta(loro::TreeID::new(id.peer, id.counter))
            .expect("meta");
        for key in [
            "corner_radius_tl",
            "corner_radius_tr",
            "corner_radius_br",
            "corner_radius_bl",
        ] {
            meta.delete(key).expect("delete");
        }
        meta.insert("corner_radius", 5.0_f64).expect("legacy key");
        a.commit_with_label("test: legacy node");
        assert_eq!(stored(&a, id), CornerRadii::uniform(Length::from_mm(5.0)));
        let b = Document::from_loro_snapshot(2, &a.export_loro_snapshot().expect("snapshot"))
            .expect("B opens");

        a.set_corner_radii(&[(id, radii(9.0, 5.0, 5.0, 5.0))])
            .expect("A edits TL");
        b.set_corner_radii(&[(id, radii(5.0, 5.0, 7.0, 5.0))])
            .expect("B edits BR");
        merge(&a, &b);
        for peer in [&a, &b] {
            assert_eq!(stored(peer, id), radii(9.0, 5.0, 7.0, 5.0));
        }
    }

    /// A resize with "Scale corner radius" off hands the stored radii back:
    /// no register is written, so a peer's corner edit survives it.
    #[test]
    fn a_resize_that_keeps_the_radii_does_not_beat_a_peers_corner_edit() {
        let (a, b, id) = two_peers();
        let before = stored(&a, id);
        a.resize_rect(id, bounds(150.0, 100.0), before, None)
            .expect("A resizes");
        b.set_corner_radii(&[(id, radii(0.0, 12.0, 0.0, 0.0))])
            .expect("B edits TR");
        merge(&a, &b);
        for peer in [&a, &b] {
            assert_eq!(stored(peer, id), radii(0.0, 12.0, 0.0, 0.0));
            let Shape::Rect { bounds, .. } = peer.primitive(id).expect("exists").shape else {
                panic!("rect");
            };
            assert!((bounds.width.as_mm() - 150.0).abs() < 1e-12, "resize kept");
        }
    }

    #[test]
    fn a_command_that_changes_no_register_writes_and_commits_nothing() {
        let document = Document::new(1);
        let id = document.create_rect(bounds(100.0, 100.0));
        document
            .set_corner_radii(&[(id, radii(1.0, 2.0, 3.0, 4.0))])
            .expect("set");
        let (changes, operations) = (document.loro().len_changes(), ops(&document));
        document
            .set_corner_radii(&[(id, radii(1.0, 2.0, 3.0, 4.0))])
            .expect("same values");
        document
            .set_corner_radius(&[id], Length::from_mm(0.0))
            .expect("changes four");
        assert_eq!(document.loro().len_changes(), changes + 1);
        assert_eq!(ops(&document), operations + 4);
        let (changes, operations) = (document.loro().len_changes(), ops(&document));
        document
            .set_corner_radius(&[id], Length::from_mm(0.0))
            .expect("unchanged");
        assert_eq!(document.loro().len_changes(), changes);
        assert_eq!(ops(&document), operations);
    }

    #[test]
    fn a_linked_write_that_changes_two_corners_writes_two_registers() {
        let document = Document::new(1);
        let id = document.create_rect(bounds(100.0, 100.0));
        document
            .set_corner_radii(&[(id, radii(5.0, 5.0, 0.0, 0.0))])
            .expect("set");
        let operations = ops(&document);
        document
            .set_corner_radius(&[id], Length::from_mm(5.0))
            .expect("all four to 5");
        assert_eq!(ops(&document), operations + 2, "BR and BL only");
        assert_eq!(stored(&document, id), radii(5.0, 5.0, 5.0, 5.0));
    }

    #[test]
    fn a_resize_writes_only_the_radii_that_change() {
        let document = Document::new(1);
        let id = document.create_rect(bounds(100.0, 100.0));
        document
            .set_corner_radii(&[(id, radii(2.0, 4.0, 6.0, 0.0))])
            .expect("set");
        let operations = ops(&document);
        // "Scale corner radius" on with factor 2: BL stays 0 and is not rewritten.
        document
            .resize_rect(id, bounds(200.0, 200.0), radii(4.0, 8.0, 12.0, 0.0), None)
            .expect("resize");
        // bounds + three radii.
        assert_eq!(ops(&document), operations + 4);
        assert_eq!(stored(&document, id), radii(4.0, 8.0, 12.0, 0.0));
    }

    #[test]
    fn a_negative_radius_is_floored_to_zero() {
        let document = Document::new(1);
        let id = document.create_rect(bounds(100.0, 100.0));
        document
            .set_corner_radii(&[(id, radii(-1.0, 2.0, -3.0, 4.0))])
            .expect("set");
        assert_eq!(stored(&document, id), radii(0.0, 2.0, 0.0, 4.0));
        document
            .set_corner_radius(&[id], Length::from_mm(-5.0))
            .expect("set all");
        assert_eq!(stored(&document, id), radii(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn a_radius_is_stored_as_entered_not_clamped() {
        let document = Document::new(1);
        let id = document.create_rect(bounds(10.0, 10.0));
        document
            .set_corner_radii(&[(id, radii(500.0, 500.0, 500.0, 500.0))])
            .expect("set");
        assert_eq!(stored(&document, id), radii(500.0, 500.0, 500.0, 500.0));
    }

    #[test]
    fn one_unknown_id_refuses_the_whole_batch_before_any_write() {
        let document = Document::new(1);
        let id = document.create_rect(bounds(10.0, 10.0));
        let ellipse = document.create_ellipse(crate::primitive_model::EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let operations = ops(&document);
        let result = document.set_corner_radii(&[
            (id, radii(1.0, 1.0, 1.0, 1.0)),
            (ellipse, radii(1.0, 1.0, 1.0, 1.0)),
        ]);
        assert_eq!(result, Err(ShapeEditError::WrongShape));
        assert_eq!(ops(&document), operations);
        assert_eq!(stored(&document, id), radii(0.0, 0.0, 0.0, 0.0));
    }

    /// The input rule: a non-finite radius refuses the call and writes nothing
    /// (so a saved file cannot reopen as damaged); a negative one is floored.
    #[test]
    fn a_non_finite_radius_is_refused_and_nothing_is_written() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let document = Document::new(1);
            let id = document.create_rect(bounds(100.0, 100.0));
            let (changes, operations) = (document.loro().len_changes(), ops(&document));
            let bad_radii = radii(1.0, bad, 3.0, 4.0);
            assert_eq!(
                document.set_corner_radius(&[id], Length::from_mm(bad)),
                Err(ShapeEditError::InvalidRadius)
            );
            assert_eq!(
                document.set_corner_radii(&[(id, bad_radii)]),
                Err(ShapeEditError::InvalidRadius)
            );
            assert_eq!(
                document.resize_rect(id, bounds(50.0, 50.0), bad_radii, None),
                Err(ShapeEditError::InvalidRadius)
            );
            assert_eq!(document.loro().len_changes(), changes, "{bad}");
            assert_eq!(ops(&document), operations, "{bad}");
            assert_eq!(stored(&document, id), radii(0.0, 0.0, 0.0, 0.0));
            // The file the document would save still opens.
            let bytes = crate::pack(&document, "t").expect("pack");
            assert!(crate::unpack(2, &bytes).is_ok());
        }
    }

    #[test]
    fn a_non_finite_radius_in_a_batch_refuses_every_rectangle() {
        let document = Document::new(1);
        let a = document.create_rect(bounds(100.0, 100.0));
        let b = document.create_rect(bounds(100.0, 100.0));
        let operations = ops(&document);
        let result = document.set_corner_radii(&[
            (a, radii(5.0, 5.0, 5.0, 5.0)),
            (b, radii(1.0, f64::NAN, 1.0, 1.0)),
        ]);
        assert_eq!(result, Err(ShapeEditError::InvalidRadius));
        assert_eq!(ops(&document), operations);
        assert_eq!(stored(&document, a), radii(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn resize_rect_floors_a_negative_radius() {
        let document = Document::new(1);
        let id = document.create_rect(bounds(100.0, 100.0));
        document
            .resize_rect(id, bounds(50.0, 50.0), radii(-2.0, 3.0, -1.0, 0.0), None)
            .expect("resize");
        assert_eq!(stored(&document, id), radii(0.0, 3.0, 0.0, 0.0));
    }
}
