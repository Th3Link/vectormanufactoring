//! The `wasm-bindgen` surface itself (ADR 0001 §3): thin wrappers that
//! convert JS-friendly values to and from [`crate::session::Session`]
//! calls, plus [`crate::gpu::Gpu`] ownership and per-frame submission.
//! No editing logic lives here — every method is a direct pass-through
//! to `Session`, `vecmanf-ui-core` or `vecmanf-render-core`.

use vecmanf_document_core::{AnchorKind, Point, ViewTransform};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use vecmanf_ui_core::NodeToolbarState as SessionNodeToolbarState;

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
        "pen" => Ok(Tool::Pen),
        "node" => Ok(Tool::Node),
        "rectangle" => Ok(Tool::Rectangle),
        "ellipse" => Ok(Tool::Ellipse),
        "polygon-star" => Ok(Tool::PolygonStar),
        other => Err(JsValue::from_str(&format!("unknown tool: {other}"))),
    }
}

fn poly_star_mode_from_str(name: &str) -> Result<vecmanf_ui_core::PolyStarMode, JsValue> {
    match name {
        "polygon" => Ok(vecmanf_ui_core::PolyStarMode::Polygon),
        "star" => Ok(vecmanf_ui_core::PolyStarMode::Star),
        other => Err(JsValue::from_str(&format!(
            "unknown polygon/star mode: {other}"
        ))),
    }
}

fn kind_from_str(name: &str) -> Result<AnchorKind, JsValue> {
    match name {
        "corner" => Ok(AnchorKind::Corner),
        "smooth" => Ok(AnchorKind::Smooth),
        other => Err(JsValue::from_str(&format!("unknown anchor kind: {other}"))),
    }
}

/// `wasm-bindgen`'s JS-facing mirror of [`vecmanf_ui_core::NodeToolbarState`]
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

/// `wasm-bindgen`'s JS-facing mirror of
/// [`crate::session::LiveReadout`] (`specs/0003-primitive-shapes/
/// specification.md`'s "Live creation feedback": the on-canvas numeric
/// readout shown during a create-drag, ux-engineer review item 2).
/// `anchor_x`/`anchor_y` are document-space coordinates — the host
/// converts them to screen pixels the same way it already does for
/// every other document-space point.
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

impl From<crate::session::LiveReadout> for LiveReadout {
    fn from(readout: crate::session::LiveReadout) -> Self {
        Self {
            text: readout.text,
            anchor_x: readout.anchor.x,
            anchor_y: readout.anchor.y,
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
    /// (`specs/0001-project-file-foundation/adrs.md`, amended 2026-10-03).
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
    /// The host does byte I/O only (`specs/0002-path-node-editing/adrs.md`'s
    /// PR review: "Open reads bytes and calls `WasmSession::open`") — it
    /// never sees a [`vecmanf_document_core::OpenError`] itself, so this
    /// maps it to the exact user-facing sentence
    /// `specs/0001-project-file-foundation/specification.md`'s "Error
    /// handling — invalid/corrupt file" names, the same mapping
    /// `vecmanf-app`'s own (now-removed) native `open_error.rs` used to
    /// do for a native-side `Document`.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) — one of the three sentences named
    /// above — describing why the file could not be opened.
    pub fn open(peer: u64, bytes: &[u8]) -> Result<WasmSession, JsValue> {
        let session = Session::open(peer, bytes)
            .map_err(|err| JsValue::from_str(crate::session::map_open_error(&err)))?;
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

    /// Switches the active tool: `"pen"`, `"node"`, `"rectangle"`,
    /// `"ellipse"` or `"polygon-star"`.
    ///
    /// # Errors
    /// A `JsValue` if `tool` is none of those.
    pub fn set_tool(&mut self, tool: &str) -> Result<(), JsValue> {
        self.session.set_tool(tool_from_str(tool)?);
        Ok(())
    }

    /// The active tool, as one of the five strings
    /// [`WasmSession::set_tool`] accepts — for the host's tool rail
    /// (which button is active) and contextual toolbars (shown only for
    /// the matching tool).
    #[must_use]
    pub fn tool(&self) -> String {
        match self.session.tool() {
            Tool::Pen => "pen".to_string(),
            Tool::Node => "node".to_string(),
            Tool::Rectangle => "rectangle".to_string(),
            Tool::Ellipse => "ellipse".to_string(),
            Tool::PolygonStar => "polygon-star".to_string(),
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

    /// The pointer moved to document-space `(x, y)`. `constrain` is the
    /// Ctrl modifier's current state, consulted only by the rectangle/
    /// ellipse tools' live create-drag preview (acceptance criteria 2,
    /// 8). Call this on every pointer move, not only while a button is
    /// held — it also feeds whatever shape-tool drag is in flight for
    /// the live preview (ux-engineer review), and the method itself is
    /// a no-op when no drag is in progress.
    pub fn pointer_hover(&mut self, x: f64, y: f64, constrain: bool) {
        self.session.pointer_hover(Point::new(x, y), constrain);
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

    /// The pointer released at document-space `(x, y)`. `constrain` is
    /// the Ctrl modifier's state at release (acceptance criteria 2, 8);
    /// ignored outside the rectangle/ellipse tools.
    pub fn pointer_up(&mut self, x: f64, y: f64, constrain: bool) {
        self.session.pointer_up(Point::new(x, y), constrain);
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

    /// Acceptance criterion 6's "remove rounding" action. A no-op
    /// outside the rectangle tool.
    pub fn remove_corner_rounding(&mut self) {
        self.session.remove_corner_rounding();
    }

    /// The polygon/star tool-options bar's current mode: `"polygon"` or
    /// `"star"`.
    #[must_use]
    pub fn poly_star_mode(&self) -> String {
        match self.session.poly_star_mode() {
            vecmanf_ui_core::PolyStarMode::Polygon => "polygon".to_string(),
            vecmanf_ui_core::PolyStarMode::Star => "star".to_string(),
        }
    }

    /// The mode toggle (acceptance criteria 11 vs. 12).
    ///
    /// # Errors
    /// A `JsValue` if `mode` is neither `"polygon"` nor `"star"`.
    pub fn set_poly_star_mode(&mut self, mode: &str) -> Result<(), JsValue> {
        self.session
            .set_poly_star_mode(poly_star_mode_from_str(mode)?);
        Ok(())
    }

    /// The polygon/star tool-options bar's current point count
    /// (acceptance criterion 10).
    #[must_use]
    pub fn poly_star_point_count(&self) -> u32 {
        self.session.poly_star_point_count().get()
    }

    /// The point-count stepper (acceptance criteria 10, 15).
    ///
    /// # Errors
    /// A `JsValue` if `count` is outside `3..=1024`.
    pub fn set_poly_star_point_count(&mut self, count: u32) -> Result<(), JsValue> {
        let count = vecmanf_document_core::PointCount::new(count)
            .map_err(|err| JsValue::from_str(&format!("{err}")))?;
        self.session.set_poly_star_point_count(count);
        Ok(())
    }

    /// The polygon/star tool-options bar's current ratio (acceptance
    /// criterion 12).
    #[must_use]
    pub fn poly_star_ratio(&self) -> f64 {
        self.session.poly_star_ratio().get()
    }

    /// The ratio field's instantaneous commit (acceptance criteria 12,
    /// 14) — one commit immediately. For a continuously-dragged slider,
    /// call [`WasmSession::preview_poly_star_ratio`] on every tick and
    /// [`WasmSession::commit_poly_star_ratio`] once instead.
    ///
    /// # Errors
    /// A `JsValue` if `ratio` is outside the open interval `(0, 1)`.
    pub fn set_poly_star_ratio(&mut self, ratio: f64) -> Result<(), JsValue> {
        let ratio = vecmanf_document_core::InnerRatio::new(ratio)
            .map_err(|err| JsValue::from_str(&format!("{err}")))?;
        self.session.set_poly_star_ratio(ratio);
        Ok(())
    }

    /// The ratio slider's live, uncommitted preview (acceptance
    /// criterion 14's "updates live") — call on every slider tick,
    /// e.g. a `<input type="range">`'s own `input` event. Writes
    /// nothing to the document.
    ///
    /// # Errors
    /// A `JsValue` if `ratio` is outside the open interval `(0, 1)`.
    pub fn preview_poly_star_ratio(&mut self, ratio: f64) -> Result<(), JsValue> {
        let ratio = vecmanf_document_core::InnerRatio::new(ratio)
            .map_err(|err| JsValue::from_str(&format!("{err}")))?;
        self.session.preview_poly_star_ratio(ratio);
        Ok(())
    }

    /// Commits whatever [`WasmSession::preview_poly_star_ratio`] has
    /// accumulated, as one commit for the whole selection — call once,
    /// on the slider's own `change`/pointer-up event.
    pub fn commit_poly_star_ratio(&mut self) {
        self.session.commit_poly_star_ratio();
    }

    /// "Object to path" (acceptance criteria 17, 21, 22).
    pub fn convert_selected_to_paths(&mut self) {
        self.session.convert_selected_to_paths();
    }

    /// The numeric readout for an in-progress create-drag
    /// (`specification.md`'s "Live creation feedback"), or `undefined`
    /// outside one — call after every [`WasmSession::pointer_hover`].
    #[must_use]
    pub fn live_readout(&self) -> Option<LiveReadout> {
        self.session.live_readout().map(LiveReadout::from)
    }

    /// Attaches this session to `canvas`, creating the `wgpu`
    /// device/surface (ADR 0001 §3). Call once, after construction,
    /// before the first [`WasmSession::render`]. `width`×`height` are the
    /// surface's backing-buffer (physical) pixel size and
    /// `device_pixel_ratio` is `window.devicePixelRatio` — the host sizes
    /// the buffer `css_size * device_pixel_ratio` (`useEditorSession.ts`'s
    /// attach effect) so the canvas renders at the display's actual
    /// resolution instead of being upscaled and softened on any `HiDPI`
    /// screen.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) if no adapter/device could be
    /// obtained — see [`crate::gpu::Gpu::attach`].
    pub async fn attach_canvas(
        &mut self,
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
        device_pixel_ratio: f64,
    ) -> Result<(), JsValue> {
        self.gpu = Some(Gpu::attach(canvas, width, height, device_pixel_ratio).await?);
        Ok(())
    }

    /// Reconfigures the attached canvas's `wgpu` surface to `width`×
    /// `height` physical pixels and `device_pixel_ratio`, **and renders
    /// the next frame immediately**, in this same call — call on every
    /// resize. Folding the two together (rather than reconfiguring here
    /// and leaving the next frame to the host's own animation-frame loop)
    /// is what the canvas-perf spike's requirement actually asks for
    /// (`specs/0002-path-node-editing/adrs.md`'s PASS note, requirement 2:
    /// "reconfigures the `wgpu` surface on every resize... then render,
    /// all in one frame"): reconfigure-then-wait-for-the-next-tick leaves
    /// one empty/stale frame on screen for every resize, which on a
    /// webview without a free-running compositor (or a throttled/
    /// backgrounded one) can be visibly stuck rather than a single
    /// imperceptible frame.
    ///
    /// # Errors
    /// A `JsValue` (a plain string) if the subsequent render's surface
    /// texture could not be acquired — see [`crate::gpu::Gpu::render`].
    /// A no-op (`Ok(())`) before [`WasmSession::attach_canvas`] has
    /// completed, same as [`WasmSession::render`].
    pub fn resize(
        &mut self,
        width: u32,
        height: u32,
        device_pixel_ratio: f64,
    ) -> Result<(), JsValue> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        gpu.resize(width, height, device_pixel_ratio);
        let draw_list = self.session.draw_list();
        gpu.render(&draw_list, self.session.view())
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
