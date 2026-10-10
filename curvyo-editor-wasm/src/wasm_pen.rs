//! The Pen's cue and the Close path command on the `wasm-bindgen` surface
//! (`specs/0034-pen-path-extension`): codes and counts only, the sentences are the host's
//! (ADR 0001 §5). A further `impl WasmSession` block, every method a pass-through to `Session`.

use curvyo_ui_core::{JoinType, PenTarget};
use wasm_bindgen::prelude::*;

use crate::wasm_api::WasmSession;

/// What a Pen press at the pointer would do: `kind` is `"none"` (no pointer or not the Pen),
/// `"place"`, `"new_path"` (Shift over an end), `"continue"`, `"join"`, `"place_over_end"` (Shift
/// over a join target) or `"close"`. For `"close"`, `join` and `as_drawn` are `"sharp"` or
/// `"smooth"` (the join a press applies and the one with no Shift), `shift` is the live Shift and
/// `style_differs` is `false`; for `"join"`, `style_differs` tells whether the continued path and
/// the target path differ in style.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PenCueView {
    /// The target kind code.
    #[wasm_bindgen(getter_with_clone)]
    pub kind: String,
    /// The identity of the target node (a path and its end), empty for a place or a close: the
    /// host restarts the rest timer of its chip when it changes.
    #[wasm_bindgen(getter_with_clone)]
    pub target: String,
    /// The join a close applies, `"sharp"` or `"smooth"`, else empty.
    #[wasm_bindgen(getter_with_clone)]
    pub join: String,
    /// The join with no Shift, else empty.
    #[wasm_bindgen(getter_with_clone)]
    pub as_drawn: String,
    /// Whether Shift is held.
    pub shift: bool,
    /// Whether a join target's path differs in style from the continued path.
    pub style_differs: bool,
    /// The unit direction (x) away from the path in progress at a close or join target, for the
    /// hint chip; 0 when there is none.
    pub away_x: f64,
    /// The unit direction (y, down) away from the path in progress.
    pub away_y: f64,
}

fn join_code(join: JoinType) -> String {
    match join {
        JoinType::Sharp => "sharp",
        JoinType::Smooth => "smooth",
    }
    .to_string()
}

/// How many open paths the Close path buttons would close and skip.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct ClosePathStateView {
    /// Open ordinary paths with three or more nodes.
    pub closable: u32,
    /// Open ordinary paths with fewer than three nodes, or three whose ends coincide.
    pub skipped: u32,
    /// How many of the skipped paths have three nodes with coinciding ends.
    pub same_ends: u32,
}

/// What a Close path press did.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct ClosePathResultView {
    /// How many paths were closed.
    pub closed: u32,
    /// How many open paths were skipped.
    pub skipped: u32,
    /// How many of the skipped paths have three nodes with coinciding ends.
    pub same_ends: u32,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[wasm_bindgen]
impl WasmSession {
    /// What a Pen press at the pointer would do now, for the cursor class and the hint chip.
    #[must_use]
    pub fn pen_cue(&self) -> PenCueView {
        let mut view = PenCueView {
            kind: "none".to_string(),
            target: String::new(),
            join: String::new(),
            as_drawn: String::new(),
            shift: false,
            style_differs: false,
            away_x: 0.0,
            away_y: 0.0,
        };
        let Some(target) = self.session.pen_target() else {
            return view;
        };
        match target {
            PenTarget::Place => "place",
            PenTarget::NewPathAt => "new_path",
            PenTarget::Continue(end) => {
                view.target = format!("{:?}:{:?}", end.path, end.end);
                "continue"
            }
            PenTarget::Join(end) => {
                view.target = format!("{:?}:{:?}", end.path, end.end);
                view.style_differs = self.session.pen_join_style_differs();
                "join"
            }
            PenTarget::PlaceOverEnd => "place_over_end",
            PenTarget::Close {
                join,
                as_drawn,
                shift,
            } => {
                view.join = join_code(join);
                view.as_drawn = join_code(as_drawn);
                view.shift = shift;
                "close"
            }
        }
        .clone_into(&mut view.kind);
        (view.away_x, view.away_y) = self.session.pen_cue_away();
        view
    }

    /// What the Close path buttons would do now.
    #[must_use]
    pub fn close_path_state(&self) -> ClosePathStateView {
        let state = self.session.close_path_state();
        ClosePathStateView {
            closable: count(state.closable),
            skipped: count(state.skipped),
            same_ends: count(state.same_ends),
        }
    }

    /// The Close path buttons: `join` is `"sharp"` or `"smooth"`.
    ///
    /// # Errors
    /// A `JsValue` if `join` is neither.
    pub fn close_path(&mut self, join: &str) -> Result<ClosePathResultView, JsValue> {
        let join = match join {
            "sharp" => JoinType::Sharp,
            "smooth" => JoinType::Smooth,
            other => return Err(JsValue::from_str(&format!("unknown join: {other}"))),
        };
        let outcome = self.session.close_paths(join);
        Ok(ClosePathResultView {
            closed: count(outcome.closed),
            skipped: count(outcome.skipped),
            same_ends: count(outcome.same_ends),
        })
    }
}
