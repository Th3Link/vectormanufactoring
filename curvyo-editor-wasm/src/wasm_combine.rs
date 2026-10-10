//! The Path card's `wasm-bindgen` surface (`specs/0035-combine-and-break-apart`): what the two
//! buttons show and the two commands. A further `impl WasmSession` block; codes and counts only
//! (the sentences are the host's, ADR 0001 §5), every method a pass-through to `Session`. The
//! refusal outline is cleared by `clear_boolean_refusal`, which serves every rail command.

use curvyo_ui_core::{BooleanAvailability, BreakApartRefusal, CombineRefusal, PathAvailability};
use wasm_bindgen::prelude::*;

use crate::session::{BreakApartOutcome, CombineOutcome};
use crate::wasm_api::WasmSession;

/// What the Path card shows: Combine as the Boolean buttons (`needs_two` dimmed; else enabled,
/// with `open` of the `of` selected objects open paths) and whether Break apart is enabled.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct PathAvailabilityView {
    /// Fewer than two objects are selected, or the Select tool is not active.
    pub needs_two: bool,
    /// How many selected objects are open paths.
    pub open: u32,
    /// How many objects are selected (0 when `needs_two`).
    pub of: u32,
    /// The selection holds a compound path.
    pub can_break_apart: bool,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl From<PathAvailability> for PathAvailabilityView {
    fn from(availability: PathAvailability) -> Self {
        let (needs_two, open, of) = match availability.combine {
            BooleanAvailability::NeedsTwo => (true, 0, 0),
            BooleanAvailability::OpenPaths { open, of } => (false, count(open), count(of)),
            BooleanAvailability::Ready => (false, 0, 0),
        };
        Self {
            needs_two,
            open,
            of,
            can_break_apart: availability.break_apart,
        }
    }
}

/// What `apply_combine` did. `kind` is `"ignored"`, `"applied"` (`count` objects became one
/// compound path with `holes` holes; `styles_differ` says the lowest object's style was used) or a
/// refusal: `"needs_two"`, `"open_paths"`, `"out_of_range"`, `"no_area"`, `"touching"` or
/// `"self_touching"` (`count` of the `of` selected objects are to blame). Nothing was changed by
/// anything but `"applied"`.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct CombineResultView {
    /// The outcome code.
    #[wasm_bindgen(getter_with_clone)]
    pub kind: String,
    /// Objects combined, or objects to blame.
    pub count: u32,
    /// Objects selected.
    pub of: u32,
    /// Holes in the result.
    pub holes: u32,
    /// The operands' styles were not all equal.
    pub styles_differ: bool,
}

impl From<CombineOutcome> for CombineResultView {
    fn from(outcome: CombineOutcome) -> Self {
        let view = |kind: &str, n: usize, of: usize, holes: usize, styles_differ: bool| Self {
            kind: kind.to_string(),
            count: count(n),
            of: count(of),
            holes: count(holes),
            styles_differ,
        };
        match outcome {
            CombineOutcome::Ignored => view("ignored", 0, 0, 0, false),
            CombineOutcome::Applied {
                operands,
                holes,
                styles_differ,
            } => view("applied", operands, operands, holes, styles_differ),
            CombineOutcome::Refused(refusal) => match refusal {
                CombineRefusal::NeedsTwo => view("needs_two", 0, 0, 0, false),
                CombineRefusal::OpenPaths { offenders, of } => {
                    view("open_paths", offenders.len(), of, 0, false)
                }
                CombineRefusal::OutOfRange { offenders, of } => {
                    view("out_of_range", offenders.len(), of, 0, false)
                }
                CombineRefusal::NoArea { offenders, of } => {
                    view("no_area", offenders.len(), of, 0, false)
                }
                CombineRefusal::Touching { offenders, of } => {
                    view("touching", offenders.len(), of, 0, false)
                }
                CombineRefusal::SelfTouching { offenders, of } => {
                    view("self_touching", offenders.len(), of, 0, false)
                }
            },
        }
    }
}

/// What `apply_break_apart` did. `kind` is `"ignored"`, `"applied"` (`compounds` compound paths
/// became `pieces` objects; `with_holes` says a piece has a hole; `left_alone` compound paths
/// were one piece already) or a refusal: `"no_compound"` or `"one_piece"` (`compounds` selected
/// compound paths are one piece each). Nothing was changed by anything but `"applied"`.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct BreakApartResultView {
    /// The outcome code.
    #[wasm_bindgen(getter_with_clone)]
    pub kind: String,
    /// Compound paths broken apart, or selected compound paths when refused as one piece.
    pub compounds: u32,
    /// Objects they became.
    pub pieces: u32,
    /// A piece has a hole.
    pub with_holes: bool,
    /// Compound paths left as they are.
    pub left_alone: u32,
}

impl From<BreakApartOutcome> for BreakApartResultView {
    fn from(outcome: BreakApartOutcome) -> Self {
        let view =
            |kind: &str, compounds: usize, pieces: usize, with_holes: bool, left: usize| Self {
                kind: kind.to_string(),
                compounds: count(compounds),
                pieces: count(pieces),
                with_holes,
                left_alone: count(left),
            };
        match outcome {
            BreakApartOutcome::Ignored => view("ignored", 0, 0, false, 0),
            BreakApartOutcome::Applied {
                compounds,
                pieces,
                with_holes,
                left_alone,
            } => view("applied", compounds, pieces, with_holes, left_alone),
            BreakApartOutcome::Refused(BreakApartRefusal::NoCompound) => {
                view("no_compound", 0, 0, false, 0)
            }
            BreakApartOutcome::Refused(BreakApartRefusal::OnePiece { compounds }) => {
                view("one_piece", compounds, 0, false, 0)
            }
        }
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// What the Path card of the tool rail shows. Call it with every other state read.
    #[must_use]
    pub fn path_availability(&self) -> PathAvailabilityView {
        self.session.path_availability().into()
    }

    /// Combines the selected objects into one compound path, in one commit.
    pub fn apply_combine(&mut self) -> CombineResultView {
        self.session.apply_combine().into()
    }

    /// Breaks the selected compound paths apart into their pieces, in one commit.
    pub fn apply_break_apart(&mut self) -> BreakApartResultView {
        self.session.apply_break_apart().into()
    }
}
