//! `Session`'s glue for the Select bar (`specs/unified-object-editing`,
//! criteria 21, 21a, 22, 23): the bar's state for the current selection and the
//! commands its controls issue. All rules live in `curvyo_ui_core`
//! (`select_bar_state`, `SelectTool`'s bar edits); the DOM holds only the
//! open text, the invalid state and "Escape restores".

use curvyo_document_core::{InnerRatio, PointCount};
use curvyo_ui_core::{
    EntryOutcome, ParamValue, SelectBarState, build_primitive_conversions, select_bar_state,
};

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

    /// "Object to path" (acceptance criteria 17, 21, 22): converts every
    /// currently selected primitive to a path, in one
    /// [`curvyo_document_core::Document::convert_to_paths`] call
    /// (the anchor geometry itself is built by `curvyo-ui-core`'s own
    /// `build_primitive_conversions` — architect review: this facade
    /// must hold no editing logic of its own, ADR 0001 §1), then
    /// switches to the node tool. The shape handles/tool-options bar
    /// disappear outright because the object is no longer a primitive
    /// at all (`specification.md`'s "Primitive vs. path: handles don't
    /// coexist"). The Select bar's last control (`unified-object-editing`
    /// criterion 22): it acts on exactly the primitives of the selection.
    ///
    /// When exactly one primitive was selected, every one of its new
    /// anchors is selected in the node tool afterward, so it reads as
    /// "immediately editable" (acceptance criterion 17). The converted
    /// ids stay in `self.selection` — unlike the pre-`canvas-navigation-
    /// and-selection` behaviour, which cleared them — because "object to
    /// path" keeps the `NodeId` (`specs/0003-primitive-shapes/adrs.md`):
    /// they are still the same objects, now sharing their id space with
    /// the Select tool's own selection
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "a multi-
    /// object conversion therefore stays selected together at object
    /// level"). A multi-object conversion (acceptance criterion 22)
    /// additionally selects the first converted path's anchors the same
    /// way — `curvyo-ui-core`'s [`curvyo_ui_core::NodeSelection`] has no
    /// representation for "these anchors across several different paths
    /// are selected together", so that part remains an approximation of
    /// AC22's "remain selected together" wording, not a literal one — a
    /// known, narrowed scope (see this crate's own report).
    pub fn convert_selected_to_paths(&mut self) {
        self.flush_select_bar_preview();
        self.select.cancel_entry();
        let ids = self.selection.ids().to_vec();
        if ids.is_empty() {
            return;
        }
        let conversions = build_primitive_conversions(&self.document, &mut self.minter, &ids);
        if conversions.is_empty() {
            return;
        }
        let first_converted = conversions[0].0;
        if self.document.convert_to_paths(&conversions).is_ok() {
            self.tool = Tool::Node;
            if let Some(path) = self.document.path(first_converted) {
                self.node.select_all_anchors(&path);
            }
        }
    }

    /// "Remove rounding" of the Select bar (`unified-object-editing` criterion
    /// 21): zeroes the radius of every selected rectangle that has one, in one
    /// commit; the other objects of the selection are left alone.
    pub fn remove_corner_rounding(&mut self) {
        let objects = self.objects();
        self.select
            .remove_rounding(&self.document, &objects, &self.selection);
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::Point;

    use super::super::{Session, Tool};

    /// A session with a 10 mm rectangle drawn at the origin; the Select tool
    /// is active with the rectangle selected (a create-drag hands over).
    fn session_with_a_rectangle() -> Session {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);
        session
    }

    /// AC17, AC21: "object to path" only runs when explicitly invoked —
    /// switching tools leaves the primitive as it is;
    /// `convert_selected_to_paths` then replaces it with a path, keeping its
    /// id, and selects its anchors in the node tool.
    #[test]
    fn ac17_ac21_object_to_path_is_explicit_and_keeps_the_id() {
        let mut session = session_with_a_rectangle();
        let id = session.document.object_ids()[0];

        session.set_tool(Tool::Node);
        session.set_tool(Tool::Select);
        assert!(session.document.primitive(id).is_some());

        session.convert_selected_to_paths();

        assert!(session.document.primitive(id).is_none());
        let path = session.document.path(id).expect("same id, now a path");
        assert!(path.closed);
        assert_eq!(path.anchors.len(), 4);
        assert_eq!(session.tool(), Tool::Node);
        assert_eq!(
            session.node.selection().nodes().len(),
            4,
            "AC17: immediately editable"
        );
    }

    /// AC22: two different primitive kinds selected together both convert,
    /// independently, in one call.
    #[test]
    fn ac22_multi_object_conversion() {
        let mut session = session_with_a_rectangle();
        let rect_id = session.document.object_ids()[0];
        session.set_tool(Tool::Ellipse);
        session.pointer_down(Point::new(50.0, 50.0), false);
        session.pointer_up(Point::new(60.0, 60.0), false, false);
        let ellipse_id = session
            .document
            .object_ids()
            .into_iter()
            .find(|id| *id != rect_id)
            .expect("ellipse exists");

        session.selection.select_single(rect_id);
        session.selection.toggle(ellipse_id);
        session.convert_selected_to_paths();

        assert!(session.document.path(rect_id).is_some());
        assert!(session.document.path(ellipse_id).is_some());
    }
}
