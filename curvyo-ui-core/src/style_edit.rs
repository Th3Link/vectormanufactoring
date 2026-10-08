//! A panel drag in flight and its single commit
//! (`specs/0007-stroke-and-fill-styling` criterion 36): while a slider or
//! colour area is dragged the selected objects are drawn in the new style
//! without touching the document, and the release writes exactly one commit,
//! to the objects the drag started on.

use curvyo_document_core::{Document, NodeId, ObjectSnapshot, StyleEdit, StyleEditError};

/// The preview of the edit being dragged.
#[derive(Debug, Clone, PartialEq)]
struct Pending {
    ids: Vec<NodeId>,
    edit: StyleEdit,
}

/// The ephemeral style override of a panel drag. Not part of the document and
/// never saved.
#[derive(Debug, Clone, Default)]
pub struct StyleEditor {
    pending: Option<Pending>,
    /// Escape dropped the preview mid-drag: further previews are ignored until
    /// the release ends the gesture.
    cancelled: bool,
}

impl StyleEditor {
    /// Shows `edit` on `ids` instead of their stored style. The objects are
    /// fixed by the first call of a drag; later calls change only the edit.
    /// Ignored after [`StyleEditor::cancel`] until the release.
    pub fn preview(&mut self, ids: &[NodeId], edit: StyleEdit) {
        if self.cancelled {
            return;
        }
        match &mut self.pending {
            Some(pending) => pending.edit = edit,
            None => {
                self.pending = Some(Pending {
                    ids: ids.to_vec(),
                    edit,
                });
            }
        }
    }

    /// Whether a preview is showing.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.pending.is_some()
    }

    /// Escape during a drag: drops the preview so the objects return to their
    /// committed style, and the release then writes nothing.
    pub fn cancel(&mut self) {
        self.cancelled = self.pending.take().is_some();
    }

    /// The release: writes the previewed edit as one commit to the objects it
    /// started on, and ends the gesture. Writes nothing if there is no
    /// preview (none was made, or Escape dropped it).
    ///
    /// # Errors
    /// The refusal of [`Document::edit_style`]; the preview is dropped either
    /// way.
    pub fn commit(&mut self, document: &Document) -> Result<(), StyleEditError> {
        self.cancelled = false;
        match self.pending.take() {
            Some(pending) => document.edit_style(&pending.ids, &pending.edit),
            None => Ok(()),
        }
    }

    /// Replaces the style of the previewed objects in `objects` with the
    /// previewed one, for drawing.
    pub fn apply_to(&self, objects: &mut [ObjectSnapshot]) {
        let Some(pending) = &self.pending else {
            return;
        };
        for object in objects.iter_mut().filter(|o| pending.ids.contains(&o.id())) {
            // A refused edit (a negative width) previews as nothing.
            let _ = pending.edit.apply_to(object.style_mut());
        }
    }
}
