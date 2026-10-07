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
        }
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
    pub fn resize(&mut self, width: f64, height: f64) {
        let (old_width, old_height) = self.canvas_size;
        if old_width > 0.0 || old_height > 0.0 {
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
mod tests {
    use super::*;

    #[test]
    fn zoom_clamps_to_the_documented_range() {
        assert!((Zoom::new(1000.0).factor() - Zoom::MAX).abs() < f64::EPSILON);
        assert!((Zoom::new(0.0).factor() - Zoom::MIN).abs() < f64::EPSILON);
    }

    #[test]
    fn zoom_percent_reads_exactly_at_the_limits() {
        assert_eq!(Zoom::new(Zoom::MIN).percent(), 2);
        assert_eq!(Zoom::new(Zoom::MAX).percent(), 8000);
        assert_eq!(Zoom::default().percent(), 100);
    }

    #[test]
    fn default_viewport_is_100_percent_at_the_document_origin() {
        let viewport = Viewport::new();
        assert_eq!(viewport.zoom_percent(), 100);
        assert_eq!(viewport.screen_to_document(0.0, 0.0), Point::new(0.0, 0.0));
    }

    #[test]
    fn pan_by_screen_delta_moves_the_origin_proportionally() {
        let mut viewport = Viewport::new();
        let before = viewport.screen_to_document(0.0, 0.0);
        viewport.pan_by_screen_delta(10.0, 20.0);
        let after = viewport.screen_to_document(0.0, 0.0);
        let moved = before.vector_to(after);
        assert!(moved.x > 0.0 && moved.y > 0.0, "content moved: {moved:?}");
        // Double the delta, double the pan (proportional, AC 1/2).
        let mut doubled = Viewport::new();
        doubled.pan_by_screen_delta(20.0, 40.0);
        let doubled_after = doubled.screen_to_document(0.0, 0.0);
        let doubled_moved = before.vector_to(doubled_after);
        assert!((doubled_moved.x - 2.0 * moved.x).abs() < 1e-9);
        assert!((doubled_moved.y - 2.0 * moved.y).abs() < 1e-9);
    }

    #[test]
    fn pan_never_changes_the_zoom() {
        let mut viewport = Viewport::new();
        viewport.pan_by_screen_delta(500.0, -300.0);
        assert_eq!(viewport.zoom_percent(), 100);
    }

    #[test]
    fn zoom_about_a_point_keeps_that_point_under_the_cursor() {
        let mut viewport = Viewport::new();
        // Pan somewhere away from the origin first, so this isn't a
        // trivially-true identity-view case.
        viewport.pan_by_screen_delta(137.0, -42.0);

        let cursor = (300.0, 150.0);
        let anchor_before = viewport.screen_to_document(cursor.0, cursor.1);
        viewport.zoom_about(cursor.0, cursor.1, 2.0);
        let anchor_after = viewport.screen_to_document(cursor.0, cursor.1);

        assert!((anchor_before.x - anchor_after.x).abs() < 1e-9);
        assert!((anchor_before.y - anchor_after.y).abs() < 1e-9);
        assert_eq!(viewport.zoom_percent(), 200);
    }

    #[test]
    fn zoom_about_a_point_keeps_it_fixed_even_when_clamped_at_the_maximum() {
        let mut viewport = Viewport::new();
        let cursor = (412.0, 88.0);
        let anchor_before = viewport.screen_to_document(cursor.0, cursor.1);

        // A factor huge enough that the pre-clamp target scale would far
        // exceed 8000% — the clamp must bite, and the point under the
        // cursor must still be exactly where it was, computed from the
        // *clamped* scale, not the pre-clamp target.
        viewport.zoom_about(cursor.0, cursor.1, 1_000_000.0);
        assert_eq!(viewport.zoom_percent(), 8000, "the clamp must have bitten");

        let anchor_after = viewport.screen_to_document(cursor.0, cursor.1);
        assert!(
            (anchor_before.x - anchor_after.x).abs() < 1e-9,
            "x drifted: {anchor_before:?} -> {anchor_after:?}"
        );
        assert!(
            (anchor_before.y - anchor_after.y).abs() < 1e-9,
            "y drifted: {anchor_before:?} -> {anchor_after:?}"
        );
    }

    #[test]
    fn zoom_about_a_point_keeps_it_fixed_even_when_clamped_at_the_minimum() {
        let mut viewport = Viewport::new();
        let cursor = (60.0, 500.0);
        let anchor_before = viewport.screen_to_document(cursor.0, cursor.1);

        viewport.zoom_about(cursor.0, cursor.1, 1e-9);
        assert_eq!(viewport.zoom_percent(), 2, "the clamp must have bitten");

        let anchor_after = viewport.screen_to_document(cursor.0, cursor.1);
        assert!((anchor_before.x - anchor_after.x).abs() < 1e-9);
        assert!((anchor_before.y - anchor_after.y).abs() < 1e-9);
    }

    #[test]
    fn drag_pan_keeps_the_anchor_point_under_a_moving_cursor() {
        let mut viewport = Viewport::new();
        let press_at = (100.0, 100.0);
        let anchor = viewport.screen_to_document(press_at.0, press_at.1);
        viewport.begin_drag_pan(press_at.0, press_at.1);
        assert!(viewport.is_drag_panning());

        for cursor in [(150.0, 100.0), (150.0, 220.0), (40.0, 300.0)] {
            viewport.continue_drag_pan(cursor.0, cursor.1);
            let now_under_cursor = viewport.screen_to_document(cursor.0, cursor.1);
            assert!((now_under_cursor.x - anchor.x).abs() < 1e-9);
            assert!((now_under_cursor.y - anchor.y).abs() < 1e-9);
        }

        viewport.end_drag_pan();
        assert!(!viewport.is_drag_panning());
    }

    #[test]
    fn continue_drag_pan_without_a_gesture_in_flight_is_a_no_op() {
        let mut viewport = Viewport::new();
        let before = viewport;
        viewport.continue_drag_pan(999.0, 999.0);
        assert_eq!(viewport, before);
    }

    #[test]
    fn resize_keeps_the_center_point_and_the_zoom() {
        let mut viewport = Viewport::new();
        viewport.resize(800.0, 600.0);
        let center_before = viewport.screen_to_document(400.0, 300.0);

        viewport.resize(1000.0, 400.0);
        let center_after = viewport.screen_to_document(500.0, 200.0);

        assert!((center_before.x - center_after.x).abs() < 1e-9);
        assert!((center_before.y - center_after.y).abs() < 1e-9);
        assert_eq!(
            viewport.zoom_percent(),
            100,
            "resize never changes the zoom"
        );
    }

    #[test]
    fn the_first_resize_only_records_the_size_with_no_center_to_preserve() {
        let mut viewport = Viewport::new();
        let origin_before = viewport.screen_to_document(0.0, 0.0);
        viewport.resize(800.0, 600.0);
        let origin_after = viewport.screen_to_document(0.0, 0.0);
        assert_eq!(origin_before, origin_after);
    }
}
