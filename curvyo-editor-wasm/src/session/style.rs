//! `Session`'s glue for the Style panel (`specs/0007-stroke-and-fill-styling`,
//! criteria 5, 6, 13, 14, 24, 36, 37): the panel's state for the current tool
//! and selection, the commands its controls issue, and the ephemeral drag
//! preview drawn in place of the stored style. All rules live in
//! `curvyo_ui_core` (`style_scope`, `style_panel_state`, `StyleEditor`); the DOM
//! holds only the open text, the invalid state and "Escape restores".

use curvyo_document_core::{LineCap, LineJoin, MarkerPlace, MarkerShape, NodeId, StyleEdit};
use curvyo_ui_core::{
    DashChoice, Grid, MarkerSlot, StyleEntryError, StyleField, StylePanelState, StyleScope,
    StyleTool, ValueField, hsv_to_rgb, parse_dash_text, style_panel_state, style_scope,
};

use super::style_view::StylePanelView;
use super::{Session, Tool};

impl Session {
    fn style_tool(&self) -> StyleTool {
        match self.tool {
            Tool::Pen => StyleTool::Pen,
            Tool::Node => StyleTool::Node,
            Tool::Select | Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar => StyleTool::Other,
        }
    }

    /// The objects the panel edits now and the line that says so
    /// (criterion 37).
    #[must_use]
    pub fn style_scope(&self) -> StyleScope {
        style_scope(
            self.style_tool(),
            &self.objects(),
            &self.selection,
            self.node.selection(),
        )
    }

    /// What the Style panel shows, or `None` when there is nothing to edit. A
    /// drag preview shows as if it were committed, so the fields follow the
    /// drag; which rows are shown follows the committed document.
    #[must_use]
    pub fn style_panel_state(&self) -> Option<StylePanelState> {
        let committed = self.objects();
        let scope = style_scope(
            self.style_tool(),
            &committed,
            &self.selection,
            self.node.selection(),
        );
        if !self.style.is_active() {
            return style_panel_state(&committed, &committed, &scope);
        }
        let mut shown = committed.to_vec();
        self.style.apply_to(&mut shown);
        style_panel_state(&committed, &shown, &scope)
            .map(|state| state.with_pending(self.style.pending_edit()))
    }

    /// [`Session::style_panel_state`] as the flat record the host reads.
    #[must_use]
    pub fn style_panel_view(&self) -> StylePanelView {
        let key = format!("{:?}", self.style_scope().ids);
        let mut view = self
            .style_panel_state()
            .map_or_else(StylePanelView::empty, |state| {
                StylePanelView::new(&state, key)
            });
        view.pick_target = self
            .colour_pick_target()
            .map_or_else(String::new, |target| target.name().to_string());
        view
    }

    /// Whether a canvas pointer press is in flight (the button is down): the
    /// host ignores `Shift+Ctrl+F` during one (criterion 38).
    #[must_use]
    pub const fn is_pointer_down(&self) -> bool {
        self.button_down
    }

    /// Commits a panel drag still pending, against the objects it started on
    /// (and a Background block drag, `0040` criterion 23), before anything that can
    /// change the selection or the tool (a release outside the control never fires
    /// the control's own).
    pub(super) fn flush_style_preview(&mut self) {
        let _ = self.style.commit(&self.document);
        self.commit_background_preview();
    }

    /// Enter or Tab in a typed field: `Ok(true)` after one commit for every
    /// edited object, `Ok(false)` when there is nothing to edit, or the reason
    /// the text was refused (nothing is written and the field stays open).
    ///
    /// # Errors
    /// The parse error of the field's own kind of value.
    pub fn set_style_text(
        &mut self,
        field: StyleField,
        text: &str,
    ) -> Result<bool, StyleEntryError> {
        let edit = field.parse_text(text)?;
        Ok(self.apply_style_edit(&edit))
    }

    /// A tick of a drag in the colour area or the hue slider: shows the colour
    /// of hue `hue` (degrees), saturation and value (`0` to `1`) without
    /// writing. Only the colour changes; each object keeps its alpha. A field
    /// that is not a colour is ignored.
    pub fn preview_style_hsv(&mut self, field: StyleField, hue: f64, saturation: f64, value: f64) {
        let color = hsv_to_rgb(hue, saturation, value);
        let edit = match field {
            StyleField::StrokeColor => StyleEdit::StrokeColor(color),
            StyleField::FillColor => StyleEdit::FillColor(color),
            _ => return,
        };
        self.preview_style_edit(edit);
    }

    fn preview_style_edit(&mut self, edit: StyleEdit) {
        // The objects are fixed by the first tick of a drag; later ticks skip
        // reading the document for them.
        let ids = if self.style.is_active() {
            Vec::new()
        } else {
            self.edit_ids(&edit)
        };
        if self.style.is_active() || !ids.is_empty() {
            self.style.preview(&ids, edit);
        }
    }

    /// The release (or key-up): writes the previewed edit as one commit to the
    /// objects the drag started on. Writes nothing after Escape.
    pub fn commit_style_preview(&mut self) {
        self.flush_style_preview();
    }

    /// Escape during a panel drag: the objects return to their committed
    /// style and the release then writes nothing.
    pub fn cancel_style_preview(&mut self) {
        self.style.cancel();
    }

    /// A tick of a value field drag (`specs/0017-style-panel-rework` criteria
    /// 36 to 47): the field's scale maps position `p` (`0` to `1`) to a value,
    /// rounded to `grid`; it is shown without writing. The gesture ends with
    /// [`Session::commit_style_preview`] or [`Session::cancel_style_preview`].
    pub fn preview_value_field(&mut self, field: ValueField, p: f64, grid: Grid) {
        // A tick that is not a number shows and writes nothing.
        if !p.is_finite() {
            return;
        }
        let scale = field.scale();
        let value = scale.round(scale.value_at(p), grid);
        self.preview_style_edit(field.edit(value));
    }

    /// An arrow key on a value field (criterion 43): `steps` steps from the
    /// shown value on `grid`, previewed; the key-up commits. Does nothing while
    /// the edited objects differ (criterion 44) or when there is nothing to edit.
    pub fn step_value_field(&mut self, field: ValueField, steps: i32, grid: Grid) {
        let Some(state) = self.style_panel_state() else {
            return;
        };
        let Some(shown) = state.value_of(field) else {
            return;
        };
        self.preview_style_edit(field.edit(field.scale().step(shown, steps, grid)));
    }

    /// The reset icon or `Ctrl+Backspace` (criterion 61): every edited object
    /// takes the field's default, one commit. Writes nothing when the value
    /// already is the default; `false` when there is nothing to edit.
    pub fn reset_value_field(&mut self, field: ValueField) -> bool {
        self.apply_style_edit(&field.reset_edit())
    }

    /// The objects `edit` goes to (the rule lives in `StyleScope::targets`).
    fn edit_ids(&self, edit: &StyleEdit) -> Vec<NodeId> {
        self.style_scope().targets(&self.objects(), edit)
    }

    /// A marker slot choice (`specs/0018-stroke-markers` criteria 1 and 22):
    /// one commit to the paths of the scope.
    pub fn set_marker_shape(&mut self, slot: MarkerSlot, shape: MarkerShape) {
        let edit = match slot {
            MarkerSlot::Start => StyleEdit::MarkerStart(shape),
            MarkerSlot::Mid => StyleEdit::MarkerMid(shape),
            MarkerSlot::End => StyleEdit::MarkerEnd(shape),
        };
        self.apply_style_edit(&edit);
    }

    /// The Place group (criterion 2): one commit to the paths of the scope.
    pub fn set_marker_place(&mut self, place: MarkerPlace) {
        self.apply_style_edit(&StyleEdit::MarkerPlace(place));
    }

    /// The stroke Paint switch (criterion 5): one commit.
    pub fn set_stroke_paint(&mut self, on: bool) {
        self.apply_style_edit(&StyleEdit::StrokeEnabled(on));
    }

    /// A Dash preset button: one commit.
    pub fn set_stroke_dash(&mut self, choice: DashChoice) {
        self.apply_style_edit(&StyleEdit::StrokeDash(choice.pattern()));
    }

    /// Enter or Tab in the pattern line: one commit, `Ok(false)` when there is
    /// nothing to edit.
    ///
    /// # Errors
    /// [`StyleEntryError::Dash`] for text that is no pattern; nothing is
    /// written and the line stays open.
    pub fn set_stroke_dash_text(&mut self, text: &str) -> Result<bool, StyleEntryError> {
        let pattern = parse_dash_text(text)?;
        Ok(self.apply_style_edit(&StyleEdit::StrokeDash(pattern)))
    }

    /// The Join toggle group: one commit.
    pub fn set_stroke_join(&mut self, join: LineJoin) {
        self.apply_style_edit(&StyleEdit::StrokeJoin(join));
    }

    /// The Cap toggle group: one commit.
    pub fn set_stroke_cap(&mut self, cap: LineCap) {
        self.apply_style_edit(&StyleEdit::StrokeCap(cap));
    }

    /// The fill Paint switch (criterion 13): one commit. The stored colour and
    /// opacity stay.
    pub fn set_fill_paint(&mut self, on: bool) {
        self.apply_style_edit(&StyleEdit::FillEnabled(on));
    }

    /// Writes `edit` to every edited object in one commit. `false` when there
    /// is nothing to edit or the document refused it.
    pub(super) fn apply_style_edit(&mut self, edit: &StyleEdit) -> bool {
        self.flush_style_preview();
        let ids = self.edit_ids(edit);
        !ids.is_empty() && self.document.edit_style(&ids, edit).is_ok()
    }
}
