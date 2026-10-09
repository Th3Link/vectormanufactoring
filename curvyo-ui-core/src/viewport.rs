//! Pan and zoom view state (`specs/0004-canvas-navigation-and-selection/
//! adrs.md`, feature-local decision "navigation state lives in a
//! `viewport` module in `curvyo-ui-core`"): a validated [`Zoom`] factor,
//! the document point at the canvas's top-left corner, the canvas's own
//! CSS-pixel size, and whichever drag-pan gesture (middle-mouse or
//! Space+drag) is currently in flight.
//!
//! Ephemeral, per ADR 0009 §2: never written to the document, never part
//! of the `.curvyo` format, and resets to the default view on every `New`/
//! `Open` — [`Session`](../../curvyo_editor_wasm/session/index.html) owns
//! one [`Viewport`] instead of a bare
//! [`curvyo_document_core::ViewTransform`]. `curvyo-render-core` still
//! only ever sees the plain `ViewTransform` ([`Viewport::view`]) — it
//! cannot depend on this crate (ADR 0011 §3).

use curvyo_document_core::{Point, Vec2, ViewTransform};

/// Screen pixels per document millimetre at 100% zoom: CSS's own "1in ==
/// 96px" convention expressed per millimetre (`96 / 25.4`) — the
/// conventional "actual size" mapping Inkscape's "1:1" and Illustrator's
/// "100%" already use (acceptance criterion 7). Moved here from
/// `frontend/src/hooks/useEditorSession.ts`'s `CSS_PX_PER_MM` — the zoom
/// limits below are defined in terms of it, so it belongs where they do,
/// not in the host.
pub const PX_PER_MM_AT_100: f64 = 96.0 / 25.4;

/// A validated zoom factor — a dimensionless multiple of 100%, clamped to
/// `2% ..= 8000%` (acceptance criteria 7, 8). `1.0` is 100%, matching
/// [`PX_PER_MM_AT_100`] exactly; `Zoom::percent` and [`Zoom::new`] round-
/// trip the limits to exactly `2`/`8000` with no floating-point residue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom(f64);

impl Zoom {
    /// The minimum zoom factor: 2% (acceptance criterion 7).
    pub const MIN: f64 = 0.02;
    /// The maximum zoom factor: 8000% (acceptance criterion 7).
    pub const MAX: f64 = 80.0;

    /// Builds a [`Zoom`] from a raw factor, clamped to `MIN..=MAX`
    /// (acceptance criterion 8: "the zoom stops exactly at that limit").
    #[must_use]
    pub fn new(factor: f64) -> Self {
        Self(factor.clamp(Self::MIN, Self::MAX))
    }

    /// The validated factor (`1.0` == 100%).
    #[must_use]
    pub const fn factor(self) -> f64 {
        self.0
    }

    /// Screen pixels per document millimetre at this zoom.
    #[must_use]
    pub fn scale(self) -> f64 {
        self.0 * PX_PER_MM_AT_100
    }

    /// The integer percentage read-out (acceptance criterion 9), e.g.
    /// `100` for 100%, `2`/`8000` exactly at the clamped limits.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn percent(self) -> i64 {
        (self.0 * 100.0).round() as i64
    }
}

impl Default for Zoom {
    /// 100% — the launch default.
    fn default() -> Self {
        Self::new(1.0)
    }
}

/// How far the document's top-left corner sits from the canvas's top-left
/// corner in a new view, CSS pixels on both axes: the tool rail's 12 + 48 + 12,
/// so the document edge and the 0 ticks of the rulers are not hidden under it
/// (`specs/0015-document-size-and-rulers/` criterion 11a).
pub const DOCUMENT_INSET_PX: f64 = 72.0;

/// How far the real width change of a panel toggle may differ from the
/// announced one, CSS pixels: the host rounds to whole device pixels.
const PANEL_TOGGLE_TOLERANCE_PX: f64 = 1.5;

/// A drag-pan gesture in flight (middle-mouse or Space+primary,
/// acceptance criteria 3, 4): the document point under the cursor at
/// press time, which [`Viewport::continue_drag_pan`] keeps fixed under
/// the live cursor for the gesture's whole duration — computed once up
/// front so the drag never accumulates rounding error one small step at a
/// time.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PanGesture {
    anchor_document_point: Point,
}

/// Which part of the document is on screen: pan, zoom and the canvas's
/// own CSS-pixel size, plus whichever drag-pan gesture is currently in
/// flight (acceptance criteria 1-11).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    zoom: Zoom,
    origin: Point,
    /// `(width, height)` in CSS pixels — `(0.0, 0.0)` before the host's
    /// first size report, which [`Viewport::resize`] treats as "not yet
    /// initialized" rather than a real shrink-to-nothing.
    canvas_size: (f64, f64),
    drag_pan: Option<PanGesture>,
    /// The width change (new minus old, CSS pixels) of one coming resize that
    /// must keep the view's top-left origin instead of its centre: the
    /// properties panel opening or closing.
    keep_origin_for: Option<f64>,
    /// Whether the view is still the untouched [`Viewport::with_document_inset`]
    /// one: resizes keep its origin, so the 72 px inset survives a window that
    /// is shown, maximised or resized after the session was created (measured
    /// in the browser: without the flag such a resize moves the document
    /// corner to wherever keeping the centre puts it) until the maker pans or
    /// zooms. The first pan, zoom or drag-pan ends it. This amends `0004`
    /// criterion 10 for an untouched view only.
    inset_view: bool,
}

impl Viewport {
    /// The default view: 100% zoom, document origin at the canvas's
    /// top-left corner, canvas size not yet known.
    #[must_use]
    pub fn new() -> Self {
        Self {
            zoom: Zoom::default(),
            origin: Point::new(0.0, 0.0),
            canvas_size: (0.0, 0.0),
            drag_pan: None,
            keep_origin_for: None,
            inset_view: false,
        }
    }

    /// The view of a project that was just created or opened: 100 % zoom with
    /// the document's top-left corner [`DOCUMENT_INSET_PX`] right of and below
    /// the canvas's top-left corner.
    #[must_use]
    pub fn with_document_inset() -> Self {
        let mut viewport = Self::new();
        viewport.inset_view = true;
        viewport.origin = Point::new(
            -DOCUMENT_INSET_PX / viewport.zoom.scale(),
            -DOCUMENT_INSET_PX / viewport.zoom.scale(),
        );
        viewport
    }

    /// The plain [`ViewTransform`] `curvyo-render-core` and every
    /// screen↔document conversion build from.
    #[must_use]
    pub fn view(&self) -> ViewTransform {
        ViewTransform::new(self.zoom.scale(), self.origin)
    }

    /// The current zoom level's integer percentage read-out (acceptance
    /// criterion 9).
    #[must_use]
    pub fn zoom_percent(&self) -> i64 {
        self.zoom.percent()
    }

    /// Converts canvas-relative CSS pixels to a document point, via the
    /// current view.
    #[must_use]
    pub fn screen_to_document(&self, screen_x: f64, screen_y: f64) -> Point {
        self.view().screen_to_document(screen_x, screen_y)
    }

    /// Pans by a screen-pixel delta, proportional to the scroll gesture
    /// that produced it (acceptance criteria 1, 2) — `origin' = origin +
    /// delta / scale`. Shift-swapping `deltaX`/`deltaY` for a vertical-
    /// only wheel into a horizontal pan (acceptance criterion 2) is the
    /// caller's job (`Session::wheel`); this function only ever applies
    /// the delta it is given, on whichever axis.
    pub fn pan_by_screen_delta(&mut self, delta_x: f64, delta_y: f64) {
        self.inset_view = false;
        let scale = self.zoom.scale();
        self.origin = self
            .origin
            .translated(Vec2::new(delta_x / scale, delta_y / scale));
    }

    /// Zooms in or out centered on the document point currently under
    /// screen pixel `(screen_x, screen_y)` (acceptance criterion 6):
    /// multiplies the current zoom by `factor` (`>1.0` zooms in, `<1.0`
    /// zooms out), then recomputes the origin so that exact point stays
    /// under the cursor — **even when the target zoom was clamped**
    /// (acceptance criterion 8; `adrs.md`'s explicit cursor-fixed-point
    /// rule): `p = screen_to_document(s)` is computed from the view
    /// *before* the zoom changes, the new zoom is computed and clamped,
    /// and only then is `origin' = p - s / scale'` computed from the
    /// *clamped* scale — recomputing from the pre-clamp target scale
    /// instead would place the wrong document point under the cursor
    /// whenever the clamp actually bites.
    pub fn zoom_about(&mut self, screen_x: f64, screen_y: f64, factor: f64) {
        self.inset_view = false;
        let anchor = self.screen_to_document(screen_x, screen_y);
        self.zoom = Zoom::new(self.zoom.factor() * factor);
        let scale = self.zoom.scale();
        self.origin = Point::new(anchor.x - screen_x / scale, anchor.y - screen_y / scale);
    }

    /// Starts a drag-pan gesture (acceptance criteria 3, 4): records the
    /// document point currently under `(screen_x, screen_y)` as the
    /// anchor [`Viewport::continue_drag_pan`] keeps fixed under the
    /// cursor.
    pub fn begin_drag_pan(&mut self, screen_x: f64, screen_y: f64) {
        self.inset_view = false;
        self.drag_pan = Some(PanGesture {
            anchor_document_point: self.screen_to_document(screen_x, screen_y),
        });
    }

    /// Continues the drag-pan gesture [`Viewport::begin_drag_pan`]
    /// started, keeping its anchor document point exactly under the live
    /// cursor at `(screen_x, screen_y)`. A no-op when no drag-pan gesture
    /// is in flight.
    pub fn continue_drag_pan(&mut self, screen_x: f64, screen_y: f64) {
        let Some(gesture) = self.drag_pan else {
            return;
        };
        let scale = self.zoom.scale();
        self.origin = Point::new(
            gesture.anchor_document_point.x - screen_x / scale,
            gesture.anchor_document_point.y - screen_y / scale,
        );
    }

    /// Ends the drag-pan gesture, if one is in flight.
    pub fn end_drag_pan(&mut self) {
        self.drag_pan = None;
    }

    /// Announces that the next resize is the properties panel opening or
    /// closing and changes the canvas width by `delta` CSS pixels (negative
    /// when the canvas shrinks): that resize keeps the view's top-left origin,
    /// so the document does not move on screen and only the right edge
    /// reveals or hides canvas (`specs/0007-stroke-and-fill-styling`
    /// criterion 39). A later resize that does not match it (another width
    /// change, or a height change) is an ordinary window resize.
    pub fn keep_origin_for_width_change(&mut self, delta: f64) {
        // Two toggles before the browser reports a resize add up: a panel that
        // opened and closed again changes nothing, so nothing is left pending
        // for a later window resize to be mistaken for.
        let total = self.keep_origin_for.unwrap_or(0.0) + delta;
        self.keep_origin_for = (total.abs() > PANEL_TOGGLE_TOLERANCE_PX).then_some(total);
    }

    /// The canvas's size in CSS pixels, `(0.0, 0.0)` before the host's first
    /// size report.
    #[must_use]
    pub const fn canvas_size(&self) -> (f64, f64) {
        self.canvas_size
    }

    /// Whether a drag-pan gesture is currently in flight — the host uses
    /// this for the grab/grabbing cursor convention
    /// (`docs/design-system.md`'s "Pan cursor").
    #[must_use]
    pub const fn is_drag_panning(&self) -> bool {
        self.drag_pan.is_some()
    }

    /// Resizes the canvas, keeping the zoom and the document point at the
    /// viewport's own center fixed (acceptance criterion 10) —
    /// `origin' = origin + ((old − new) / 2) / scale`, matching Inkscape's
    /// default "sticky zoom off" resize behaviour. The very first call
    /// (canvas size not yet known, `(0.0, 0.0)`) only records the size,
    /// since there is no prior center to preserve.
    ///
    /// A resize that matches a pending
    /// [`Viewport::keep_origin_for_width_change`] (that width change, same
    /// height) keeps the top-left origin instead, and consumes the request.
    pub fn resize(&mut self, width: f64, height: f64) {
        let (old_width, old_height) = self.canvas_size;
        let keep_origin = self.keep_origin_for.take().is_some_and(|expected| {
            (width - old_width - expected).abs() <= PANEL_TOGGLE_TOLERANCE_PX
                && (height - old_height).abs() <= PANEL_TOGGLE_TOLERANCE_PX
        });
        if !keep_origin && !self.inset_view && (old_width > 0.0 || old_height > 0.0) {
            let scale = self.zoom.scale();
            let delta = Vec2::new((old_width - width) / 2.0, (old_height - height) / 2.0)
                .scaled(1.0 / scale);
            self.origin = self.origin.translated(delta);
        }
        self.canvas_size = (width, height);
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
