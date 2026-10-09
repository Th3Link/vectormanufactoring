//! The boolean command's `wasm-bindgen` surface (`specs/0016-boolean-operations`): what the
//! rail section shows, the command, and the refusal outline. A further `impl WasmSession`
//! block; codes and counts only (the sentences are the host's, ADR 0001 §5), every method a
//! pass-through to `Session`.

use curvyo_ui_core::{BooleanAvailability, BooleanOp, BooleanRefusal};
use wasm_bindgen::prelude::*;

use crate::session::BooleanOutcome;
use crate::wasm_api::WasmSession;

/// `wasm-bindgen`'s JS-facing mirror of [`curvyo_ui_core::BooleanAvailability`]: `needs_two`
/// dimmed; else enabled, with `open` of the `of` selected objects open paths (0 when all are
/// closed).
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct BooleanAvailabilityView {
    /// Fewer than two objects are selected, or the Select tool is not active.
    pub needs_two: bool,
    /// How many selected objects are open paths.
    pub open: u32,
    /// How many objects are selected (0 when `needs_two`).
    pub of: u32,
}

impl From<BooleanAvailability> for BooleanAvailabilityView {
    fn from(availability: BooleanAvailability) -> Self {
        match availability {
            BooleanAvailability::NeedsTwo => Self {
                needs_two: true,
                open: 0,
                of: 0,
            },
            BooleanAvailability::OpenPaths { open, of } => Self {
                needs_two: false,
                open: count(open),
                of: count(of),
            },
            BooleanAvailability::Ready => Self {
                needs_two: false,
                open: 0,
                of: 0,
            },
        }
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// What `apply_boolean` did. `kind` is `"ignored"`, `"applied"` (`count` objects became one;
/// `compound` says whether it is a compound path) or a refusal: `"needs_two"`, `"open_paths"`,
/// `"no_area"`, `"out_of_range"` (`count` of the `of` selected objects are to blame) or
/// `"empty"`. Nothing was changed by anything but `"applied"`.
#[wasm_bindgen(getter_with_clone)]
#[derive(Debug, Clone)]
pub struct BooleanResultView {
    /// The outcome code.
    pub kind: String,
    /// Objects replaced, or objects to blame.
    pub count: u32,
    /// Objects selected.
    pub of: u32,
    /// The result is a compound path.
    pub compound: bool,
}

impl From<BooleanOutcome> for BooleanResultView {
    fn from(outcome: BooleanOutcome) -> Self {
        let view = |kind: &str, count: usize, of: usize, compound: bool| Self {
            kind: kind.to_string(),
            count: self::count(count),
            of: self::count(of),
            compound,
        };
        match outcome {
            BooleanOutcome::Ignored => view("ignored", 0, 0, false),
            BooleanOutcome::Applied { operands, compound } => {
                view("applied", operands, operands, compound)
            }
            BooleanOutcome::Refused(refusal) => match refusal {
                BooleanRefusal::NeedsTwo => view("needs_two", 0, 0, false),
                BooleanRefusal::OpenPaths { offenders, of } => {
                    view("open_paths", offenders.len(), of, false)
                }
                BooleanRefusal::NoArea { offenders, of } => {
                    view("no_area", offenders.len(), of, false)
                }
                BooleanRefusal::OutOfRange { offenders, of } => {
                    view("out_of_range", offenders.len(), of, false)
                }
                BooleanRefusal::Empty => view("empty", 0, 0, false),
            },
        }
    }
}

#[wasm_bindgen]
impl WasmSession {
    /// What the Boolean section of the tool rail shows. Call it with every other state read.
    #[must_use]
    pub fn boolean_availability(&self) -> BooleanAvailabilityView {
        self.session.boolean_availability().into()
    }

    /// Runs one operation (`"union"`, `"difference"`, `"intersection"`, `"exclusion"` or
    /// `"reverse_difference"`) on the selected objects, in one commit.
    ///
    /// # Errors
    /// A `JsValue` if `op` is none of those.
    pub fn apply_boolean(&mut self, op: &str) -> Result<BooleanResultView, JsValue> {
        let op = BooleanOp::from_code(op)
            .ok_or_else(|| JsValue::from_str(&format!("unknown boolean operation: {op}")))?;
        Ok(self.session.apply_boolean(op).into())
    }

    /// Removes the red outline of a refusal (its notice ran out, or the next action).
    pub fn clear_boolean_refusal(&mut self) {
        self.session.clear_boolean_refusal();
    }
}
