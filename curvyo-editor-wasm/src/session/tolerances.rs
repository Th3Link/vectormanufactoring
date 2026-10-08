//! The hit-test tolerances of `Session`, converted from screen pixels to
//! document millimetres at the current zoom. Split out of `session/mod.rs`
//! (`docs/technical-debt.md`, "`Session` is one module past the size limit")
//! with no change of behaviour.

use curvyo_document_core::{Length, Tolerance};
use curvyo_ui_core::HitTolerances;

use super::Session;

/// 16px node hit-test radius (`docs/design-system.md`; 2026-10-05:
/// doubled from 8px — customer feedback: "you can click on the nodes
/// too — the node squares and diamonds need to be bigger too," the same
/// fix one round earlier applied to `HANDLE_TOLERANCE_PX` below, now
/// extended to nodes alongside `curvyo-render-core::theme::
/// NODE_SIZE_PX`'s own doubling. Now equal to `HANDLE_TOLERANCE_PX` —
/// that is not a problem: `curvyo-ui-core::hit_test` picks the nearer
/// candidate regardless of either tolerance's value, a handle winning
/// only an exact tie, so two equal tolerances do not change which of a
/// coincident node and handle wins, only that both are now reachable
/// from farther away.
const POINT_TOLERANCE_PX: f64 = 16.0;
/// 16px handle hit-test radius (`docs/design-system.md`; 2026-10-05:
/// doubled from the node's own then-8px alongside the handle glyph's
/// doubled visual size, `curvyo-render-core::theme::
/// HANDLE_DIAMETER_PX`'s own doc comment).
const HANDLE_TOLERANCE_PX: f64 = 16.0;
/// 4px segment hit-test tolerance (`docs/design-system.md`): the Node tool's
/// segment pick and the shape tools' outline test.
const SEGMENT_TOLERANCE_PX: f64 = 4.0;
/// 8px object hit-test tolerance of the Select tool (`advanced-selection`
/// criterion 1; `docs/design-system.md`): twice the segment tolerance, the
/// same margin the node and handle radii got. Separate from the segment
/// tolerance, which stays 4px where several segments of one path run close
/// together (criterion 2).
const OBJECT_TOLERANCE_PX: f64 = 8.0;

impl Session {
    pub(super) fn point_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(POINT_TOLERANCE_PX / self.view().scale())
    }

    pub(super) fn handle_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(HANDLE_TOLERANCE_PX / self.view().scale())
    }

    pub(super) fn segment_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(SEGMENT_TOLERANCE_PX / self.view().scale())
    }

    /// The Select tool's outline tolerance: a press, hover, cursor, badge or
    /// double-click within it of an object's outline hits the object.
    pub(super) fn object_tolerance(&self) -> Tolerance {
        Tolerance::from_mm(OBJECT_TOLERANCE_PX / self.view().scale())
    }

    pub(super) fn hit_tolerances(&self) -> HitTolerances {
        HitTolerances {
            point: self.point_tolerance(),
            handle: self.handle_tolerance(),
            segment: self.segment_tolerance(),
        }
    }

    pub(super) fn point_tolerance_as_length(&self) -> Length {
        Length::from_mm(self.point_tolerance().as_mm())
    }
}
