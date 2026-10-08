//! A panel drag in flight and its single commit
//! (`specs/0007-stroke-and-fill-styling` criterion 36): while a slider or
//! colour area is dragged the selected objects are drawn in the new style
//! without touching the document, and the release writes exactly one commit,
//! to the objects the drag started on.

use curvyo_document_core::{
    Document, NodeId, ObjectSnapshot, StopChange, StopId, StyleEdit, StyleEditError,
};

use crate::style_stops::stop_edits;

/// The preview of the edit being dragged: a style property of whole objects,
/// or one value of one stop of each.
#[derive(Debug, Clone, PartialEq)]
enum Pending {
    Style {
        ids: Vec<NodeId>,
        edit: StyleEdit,
    },
    Stops {
        targets: Vec<(NodeId, StopId)>,
        change: StopChange,
    },
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
            Some(Pending::Style { edit: shown, .. }) => *shown = edit,
            _ => {
                self.pending = Some(Pending::Style {
                    ids: ids.to_vec(),
                    edit,
                });
            }
        }
    }

    /// Shows `change` on the stops `targets` name (one per edited object)
    /// instead of their stored values. The stops are fixed by the first call of
    /// a drag, so a thumb dragged past a neighbour keeps editing the stop it
    /// started on; later calls change only the value. Ignored after
    /// [`StyleEditor::cancel`] until the release.
    pub fn preview_stops(&mut self, targets: &[(NodeId, StopId)], change: StopChange) {
        if self.cancelled {
            return;
        }
        match &mut self.pending {
            Some(Pending::Stops { change: shown, .. }) => *shown = change,
            _ => {
                self.pending = Some(Pending::Stops {
                    targets: targets.to_vec(),
                    change,
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
            Some(Pending::Style { ids, edit }) => document.edit_style(&ids, &edit),
            Some(Pending::Stops { targets, change }) => {
                document.edit_stops(&stop_edits(&targets, change))
            }
            None => Ok(()),
        }
    }

    /// Replaces the style of the previewed objects (or the previewed stop of
    /// each) in `objects` with the previewed one, for drawing.
    pub fn apply_to(&self, objects: &mut [ObjectSnapshot]) {
        match &self.pending {
            Some(Pending::Style { ids, edit }) => {
                for object in objects.iter_mut().filter(|o| ids.contains(&o.id())) {
                    // A refused edit (a negative width) previews as nothing.
                    let _ = edit.apply_to(object.style_mut());
                }
            }
            Some(Pending::Stops { targets, change }) => {
                for object in objects.iter_mut() {
                    let Some((_, stop)) = targets.iter().find(|(id, _)| *id == object.id()) else {
                        continue;
                    };
                    if let Some(shown) = object
                        .style_mut()
                        .fill
                        .stops
                        .iter_mut()
                        .find(|s| s.id == *stop)
                    {
                        match change {
                            StopChange::Position(position) => shown.position = *position,
                            StopChange::Color(color) => shown.color = *color,
                            StopChange::Opacity(opacity) => shown.opacity = *opacity,
                        }
                    }
                }
            }
            None => {}
        }
    }
}
