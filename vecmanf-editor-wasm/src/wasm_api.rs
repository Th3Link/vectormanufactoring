//! The `wasm-bindgen` surface itself (ADR 0001 §3): thin wrappers that
//! convert JS-friendly values to and from [`crate::session::Session`]
//! calls, plus [`crate::gpu::Gpu`] ownership and per-frame submission.
//! No editing logic lives here — every method is a direct pass-through
//! to `Session`, `vecmanf-ui-core` or `vecmanf-render-core`.

use vecmanf_document_core::{AnchorKind, Point, ViewTransform};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::gpu::Gpu;
use crate::session::{NodeToolbarState as SessionNodeToolbarState, Session, Tool};

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
        "pen" => Ok(Tool::Pen),
        "node" => Ok(Tool::Node),
        other => Err(JsValue::from_str(&format!("unknown tool: {other}"))),
    }
}

fn kind_from_str(name: &str) -> Result<AnchorKind, JsValue> {
    match name {
        "corner" => Ok(AnchorKind::Corner),
        "smooth" => Ok(AnchorKind::Smooth),
        other => Err(JsValue::from_str(&format!("unknown anchor kind: {other}"))),
    }
}

/// `wasm-bindgen`'s JS-facing mirror of [`crate::session::NodeToolbarState`]
/// — a plain `bool`-fields struct needs no getter methods, unlike a type
/// `wasm-bindgen` can't expose by value. See that type's own doc comment
/// for why six independent `bool`s, not an enum.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct NodeToolbarState {
    pub can_insert: bool,
    pub can_delete: bool,
    pub can_convert_to_corner: bool,
    pub can_convert_to_smooth: bool,
    pub can_make_line: bool,
    pub can_make_curve: bool,
}

impl From<SessionNodeToolbarState> for NodeToolbarState {
    fn from(state: SessionNodeToolbarState) -> Self {
        Self {
            can_insert: state.can_insert,
            can_delete: state.can_delete,
            can_convert_to_corner: state.can_convert_to_corner,
            can_convert_to_smooth: state.can_convert_to_smooth,
            can_make_line: state.can_make_line,
            can_make_curve: state.can_make_curve,
        }
    }
}

/// One open document's whole session, as the host (`frontend/`) sees it:
/// edit it through the methods below, read back `draw_list_*` each
/// frame.
#[wasm_bindgen]
pub struct WasmSession {
    session: Session,
    gpu: Option<Gpu>,
}

#[wasm_bindgen]
impl WasmSession {
    /// A brand-new, empty document, bound to `peer` — the same fresh
    /// Loro peer id the host already mints per open session
    /// (`specs/project-file-foundation/adrs.md`, amended 2026-10-03).
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(peer: u64) -> Self {
        Self {
            session: Session::new(peer),
            gpu: None,
        }
    }

    /// Reopens a previously saved `.vmf` container's bytes.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) describing why the file could not
    /// be opened.
    pub fn open(peer: u64, bytes: &[u8]) -> Result<WasmSession, JsValue> {
        let session =
            Session::open(peer, bytes).map_err(|err| JsValue::from_str(&format!("{err}")))?;
        Ok(Self { session, gpu: None })
    }

    /// Packs the current document into `.vmf` container bytes.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) if the document could not be
    /// serialized.
    pub fn pack(&self, app_version: &str) -> Result<Vec<u8>, JsValue> {
        self.session
            .pack(app_version)
            .map_err(|err| JsValue::from_str(&format!("{err}")))
    }

    /// Switches the active tool: `"pen"` or `"node"`.
    ///
    /// # Errors
    /// A `JsValue` if `tool` is neither.
    pub fn set_tool(&mut self, tool: &str) -> Result<(), JsValue> {
        self.session.set_tool(tool_from_str(tool)?);
        Ok(())
    }

    /// The active tool, as `"pen"` or `"node"` — for the host's tool
    /// rail (which button is active) and contextual toolbar (shown only
    /// for the node tool).
    #[must_use]
    pub fn tool(&self) -> String {
        match self.session.tool() {
            Tool::Pen => "pen".to_string(),
            Tool::Node => "node".to_string(),
        }
    }

    /// Updates the view transform: `scale` is screen pixels per document
    /// millimetre; `origin_x`/`origin_y` is the document point currently
    /// at the canvas's top-left corner.
    pub fn set_view(&mut self, scale: f64, origin_x: f64, origin_y: f64) {
        self.session
            .set_view(ViewTransform::new(scale, Point::new(origin_x, origin_y)));
    }

    /// The pointer went down at document-space `(x, y)`.
    pub fn pointer_down(&mut self, x: f64, y: f64, shift: bool) {
        self.session.pointer_down(Point::new(x, y), shift);
    }

    /// The pointer moved to document-space `(x, y)` with no button held.
    pub fn pointer_hover(&mut self, x: f64, y: f64) {
        self.session.pointer_hover(Point::new(x, y));
    }

    /// The pointer left the canvas entirely (a DOM `pointerleave`).
    pub fn pointer_leave(&mut self) {
        self.session.pointer_leave();
    }

    /// The pointer released at document-space `(x, y)`.
    pub fn pointer_up(&mut self, x: f64, y: f64) {
        self.session.pointer_up(Point::new(x, y));
    }

    /// Acceptance criterion 3 / the dedicated "finish path" action.
    pub fn finish_pen(&mut self) {
        self.session.finish_pen();
    }

    /// Escape.
    pub fn escape(&mut self) {
        self.session.escape();
    }

    /// Acceptance criterion 13 (Delete/Backspace, or the toolbar).
    pub fn delete_selected(&mut self) {
        self.session.delete_selected();
    }

    /// Acceptance criterion 11's convert actions: `"corner"` or
    /// `"smooth"`.
    ///
    /// # Errors
    /// A `JsValue` if `kind` is neither.
    pub fn convert_selected(&mut self, kind: &str) -> Result<(), JsValue> {
        self.session.convert_selected(kind_from_str(kind)?);
        Ok(())
    }

    /// Acceptance criterion 14's "make line".
    pub fn make_line(&mut self) {
        self.session.make_line();
    }

    /// Acceptance criterion 14's "make curve".
    pub fn make_curve(&mut self) {
        self.session.make_curve();
    }

    /// Acceptance criterion 12: a double-click (already recognized by
    /// the host) at document-space `(x, y)`.
    pub fn insert_at(&mut self, x: f64, y: f64) {
        self.session.insert_at(Point::new(x, y));
    }

    /// The contextual toolbar's "Insert node" button: splits the
    /// currently selected segment at its midpoint.
    pub fn insert_selected(&mut self) {
        self.session.insert_selected();
    }

    /// Which contextual-toolbar buttons are currently enabled
    /// (`specification.md`'s UX notes: "Buttons disable (not hide) when
    /// nothing selected/applicable").
    #[must_use]
    pub fn node_toolbar_state(&self) -> NodeToolbarState {
        self.session.node_toolbar_state().into()
    }

    /// Attaches this session to `canvas`, creating the `wgpu`
    /// device/surface (ADR 0001 §3). Call once, after construction,
    /// before the first [`WasmSession::render`].
    ///
    /// # Errors
    /// A `JsValue` (a plain string) if no adapter/device could be
    /// obtained — see [`crate::gpu::Gpu::attach`].
    pub async fn attach_canvas(
        &mut self,
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        self.gpu = Some(Gpu::attach(canvas, width, height).await?);
        Ok(())
    }

    /// Reconfigures the attached canvas's `wgpu` surface — call on every
    /// resize, before the next [`WasmSession::render`]
    /// (`specs/path-node-editing/adrs.md`'s PASS note, requirement 2).
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(width, height);
        }
    }

    /// Builds this frame's draw list and submits it. A no-op (not an
    /// error) before [`WasmSession::attach_canvas`] has completed.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) if the frame could not be
    /// submitted.
    pub fn render(&mut self) -> Result<(), JsValue> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let draw_list = self.session.draw_list();
        gpu.render(&draw_list, self.session.view())
    }
}
