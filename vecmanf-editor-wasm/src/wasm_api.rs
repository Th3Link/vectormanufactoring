//! The `wasm-bindgen` surface itself (ADR 0001 §3): thin wrappers that
//! convert JS-friendly values to and from [`crate::session::Session`]
//! calls, plus [`crate::gpu::Gpu`] ownership and per-frame submission.
//! No editing logic lives here — every method is a direct pass-through
//! to `Session`, `vecmanf-ui-core` or `vecmanf-render-core`.

use vecmanf_document_core::{AnchorKind, ViewTransform};
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
        "select" => Ok(Tool::Select),
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

/// `"corner" | "symmetric" | "asymmetric"`
/// (`specs/0006-path-merge-split-and-node-types/adrs.md`: "the wasm
/// binding string (`"smooth"` in `wasm_api.rs`...) is not persisted and
/// is renamed outright" — the old `"smooth"` spelling is gone here too,
/// not kept as an accepted alias the way the on-disk tag is).
fn kind_from_str(name: &str) -> Result<AnchorKind, JsValue> {
    match name {
        "corner" => Ok(AnchorKind::Corner),
        "symmetric" => Ok(AnchorKind::Symmetric),
        "asymmetric" => Ok(AnchorKind::Asymmetric),
        other => Err(JsValue::from_str(&format!("unknown anchor kind: {other}"))),
    }
}

/// `wasm-bindgen`'s JS-facing mirror of [`vecmanf_ui_core::NodeToolbarState`]
/// — a plain `bool`-fields struct needs no getter methods, unlike a type
/// `wasm-bindgen` can't expose by value. See that type's own doc comment
/// for why independent `bool`s, not an enum.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct NodeToolbarState {
    pub can_insert: bool,
    pub can_delete: bool,
    pub can_convert_to_corner: bool,
    pub can_convert_to_symmetric: bool,
    pub can_convert_to_asymmetric: bool,
    pub can_make_line: bool,
    pub can_make_curve: bool,
    pub can_join: bool,
    pub can_split: bool,
}

impl From<SessionNodeToolbarState> for NodeToolbarState {
    fn from(state: SessionNodeToolbarState) -> Self {
        Self {
            can_insert: state.can_insert,
            can_delete: state.can_delete,
            can_convert_to_corner: state.can_convert_to_corner,
            can_convert_to_symmetric: state.can_convert_to_symmetric,
            can_convert_to_asymmetric: state.can_convert_to_asymmetric,
            can_make_line: state.can_make_line,
            can_make_curve: state.can_make_curve,
            can_join: state.can_join,
            can_split: state.can_split,
        }
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

/// `wasm-bindgen`'s JS-facing mirror of [`crate::session::EntryView`]
/// (`specs/object-transform-refinements/adrs.md`, "wasm surface": strings
/// and scalars only): the typed numeric entry chip's content and where it
/// belongs. `handle_*`/`center_*` are canvas-relative CSS pixels, already
/// converted; the host puts the chip outward of the handle, along the line
/// from the center through it.
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct TransformEntryView {
    kind: String,
    fields: Vec<crate::session::EntryFieldView>,
    pub linked: bool,
    pub handle_x: f64,
    pub handle_y: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub glyph_reach: f64,
}

#[wasm_bindgen]
impl TransformEntryView {
    /// `"angle"`, `"size"`, `"radius"` (a polygon or star's outer radius),
    /// `"corner-radius"` or `"inner-ratio"`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    /// How many fields the chip has (one or two).
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn field_count(&self) -> u32 {
        u32::try_from(self.fields.len()).unwrap_or(0)
    }

    /// Field `index`'s visible label ("W", "H", "r"; empty for the angle).
    #[must_use]
    pub fn field_label(&self, index: u32) -> String {
        self.field(index)
            .map(|f| f.label.to_string())
            .unwrap_or_default()
    }

    /// Field `index`'s accessible name ("Width", "Height", "Outer radius", "Angle").
    #[must_use]
    pub fn field_name(&self, index: u32) -> String {
        self.field(index)
            .map(|f| f.accessible_name.to_string())
            .unwrap_or_default()
    }

    /// The text field `index` opens with.
    #[must_use]
    pub fn field_prefill(&self, index: u32) -> String {
        self.field(index)
            .map(|f| f.prefill.clone())
            .unwrap_or_default()
    }

    /// Whether field `index` can be edited.
    #[must_use]
    pub fn field_editable(&self, index: u32) -> bool {
        self.field(index).is_some_and(|f| f.editable)
    }
}

impl TransformEntryView {
    fn field(&self, index: u32) -> Option<&crate::session::EntryFieldView> {
        self.fields.get(usize::try_from(index).ok()?)
    }

    /// Converts a document-space [`crate::session::EntryView`] to this
    /// screen-pixel mirror — [`WasmSession::transform_entry`] is the one
    /// caller.
    fn from_document_space(entry: crate::session::EntryView, view: ViewTransform) -> Self {
        let (handle_x, handle_y) = view.document_to_screen(entry.handle);
        let (center_x, center_y) = view.document_to_screen(entry.center);
        Self {
            kind: entry.kind.to_string(),
            fields: entry.fields,
            linked: entry.linked,
            handle_x,
            handle_y,
            center_x,
            center_y,
            glyph_reach: entry.glyph_reach_px,
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
    pub fn pointer_down(&mut self, x: f64, y: f64, shift: bool) {
        let point = self.session.screen_to_document(x, y);
        self.session.pointer_down(point, shift);
    }

    /// The pointer moved to canvas-relative CSS pixel `(x, y)`.
    /// `constrain` is the Ctrl modifier's current state, consulted by
    /// the rectangle/ellipse tools' live create-drag preview (acceptance
    /// criteria 2, 8) and, since `object-transform`, by the Select
    /// tool's own live resize/rotate preview alongside `shift`
    /// (acceptance criteria 5, 7, 16, 17). Call this on every pointer
    /// move, not only while a button is held — it also feeds whatever
    /// shape-tool drag is in flight for the live preview (ux-engineer
    /// review), and the method itself is a no-op when no drag is in
    /// progress.
    pub fn pointer_hover(&mut self, x: f64, y: f64, shift: bool, constrain: bool) {
        let point = self.session.screen_to_document(x, y);
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
    /// commit alongside `shift` (acceptance criteria 5, 7, 16, 17);
    /// both are ignored outside those tools.
    pub fn pointer_up(&mut self, x: f64, y: f64, shift: bool, constrain: bool) {
        let point = self.session.screen_to_document(x, y);
        self.session.pointer_up(point, shift, constrain);
    }

    /// The one double-click dispatch point (acceptance criteria 3, 12,
    /// 22, 23) at canvas-relative CSS pixel `(x, y)` — the host's own
    /// double-click detector (unchanged, now in pixels) calls this
    /// instead of choosing per tool itself
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`).
    pub fn double_click(&mut self, x: f64, y: f64, shift: bool, ctrl: bool) {
        let point = self.session.screen_to_document(x, y);
        self.session.double_click(point, shift, ctrl);
    }

    /// Shift or Ctrl changed with no pointer movement
    /// (`object-transform-refinements`, "Shift reveal"): call from
    /// window-level key events, and with `(false, false)` when the window or
    /// canvas loses focus.
    pub fn modifiers_changed(&mut self, shift: bool, ctrl: bool) {
        self.session.modifiers_changed(shift, ctrl);
    }

    /// Which hint the handle under the pointer earns (`""`, `"resize-edge"`,
    /// `"resize-corner"`, `"resize-corner-uniform"`, `"rotate-corner"`,
    /// `"rotate-side"`, `"skew"`, `"move"`, `"param-radius"` or
    /// `"param-inner"`) — for the host's 600 ms hover
    /// chip. Call after every [`WasmSession::pointer_hover`].
    #[must_use]
    pub fn handle_hint(&self) -> String {
        self.session.handle_hint()
    }

    /// The typed numeric entry to show, or `undefined` (criteria 18, 25, 26
    /// of `object-transform-refinements`). Call after every pointer release
    /// and tool or selection change.
    #[must_use]
    pub fn transform_entry(&self) -> Option<TransformEntryView> {
        let view = self.session.view();
        self.session
            .transform_entry()
            .map(|entry| TransformEntryView::from_document_space(entry, view))
    }

    /// For linked fields (criterion 29): the text the other field takes after
    /// field `field` became `text`, or `undefined`.
    #[must_use]
    pub fn transform_entry_linked(&self, field: u32, text: &str) -> Option<String> {
        self.session
            .transform_entry_linked(usize::try_from(field).ok()?, text)
    }

    /// Enter in the entry chip: `"committed"`, `"unchanged"` (both close the
    /// chip), or `"invalid:<field>:<reason>"` with the reason `number`,
    /// `positive`, `negative` or `ratio-range` (the chip stays open; criteria
    /// 19, 21, 27, 30, 31; 18, 19 of `unified-object-editing`). `last_edited` is
    /// the index of the field edited last.
    pub fn commit_transform_entry(
        &mut self,
        first: &str,
        second: &str,
        last_edited: u32,
    ) -> String {
        let last = usize::try_from(last_edited).unwrap_or(0);
        match self.session.commit_transform_entry(first, second, last) {
            vecmanf_ui_core::EntryOutcome::Committed => "committed".to_string(),
            vecmanf_ui_core::EntryOutcome::Unchanged => "unchanged".to_string(),
            vecmanf_ui_core::EntryOutcome::Invalid { field, reason } => format!(
                "invalid:{field}:{}",
                match reason {
                    vecmanf_ui_core::InvalidReason::NotANumber => "number",
                    vecmanf_ui_core::InvalidReason::NotPositive => "positive",
                    vecmanf_ui_core::InvalidReason::Negative => "negative",
                    vecmanf_ui_core::InvalidReason::RatioRange => "ratio-range",
                }
            ),
        }
    }

    /// Closes the numeric entry without writing (criterion 20): Escape, a
    /// blur, a window blur. Idempotent.
    pub fn cancel_transform_entry(&mut self) {
        self.session.cancel_transform_entry();
    }

    /// A wheel event at canvas-relative CSS pixel `(x, y)`: pans
    /// (acceptance criteria 1, 2) or, with Ctrl/Cmd held, zooms about
    /// that point (acceptance criterion 6). `delta_x`/`delta_y` are
    /// already normalized to pixels by the host (`deltaMode`).
    pub fn wheel(&mut self, delta_x: f64, delta_y: f64, x: f64, y: f64, shift: bool, ctrl: bool) {
        self.session.wheel(delta_x, delta_y, x, y, shift, ctrl);
    }

    /// Starts a drag-pan gesture (middle-mouse or Space+primary,
    /// acceptance criteria 3, 4) at canvas-relative CSS pixel `(x, y)`.
    /// The host calls this instead of [`WasmSession::pointer_down`] for
    /// exactly these two gestures.
    pub fn begin_pan(&mut self, x: f64, y: f64) {
        self.session.begin_pan(x, y);
    }

    /// Continues the drag-pan gesture [`WasmSession::begin_pan`] started.
    pub fn pan_to(&mut self, x: f64, y: f64) {
        self.session.pan_to(x, y);
    }

    /// Ends the drag-pan gesture, if one is in flight.
    pub fn end_pan(&mut self) {
        self.session.end_pan();
    }

    /// Whether a drag-pan gesture is currently in flight — the host's
    /// grab/grabbing cursor convention (`docs/design-system.md`'s "Pan
    /// cursor").
    #[must_use]
    pub fn is_panning(&self) -> bool {
        self.session.is_panning()
    }

    /// The current zoom level's integer percentage read-out (acceptance
    /// criterion 9), for the status bar's center segment.
    #[must_use]
    pub fn zoom_percent(&self) -> i32 {
        #[allow(clippy::cast_possible_truncation)]
        (self.session.zoom_percent() as i32)
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

    /// The node-kind conversion actions: `"corner"`, `"symmetric"` or
    /// `"asymmetric"` (`specs/0006-path-merge-split-and-node-types/
    /// specification.md` acceptance criteria 1, 2, 4, 5, 16).
    ///
    /// # Errors
    /// A `JsValue` if `kind` is none of those.
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

    /// Acceptance criteria 8-11: the "Join" toolbar/context-menu button.
    pub fn join_selected(&mut self) {
        self.session.join_selected();
    }

    /// Acceptance criteria 12-15: the "Split" toolbar/context-menu
    /// button.
    pub fn split_selected(&mut self) {
        self.session.split_selected();
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

    /// Acceptance criterion 6's "remove rounding" action, from the Select
    /// bar (`unified-object-editing` criterion 21) or the Rectangle tool's:
    /// zeroes the radius of every selected rectangle. A no-op in every other
    /// tool.
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

    /// The Select tool's "Scale stroke width" switch (`object-transform`
    /// AC 26-31). Off in every new session; not persisted.
    #[must_use]
    pub fn scale_stroke_width(&self) -> bool {
        self.session.scale_stroke_width()
    }

    /// Sets the "Scale stroke width" switch for the next resize drag.
    pub fn set_scale_stroke_width(&mut self, on: bool) {
        self.session.set_scale_stroke_width(on);
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
        self.set_viewport_css_size(width, height, device_pixel_ratio);
        Ok(())
    }

    /// Records the canvas's CSS (layout) pixel size on the viewport
    /// (acceptance criterion 10) — `width`/`height` are the backing-
    /// buffer (physical) pixel size this method's two callers both
    /// receive; dividing by `device_pixel_ratio` recovers the CSS size
    /// pointer events and `Session::screen_to_document` already agree on.
    fn set_viewport_css_size(&mut self, width: u32, height: u32, device_pixel_ratio: f64) {
        let ratio = if device_pixel_ratio > 0.0 {
            device_pixel_ratio
        } else {
            1.0
        };
        self.session
            .resize_viewport(f64::from(width) / ratio, f64::from(height) / ratio);
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
        self.set_viewport_css_size(width, height, device_pixel_ratio);
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
