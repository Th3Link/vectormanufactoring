//! The Select bar's edits (`specs/unified-object-editing/specification.md`,
//! criteria 21, 21a and 22): a slider edit previewed and committed once, the
//! one-shot commits of "Remove rounding", "Points", "Ratio" and the typed
//! "Radius" field. Every value goes through [`crate::apply_param`] and
//! [`crate::commit_param_batch`], on exactly the objects of the control's
//! kind ([`crate::ids_of_kind`]), so a mixed selection writes only to the
//! objects that match, in one commit. A child module of `select_tool.rs`, so
//! it owns `SelectTool`'s pending-edit field.

use vecmanf_document_core::{
    Document, Length, NodeId, ObjectSnapshot, PrimitiveSnapshot, Shape, ShapeEditError,
};

use super::SelectTool;
use crate::object_selection::ObjectSelection;
use crate::param_edit::{ParamValue, apply_param, commit_param_batch};
use crate::select_bar::{BarPreview, ObjectKind, ids_of_kind};
use crate::transform_commit::MAX_COORDINATE_MM;
use crate::transform_entry::{EntryOutcome, InvalidReason, parse_entry_number};

/// The kind of object a bar `value` acts on.
const fn kind_of(value: ParamValue) -> ObjectKind {
    match value {
        ParamValue::Radius(_) => ObjectKind::Rectangle,
        ParamValue::Ratio(_) => ObjectKind::Star,
        ParamValue::PointCount(_) => ObjectKind::PolygonOrStar,
    }
}

/// Half the shorter side of a rectangle: the largest radius it allows.
fn half_shorter_side(object: &ObjectSnapshot) -> Option<f64> {
    match object {
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect { bounds, .. },
            ..
        }) => Some(bounds.width.as_mm().min(bounds.height.as_mm()) / 2.0),
        _ => None,
    }
}

impl SelectTool {
    /// The slider edit in flight, if any.
    #[must_use]
    pub const fn bar_preview(&self) -> Option<&BarPreview> {
        self.bar_preview.as_ref()
    }

    /// Previews `value` on every selected object of the control's kind: a
    /// slider tick. Nothing is written; [`SelectTool::live_edit`] shows the
    /// result in blue until [`SelectTool::flush_bar_preview`] commits it once.
    /// The objects are fixed when the edit starts and kept for its whole
    /// life, so a selection change cannot redirect it.
    pub fn preview_bar_value(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        value: ParamValue,
    ) {
        let ids = match &self.bar_preview {
            Some(pending) if kind_of(pending.value) == kind_of(value) => pending.ids.clone(),
            _ => ids_of_kind(objects, selection, kind_of(value)),
        };
        self.bar_preview = (!ids.is_empty()).then_some(BarPreview { ids, value });
    }

    /// Commits the slider edit in flight as one commit for the objects it was
    /// started against, then forgets it: the slider's release, key-up or blur,
    /// and the flush before anything that could change the selection. Writes
    /// nothing when no edit is pending or it changes nothing. `true` when a
    /// commit was made.
    pub fn flush_bar_preview(&mut self, document: &Document) -> bool {
        let Some(pending) = self.bar_preview.take() else {
            return false;
        };
        let changes_something = pending.ids.iter().any(|id| {
            document
                .object(*id)
                .is_some_and(|old| apply_param(&old, pending.value) != old)
        });
        changes_something && commit_param_batch(document, &pending.ids, pending.value).is_ok()
    }

    /// Drops the slider edit in flight without writing.
    pub fn cancel_bar_preview(&mut self) {
        self.bar_preview = None;
    }

    /// One commit of `value` on every selected object of the control's kind:
    /// a stepper click or a typed "Points" or "Ratio". Any slider edit in
    /// flight is flushed first.
    ///
    /// # Errors
    /// The `Document` command's error if an object no longer exists; nothing
    /// is written then.
    pub fn commit_bar_value(
        &mut self,
        document: &Document,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        value: ParamValue,
    ) -> Result<(), ShapeEditError> {
        self.flush_bar_preview(document);
        let ids = ids_of_kind(objects, selection, kind_of(value));
        let changes_something = ids.iter().any(|id| {
            objects
                .iter()
                .find(|object| object.id() == *id)
                .is_some_and(|old| apply_param(old, value) != *old)
        });
        if changes_something {
            commit_param_batch(document, &ids, value)
        } else {
            Ok(())
        }
    }

    /// "Remove rounding" (criterion 21): zeroes the radius of every selected
    /// rectangle that has one, in one commit. Writes nothing when none has.
    pub fn remove_rounding(
        &mut self,
        document: &Document,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
    ) {
        self.flush_bar_preview(document);
        let rounded: Vec<NodeId> = ids_of_kind(objects, selection, ObjectKind::Rectangle)
            .into_iter()
            .filter(|id| {
                objects.iter().any(|object| {
                    object.id() == *id
                        && matches!(
                            object,
                            ObjectSnapshot::Primitive(PrimitiveSnapshot {
                                shape: Shape::Rect { corner_radius, .. },
                                ..
                            }) if corner_radius.as_mm() > 0.0
                        )
                })
            })
            .collect();
        let _ = commit_param_batch(document, &rounded, ParamValue::Radius(Length::from_mm(0.0)));
    }

    /// The typed "Radius" field (criterion 21a): Enter writes the value to
    /// every selected rectangle in one commit, limited to the largest half
    /// shorter side among them (the register is raw and clamped where it is
    /// evaluated, so a smaller rectangle shows its effective radius). An
    /// empty or non-numeric text and a negative value are refused and write
    /// nothing; an equal value writes nothing.
    pub fn commit_bar_radius_text(
        &mut self,
        document: &Document,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        text: &str,
    ) -> EntryOutcome {
        self.flush_bar_preview(document);
        let invalid = |reason| EntryOutcome::Invalid { field: 0, reason };
        let Some(value) = parse_entry_number(text, false) else {
            return invalid(InvalidReason::NotANumber);
        };
        if value < 0.0 {
            return invalid(InvalidReason::Negative);
        }
        let ids = ids_of_kind(objects, selection, ObjectKind::Rectangle);
        let targets: Vec<&ObjectSnapshot> = ids
            .iter()
            .filter_map(|id| objects.iter().find(|object| object.id() == *id))
            .collect();
        let Some(largest_half) = targets
            .iter()
            .filter_map(|object| half_shorter_side(object))
            .reduce(f64::max)
        else {
            return EntryOutcome::Unchanged;
        };
        let limited = Length::from_mm(value.min(MAX_COORDINATE_MM).min(largest_half));
        let typed = ParamValue::Radius(limited);
        if targets
            .iter()
            .all(|object| apply_param(object, typed) == **object)
        {
            return EntryOutcome::Unchanged;
        }
        match commit_param_batch(document, &ids, typed) {
            Ok(()) => EntryOutcome::Committed,
            Err(_) => EntryOutcome::Unchanged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Angle, InnerRatio, Point, PointCount, RectBounds, StarFrame};

    struct Rig {
        document: Document,
        selection: ObjectSelection,
        tool: SelectTool,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                document: Document::new(1),
                selection: ObjectSelection::new(),
                tool: SelectTool::new(),
            }
        }

        fn rect(&mut self, size: f64, radius: f64) -> NodeId {
            let id = self.document.create_rect(RectBounds {
                origin: Point::new(0.0, 0.0),
                width: Length::from_mm(size),
                height: Length::from_mm(size),
            });
            self.document
                .set_corner_radius(&[id], Length::from_mm(radius))
                .unwrap();
            self.selection.toggle(id);
            id
        }

        fn star(&mut self, points: u32, ratio: f64) -> NodeId {
            let id = self.document.create_star(
                StarFrame {
                    center: Point::new(0.0, 0.0),
                    radius: Length::from_mm(10.0),
                    angle: Angle::from_radians(0.0),
                },
                PointCount::new(points).unwrap(),
                InnerRatio::new(ratio).unwrap(),
            );
            self.selection.toggle(id);
            id
        }

        fn objects(&self) -> Vec<ObjectSnapshot> {
            self.document
                .object_ids()
                .into_iter()
                .filter_map(|id| self.document.object(id))
                .collect()
        }

        fn radius(&self, id: NodeId) -> f64 {
            let Some(ObjectSnapshot::Primitive(PrimitiveSnapshot {
                shape: Shape::Rect { corner_radius, .. },
                ..
            })) = self.document.object(id)
            else {
                panic!("a rectangle");
            };
            corner_radius.as_mm()
        }

        fn ratio(&self, id: NodeId) -> f64 {
            let Some(ObjectSnapshot::Primitive(PrimitiveSnapshot {
                shape: Shape::Star { inner_ratio, .. },
                ..
            })) = self.document.object(id)
            else {
                panic!("a star");
            };
            inner_ratio.get()
        }

        fn snapshot(&self) -> Vec<u8> {
            self.document.export_loro_snapshot().unwrap()
        }
    }

    #[test]
    fn remove_rounding_zeroes_only_the_rectangles_in_one_commit() {
        let mut rig = Rig::new();
        let (a, b) = (rig.rect(40.0, 5.0), rig.rect(40.0, 8.0));
        let star = rig.star(5, 0.4);
        let objects = rig.objects();
        rig.tool
            .remove_rounding(&rig.document, &objects, &rig.selection);
        assert_eq!((rig.radius(a), rig.radius(b)), (0.0, 0.0));
        assert_eq!(rig.ratio(star), 0.4, "the star is left alone");
    }

    #[test]
    fn remove_rounding_with_nothing_to_remove_writes_nothing() {
        let mut rig = Rig::new();
        rig.rect(40.0, 0.0);
        let before = rig.snapshot();
        let objects = rig.objects();
        rig.tool
            .remove_rounding(&rig.document, &objects, &rig.selection);
        assert_eq!(rig.snapshot(), before);
    }

    /// Criterion 21: a slider drag is one commit, the preview is a blue
    /// overlay (nothing written) until the flush.
    #[test]
    fn a_slider_edit_previews_then_commits_once_on_the_flush() {
        let mut rig = Rig::new();
        let star = rig.star(5, 0.4);
        let objects = rig.objects();
        let before = rig.snapshot();
        for tick in [0.5, 0.6, 0.7] {
            rig.tool.preview_bar_value(
                &objects,
                &rig.selection,
                ParamValue::Ratio(InnerRatio::new(tick).unwrap()),
            );
        }
        assert_eq!(rig.snapshot(), before, "nothing written while dragging");
        let live = rig
            .tool
            .live_edit(&objects, &rig.selection, Point::new(0.0, 0.0), false, false)
            .expect("a preview");
        assert_eq!(live.objects.len(), 1);
        assert!(rig.tool.flush_bar_preview(&rig.document));
        assert!((rig.ratio(star) - 0.7).abs() < 1e-12);
        assert!(!rig.tool.flush_bar_preview(&rig.document), "once");
    }

    /// The pending edit is fixed to the objects it started on: a selection
    /// change before the flush cannot redirect it.
    #[test]
    fn a_slider_edit_stays_on_the_objects_it_started_with() {
        let mut rig = Rig::new();
        let first = rig.star(5, 0.4);
        let objects = rig.objects();
        rig.tool.preview_bar_value(
            &objects,
            &rig.selection,
            ParamValue::Ratio(InnerRatio::new(0.8).unwrap()),
        );
        let second = rig.star(6, 0.3);
        rig.selection.select_single(second);
        rig.tool.flush_bar_preview(&rig.document);
        assert!((rig.ratio(first) - 0.8).abs() < 1e-12);
        assert!((rig.ratio(second) - 0.3).abs() < 1e-12);
    }

    #[test]
    fn a_stepper_commit_acts_on_the_matching_kind_only() {
        let mut rig = Rig::new();
        let rect = rig.rect(40.0, 3.0);
        let star = rig.star(5, 0.4);
        let objects = rig.objects();
        rig.tool
            .commit_bar_value(
                &rig.document,
                &objects,
                &rig.selection,
                ParamValue::PointCount(PointCount::new(9).unwrap()),
            )
            .unwrap();
        let Some(ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Star { point_count, .. },
            ..
        })) = rig.document.object(star)
        else {
            panic!("a star");
        };
        assert_eq!(point_count.get(), 9);
        assert_eq!(rig.radius(rect), 3.0);
    }

    #[test]
    fn the_typed_radius_applies_to_every_rectangle_and_is_limited_to_the_largest_half() {
        let mut rig = Rig::new();
        let (big, small) = (rig.rect(40.0, 1.0), rig.rect(10.0, 1.0));
        let objects = rig.objects();
        let outcome =
            rig.tool
                .commit_bar_radius_text(&rig.document, &objects, &rig.selection, "100");
        assert_eq!(outcome, EntryOutcome::Committed);
        assert_eq!(rig.radius(big), 20.0, "limited to half of 40");
        assert_eq!(rig.radius(small), 20.0, "raw, clamped where evaluated");
    }

    #[test]
    fn the_typed_radius_refuses_bad_text_and_ignores_an_equal_value() {
        let mut rig = Rig::new();
        let id = rig.rect(40.0, 3.0);
        let objects = rig.objects();
        let before = rig.snapshot();
        let mut try_text = |text: &str| {
            rig.tool
                .commit_bar_radius_text(&rig.document, &objects, &rig.selection, text)
        };
        assert_eq!(
            try_text(""),
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::NotANumber
            }
        );
        assert_eq!(
            try_text("-2"),
            EntryOutcome::Invalid {
                field: 0,
                reason: InvalidReason::Negative
            }
        );
        assert_eq!(try_text("3"), EntryOutcome::Unchanged);
        assert_eq!(rig.snapshot(), before);
        assert_eq!(rig.radius(id), 3.0);
    }
}
