//! The session's boolean command: what the rail buttons show, the one write
//! (`Document::replace_with_path`) and the red outline of the objects a refusal names
//! (`specs/0016-boolean-operations`).

use curvyo_ui_core::{
    BooleanAvailability, BooleanOp, BooleanRefusal, ObjectSelection, boolean_availability,
    plan_boolean,
};

use super::refusal::RefusalMarks;
use super::{Session, Tool};

/// What [`Session::apply_boolean`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanOutcome {
    /// Nothing: the Select tool is not active, a drag is in flight or a typed entry is open (the
    /// entry is not discarded), or the objects went away.
    Ignored,
    /// The operands were replaced by one object.
    Applied {
        /// How many objects took part.
        operands: usize,
        /// Whether the result is a compound path.
        compound: bool,
    },
    /// Refused; nothing was changed. The offending objects, if any, are outlined until the
    /// selection or the tool changes or the host clears them.
    Refused(BooleanRefusal),
}

impl Session {
    /// The selection the buttons act on: the object selection while the Select tool is active,
    /// otherwise none (the selection is not drawn then, and a command must act only on a
    /// selection the maker can see).
    pub(super) fn boolean_selection(&self) -> ObjectSelection {
        if self.tool == Tool::Select {
            self.selection.clone()
        } else {
            ObjectSelection::new()
        }
    }

    /// Whether a rail command may run now: the Select tool is active, no drag is in flight and no
    /// typed entry is open (the entry is not discarded).
    pub(super) fn rail_command_ready(&self) -> bool {
        self.tool == Tool::Select
            && !self.select.drag_in_flight()
            && self.move_entry().is_none()
            && self.transform_entry().is_none()
    }

    /// What the Boolean toolbox of the tool rail shows now. Read it after every pointer release
    /// and tool or selection change.
    #[must_use]
    pub fn boolean_availability(&self) -> BooleanAvailability {
        boolean_availability(&self.objects(), &self.boolean_selection())
    }

    /// Runs `op` on the selected objects: plans it, then replaces the operands by the result in
    /// one commit labelled `boolean_<op>`, and selects the result alone. The tool stays Select.
    /// A refusal changes nothing (no commit, same selection) and outlines its offenders.
    pub fn apply_boolean(&mut self, op: BooleanOp) -> BooleanOutcome {
        if !self.rail_command_ready() {
            return BooleanOutcome::Ignored;
        }
        self.flush_select_bar_preview();
        self.end_colour_pick();
        let objects = self.objects();
        let selection = self.boolean_selection();
        let plan = match plan_boolean(&objects, &selection, op, &mut self.minter) {
            Ok(plan) => plan,
            Err(refusal) => {
                self.command_refusal = match &refusal {
                    BooleanRefusal::OpenPaths { offenders, .. }
                    | BooleanRefusal::NoArea { offenders, .. }
                    | BooleanRefusal::OutOfRange { offenders, .. } => {
                        Some(RefusalMarks::new(offenders.clone(), &selection))
                    }
                    BooleanRefusal::NeedsTwo | BooleanRefusal::Empty => None,
                };
                return BooleanOutcome::Refused(refusal);
            }
        };
        let replaced = self.document.replace_with_path(
            &plan.operands,
            plan.base,
            &plan.outlines,
            op.commit_label(),
        );
        let Ok(result) = replaced else {
            // invariant: the plan was made from the objects just read, with a base among the
            // operands, at least one outline and fresh anchor ids, so the write cannot fail.
            debug_assert!(false, "a planned boolean operation could not be written");
            return BooleanOutcome::Ignored;
        };
        self.command_refusal = None;
        // The operands are gone, and so are the nodes a Node-tool selection held.
        self.node.clear_selection();
        self.selection.select_single(result);
        self.hovered_object = None;
        BooleanOutcome::Applied {
            operands: plan.operands.len(),
            compound: plan.compound,
        }
    }
}
