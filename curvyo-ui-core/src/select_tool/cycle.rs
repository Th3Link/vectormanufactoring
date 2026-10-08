//! The Alt-click cycle (`specs/0014-advanced-selection/specification.md`,
//! criteria 3 to 7): which objects lie under the point of the last plain
//! click and which one an Alt-click on that point selects next.

use curvyo_document_core::{NodeId, ObjectSnapshot, Point, Tolerance};

use super::PressTarget;
use super::handles::sole_selected;
use crate::hit_test_object::hit_test_objects;
use crate::object_selection::ObjectSelection;

/// A cycle through the candidates under one point. The candidate list is
/// captured the first time the cycle advances and kept, so the order stays
/// stable while the pointer jitters inside the radius (criterion 5); the
/// list is `hit_test_objects`, whose first element is a plain click's answer.
#[derive(Debug, Clone)]
pub(super) struct ClickCycle {
    /// Where the cycle started: the plain click, or the first Alt-click.
    at: Point,
    /// The candidate the selection stands on now, `None` before the first
    /// step of a cycle that no plain click began.
    current: Option<NodeId>,
    /// The candidates under `at`, empty until the first step.
    candidates: Vec<NodeId>,
}

impl ClickCycle {
    /// The cycle a plain press on `target` begins: one on an object, or inside
    /// the sole selected box (its object); none for a handle, nothing, Alt
    /// (that press continues the cycle at its release) or with Shift held.
    pub(super) fn begun_by(
        target: PressTarget,
        point: Point,
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        shift: bool,
    ) -> Option<Self> {
        if shift {
            return None;
        }
        match target {
            PressTarget::Object(hit) => Some(Self::after_click(point, hit)),
            PressTarget::InsideSelectedBox => sole_selected(objects, selection)
                .map(|selected| Self::after_click(point, selected.id())),
            PressTarget::Handle(_)
            | PressTarget::CentreHandle
            | PressTarget::Empty
            | PressTarget::Lasso => None,
        }
    }

    /// The cycle a plain click at `at` begins: the click selected `selected`,
    /// so the next step is the candidate after it.
    pub(super) const fn after_click(at: Point, selected: NodeId) -> Self {
        Self {
            at,
            current: Some(selected),
            candidates: Vec::new(),
        }
    }

    /// A cycle with no plain click behind it: its first step selects the
    /// nearest candidate (criterion 6).
    pub(super) const fn fresh(at: Point) -> Self {
        Self {
            at,
            current: None,
            candidates: Vec::new(),
        }
    }

    /// Whether a click at `point` continues this cycle: it lies within
    /// `tolerance` (the hit radius, criterion 6's 8 px) of where it started.
    pub(super) fn continues_at(&self, point: Point, tolerance: Tolerance) -> bool {
        self.at.vector_to(point).length() <= tolerance.as_mm()
    }

    /// The next candidate, wrapping around (criterion 5); a one-candidate
    /// cycle returns the same object again (criterion 7). `None` when nothing
    /// lies under the start point. An id that no longer names an object is
    /// dropped first.
    pub(super) fn advance(
        &mut self,
        objects: &[ObjectSnapshot],
        tolerance: Tolerance,
    ) -> Option<NodeId> {
        if self.candidates.is_empty() {
            self.candidates = hit_test_objects(objects, self.at, tolerance);
        }
        self.candidates
            .retain(|id| objects.iter().any(|object| object.id() == *id));
        let count = self.candidates.len();
        let next = self
            .current
            .and_then(|id| self.candidates.iter().position(|&c| c == id))
            .map_or(0, |index| (index + 1) % count.max(1));
        let chosen = self.candidates.get(next).copied();
        self.current = chosen;
        chosen
    }
}
