//! `Session`'s glue for the Style panel (`specs/0007-stroke-and-fill-styling`,
//! criteria 5, 6, 13, 14, 24, 36, 37): the panel's state for the current tool
//! and selection, the commands its controls issue, and the ephemeral drag
//! preview drawn in place of the stored style. All rules live in
//! `curvyo_ui_core` (`style_scope`, `style_panel_state`, `StyleEditor`); the DOM
//! holds only the open text, the invalid state and "Escape restores".

use curvyo_document_core::{Color, LineCap, LineJoin, StyleEdit};
use curvyo_ui_core::{
    BarValue, DashChoice, Grid, StyleEntryError, StyleField, StylePanelState, StyleScope,
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
        let mut shown = committed.clone();
        self.style.apply_to(&mut shown);
        style_panel_state(&committed, &shown, &scope)
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
        // A width dragged to 0 previews as a stroke that is off with its last
        // width kept; the field shows the 0 the drag is at (criterion 8).
        if let Some(StyleEdit::StrokeWidth(width)) = self.style.pending_edit() {
            view.show_width(width.as_mm());
        }
        view
    }

    /// Whether a canvas pointer press is in flight (the button is down): the
    /// host ignores `Shift+Ctrl+F` during one (criterion 38).
    #[must_use]
    pub const fn is_pointer_down(&self) -> bool {
        self.button_down
    }

    /// Commits a panel drag still pending, against the objects it started on,
    /// before anything that can change the selection or the tool (a release
    /// outside the control never fires the control's own).
    pub(super) fn flush_style_preview(&mut self) {
        let _ = self.style.commit(&self.document);
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

    /// A colour area's or hue slider's drag tick: shows `color` on the edited
    /// objects without writing anything.
    pub fn preview_style_color(&mut self, field: StyleField, color: Color) {
        if let Some(edit) = field.color_edit(color) {
            self.preview_style_edit(edit);
        }
    }

    /// A tick of a drag in the colour area or the hue slider: shows the colour
    /// of hue `hue` (degrees), saturation and value (`0` to `1`) without
    /// writing. Only the colour changes; each object keeps its alpha.
    pub fn preview_style_hsv(&mut self, field: StyleField, hue: f64, saturation: f64, value: f64) {
        self.preview_style_color(field, hsv_to_rgb(hue, saturation, value));
    }

    /// An opacity slider's drag tick (a percent): shows it without writing.
    pub fn preview_style_opacity(&mut self, field: StyleField, percent: f64) {
        if let Some(edit) = field.opacity_edit(percent) {
            self.preview_style_edit(edit);
        }
    }

    fn preview_style_edit(&mut self, edit: StyleEdit) {
        // The objects are fixed by the first tick of a drag; later ticks skip
        // reading the document for them.
        let ids = if self.style.is_active() {
            Vec::new()
        } else {
            self.style_scope().ids
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
        let shown = match field {
            ValueField::StrokeWidth => match state.stroke.width {
                BarValue::Uniform(width) => width.as_mm(),
                BarValue::Mixed => return,
            },
            ValueField::StrokeOpacity => match state.stroke.opacity {
                BarValue::Uniform(opacity) => opacity.get() * 100.0,
                BarValue::Mixed => return,
            },
            ValueField::FillOpacity => match state.fill.opacity {
                BarValue::Uniform(opacity) => opacity.get() * 100.0,
                BarValue::Mixed => return,
            },
        };
        self.preview_style_edit(field.edit(field.scale().step(shown, steps, grid)));
    }

    /// The reset icon or `Ctrl+Backspace` (criterion 61): every edited object
    /// takes the field's default, one commit. Writes nothing when the value
    /// already is the default; `false` when there is nothing to edit.
    pub fn reset_value_field(&mut self, field: ValueField) -> bool {
        self.apply_style_edit(&field.reset_edit())
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
    fn apply_style_edit(&mut self, edit: &StyleEdit) -> bool {
        self.flush_style_preview();
        let ids = self.style_scope().ids;
        !ids.is_empty() && self.document.edit_style(&ids, edit).is_ok()
    }
}
