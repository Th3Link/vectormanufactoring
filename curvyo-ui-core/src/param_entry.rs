//! The typed numeric entry on a parameter handle: a rectangle's corner radius
//! and a star's inner ratio (`specs/unified-object-editing/specification.md`,
//! criteria 18 and 19). One field, validated here; the DOM chip only holds the
//! text, the caret and the focus. The typed value goes through the same
//! [`apply_param`] a drag and the bar use, so the three cannot disagree, and
//! every close path of the refinements entry writes nothing here too.

use curvyo_document_core::{
    Document, Length, ObjectSnapshot, PrimitiveSnapshot, Shape, effective_corner_radii,
};

use crate::oriented_box::OrientedBox;
use crate::param_edit::{
    MAX_INNER_RATIO, MIN_INNER_RATIO, PARAM_EQUAL_EPSILON, ParamValue, apply_param, commit_param,
    radius_is_limited,
};
use crate::param_handles::{Corner, ParamHandle};
use crate::transform_commit::MAX_COORDINATE_MM;
use crate::transform_entry::{
    EntryField, EntryKind, EntryOutcome, InvalidReason, format_mm, parse_entry_number,
};
use crate::transform_handle_layout::EditHandle;

/// The scope row of a corner radius entry (`specs/rectangle-corner-radii/`
/// criterion 6): which corners a typed value writes.
const SCOPE_ALL_FOUR: &str = "All four corners";
const SCOPE_ONE_CORNER: &str = "This corner only";

/// The accessible name of the radius field: "Corner radius, all corners" when
/// the scope is all four, else the corner's own name in the rectangle's frame.
const fn radius_field_name(corner: Corner, unlinked: bool) -> &'static str {
    if !unlinked {
        return "Corner radius, all corners";
    }
    match corner {
        Corner::Tl => "Top-left corner radius",
        Corner::Tr => "Top-right corner radius",
        Corner::Br => "Bottom-right corner radius",
        Corner::Bl => "Bottom-left corner radius",
    }
}

/// An open entry on a parameter handle: the primitive and box as they were
/// when it opened (an entry whose object changed or went away since writes
/// nothing).
#[derive(Debug, Clone)]
pub struct ParamEntry {
    handle: ParamHandle,
    start: ObjectSnapshot,
    start_box: OrientedBox,
    fields: [EntryField; 1],
    /// Whether a typed corner radius is written to one corner only, fixed when
    /// the field opened (criterion 6). `false` for an inner ratio.
    unlinked: bool,
}

impl ParamEntry {
    /// An entry for `handle` of `object`, pre-filled with the current
    /// effective radius of that corner (one decimal, millimetres) or ratio (two
    /// decimals); `None` when the primitive has no such handle. `unlinked` is
    /// the scope of a corner radius, decided by the caller from the "Link
    /// corners" switch and Shift at the second press
    /// ([`crate::CornerLinking::is_unlinked_with`]) and kept for the life of
    /// the entry.
    #[must_use]
    pub fn for_handle(
        object: &ObjectSnapshot,
        box_: &OrientedBox,
        handle: ParamHandle,
        unlinked: bool,
    ) -> Option<Self> {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot { shape, .. }) = object else {
            return None;
        };
        let field = match (handle, *shape) {
            (
                ParamHandle::CornerRadius(corner),
                Shape::Rect {
                    bounds,
                    corner_radii,
                },
            ) => EntryField::for_param(
                "r",
                radius_field_name(corner, unlinked),
                format_mm(
                    effective_corner_radii(bounds, corner_radii)
                        .get(corner)
                        .as_mm(),
                ),
            ),
            (ParamHandle::InnerRadius, Shape::Star { inner_ratio, .. }) => {
                EntryField::for_param("ratio", "Inner ratio", format!("{:.2}", inner_ratio.get()))
            }
            _ => return None,
        };
        Some(Self {
            handle,
            start: object.clone(),
            start_box: *box_,
            fields: [field],
            unlinked: matches!(handle, ParamHandle::CornerRadius(_)) && unlinked,
        })
    }

    /// The scope row under a corner radius field: "All four corners" or "This
    /// corner only"; `None` for an inner ratio.
    #[must_use]
    pub const fn scope(&self) -> Option<&'static str> {
        match self.handle {
            ParamHandle::CornerRadius(_) if self.unlinked => Some(SCOPE_ONE_CORNER),
            ParamHandle::CornerRadius(_) => Some(SCOPE_ALL_FOUR),
            ParamHandle::InnerRadius => None,
        }
    }

    /// Whether `text` asks for a radius past the limit, so that the committed
    /// value is the limited one and the "max" readout shows (criterion 6).
    /// `false` for text that is not a valid radius.
    #[must_use]
    pub fn is_limited(&self, text: &str) -> bool {
        self.typed_radius(text)
            .is_some_and(|value| radius_is_limited(&self.start, value))
    }

    /// The radius `text` asks for as the value of this entry's scope.
    fn typed_radius(&self, text: &str) -> Option<ParamValue> {
        let ParamHandle::CornerRadius(corner) = self.handle else {
            return None;
        };
        let value = parse_entry_number(text, false)?;
        if value < 0.0 {
            return None;
        }
        let radius = Length::from_mm(value.min(MAX_COORDINATE_MM));
        Some(if self.unlinked {
            ParamValue::CornerRadius(corner, radius)
        } else {
            ParamValue::Radius(radius)
        })
    }

    /// What this entry edits.
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        match self.handle {
            ParamHandle::CornerRadius(_) => EntryKind::CornerRadius,
            ParamHandle::InnerRadius => EntryKind::InnerRatio,
        }
    }

    /// The handle the entry belongs to (it keeps its dragging look while the
    /// chip is open).
    #[must_use]
    pub const fn handle(&self) -> EditHandle {
        EditHandle::Param(self.handle)
    }

    /// The object being edited.
    #[must_use]
    pub fn object(&self) -> &ObjectSnapshot {
        &self.start
    }

    /// The box the object had when the entry opened.
    #[must_use]
    pub const fn start_box(&self) -> &OrientedBox {
        &self.start_box
    }

    /// The chip's one field.
    #[must_use]
    pub fn fields(&self) -> &[EntryField] {
        &self.fields
    }

    /// The object after applying the typed `text`, `Ok(None)` if nothing would
    /// change (untouched text, an equal value), or why the text was refused.
    ///
    /// # Errors
    /// [`InvalidReason::NotANumber`] for text that is not a number,
    /// [`InvalidReason::Negative`] for a negative radius and
    /// [`InvalidReason::RatioRange`] for a ratio outside 0.01 to 0.99. A radius
    /// above half the shorter side is limited, not refused.
    pub fn resolve(&self, text: &str) -> Result<Option<ObjectSnapshot>, InvalidReason> {
        if text == self.fields[0].prefill {
            return Ok(None);
        }
        let value = parse_entry_number(text, false).ok_or(InvalidReason::NotANumber)?;
        let typed = match self.handle {
            ParamHandle::CornerRadius(_) => {
                if value < 0.0 {
                    return Err(InvalidReason::Negative);
                }
                // invariant: a non-negative number is a radius.
                self.typed_radius(text).ok_or(InvalidReason::NotANumber)?
            }
            ParamHandle::InnerRadius => {
                if !(MIN_INNER_RATIO - PARAM_EQUAL_EPSILON..=MAX_INNER_RATIO + PARAM_EQUAL_EPSILON)
                    .contains(&value)
                {
                    return Err(InvalidReason::RatioRange);
                }
                // invariant: the range check above leaves 0.01..=0.99 up to
                // 1e-9, which `clamped_ratio` brings inside `(0, 1)`.
                ParamValue::Ratio(crate::param_edit::clamped_ratio(value))
            }
        };
        let result = apply_param(&self.start, typed);
        Ok((result != self.start).then_some(result))
    }

    /// Validates `text` and, if valid and changed, writes it as one commit
    /// (criteria 18, 19). An entry whose object changed or vanished since it
    /// opened writes nothing.
    #[must_use]
    pub fn commit(&self, document: &Document, text: &str) -> EntryOutcome {
        if document.object(self.start.id()).as_ref() != Some(&self.start) {
            return EntryOutcome::Unchanged;
        }
        match self.resolve(text) {
            Err(reason) => EntryOutcome::Invalid { field: 0, reason },
            Ok(None) => EntryOutcome::Unchanged,
            Ok(Some(result)) => {
                commit_param(document, self.handle, &result);
                EntryOutcome::Committed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oriented_box::oriented_bounds;
    use crate::param_handles::Corner;
    use curvyo_document_core::{
        Angle, CornerRadii, InnerRatio, Point, PointCount, RectBounds, StarFrame,
    };

    fn rect_entry(radius: f64) -> (Document, ParamEntry) {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(60.0),
        });
        document
            .set_corner_radius(&[id], Length::from_mm(radius))
            .unwrap();
        let object = document.object(id).unwrap();
        let entry = ParamEntry::for_handle(
            &object,
            &oriented_bounds(&object),
            ParamHandle::CornerRadius(Corner::Tr),
            false,
        )
        .unwrap();
        (document, entry)
    }

    fn star_entry(ratio: f64) -> (Document, ParamEntry) {
        let document = Document::new(1);
        let id = document.create_star(
            StarFrame {
                center: Point::new(0.0, 0.0),
                radius: Length::from_mm(30.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(ratio).unwrap(),
        );
        let object = document.object(id).unwrap();
        let entry = ParamEntry::for_handle(
            &object,
            &oriented_bounds(&object),
            ParamHandle::InnerRadius,
            false,
        )
        .unwrap();
        (document, entry)
    }

    fn radius(document: &Document, entry: &ParamEntry) -> f64 {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radii, .. },
            ..
        }) = document.object(entry.object().id()).unwrap()
        else {
            panic!("a rectangle");
        };
        assert_eq!(
            corner_radii,
            CornerRadii::uniform(corner_radii.tl),
            "four equal radii"
        );
        corner_radii.tl.as_mm()
    }

    #[test]
    fn the_field_opens_with_the_effective_radius_and_the_corner_radius_name() {
        let (_, entry) = rect_entry(3.46);
        let field = &entry.fields()[0];
        assert_eq!(
            (field.label, field.accessible_name, field.prefill.as_str()),
            ("r", "Corner radius, all corners", "3.5")
        );
        assert_eq!(entry.kind(), EntryKind::CornerRadius);
        let (_, star) = star_entry(0.456);
        let field = &star.fields()[0];
        assert_eq!(
            (field.label, field.accessible_name, field.prefill.as_str()),
            ("ratio", "Inner ratio", "0.46")
        );
    }

    #[test]
    fn a_typed_radius_is_written_in_one_commit_and_zero_is_valid() {
        let (document, entry) = rect_entry(3.0);
        assert_eq!(entry.commit(&document, "7,5"), EntryOutcome::Committed);
        assert!((radius(&document, &entry) - 7.5).abs() < 1e-12);
        let (document, entry) = rect_entry(3.0);
        assert_eq!(entry.commit(&document, "0"), EntryOutcome::Committed);
        assert_eq!(radius(&document, &entry), 0.0, "sharp corners");
    }

    #[test]
    fn a_radius_above_half_the_shorter_side_is_limited_and_the_limit_is_written() {
        let (document, entry) = rect_entry(3.0);
        assert_eq!(entry.commit(&document, "500"), EntryOutcome::Committed);
        assert!((radius(&document, &entry) - 30.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_text_keeps_the_field_open_and_writes_nothing() {
        let (document, entry) = rect_entry(3.0);
        let invalid = |reason| EntryOutcome::Invalid { field: 0, reason };
        assert_eq!(
            entry.commit(&document, ""),
            invalid(InvalidReason::NotANumber)
        );
        assert_eq!(
            entry.commit(&document, "abc"),
            invalid(InvalidReason::NotANumber)
        );
        assert_eq!(
            entry.commit(&document, "-1"),
            invalid(InvalidReason::Negative)
        );
        assert_eq!(radius(&document, &entry), 3.0);
    }

    #[test]
    fn an_untouched_or_equal_entry_writes_nothing() {
        let (document, entry) = rect_entry(3.46);
        assert_eq!(entry.commit(&document, "3.5"), EntryOutcome::Unchanged);
        assert_eq!(
            radius(&document, &entry),
            3.46,
            "the rounded prefill is not written"
        );
        assert_eq!(entry.commit(&document, "3.46"), EntryOutcome::Unchanged);
    }

    #[test]
    fn a_stale_entry_writes_nothing() {
        let (document, entry) = rect_entry(3.0);
        document
            .set_corner_radius(&[entry.object().id()], Length::from_mm(9.0))
            .unwrap();
        assert_eq!(entry.commit(&document, "5"), EntryOutcome::Unchanged);
        assert_eq!(radius(&document, &entry), 9.0);
    }

    #[test]
    fn a_ratio_is_limited_to_the_open_range_by_validation() {
        let (document, entry) = star_entry(0.5);
        let invalid = EntryOutcome::Invalid {
            field: 0,
            reason: InvalidReason::RatioRange,
        };
        assert_eq!(entry.commit(&document, "0"), invalid);
        assert_eq!(entry.commit(&document, "1"), invalid);
        assert_eq!(entry.commit(&document, "0.005"), invalid);
        assert_eq!(entry.commit(&document, "0.99"), EntryOutcome::Committed);
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Star { inner_ratio, .. },
            ..
        }) = document.object(entry.object().id()).unwrap()
        else {
            panic!("a star");
        };
        assert!((inner_ratio.get() - 0.99).abs() < 1e-12);
        let (document, entry) = star_entry(0.5);
        assert_eq!(entry.commit(&document, "0.01"), EntryOutcome::Committed);
    }

    #[test]
    fn a_polygon_and_an_ellipse_have_no_parameter_entry() {
        let document = Document::new(1);
        let id = document.create_polygon(
            StarFrame {
                center: Point::new(0.0, 0.0),
                radius: Length::from_mm(30.0),
                angle: Angle::from_radians(0.0),
            },
            PointCount::new(5).unwrap(),
        );
        let object = document.object(id).unwrap();
        assert!(
            ParamEntry::for_handle(
                &object,
                &oriented_bounds(&object),
                ParamHandle::InnerRadius,
                false
            )
            .is_none()
        );
    }

    /// Criterion 18: typed, dragged and bar values share one rule: the same
    /// value written by entry and by `apply_param` is the same snapshot.
    #[test]
    fn a_typed_value_equals_apply_param() {
        let (document, entry) = rect_entry(3.0);
        let typed = entry.resolve("12.25").unwrap().unwrap();
        let direct = apply_param(
            &document.object(entry.object().id()).unwrap(),
            ParamValue::Radius(Length::from_mm(12.25)),
        );
        assert_eq!(typed, direct);
    }

    /// Criterion 6: the scope is fixed when the field opens and names the
    /// field: all four corners, or one corner by its own name.
    #[test]
    fn the_scope_row_and_the_accessible_name_follow_the_link_state_at_open() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(60.0),
        });
        let object = document.object(id).unwrap();
        let box_ = oriented_bounds(&object);
        let open = |corner, unlinked| {
            ParamEntry::for_handle(&object, &box_, ParamHandle::CornerRadius(corner), unlinked)
                .unwrap()
        };
        let linked = open(Corner::Bl, false);
        assert_eq!(linked.scope(), Some("All four corners"));
        assert_eq!(
            linked.fields()[0].accessible_name,
            "Corner radius, all corners"
        );
        for (corner, name) in [
            (Corner::Tl, "Top-left corner radius"),
            (Corner::Tr, "Top-right corner radius"),
            (Corner::Br, "Bottom-right corner radius"),
            (Corner::Bl, "Bottom-left corner radius"),
        ] {
            let entry = open(corner, true);
            assert_eq!(entry.scope(), Some("This corner only"));
            assert_eq!(entry.fields()[0].accessible_name, name);
        }
        let (_, star) = star_entry(0.4);
        assert_eq!(star.scope(), None);
    }

    /// Criterion 6: an unlinked entry writes its own corner only and shows that
    /// corner's effective radius; a value past the limit is limited and says so.
    #[test]
    fn an_unlinked_entry_writes_one_corner_and_reports_the_limit() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(100.0),
            height: Length::from_mm(60.0),
        });
        document
            .set_corner_radii(&[(
                id,
                CornerRadii {
                    tl: Length::from_mm(10.0),
                    tr: Length::from_mm(40.0),
                    br: Length::from_mm(0.0),
                    bl: Length::from_mm(20.0),
                },
            )])
            .unwrap();
        let object = document.object(id).unwrap();
        let entry = ParamEntry::for_handle(
            &object,
            &oriented_bounds(&object),
            ParamHandle::CornerRadius(Corner::Tl),
            true,
        )
        .unwrap();
        assert_eq!(entry.fields()[0].prefill, "10.0");
        assert!(!entry.is_limited("30"));
        // TL limit: min(100 - 40, 60 - 20) = 40.
        assert!(entry.is_limited("55"));
        assert!(!entry.is_limited("x"));
        assert_eq!(entry.commit(&document, "55"), EntryOutcome::Committed);
        let Some(ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { corner_radii, .. },
            ..
        })) = document.object(id)
        else {
            panic!("a rectangle");
        };
        assert_eq!(
            corner_radii,
            CornerRadii {
                tl: Length::from_mm(40.0),
                tr: Length::from_mm(40.0),
                br: Length::from_mm(0.0),
                bl: Length::from_mm(20.0),
            }
        );
    }
}
