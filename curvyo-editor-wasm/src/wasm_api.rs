//! The `wasm-bindgen` surface itself (ADR 0001 §3): thin wrappers that
//! convert JS-friendly values to and from [`crate::session::Session`]
//! calls, plus [`crate::gpu::Gpu`] ownership and per-frame submission.
//! No editing logic lives here — every method is a direct pass-through
//! to `Session`, `curvyo-ui-core` or `curvyo-render-core`.
//!
//! This file holds the struct, the constructors and the pointer, document
//! and live-readout calls every tool shares. The rest is one `impl
//! WasmSession` block per tool or concern, in its own file
//! (`wasm_navigation.rs`, `wasm_node_tool.rs`, `wasm_select_tool.rs`,
//! `wasm_shape_tools.rs`, `wasm_render.rs`, `wasm_select_bar.rs`, ...).

use curvyo_document_core::ViewTransform;
use wasm_bindgen::prelude::*;

use crate::gpu::Gpu;
use crate::session::{Session, Tool};

/// Installs a panic hook that logs Rust panics to the browser console —
/// otherwise a panic in `wasm32` surfaces as an opaque
/// `"unreachable executed"` trap with no message at all. Idempotent;
/// the host calls this once, before anything else.
#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

fn tool_from_str(name: &str) -> Result<Tool, JsValue> {
    match name {
        "select" => Ok(Tool::Select),
        "pen" => Ok(Tool::Pen),
        "node" => Ok(Tool::Node),
        "rectangle" => Ok(Tool::Rectangle),
        "ellipse" => Ok(Tool::Ellipse),
        "polygon-star" => Ok(Tool::PolygonStar),
        other => Err(JsValue::from_str(&format!("unknown tool: {other}"))),
    }
}

/// `wasm-bindgen`'s JS-facing mirror of
/// [`crate::session::LiveReadout`] (`specs/0003-primitive-shapes/
/// specification.md`'s "Live creation feedback": the on-canvas numeric
/// readout shown during a create-drag, ux-engineer review item 2).
/// `anchor_x`/`anchor_y` are already canvas-relative CSS pixels
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "Anything the
/// DOM positions... is returned already converted") — the host positions
/// its overlay `<div>` directly from these, with no conversion math of
/// its own.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct LiveReadout {
    text: String,
    pub anchor_x: f64,
    pub anchor_y: f64,
}

#[wasm_bindgen]
impl LiveReadout {
    /// The formatted text (e.g. `"20.0 × 10.0 mm"`).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn text(&self) -> String {
        self.text.clone()
    }
}

impl LiveReadout {
    /// Converts a document-space [`crate::session::LiveReadout`] to this
    /// screen-pixel-anchored mirror, via `view`'s own
    /// `document_to_screen` — [`WasmSession::live_readout`] is the one
    /// caller, and the one place this conversion happens.
    fn from_document_space(readout: crate::session::LiveReadout, view: ViewTransform) -> Self {
        let (anchor_x, anchor_y) = view.document_to_screen(readout.anchor);
        Self {
            text: readout.text,
            anchor_x,
            anchor_y,
        }
    }
}

/// A document-space point, for [`WasmSession::screen_to_document`] — a
/// plain `f64`-fields struct needs no getter methods, same reasoning as
/// [`NodeToolbarState`]'s own doc comment.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct DocumentPoint {
    pub x: f64,
    pub y: f64,
}

/// One open document's whole session, as the host (`frontend/`) sees it:
/// edit it through the methods below, read back `draw_list_*` each
/// frame.
#[wasm_bindgen]
pub struct WasmSession {
    pub(crate) session: Session,
    pub(crate) gpu: Option<Gpu>,
}

#[wasm_bindgen]
impl WasmSession {
    /// A brand-new, empty document, bound to `peer` — the same fresh
    /// Loro peer id the host already mints per open session
    /// (`specs/0001-project-file-foundation/adrs.md`, amended 2026-10-03).
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(peer: u64) -> Self {
        let mut session = Session::new(peer);
        session.show_default_view();
        Self { session, gpu: None }
    }

    /// Reopens a previously saved `.curvyo` container's bytes.
    ///
    /// The host does byte I/O only (`specs/0002-path-node-editing/adrs.md`'s
    /// PR review: "Open reads bytes and calls `WasmSession::open`") — it
    /// never sees a [`curvyo_document_core::OpenError`] itself, so this
    /// maps it to the exact user-facing sentence
    /// `specs/0001-project-file-foundation/specification.md`'s "Error
    /// handling — invalid/corrupt file" names, the same mapping
    /// `curvyo-app`'s own (now-removed) native `open_error.rs` used to
    /// do for a native-side `Document`.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) — one of the three sentences named
    /// above — describing why the file could not be opened.
    pub fn open(peer: u64, bytes: &[u8]) -> Result<WasmSession, JsValue> {
        let mut session = Session::open(peer, bytes)
            .map_err(|err| JsValue::from_str(crate::session::map_open_error(&err)))?;
        session.show_default_view();
        Ok(Self { session, gpu: None })
    }

    /// Packs the current document into `.curvyo` container bytes.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) if the document could not be
    /// serialized.
    pub fn pack(&self, app_version: &str) -> Result<Vec<u8>, JsValue> {
        self.session
            .pack(app_version)
            .map_err(|err| JsValue::from_str(&format!("{err}")))
    }

    /// Switches the active tool: `"select"`, `"pen"`, `"node"`,
    /// `"rectangle"`, `"ellipse"` or `"polygon-star"`.
    ///
    /// # Errors
    /// A `JsValue` if `tool` is none of those.
    pub fn set_tool(&mut self, tool: &str) -> Result<(), JsValue> {
        self.session.set_tool(tool_from_str(tool)?);
        Ok(())
    }

    /// The active tool, as one of the six strings
    /// [`WasmSession::set_tool`] accepts — for the host's tool rail
    /// (which button is active) and contextual toolbars (shown only for
    /// the matching tool).
    #[must_use]
    pub fn tool(&self) -> String {
        match self.session.tool() {
            Tool::Select => "select".to_string(),
            Tool::Pen => "pen".to_string(),
            Tool::Node => "node".to_string(),
            Tool::Rectangle => "rectangle".to_string(),
            Tool::Ellipse => "ellipse".to_string(),
            Tool::PolygonStar => "polygon-star".to_string(),
        }
    }

    /// Converts canvas-relative CSS pixels to a document point
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "all
    /// screen↔document conversion happens in Rust") — the host's own
    /// status-bar cursor-position readout calls this instead of
    /// dividing by a fixed scale itself.
    #[must_use]
    pub fn screen_to_document(&self, x: f64, y: f64) -> DocumentPoint {
        let point = self.session.screen_to_document(x, y);
        DocumentPoint {
            x: point.x,
            y: point.y,
        }
    }

    /// The pointer went down at canvas-relative CSS pixel `(x, y)`
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "all
    /// screen↔document conversion happens in Rust" — converted via
    /// [`crate::session::Session::screen_to_document`] before dispatch).
    /// `shift`, `ctrl` (Cmd folded in) and `alt` are the press event's
    /// modifiers; Alt at the press arms the Select tool's lasso, Ctrl on empty
    /// canvas its removing marquee (`advanced-selection`).
    pub fn pointer_down(&mut self, x: f64, y: f64, shift: bool, ctrl: bool, alt: bool) {
        let point = self.session.screen_to_document(x, y);
        self.session.modifiers_changed(shift, ctrl, alt);
        self.session.pointer_down(point, shift);
    }

    /// The pointer moved to canvas-relative CSS pixel `(x, y)`.
    /// `constrain` is the Ctrl modifier's current state, consulted by
    /// the rectangle/ellipse tools' live create-drag preview (acceptance
    /// criteria 2, 8; `shift` centres the box on the press point) and, since `object-transform`, by the Select
    /// tool's own live resize/rotate preview alongside `shift`
    /// (acceptance criteria 5, 7, 16, 17); `alt` inverts a running marquee's
    /// mode (`advanced-selection` criterion 11). Call this on every pointer
    /// move, not only while a button is held — it also feeds whatever
    /// shape-tool drag is in flight for the live preview (ux-engineer
    /// review), and the method itself is a no-op when no drag is in
    /// progress.
    pub fn pointer_hover(&mut self, x: f64, y: f64, shift: bool, constrain: bool, alt: bool) {
        let point = self.session.screen_to_document(x, y);
        self.session.modifiers_changed(shift, constrain, alt);
        self.session.pointer_hover(point, shift, constrain);
    }

    /// The pointer left the canvas entirely (a DOM `pointerleave`).
    pub fn pointer_leave(&mut self) {
        self.session.pointer_leave();
    }

    /// Acceptance criterion 5's cursor cue
    /// (`specification.md`'s "Cursors"): whether the live cursor is
    /// currently over the in-progress pen path's own close target, so
    /// the host can swap in the close-path cursor variant. Call after
    /// every [`WasmSession::pointer_hover`].
    #[must_use]
    pub fn is_hovering_pen_close_target(&self) -> bool {
        self.session.is_hovering_pen_close_target()
    }

    /// The pointer released at canvas-relative CSS pixel `(x, y)`.
    /// `constrain` is the Ctrl modifier's state at release (acceptance
    /// criteria 2, 8), consulted by the rectangle/ellipse tools and,
    /// since `object-transform`, by the Select tool's own resize/rotate
    /// commit alongside `shift` (acceptance criteria 5, 7, 16, 17); the
    /// Select tool's marquee and lasso combine their result with the
    /// selection by `shift` and `constrain` and read `alt` as the box's mode
    /// (`advanced-selection`); all three are ignored by the other tools.
    pub fn pointer_up(&mut self, x: f64, y: f64, shift: bool, constrain: bool, alt: bool) {
        let point = self.session.screen_to_document(x, y);
        self.session.modifiers_changed(shift, constrain, alt);
        self.session.pointer_up(point, shift, constrain);
    }

    /// The one double-click dispatch point (acceptance criteria 3, 12,
    /// 22; `unified-object-editing` 31 to 34) at canvas-relative CSS pixel
    /// `(x, y)` — the host's own double-click detector (unchanged, now in
    /// pixels) calls this instead of choosing per tool itself
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`). Returns
    /// the code of the hint the host should show: `"edit_hint"` (a double-click on a
    /// primitive's outline, body or centre handle: nothing else changes),
    /// `"compound_path"` (on a compound path: its nodes cannot be edited yet) or `""`.
    pub fn double_click(&mut self, x: f64, y: f64, shift: bool, ctrl: bool) -> String {
        let point = self.session.screen_to_document(x, y);
        self.session
            .double_click_hint(point, shift, ctrl)
            .code()
            .to_string()
    }

    /// Shift, Ctrl or Alt changed with no pointer movement
    /// (`object-transform-refinements`, "Shift reveal"): call from
    /// window-level key events, and with `(false, false, false)` when the
    /// window or canvas loses focus.
    pub fn modifiers_changed(&mut self, shift: bool, ctrl: bool, alt: bool) {
        self.session.modifiers_changed(shift, ctrl, alt);
    }

    /// Acceptance criterion 13 (Delete/Backspace, or the toolbar).
    pub fn delete_selected(&mut self) {
        self.session.delete_selected();
    }

    /// The numeric readout for an in-progress create-drag
    /// (`specification.md`'s "Live creation feedback"), or `undefined`
    /// outside one — call after every [`WasmSession::pointer_hover`].
    #[must_use]
    pub fn live_readout(&self) -> Option<LiveReadout> {
        let view = self.session.view();
        self.session
            .live_readout()
            .map(|readout| LiveReadout::from_document_space(readout, view))
    }

    /// Which cursor the canvas should show: `"default"`, `"rotate"`, or
    /// `"resize:<degrees>"` (a double-headed arrow turned that many
    /// degrees clockwise from horizontal) — `object-transform`'s
    /// transform-handle cursors. Call after every
    /// [`WasmSession::pointer_hover`].
    #[must_use]
    pub fn cursor_hint(&self) -> String {
        self.session.cursor_hint()
    }
}
