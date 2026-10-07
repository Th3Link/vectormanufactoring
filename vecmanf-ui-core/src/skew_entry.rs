//! The typed skew entry of a path
//! (`specs/edit-interaction-polish/specification.md`, criteria 9 to 14, 58):
//! its state, validation and commit. The typed angle goes through
//! the skew drag's own arithmetic (`skew_by_angle`, through its unchecked
//! half), so a typed value and a
//! dragged one cannot disagree (`adrs.md`, decision 3). The DOM chip only
//! holds the text, the caret and the focus.

use vecmanf_document_core::{Angle, Document, ObjectSnapshot, Point};

use crate::oriented_box::OrientedBox;
use crate::skew_math::{MIN_SKEW_LEVER_MM, skew_frame};
use crate::transform_commit::{commit_gesture, is_sane, same_within_tolerance};
use crate::transform_drag::{ScaleModes, skewed_unchecked};
use crate::transform_entry::{
    EntryField, EntryKind, EntryOutcome, InvalidReason, format_degrees, parse_entry_number,
};
use crate::transform_handle_layout::{EditHandle, Side};
use crate::transform_math::ANGLE_EQUAL_EPSILON_RAD;

/// An open skew entry: the path and box as they were when it opened and the
/// fixed line fixed then (the opposite side's, or the box's centre line when
/// Shift was held at the second press of a double-click), so later Shift
/// changes change nothing.
#[derive(Debug, Clone)]
pub struct SkewEntry {
    start: ObjectSnapshot,
    start_box: OrientedBox,
    side: Side,
    shift: bool,
    pivot: Point,
    fields: [EntryField; 1],
}

impl SkewEntry {
    /// An entry for skew handle `side` of `object`, pre-filled "0". `None`
    /// for anything but a path (primitives have no skew handles). The field
    /// is read-only when the box has no extent across the skew axis: such a
    /// skew has no distance to scale by, and a drag cannot do it either.
    #[must_use]
    pub fn for_handle(
        object: &ObjectSnapshot,
        box_: &OrientedBox,
        side: Side,
        shift: bool,
    ) -> Option<Self> {
        if !matches!(object, ObjectSnapshot::Path(_)) {
            return None;
        }
        let frame = skew_frame(box_, side, shift);
        let mut field = EntryField::for_param(
            "",
            if side.skews_along_u() {
                "Skew angle x"
            } else {
                "Skew angle y"
            },
            format_degrees(0.0).trim_end_matches('°').to_string(),
        );
        field.editable = frame.lever.abs() >= MIN_SKEW_LEVER_MM;
        Some(Self {
            start: object.clone(),
            start_box: *box_,
            side,
            shift,
            pivot: frame.fixed_point,
            fields: [field],
        })
    }

    /// What this entry edits.
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        EntryKind::Skew
    }

    /// The handle the entry belongs to (it keeps its dragging look while the
    /// chip is open, where it is drawn).
    #[must_use]
    pub const fn handle(&self) -> EditHandle {
        EditHandle::Skew(self.side)
    }

    /// The path being edited.
    #[must_use]
    pub fn object(&self) -> &ObjectSnapshot {
        &self.start
    }

    /// The box the path had when the entry opened.
    #[must_use]
    pub const fn start_box(&self) -> &OrientedBox {
        &self.start_box
    }

    /// A point on the fixed line, shown by the pivot marker while the entry
    /// is open.
    #[must_use]
    pub const fn pivot(&self) -> Point {
        self.pivot
    }

    /// The chip's one field.
    #[must_use]
    pub fn fields(&self) -> &[EntryField] {
        &self.fields
    }

    /// The path after skewing by the typed `text`, `Ok(None)` if nothing would
    /// change (untouched text, an angle of zero, a read-only field), or why
    /// the text was refused.
    ///
    /// # Errors
    /// [`InvalidReason::NotANumber`] for text that is not a number,
    /// [`InvalidReason::SkewRange`] for 90° or more in size, and
    /// [`InvalidReason::TooLarge`] only when the result would put a coordinate
    /// beyond the document's limit; a skew too small to change anything is
    /// no change.
    pub fn resolve(&self, text: &str) -> Result<Option<ObjectSnapshot>, InvalidReason> {
        let field = &self.fields[0];
        if !field.editable || text == field.prefill {
            return Ok(None);
        }
        let degrees = parse_entry_number(text, true).ok_or(InvalidReason::NotANumber)?;
        if degrees.abs() >= 90.0 {
            return Err(InvalidReason::SkewRange);
        }
        let angle = Angle::from_radians(degrees.to_radians());
        if angle.as_radians().abs() <= ANGLE_EQUAL_EPSILON_RAD {
            return Ok(None);
        }
        let result = skewed_unchecked(&self.start, &self.start_box, self.side, self.shift, angle);
        if !is_sane(&result) {
            return Err(InvalidReason::TooLarge);
        }
        // A skew too small to move any number by 1e-9 mm (also: far from the
        // origin, where it vanishes in the coordinates' resolution) is no
        // change, as for a drag: no commit and no message.
        Ok((!same_within_tolerance(&result, &self.start)).then_some(result))
    }

    /// Validates the typed text and, if valid and changed, writes the skew as
    /// one commit, the one a skew drag writes. An entry whose path changed or
    /// vanished since it opened writes nothing.
    #[must_use]
    pub fn commit(&self, document: &Document, text: &str) -> EntryOutcome {
        if document.object(self.start.id()).as_ref() != Some(&self.start) {
            return EntryOutcome::Unchanged;
        }
        match self.resolve(text) {
            Err(reason) => EntryOutcome::Invalid { field: 0, reason },
            Ok(None) => EntryOutcome::Unchanged,
            Ok(Some(result)) => {
                commit_gesture(document, self.handle(), &result, ScaleModes::default());
                EntryOutcome::Committed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{AnchorId, AnchorKind, NewAnchor, NodeId, Vec2};

    use super::*;
    use crate::oriented_box::oriented_bounds;
    use crate::skew_math::skew_angle;
    use crate::transform_drag::{DragOrigin, TransformDrag};

    fn path(document: &Document, rotation_deg: f64) -> NodeId {
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: Point::new(20.0, 3.0),
                    handle_in: Vec2::new(-4.0, 1.0),
                    handle_out: Vec2::new(4.0, -1.0),
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(20.0, 10.0)),
                NewAnchor::corner(AnchorId::new(1, 4), Point::new(0.0, 10.0)),
            ],
            true,
        );
        if rotation_deg != 0.0 {
            let turned = document.object(id).expect("exists").rotated(
                Point::new(10.0, 5.0),
                Angle::from_radians(rotation_deg.to_radians()),
            );
            document.rotate_object(&turned).expect("rotates");
        }
        id
    }

    fn entry_of(document: &Document, id: NodeId, side: Side, shift: bool) -> SkewEntry {
        let object = document.object(id).expect("exists");
        SkewEntry::for_handle(&object, &oriented_bounds(&object), side, shift).expect("a path")
    }

    /// Criterion 9: the field is "0", named per axis, and only a path has one.
    #[test]
    fn the_field_is_zero_and_named_for_the_axis() {
        let document = Document::new(1);
        let id = path(&document, 0.0);
        for (side, name) in [
            (Side::Top, "Skew angle x"),
            (Side::Bottom, "Skew angle x"),
            (Side::Left, "Skew angle y"),
            (Side::Right, "Skew angle y"),
        ] {
            let entry = entry_of(&document, id, side, false);
            assert_eq!(entry.fields()[0].accessible_name, name);
            assert_eq!(entry.fields()[0].prefill, "0");
            assert_eq!(entry.fields()[0].label, "");
            assert!(entry.fields()[0].editable);
            assert_eq!(entry.kind(), EntryKind::Skew);
            assert_eq!(entry.handle(), EditHandle::Skew(side));
        }
        let rect = document.create_rect(vecmanf_document_core::RectBounds::from_corners(
            Point::new(0.0, 0.0),
            Point::new(5.0, 5.0),
        ));
        let object = document.object(rect).expect("exists");
        assert!(
            SkewEntry::for_handle(&object, &oriented_bounds(&object), Side::Top, false).is_none()
        );
    }

    /// Criterion 10, worked example: a 20 x 10 straight path, top handle, 45
    /// typed: the top edge moves +10 mm in x, the bottom edge does not move.
    #[test]
    fn the_worked_example_skews_the_top_edge_by_ten() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(20.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(20.0, 10.0)),
                NewAnchor::corner(AnchorId::new(1, 4), Point::new(0.0, 10.0)),
            ],
            true,
        );
        let entry = entry_of(&document, id, Side::Top, false);
        assert_eq!(entry.commit(&document, "45"), EntryOutcome::Committed);
        let ObjectSnapshot::Path(after) = document.object(id).expect("exists") else {
            panic!("a path");
        };
        let xs: Vec<f64> = after.anchors.iter().map(|a| a.point.x).collect();
        for (got, want) in xs.iter().zip([10.0, 30.0, 20.0, 0.0]) {
            assert!((got - want).abs() < 1e-9, "{xs:?}");
        }
        for (anchor, y) in after.anchors.iter().zip([0.0, 0.0, 10.0, 10.0]) {
            assert!((anchor.point.y - y).abs() < 1e-9);
        }
        assert_eq!(after.rotation.as_radians(), 0.0, "rotation is unchanged");
    }

    /// Criterion 10: the entry equals a drag of the same handle ending at the
    /// same angle, at rotation 0 and 30 degrees, for every side, with and
    /// without Shift (one resolving function).
    #[test]
    fn the_entry_equals_a_drag_ending_at_the_same_angle() {
        for rotation in [0.0, 30.0] {
            let document = Document::new(1);
            let id = path(&document, rotation);
            let object = document.object(id).expect("exists");
            let box_ = oriented_bounds(&object);
            for side in Side::ALL {
                for shift in [false, true] {
                    for degrees in [12.5_f64, -40.0, 73.0] {
                        let entry = entry_of(&document, id, side, shift);
                        let typed = entry
                            .resolve(&format!("{degrees}"))
                            .expect("valid")
                            .expect("changes");
                        // A pointer displacement along the side's direction
                        // that resolves to `degrees`.
                        let frame = skew_frame(&box_, side, shift);
                        let d = frame.lever.abs() * degrees.to_radians().tan();
                        let down = box_.to_document(box_.local_center());
                        let local = box_.to_local(down);
                        let moved = if side.skews_along_u() {
                            Point::new(local.x + d, local.y)
                        } else {
                            Point::new(local.x, local.y + d)
                        };
                        let current = box_.to_document(moved);
                        let drag = TransformDrag {
                            origin: DragOrigin::new(down, 0.0, shift),
                            start: object.clone(),
                            start_box: box_,
                            handle: EditHandle::Skew(side),
                            modes: ScaleModes::default(),
                            param_gain: 1.0,
                        };
                        let angle = skew_angle(&box_, side, down, current, shift, false);
                        assert!((angle.as_radians().to_degrees() - degrees).abs() < 1e-9);
                        let dragged = drag.resolve(current, shift, false);
                        assert!(
                            crate::transform_commit::same_within_tolerance(&typed, &dragged),
                            "rotation {rotation} {side:?} shift {shift} {degrees}"
                        );
                    }
                }
            }
        }
    }

    /// Criterion 13: a skew by entry and then the negated skew, same handle
    /// and Shift state, restore every anchor and handle vector.
    #[test]
    fn a_skew_and_its_negation_restore_the_path() {
        for rotation in [0.0, 30.0] {
            for side in Side::ALL {
                for shift in [false, true] {
                    let document = Document::new(1);
                    let id = path(&document, rotation);
                    let original = document.object(id).expect("exists");
                    assert_eq!(
                        entry_of(&document, id, side, shift).commit(&document, "33"),
                        EntryOutcome::Committed
                    );
                    assert_ne!(document.object(id).expect("exists"), original);
                    assert_eq!(
                        entry_of(&document, id, side, shift).commit(&document, "-33"),
                        EntryOutcome::Committed
                    );
                    let back = document.object(id).expect("exists");
                    assert!(
                        crate::transform_commit::same_within_tolerance(&back, &original),
                        "rotation {rotation} {side:?} shift {shift}"
                    );
                }
            }
        }
    }

    /// Criterion 11: not a number, 90 or more in size, and a result beyond the
    /// coordinate limit are refused and write nothing; the decimal comma, the
    /// point and a trailing degree sign are accepted.
    #[test]
    fn invalid_text_is_refused_and_writes_nothing() {
        let document = Document::new(1);
        let id = path(&document, 0.0);
        let before = document.object(id).expect("exists");
        let entry = entry_of(&document, id, Side::Top, false);
        for text in ["", "abc", "1,2,3", "1e3", "--5"] {
            assert_eq!(
                entry.resolve(text),
                Err(InvalidReason::NotANumber),
                "{text:?}"
            );
        }
        for text in ["90", "-90", "100", "-179", "90.0"] {
            assert_eq!(
                entry.resolve(text),
                Err(InvalidReason::SkewRange),
                "{text:?}"
            );
        }
        assert_eq!(
            entry.commit(&document, "90"),
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::SkewRange
            }
        );
        assert_eq!(document.object(id), Some(before.clone()));
        for text in ["20,5", "20.5", "20.5°", " -20.5 °"] {
            assert!(entry.resolve(text).expect("valid").is_some(), "{text:?}");
        }
        // 89.9999 degrees on a path whose lever is 10 mm: tan is about 5.7e5,
        // so the skewed points lie about 5.7e6 mm away, still inside; a longer
        // lever pushes it past 1e7.
        let far = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(2, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(2, 2), Point::new(20.0, 5000.0)),
            ],
            false,
        );
        let entry = entry_of(&document, far, Side::Top, false);
        assert_eq!(entry.resolve("89.9999"), Err(InvalidReason::TooLarge));
        assert_eq!(
            entry.commit(&document, "89.9999"),
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::TooLarge
            }
        );
    }

    /// Criterion 12: an untouched field, a typed 0 and a stale entry write
    /// nothing.
    #[test]
    fn untouched_zero_and_stale_write_nothing() {
        let document = Document::new(1);
        let id = path(&document, 0.0);
        let entry = entry_of(&document, id, Side::Left, false);
        let before = document.object(id).expect("exists");
        for text in ["0", "0.0", "-0", "0°"] {
            assert_eq!(
                entry.commit(&document, text),
                EntryOutcome::Unchanged,
                "{text:?}"
            );
        }
        assert_eq!(document.object(id), Some(before));
        // Another edit lands while the chip is open.
        document
            .translate_objects(&[id], Vec2::new(5.0, 0.0))
            .expect("moves");
        assert_eq!(entry.commit(&document, "20"), EntryOutcome::Unchanged);
        document.delete_objects(&[id]).expect("deletes");
        assert_eq!(entry.commit(&document, "20"), EntryOutcome::Unchanged);
    }

    /// A flat path has no distance to scale a skew by: its field is read-only
    /// and typing writes nothing.
    #[test]
    fn a_flat_path_has_a_read_only_field() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 5.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(20.0, 5.0)),
            ],
            false,
        );
        let top = entry_of(&document, id, Side::Top, false);
        assert!(!top.fields()[0].editable);
        assert_eq!(top.commit(&document, "30"), EntryOutcome::Unchanged);
        let left = entry_of(&document, id, Side::Left, false);
        assert!(left.fields()[0].editable, "the other axis has extent");
    }

    /// Criterion 10: the pivot is the fixed edge's midpoint, or the box centre
    /// under Shift.
    #[test]
    fn the_pivot_is_the_fixed_line() {
        let document = Document::new(1);
        let id = path(&document, 0.0);
        let top = entry_of(&document, id, Side::Top, false);
        assert!((top.pivot().y - 10.0).abs() < 1e-9, "the bottom edge");
        let shifted = entry_of(&document, id, Side::Top, true);
        assert!((shifted.pivot().y - 5.0).abs() < 1e-9, "the centre line");
    }

    /// A tiny angle on a path far from the origin vanishes in the
    /// coordinates' resolution: it is no change, never "Too large"; on a
    /// normal path a skew that moves nothing by 1e-9 mm is no change too, and
    /// writes no commit.
    #[test]
    fn a_vanishing_skew_is_no_change_not_too_large() {
        let document = Document::new(1);
        let far = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(5_000_000.0, 5_000_000.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(5_000_020.0, 5_000_000.0)),
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(5_000_020.0, 5_000_010.0)),
            ],
            true,
        );
        let near = path(&document, 0.0);
        for id in [far, near] {
            let before = document.object(id).expect("exists");
            let entry = entry_of(&document, id, Side::Top, false);
            for text in ["0.000000001", "0.0000000001", "1e-9"] {
                let outcome = entry.commit(&document, text);
                if text == "1e-9" {
                    assert_eq!(
                        outcome,
                        EntryOutcome::Invalid {
                            field: 0,
                            reason: InvalidReason::NotANumber
                        }
                    );
                } else {
                    assert_eq!(outcome, EntryOutcome::Unchanged, "{text}");
                }
            }
            assert_eq!(document.object(id), Some(before));
        }
        // Past the limit stays "Too large".
        let big = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(2, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(2, 2), Point::new(20.0, 5000.0)),
            ],
            false,
        );
        assert_eq!(
            entry_of(&document, big, Side::Top, false).resolve("89.9999"),
            Err(InvalidReason::TooLarge)
        );
    }

    /// The real minus sign of the move readout is accepted.
    #[test]
    fn the_real_minus_sign_is_accepted() {
        let document = Document::new(1);
        let id = path(&document, 0.0);
        let entry = entry_of(&document, id, Side::Top, false);
        let typed = entry
            .resolve("\u{2212}20")
            .expect("valid")
            .expect("changes");
        let ascii = entry.resolve("-20").expect("valid").expect("changes");
        assert_eq!(typed, ascii);
    }
}
