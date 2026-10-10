//! `Session`'s glue for the typed numeric entry
//! (`specs/0008-object-transform-refinements/specification.md`, acceptance
//! criteria 18-32): the entry's view for the DOM chip, the linked-field
//! text, commit and cancel. All rules (parser, validation, linking,
//! resolution) live in `curvyo_ui_core::transform_entry`; the DOM holds only
//! text, caret and focus. Closing without writing on a tool switch, a
//! selection change, Delete, "Object to path", Escape and a press elsewhere
//! happens at those call sites through [`Session::cancel_transform_entry`]
//! and `SelectTool::pointer_down`.

use curvyo_document_core::Point;
use curvyo_ui_core::{
    EditHandle, EntryField, EntryKind, EntryOutcome, GroupEntry, ParamDragInfo, ParamEntry,
    SelectTool, SkewEntry, TransformEntry, entry_anchor,
};

use super::corner_readout::param_readout;
use super::{Session, Tool};

/// One field of the entry chip, as the host renders it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryFieldView {
    /// The visible label ("W", "H", "r"; empty for the angle).
    pub label: &'static str,
    /// The accessible name ("Width", "Height", "Outer radius", "Angle",
    /// "Corner radius", "Inner ratio").
    pub accessible_name: &'static str,
    /// The text the field opens with.
    pub prefill: String,
    /// Whether the field can be edited.
    pub editable: bool,
}

/// An open entry as the host sees it: what to show and where.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryView {
    /// `"angle"`, `"size"`, `"radius"` (a polygon or star's outer radius),
    /// `"corner-radius"` or `"inner-ratio"`.
    pub kind: &'static str,
    /// One or two fields.
    pub fields: Vec<EntryFieldView>,
    /// Whether the two fields are linked (a chain glyph between them).
    pub linked: bool,
    /// The muted second row of a corner radius entry, "All four corners" or
    /// "This corner only" (`specs/0013-rectangle-corner-radii/` criterion 6); `None`
    /// for every other entry.
    pub scope: Option<&'static str>,
    /// The grabbed handle, in document space.
    pub handle: Point,
    /// The box center, in document space: the chip goes outward from it,
    /// through the handle.
    pub center: Point,
    /// How far past the handle's position its outermost glyph reaches, in
    /// screen pixels: 6 normally, 22 for an edge resize handle with a skew
    /// arrow on the same side, which the chip must clear.
    pub glyph_reach_px: f64,
    /// The chip goes by the box centre (16 px right of and below it, as the
    /// move chip does) instead of outward from the handle: the key S, whose
    /// fixed point is the centre (`edit-interaction-polish` criterion 59).
    pub at_centre: bool,
    /// The chip edits a multi-selection (`multi-object-transform` criteria 33,
    /// 34, 36): the size chip is named "Resize selection".
    pub selection: bool,
}

impl EntryFieldView {
    fn of(field: &EntryField) -> Self {
        Self {
            label: field.label,
            accessible_name: field.accessible_name,
            prefill: field.prefill.clone(),
            editable: field.editable,
        }
    }
}

impl Session {
    /// The open entry, if the Select tool is active and its object is still
    /// the sole selection.
    fn open_entry(&self) -> Option<&TransformEntry> {
        if self.tool != Tool::Select {
            return None;
        }
        let entry = self.select.entry()?;
        (self.selection.ids() == [entry.object().id()]).then_some(entry)
    }

    /// The open skew entry, with the same checks.
    fn open_skew_entry(&self) -> Option<&SkewEntry> {
        if self.tool != Tool::Select {
            return None;
        }
        let entry = self.select.skew_entry()?;
        (self.selection.ids() == [entry.object().id()]).then_some(entry)
    }

    /// The chip of an open skew entry: one field at the skew handle's
    /// position, computed from the box so it is there when the handle is not
    /// drawn (`edit-interaction-polish` criteria 9, 58, 59).
    fn skew_entry_view(&self, entry: &SkewEntry) -> EntryView {
        let box_ = entry.start_box();
        // invariant: `entry_anchor` is `Some` for every handle but a parameter
        // handle, and a skew entry's handle is a skew handle.
        let handle = entry_anchor(box_, entry.handle(), &self.transform_handle_tolerances())
            .unwrap_or_else(|| box_.to_document(box_.local_center()));
        EntryView {
            kind: "skew",
            fields: entry.fields().iter().map(EntryFieldView::of).collect(),
            linked: false,
            scope: None,
            handle,
            center: box_.to_document(box_.local_center()),
            glyph_reach_px: 6.0,
            at_centre: false,
            selection: false,
        }
    }

    /// The open corner-radius or inner-ratio entry, with the same checks.
    fn open_param_entry(&self) -> Option<&ParamEntry> {
        if self.tool != Tool::Select {
            return None;
        }
        let entry = self.select.param_entry()?;
        (self.selection.ids() == [entry.object().id()]).then_some(entry)
    }

    /// The chip of an open parameter-handle entry: one field next to its
    /// knob (`unified-object-editing` criteria 18, 19).
    fn param_entry_view(&self, entry: &ParamEntry) -> Option<EntryView> {
        let objects = self.objects();
        let handle = SelectTool::transform_handles(
            &objects,
            &self.selection,
            self.transform_handle_tolerances(),
            false,
        )
        .into_iter()
        .find(|(handle, _)| *handle == entry.handle())?
        .1;
        let box_ = entry.start_box();
        Some(EntryView {
            kind: match entry.kind() {
                EntryKind::CornerRadius => "corner-radius",
                _ => "inner-ratio",
            },
            fields: entry.fields().iter().map(EntryFieldView::of).collect(),
            linked: false,
            scope: entry.scope(),
            handle,
            center: box_.to_document(box_.local_center()),
            glyph_reach_px: 6.0,
            at_centre: false,
            selection: false,
        })
    }

    /// The open group entry, with the same checks: the Select tool is active
    /// and its objects are still the selection.
    fn open_group_entry(&self) -> Option<&GroupEntry> {
        if self.tool != Tool::Select {
            return None;
        }
        let entry = self.select.group_entry()?;
        (self.selection.ids() == entry.ids()).then_some(entry)
    }

    /// The chip of an open group entry (an angle, a size or a skew of a
    /// multi-selection), placed as a single object's chip of the same handle is,
    /// from the group box.
    fn group_entry_view(&self, entry: &GroupEntry) -> Option<EntryView> {
        let box_ = entry.start_box();
        let tolerances = self.transform_handle_tolerances();
        let handle = if entry.centre_chip() {
            box_.to_document(box_.local_center())
        } else {
            entry_anchor(box_, entry.handle(), &tolerances)?
        };
        // An edge resize handle with a skew arrow on its side: the arrow sits 16
        // px out and is 12 px deep, so the glyphs reach 22 px.
        let drawn = SelectTool::group_handle_positions(
            &self.objects(),
            &self.selection,
            tolerances,
            entry.side_rotate_revealed(),
        );
        let skew_beyond = matches!(
            entry.handle(),
            EditHandle::Resize(direction)
                if drawn.iter().any(|(h, _)| matches!(
                    h, EditHandle::Skew(side) if side.direction() == direction
                ))
        );
        Some(EntryView {
            kind: match entry.kind() {
                EntryKind::Angle => "angle",
                EntryKind::Size => "size",
                _ => "skew",
            },
            fields: entry.fields().iter().map(EntryFieldView::of).collect(),
            linked: entry.linked(),
            scope: None,
            handle,
            center: box_.to_document(box_.local_center()),
            glyph_reach_px: if skew_beyond { 22.0 } else { 6.0 },
            at_centre: entry.centre_chip(),
            selection: true,
        })
    }

    /// The numeric entry to show, or `None` (criteria 18, 25, 26; 18, 19 of
    /// `unified-object-editing`; 33, 34, 36 of `multi-object-transform`).
    #[must_use]
    pub fn transform_entry(&self) -> Option<EntryView> {
        if let Some(entry) = self.open_group_entry() {
            return self.group_entry_view(entry);
        }
        if let Some(entry) = self.open_param_entry() {
            return self.param_entry_view(entry);
        }
        if let Some(entry) = self.open_skew_entry() {
            return Some(self.skew_entry_view(entry));
        }
        let entry = self.open_entry()?;
        let objects = self.objects();
        let handles = SelectTool::transform_handles(
            &objects,
            &self.selection,
            self.transform_handle_tolerances(),
            entry.side_rotate_revealed(),
        );
        // The anchor comes from the box, not from the drawn set, so the chip
        // of a key opens at a handle that is hidden too.
        let handle = if entry.centre_chip() {
            entry
                .start_box()
                .to_document(entry.start_box().local_center())
        } else {
            entry_anchor(
                entry.start_box(),
                entry.handle(),
                &self.transform_handle_tolerances(),
            )?
        };
        // An edge resize handle with a skew arrow on its side: the arrow
        // sits 16 px out and is 12 px deep, so the glyphs reach 22 px.
        let skew_beyond = matches!(
            entry.handle(),
            EditHandle::Resize(direction)
                if handles.iter().any(|(h, _)| matches!(
                    h, EditHandle::Skew(side) if side.direction() == direction
                ))
        );
        let box_ = entry.start_box();
        Some(EntryView {
            kind: match entry.kind() {
                EntryKind::Angle => "angle",
                EntryKind::Size => "size",
                EntryKind::OuterRadius => "radius",
                EntryKind::CornerRadius => "corner-radius",
                EntryKind::InnerRatio => "inner-ratio",
                EntryKind::Skew => "skew",
            },
            fields: entry.fields().iter().map(EntryFieldView::of).collect(),
            linked: entry.linked(),
            scope: None,
            handle,
            center: box_.to_document(box_.local_center()),
            glyph_reach_px: if skew_beyond { 22.0 } else { 6.0 },
            at_centre: entry.centre_chip(),
            selection: false,
        })
    }

    /// For linked fields (criterion 29): the text the *other* field takes
    /// after field `field` was edited to `text`; `None` if the fields are not
    /// linked or `text` is not a positive number yet.
    #[must_use]
    pub fn transform_entry_linked(&self, field: usize, text: &str) -> Option<String> {
        if let Some(entry) = self.open_group_entry() {
            return entry.linked_text(field, text);
        }
        self.open_entry()?.linked_text(field, text)
    }

    /// Enter in the chip: validates and commits (criteria 19, 21, 27, 30,
    /// 31). `first` and `second` are the field texts (`second` is ignored by
    /// a one-field entry), `last_edited` the index of the field the maker
    /// edited last. A committed or unchanged entry closes; a refused one
    /// stays open.
    pub fn commit_transform_entry(
        &mut self,
        first: &str,
        second: &str,
        last_edited: usize,
    ) -> EntryOutcome {
        if self.open_entry().is_none()
            && self.open_param_entry().is_none()
            && self.open_skew_entry().is_none()
            && self.open_group_entry().is_none()
        {
            self.select.cancel_entry();
            return EntryOutcome::Unchanged;
        }
        // A typed corner radius past its limit is committed limited, and says
        // so at the knob for a moment (criterion 6): the notice is built from
        // the entry as it was before it closes.
        let notice = self.open_param_entry().and_then(|entry| {
            let limited = entry.is_limited(first);
            let knob = self.param_entry_view(entry)?.handle;
            limited.then(|| (entry.clone(), knob))
        });
        let outcome = self
            .select
            .commit_entry(&self.document, [first, second], last_edited);
        if outcome == EntryOutcome::Committed
            && let Some((entry, anchor)) = notice
            && let Some(object) = self.document.object(entry.object().id())
            && let EditHandle::Param(param) = entry.handle()
            && let Some(text) = param_readout(
                &object,
                param,
                Some(ParamDragInfo {
                    limited: true,
                    overwrites_unequal: false,
                }),
            )
        {
            self.limit_notice = Some(super::shapes::LiveReadout { text, anchor });
        }
        outcome
    }

    /// Closes the entry without writing (criterion 20): Escape in the chip,
    /// a press elsewhere, a blur, a tool switch. Idempotent.
    pub fn cancel_transform_entry(&mut self) {
        self.select.cancel_entry();
    }
}
