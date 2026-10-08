//! `Session`'s glue for the gradient stop editor
//! (`specs/0007-stroke-and-fill-styling` criteria 17 to 20, 34, 35): selecting a
//! stop, editing one value of the stop of a rank, a drag of it, and adding and
//! removing stops. The rules (rank, the values a new stop gets, the seeds) are
//! `curvyo_ui_core`'s (`style_stops`); this holds the selection and issues the
//! commands.

use curvyo_document_core::{FillKind, FillMode, FillModeTarget, GradientStop, StopChange};
use curvyo_ui_core::{
    StopField, StyleEntryError, new_stop_values, selected_rank, stop_edits, stop_targets,
};

use super::Session;

impl Session {
    /// The rank, in the first edited object, of the selected stop, or `None`.
    pub(super) fn selected_stop_rank(&self) -> Option<usize> {
        let objects = self.objects();
        let ids = self.style_scope().ids;
        selected_rank(&objects, &ids, &self.selected_stops)
    }

    /// Selects the stop of `rank` in every edited object (a row got focus or a
    /// thumb was pressed). The selection holds stop ids, so it follows the stop
    /// when an edit re-sorts the list.
    pub fn select_stop(&mut self, rank: usize) {
        let targets = stop_targets(&self.objects(), &self.style_scope().ids, rank);
        if !targets.is_empty() {
            self.selected_stops = targets;
        }
    }

    /// Enter or Tab in a stop field: one commit changing one value of the stop
    /// of `rank` in every edited object, and that stop is now the selected one.
    /// `Ok(false)` when there is no such stop.
    ///
    /// # Errors
    /// The parse error of the field's own kind of value; nothing is written.
    pub fn set_stop_text(
        &mut self,
        rank: usize,
        field: StopField,
        text: &str,
    ) -> Result<bool, StyleEntryError> {
        let change = field.parse_text(text)?;
        Ok(self.apply_stop_change(rank, change))
    }

    fn apply_stop_change(&mut self, rank: usize, change: StopChange) -> bool {
        self.flush_style_preview();
        let targets = stop_targets(&self.objects(), &self.style_scope().ids, rank);
        if targets.is_empty() {
            return false;
        }
        let written = self
            .document
            .edit_stops(&stop_edits(&targets, change))
            .is_ok();
        self.selected_stops = targets;
        written
    }

    /// A tick of a drag on the stop of `rank` (a thumb, a colour area, a
    /// slider): shows `value` (a percent, or `0xRRGGBB` for the colour) on the
    /// stop without writing. The stop is fixed by the first tick of the drag.
    pub fn preview_stop(&mut self, rank: usize, field: StopField, value: f64) {
        let targets = if self.style.is_active() {
            Vec::new()
        } else {
            stop_targets(&self.objects(), &self.style_scope().ids, rank)
        };
        if self.style.is_active() || !targets.is_empty() {
            self.style.preview_stops(&targets, field.drag_change(value));
            if !targets.is_empty() {
                self.selected_stops = targets;
            }
        }
    }

    /// Adds a stop to the one edited gradient in one commit: at `at` (a
    /// fraction, a click on the bar) or, for the Add button (`None`), in the
    /// middle of the widest gap, with the colour and opacity the ramp has
    /// there. The new stop is selected. `false` when it is refused: several
    /// objects are edited, the gradient already holds 16 stops, or nothing is
    /// being edited.
    pub fn add_stop(&mut self, at: Option<f64>) -> bool {
        self.flush_style_preview();
        let objects = self.objects();
        let ids = self.style_scope().ids;
        let [id] = ids[..] else {
            return false;
        };
        let Some(object) = objects.iter().find(|object| object.id() == id) else {
            return false;
        };
        let new = new_stop_values(&object.style().fill, at);
        let stop = GradientStop {
            id: self.minter.mint_stop(),
            position: new.position,
            color: new.color,
            opacity: new.opacity,
        };
        let added = if object.style().fill.stops.is_empty() {
            // The object has no stop list yet, and only the fill-mode switch
            // creates one: it takes this stop as its seed.
            let mode = match object.style().fill.kind {
                FillKind::Radial => FillMode::Radial,
                _ => FillMode::Linear,
            };
            let target = FillModeTarget {
                id,
                seed_stops: vec![stop],
            };
            self.document.set_fill_mode(mode, &[target]).is_ok()
        } else {
            self.document.add_stop(id, stop).is_ok()
        };
        if added {
            self.selected_stops = vec![(id, stop.id)];
        }
        added
    }

    /// Removes the stop of `rank` from the one edited gradient in one commit.
    /// `false` when it is refused: several objects are edited, or only two
    /// stops remain.
    pub fn remove_stop(&mut self, rank: usize) -> bool {
        self.flush_style_preview();
        let ids = self.style_scope().ids;
        if ids.len() != 1 {
            return false;
        }
        let targets = stop_targets(&self.objects(), &ids, rank);
        let [(id, stop)] = targets[..] else {
            return false;
        };
        let removed = self.document.remove_stop(id, stop).is_ok();
        if removed {
            self.selected_stops.clear();
        }
        removed
    }
}
