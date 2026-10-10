//! What `Session` tells the host about shapes a stretch turns into paths
//! (`specs/0019-multi-object-transform/` criteria 23, 34, 54, 55): per-kind counts
//! only, as `[polygons, stars, rotated rectangles, rotated ellipses]`; the words
//! ("Stretching turns 2 shapes into paths") are the frontend's. A child module of
//! `session`.

use curvyo_ui_core::{ConversionCounts, EditHandle, SelectTool, is_corner};

use super::{Session, Tool};

fn as_vec(counts: ConversionCounts) -> Vec<u32> {
    if counts.is_empty() {
        Vec::new()
    } else {
        counts.as_array().to_vec()
    }
}

impl Session {
    /// The shapes an edge stretch of the selection would turn into paths, while
    /// the pointer rests on one of its edge resize handles (the extra line of the
    /// hint chip, criterion 55); empty on any other handle, for a selection with
    /// nothing to convert, or while a drag runs or an entry is open.
    #[must_use]
    pub fn hover_conversion_counts(&self) -> Vec<u32> {
        if self.tool != Tool::Select {
            return Vec::new();
        }
        let objects = self.objects();
        if self.selection.ids().len() >= 2 {
            return match self.group_hovered_handle(&objects) {
                Some((group, EditHandle::Resize(direction))) if !is_corner(direction) => {
                    as_vec(group.converting())
                }
                _ => Vec::new(),
            };
        }
        match self.select_hovered_handle(&objects) {
            Some((object, _, EditHandle::Resize(direction))) if !is_corner(direction) => {
                as_vec(ConversionCounts::of_single(&object))
            }
            _ => Vec::new(),
        }
    }

    /// The shapes the drag in flight would turn into paths if it were released now
    /// (the readout's second line, criterion 23); empty for any other drag, inside
    /// the dead zone, or while the preview is not a stretch.
    #[must_use]
    pub fn live_conversion_counts(&self) -> Vec<u32> {
        if self.tool != Tool::Select {
            return Vec::new();
        }
        let Some(pointer) = self.pointer_position else {
            return Vec::new();
        };
        as_vec(
            self.select
                .live_conversion_counts(pointer, self.held.shift, self.held.ctrl),
        )
    }

    /// The shapes the open size entry would turn into paths with the typed texts
    /// (the note under its fields, criterion 34); empty when it is not a stretch,
    /// a text is refused, or no such entry is open.
    #[must_use]
    pub fn entry_conversion_counts(
        &self,
        first: &str,
        second: &str,
        last_edited: usize,
    ) -> Vec<u32> {
        if self.tool != Tool::Select {
            return Vec::new();
        }
        let texts = [first, second];
        if let Some(entry) = self.select.group_entry() {
            return as_vec(entry.conversion_counts(texts, last_edited));
        }
        self.select
            .entry()
            .map(|entry| as_vec(entry.conversion_counts(texts, last_edited)))
            .unwrap_or_default()
    }

    /// The shapes the last commit turned into paths, once (the notice of criterion
    /// 54, "Stretching turned 2 shapes into paths. No undo yet."); empty when
    /// nothing was converted or the notice was already taken.
    pub fn take_conversion_notice(&mut self) -> Vec<u32> {
        as_vec(SelectTool::take_conversion_notice(&mut self.select))
    }
}
