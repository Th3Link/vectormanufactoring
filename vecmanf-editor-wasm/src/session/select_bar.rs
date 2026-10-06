//! `Session`'s glue for the Select bar (`specs/unified-object-editing`,
//! criteria 21, 21a, 22, 23): the bar's state for the current selection and the
//! commands its controls issue. All rules live in `vecmanf_ui_core`
//! (`select_bar_state`, `SelectTool`'s bar edits); the DOM holds only the
//! open text, the invalid state and "Escape restores".

use vecmanf_document_core::{InnerRatio, PointCount};
use vecmanf_ui_core::{EntryOutcome, ParamValue, SelectBarState, select_bar_state};

use super::{Session, Tool};

impl Session {
    /// What the Select bar shows for the current selection: which kind
    /// controls are shown, their values ("Mixed" where the selected objects
    /// differ) and whether "Remove rounding" would change anything. A pending
    /// slider edit shows as if it were committed. The two switches are not
    /// part of it ([`Session::scale_stroke_width`],
    /// [`Session::scale_corner_radius`]).
    #[must_use]
    pub fn select_bar_state(&self) -> SelectBarState {
        select_bar_state(&self.objects(), &self.selection, self.select.bar_preview())
    }

    /// Commits a slider edit still pending, against the objects it was
    /// started on, before anything that can change the selection (a released
    /// slider outside its element never fires its own release): a tool switch,
    /// a canvas press, Delete, "Object to path".
    pub(super) fn flush_select_bar_preview(&mut self) {
        self.select.flush_bar_preview(&self.document);
    }

    /// The typed "Radius" field (criterion 21a): Enter writes the value to
    /// every selected rectangle in one commit. `"committed"`, `"unchanged"`
    /// (the field keeps the shown value) or a refusal: the field stays open
    /// marked invalid and nothing is written.
    pub fn set_selected_radius_text(&mut self, text: &str) -> EntryOutcome {
        if self.tool != Tool::Select {
            return EntryOutcome::Unchanged;
        }
        let objects = self.objects();
        self.select
            .commit_bar_radius_text(&self.document, &objects, &self.selection, text)
    }

    /// The "Points" field and its stepper (criterion 21): one commit for
    /// every selected polygon and star.
    pub fn set_selected_point_count(&mut self, count: PointCount) {
        let objects = self.objects();
        let _ = self.select.commit_bar_value(
            &self.document,
            &objects,
            &self.selection,
            ParamValue::PointCount(count),
        );
    }

    /// The "Ratio" field's discrete commit (a typed value or a stepper
    /// click): one commit for every selected star.
    pub fn set_selected_ratio(&mut self, ratio: InnerRatio) {
        let objects = self.objects();
        let _ = self.select.commit_bar_value(
            &self.document,
            &objects,
            &self.selection,
            ParamValue::Ratio(ratio),
        );
    }

    /// The "Ratio" slider's live preview, on every tick: nothing is written;
    /// the stars show in blue over their unchanged old shape.
    pub fn preview_selected_ratio(&mut self, ratio: InnerRatio) {
        let objects = self.objects();
        self.select
            .preview_bar_value(&objects, &self.selection, ParamValue::Ratio(ratio));
    }

    /// Commits the slider edit pending as one commit, when the slider is
    /// released.
    pub fn commit_selected_ratio(&mut self) {
        self.flush_select_bar_preview();
    }

    /// "Remove rounding" of the Select bar: zeroes the radius of every
    /// selected rectangle that has one, in one commit.
    pub(super) fn select_remove_rounding(&mut self) {
        let objects = self.objects();
        self.select
            .remove_rounding(&self.document, &objects, &self.selection);
    }
}
