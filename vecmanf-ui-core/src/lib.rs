//! The vecmanf interaction layer (ADR 0001 §1, §2): pen- and node-tool
//! state machines, hit-testing, selection and command dispatch, as plain
//! state and pure functions (`specs/path-node-editing/adrs.md`).
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads or UI. The frontend renders this crate's state and
//! forwards input events into it; it holds no editing logic of its own.
//! Depends on `vecmanf-document-core` (the commands these tools dispatch)
//! and `vecmanf-geometry-core` (hit-testing a curved segment, subdividing
//! one for an insert) — never on `vecmanf-render-core` or the wasm
//! facade (ADR 0011 §3).

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod anchor_id_minter;
mod hit_test;
mod node_tool;
mod pen_tool;
mod selection;

pub use anchor_id_minter::AnchorIdMinter;
pub use hit_test::{Hit, hit_test};
pub use node_tool::{
    HitTolerances, NodeTool, NodeToolbarState, PointerDownOutcome as NodePointerDownOutcome,
    PointerUpOutcome as NodePointerUpOutcome,
};
pub use pen_tool::{PenTool, PointerUpOutcome as PenPointerUpOutcome};
pub use selection::NodeSelection;
