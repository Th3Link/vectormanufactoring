//! `Session`'s glue for the Style panel (`specs/0007-stroke-and-fill-styling`,
//! criteria 5, 6, 13, 14, 24, 36, 37): the panel's state for the current tool
//! and selection, the commands its controls issue, and the ephemeral drag
//! preview drawn in place of the stored style. All rules live in
//! `curvyo_ui_core` (`style_scope`, `style_panel_state`, `StyleEditor`); the DOM
//! holds only the open text, the invalid state and "Escape restores".

use curvyo_document_core::{Color, LineCap, LineJoin, StyleEdit};
use curvyo_ui_core::{
    DashChoice, StyleEntryError, StyleField, StylePanelState, StyleScope, StyleTool,
    style_panel_state, style_scope,
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

    /// What the Style panel shows. A drag preview shows as if it were
    /// committed, so the fields follow the drag.
    #[must_use]
    pub fn style_panel_state(&self) -> StylePanelState {
        let mut objects = self.objects();
        let scope = style_scope(
            self.style_tool(),
            &objects,
            &self.selection,
            self.node.selection(),
        );
        self.style.apply_to(&mut objects);
        style_panel_state(&objects, &scope)
    }

    /// [`Session::style_panel_state`] as the flat record the host reads.
    #[must_use]
    pub fn style_panel_view(&self) -> StylePanelView {
        let key = format!("{:?}", self.style_scope().ids);
        StylePanelView::new(&self.style_panel_state(), key)
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

    /// The stroke Paint switch (criterion 5): one commit.
    pub fn set_stroke_paint(&mut self, on: bool) {
        self.apply_style_edit(&StyleEdit::StrokeEnabled(on));
    }

    /// The Dash select: one commit. `Custom` is not a choice and writes
    /// nothing.
    pub fn set_stroke_dash(&mut self, choice: DashChoice) {
        if let Some(pattern) = choice.pattern() {
            self.apply_style_edit(&StyleEdit::StrokeDash(pattern));
        }
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
