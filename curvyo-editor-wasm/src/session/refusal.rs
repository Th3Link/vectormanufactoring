//! The red outline of the objects a refused rail command names (`specs/0016-boolean-operations`
//! criterion 15, `0035-combine-and-break-apart` criteria 9 to 11): one mark for any command, kept
//! only while the selection and the tool it was refused for stay.

use curvyo_document_core::{NodeId, ObjectSnapshot};
use curvyo_ui_core::ObjectSelection;

use super::{Session, Tool};

/// At most this many offenders are outlined; the count in the sentence is exact.
const MAX_OUTLINED: usize = 200;

/// The objects a refusal outlines, and the selection it was refused for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RefusalMarks {
    offenders: Vec<NodeId>,
    selection: Vec<NodeId>,
}

impl RefusalMarks {
    /// The mark for a refusal of `selection` that names `offenders`.
    pub(super) fn new(offenders: Vec<NodeId>, selection: &ObjectSelection) -> Self {
        Self {
            offenders,
            selection: selection.ids().to_vec(),
        }
    }
}

impl Session {
    /// Removes the red outline of a refusal of any rail command, Boolean or Path (the host's
    /// timer ran out, or the next action).
    pub fn clear_boolean_refusal(&mut self) {
        self.command_refusal = None;
    }

    /// The offending objects of the refusal that is showing, or none once the selection or the
    /// tool has changed since.
    pub(super) fn refusal_objects(&self, objects: &[ObjectSnapshot]) -> Vec<ObjectSnapshot> {
        let Some(marks) = &self.command_refusal else {
            return Vec::new();
        };
        if self.tool != Tool::Select || self.selection.ids() != marks.selection.as_slice() {
            return Vec::new();
        }
        objects
            .iter()
            .filter(|object| marks.offenders.contains(&object.id()))
            .take(MAX_OUTLINED)
            .cloned()
            .collect()
    }
}
