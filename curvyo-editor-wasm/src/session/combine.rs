//! The session's Combine and Break apart commands: what the Path card shows, the two writes
//! (`Document::replace_with_path` and `Document::break_apart`) and the red outline of the objects
//! a refusal names (`specs/0035-combine-and-break-apart`).

use curvyo_ui_core::{
    BreakApartRefusal, COMBINE_COMMIT_LABEL, CombineRefusal, PathAvailability, path_availability,
    plan_break_apart, plan_combine,
};

use super::Session;
use super::refusal::RefusalMarks;

/// What [`Session::apply_combine`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombineOutcome {
    /// Nothing: the Select tool is not active, a drag is in flight or a typed entry is open, or
    /// the write could not be made.
    Ignored,
    /// The operands were replaced by one compound path.
    Applied {
        /// How many objects took part.
        operands: usize,
        /// How many of the result's outlines are holes.
        holes: usize,
        /// Whether the operands' styles were not all equal (the result has the lowest one).
        styles_differ: bool,
    },
    /// Refused; nothing was changed. The offending objects, if any, are outlined until the
    /// selection or the tool changes or the host clears them.
    Refused(CombineRefusal),
}

/// What [`Session::apply_break_apart`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakApartOutcome {
    /// Nothing, as for [`CombineOutcome::Ignored`].
    Ignored,
    /// Compound paths were replaced by their pieces.
    Applied {
        /// How many compound paths were broken apart.
        compounds: usize,
        /// How many objects they became.
        pieces: usize,
        /// Whether a piece has a hole.
        with_holes: bool,
        /// How many selected compound paths were one piece and left as they are.
        left_alone: usize,
    },
    /// Refused; nothing was changed.
    Refused(BreakApartRefusal),
}

impl Session {
    /// What the Path card of the tool rail shows now. Read it after every pointer release and tool
    /// or selection change.
    #[must_use]
    pub fn path_availability(&self) -> PathAvailability {
        path_availability(&self.objects(), &self.boolean_selection())
    }

    /// Combines the selected objects into one compound path in one commit labelled
    /// `combine_paths`, and selects the result alone. The tool stays Select. A refusal changes
    /// nothing (no commit, same selection) and outlines its offenders.
    pub fn apply_combine(&mut self) -> CombineOutcome {
        if !self.rail_command_ready() {
            return CombineOutcome::Ignored;
        }
        self.flush_select_bar_preview();
        let objects = self.objects();
        let selection = self.boolean_selection();
        let plan = match plan_combine(&objects, &selection, &mut self.minter) {
            Ok(plan) => plan,
            Err(refusal) => {
                self.command_refusal = match &refusal {
                    CombineRefusal::OpenPaths { offenders, .. }
                    | CombineRefusal::OutOfRange { offenders, .. }
                    | CombineRefusal::NoArea { offenders, .. }
                    | CombineRefusal::Touching { offenders, .. }
                    | CombineRefusal::SelfTouching { offenders, .. } => {
                        Some(RefusalMarks::new(offenders.clone(), &selection))
                    }
                    CombineRefusal::NeedsTwo => None,
                };
                return CombineOutcome::Refused(refusal);
            }
        };
        let replaced = self.document.replace_with_path(
            &plan.operands,
            plan.base,
            &plan.outlines,
            COMBINE_COMMIT_LABEL,
        );
        let Ok(result) = replaced else {
            // invariant: the plan was made from the objects just read, with a base among the
            // operands, at least one outline and fresh anchor ids, so the write cannot fail.
            debug_assert!(false, "a planned combine could not be written");
            return CombineOutcome::Ignored;
        };
        self.command_refusal = None;
        // The operands are gone, and so are the nodes a Node-tool selection held.
        self.node.clear_selection();
        self.selection.select_single(result);
        self.hovered_object = None;
        CombineOutcome::Applied {
            operands: plan.operands.len(),
            holes: plan.holes,
            styles_differ: plan.styles_differ,
        }
    }

    /// Replaces each selected compound path of more than one region by one object per region, in
    /// one commit labelled `break_apart`, and selects all the pieces together with the selected
    /// objects that stayed. The tool stays Select. A refusal changes nothing.
    pub fn apply_break_apart(&mut self) -> BreakApartOutcome {
        if !self.rail_command_ready() {
            return BreakApartOutcome::Ignored;
        }
        self.flush_select_bar_preview();
        let objects = self.objects();
        let selection = self.boolean_selection();
        let plan = match plan_break_apart(&objects, &selection, &mut self.minter) {
            Ok(plan) => plan,
            Err(refusal) => {
                self.command_refusal = None;
                return BreakApartOutcome::Refused(refusal);
            }
        };
        let Ok(created) = self.document.break_apart(&plan.parts) else {
            // invariant: the plan was made from the objects just read, each compound still
            // exists, every region has an outline with nodes and the anchor ids are fresh.
            debug_assert!(false, "a planned break apart could not be written");
            return BreakApartOutcome::Ignored;
        };
        self.command_refusal = None;
        self.node.clear_selection();
        self.selection.clear();
        for id in plan.kept.iter().chain(&created) {
            self.selection.add(*id);
        }
        self.hovered_object = None;
        BreakApartOutcome::Applied {
            compounds: plan.parts.len(),
            pieces: plan.pieces,
            with_holes: plan.with_holes,
            left_alone: plan.left_alone,
        }
    }
}
