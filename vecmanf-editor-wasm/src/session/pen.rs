//! `Session`'s Pen-tool glue (`specs/0002-path-node-editing`): finishing
//! the in-progress path, the pen's close-target cue and the pen's drag
//! threshold. Split out of `session/mod.rs` (`docs/technical-debt.md`,
//! "`Session` is one module past the size limit").

use vecmanf_document_core::Length;

use super::{PEN_DRAG_THRESHOLD_PX, Session, Tool};

impl Session {
    pub(super) fn drag_threshold(&self) -> Length {
        Length::from_mm(PEN_DRAG_THRESHOLD_PX / self.view().scale())
    }

    /// Acceptance criterion 3 / the dedicated "finish path" action
    /// (Enter, or a double-click the host has already recognized).
    pub fn finish_pen(&mut self) {
        self.pen.finish(&self.document);
    }

    /// The in-progress pen path's placed nodes, for the host's
    /// rubber-band/live-curve preview — `None` when idle or the node
    /// tool is active.
    #[must_use]
    pub fn pen_in_progress(&self) -> Option<&[vecmanf_document_core::NewAnchor]> {
        if self.tool == Tool::Pen {
            self.pen.in_progress_nodes()
        } else {
            None
        }
    }

    /// Acceptance criterion 5's cursor cue (`specification.md`'s
    /// "Cursors": "cursor swaps to a pen-with-small-circle... variant"):
    /// whether the live cursor is currently over the in-progress pen
    /// path's own close target. The host uses this to pick the cursor
    /// class; `false` outside the pen tool, with no path in progress, or
    /// before the pointer has ever moved over the canvas.
    #[must_use]
    pub fn is_hovering_pen_close_target(&self) -> bool {
        if self.tool != Tool::Pen {
            return false;
        }
        let Some(point) = self.pointer_position else {
            return false;
        };
        self.pen
            .is_hovering_close_target(point, self.point_tolerance_as_length())
    }
}
