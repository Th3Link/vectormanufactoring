//! `Session`'s primitive-shape-tool glue (`specs/0003-primitive-shapes/
//! specification.md`, acceptance criteria 1-22): dispatching
//! pointer events to whichever of the three shape tools is active,
//! their tool-options bar, the live preview/readout the ux-engineer
//! review asked for, "object to path", and the shape decoration input
//! `vecmanf-render-core` draws from. Split out of `session/mod.rs`
//! (architect review: that file alone grew past "one responsibility")
//! — a child module of `session`, so it shares `Session`'s privacy
//! boundary and its methods below join the same type's `impl Session`
//! `session/mod.rs` itself defines.

use vecmanf_document_core::{InnerRatio, NodeId, Point, PointCount, PrimitiveSnapshot, Shape};
use vecmanf_render_core::{RenderShapeHandle, ShapeDecorationInput, ShapeHandleKind};
use vecmanf_ui_core::{
    HandleKind, LiveShape, PolyStarMode, ShapeHitTolerances, build_primitive_conversions,
};

use super::{Session, Tool};

/// The live, uncommitted numeric readout for a shape tool's create-drag
/// (`specification.md`'s "Live creation feedback" UX notes) — the text
/// to show, and the document-space point it is anchored near (point B).
/// `None` for every other case (idle, a resize/radius/ratio adjustment,
/// or a still-degenerate create-drag) — the readout only ever applies
/// to a create-drag in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveReadout {
    /// The formatted text (e.g. `"20.0 × 10.0 mm"`).
    pub text: String,
    /// Where to anchor it, in document space — the live pointer
    /// position (point B).
    pub anchor: Point,
}

impl Session {
    /// The same two tolerances, reused for every shape tool
    /// (`specs/0003-primitive-shapes/specification.md`: "reuses that slice's
    /// ... hit-testing tolerances").
    fn shape_tolerances(&self) -> ShapeHitTolerances {
        ShapeHitTolerances {
            outline: self.segment_tolerance(),
            handle: self.point_tolerance(),
        }
    }

    /// Every primitive currently in the document, in z-order — the
    /// shape-tool counterpart to `Session::paths`. The authoritative
    /// (uncommitted-preview-free) read: hit-testing and every other
    /// non-rendering use goes through this one, never
    /// `primitives_for_render`.
    pub(super) fn primitives(&self) -> Vec<PrimitiveSnapshot> {
        self.document
            .object_ids()
            .into_iter()
            .filter_map(|id| self.document.primitive(id))
            .collect()
    }

    /// Same as `primitives`, except every currently selected star's
    /// `inner_ratio` is overridden by the ratio slider's own live,
    /// uncommitted preview if one is in flight (architect review: the
    /// slider commits once on release, not once per tick, so this is
    /// how the selected star still visibly tracks it on every frame in
    /// between — acceptance criterion 14's "updates live"). Used only
    /// by rendering (`Session::draw_list`, `shape_decoration_input`),
    /// never by hit-testing or any other read of the document's real
    /// state.
    pub(super) fn primitives_for_render(&self) -> Vec<PrimitiveSnapshot> {
        let mut primitives = self.primitives();
        if let Tool::PolygonStar = self.tool
            && let Some(preview_ratio) = self.poly_star.ratio_preview()
        {
            for primitive in &mut primitives {
                if self.selection.contains(primitive.id)
                    && let Shape::Star { inner_ratio, .. } = &mut primitive.shape
                {
                    *inner_ratio = preview_ratio;
                }
            }
        }
        // The Select tool's own live move offset (acceptance criterion
        // 20's "live"), applied via the one shared `ObjectSnapshot::
        // translated` rule `Document::translate_objects` commits with
        // (`specs/0004-canvas-navigation-and-selection/adrs.md`) — so the
        // preview and the eventual commit can never disagree.
        if let Some(offset) = self.select_live_offset() {
            for primitive in &mut primitives {
                if self.selection.contains(primitive.id) {
                    let translated = vecmanf_document_core::ObjectSnapshot::Primitive(*primitive)
                        .translated(offset);
                    if let vecmanf_document_core::ObjectSnapshot::Primitive(moved) = translated {
                        *primitive = moved;
                    }
                }
            }
        }
        // The Select tool's own live resize/rotate preview
        // (`specs/0005-object-transform/specification.md`, acceptance
        // criteria 14, 22) — same "preview and commit share one
        // implementation" reasoning as the move offset above.
        if let Some(vecmanf_document_core::ObjectSnapshot::Primitive(live)) =
            self.select_live_transform()
        {
            for primitive in &mut primitives {
                if primitive.id == live.id {
                    *primitive = live;
                }
            }
        }
        primitives
    }

    /// Whether `shape` is the kind the currently active shape tool
    /// creates/edits — the "tool mismatch" rule
    /// (`specification.md`'s "Selection and hover convention for
    /// primitives": a primitive's selection visuals only render while
    /// its own matching tool is active). Derived from
    /// [`super::select::tool_for_shape`]
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "The
    /// existing `shape_matches_active_tool` is then derived from it, so
    /// the two directions of the mapping cannot drift apart") — the same
    /// per-shape mapping the Select tool's double-click handoff uses to
    /// pick which tool a primitive hands off to.
    fn shape_matches_active_tool(&self, shape: &Shape) -> bool {
        super::select::tool_for_shape(shape) == self.tool
    }

    /// Acceptance criteria 1, 2 (rectangle), 7, 8 (ellipse), 11, 12
    /// (polygon/star): dispatches a press to whichever shape tool is
    /// active.
    pub(super) fn shape_pointer_down(&mut self, point: Point, shift: bool) {
        let primitives = self.primitives();
        let tolerances = self.shape_tolerances();
        match self.tool {
            Tool::Rectangle => {
                self.rectangle.pointer_down(
                    &primitives,
                    &mut self.selection,
                    point,
                    tolerances,
                    shift,
                );
            }
            Tool::Ellipse => {
                self.ellipse.pointer_down(
                    &primitives,
                    &mut self.selection,
                    point,
                    tolerances,
                    shift,
                );
            }
            Tool::PolygonStar => {
                self.poly_star.pointer_down(
                    &primitives,
                    &mut self.selection,
                    point,
                    tolerances,
                    shift,
                );
            }
            Tool::Select | Tool::Pen | Tool::Node => {}
        }
    }

    /// Feeds whatever shape-tool drag is in flight for the live preview
    /// (ux-engineer review: acceptance criteria 3, 4, 5, 9, 13, 14,
    /// 15's "updates live" wording) — writes nothing to the document.
    pub(super) fn shape_pointer_move(&mut self, point: Point, constrain: bool) {
        match self.tool {
            Tool::Rectangle => self.rectangle.pointer_move(point, constrain),
            Tool::Ellipse => self.ellipse.pointer_move(point, constrain),
            Tool::PolygonStar => self.poly_star.pointer_move(point),
            Tool::Select | Tool::Pen | Tool::Node => {}
        }
    }

    /// Updates [`Session::hovered_primitive`] for the active shape
    /// tool's own kind only (the same "tool mismatch" rule
    /// `shape_matches_active_tool` applies elsewhere).
    pub(super) fn update_hovered_primitive(&mut self, point: Point) {
        let primitives: Vec<PrimitiveSnapshot> = self
            .primitives()
            .into_iter()
            .filter(|p| self.shape_matches_active_tool(&p.shape))
            .collect();
        self.hovered_primitive =
            vecmanf_ui_core::hit_test_primitive(&primitives, point, self.segment_tolerance());
    }

    /// Acceptance criteria 1-6 (rectangle), 7-9 (ellipse), 11-15
    /// (polygon/star): commits whatever gesture `shape_pointer_down`
    /// began.
    pub(super) fn shape_pointer_up(&mut self, point: Point, constrain: bool) {
        match self.tool {
            Tool::Rectangle => {
                self.rectangle.pointer_up(&self.document, point, constrain);
            }
            Tool::Ellipse => {
                self.ellipse.pointer_up(&self.document, point, constrain);
            }
            Tool::PolygonStar => {
                self.poly_star.pointer_up(&self.document, point);
            }
            Tool::Select | Tool::Pen | Tool::Node => {}
        }
    }

    /// Cancels whichever shape tool's in-progress drag, writing
    /// nothing.
    pub(super) fn shape_escape(&mut self) {
        match self.tool {
            Tool::Rectangle => {
                self.rectangle.escape();
            }
            Tool::Ellipse => {
                self.ellipse.escape();
            }
            Tool::PolygonStar => {
                self.poly_star.escape();
            }
            Tool::Select | Tool::Pen | Tool::Node => {}
        }
    }

    /// Acceptance criterion 6's "remove rounding" action: zeroes the
    /// corner radius of every currently selected rectangle. A no-op
    /// outside the rectangle tool.
    pub fn remove_corner_rounding(&mut self) {
        if self.tool == Tool::Rectangle {
            self.rectangle
                .remove_rounding(&self.document, &self.selection);
        }
    }

    /// The polygon/star tool-options bar's current mode.
    #[must_use]
    pub fn poly_star_mode(&self) -> PolyStarMode {
        self.poly_star.mode()
    }

    /// The polygon/star tool-options bar's current point count.
    #[must_use]
    pub fn poly_star_point_count(&self) -> PointCount {
        self.poly_star.point_count()
    }

    /// The polygon/star tool-options bar's current ratio.
    #[must_use]
    pub fn poly_star_ratio(&self) -> InnerRatio {
        self.poly_star.ratio()
    }

    /// The mode toggle (acceptance criteria 11 vs. 12): only affects
    /// shapes drawn after the switch.
    pub fn set_poly_star_mode(&mut self, mode: PolyStarMode) {
        self.poly_star.set_mode(mode);
    }

    /// The point-count stepper (acceptance criteria 10, 15).
    pub fn set_poly_star_point_count(&mut self, count: PointCount) {
        self.poly_star
            .set_point_count(count, &self.document, &self.selection);
    }

    /// The ratio field's instantaneous commit (acceptance criteria 12,
    /// 14) — one commit immediately. For a continuously-dragged slider,
    /// use [`Session::preview_poly_star_ratio`] on every tick and
    /// [`Session::commit_poly_star_ratio`] once instead (architect
    /// review: one commit per tick is the bug this pair of methods
    /// exists to avoid).
    pub fn set_poly_star_ratio(&mut self, ratio: InnerRatio) {
        self.poly_star
            .set_ratio(ratio, &self.document, &self.selection);
    }

    /// The ratio slider's live, uncommitted preview (acceptance
    /// criterion 14's "updates live"): call on every slider tick.
    /// Writes nothing to the document — `Session::draw_list` picks it
    /// up through `primitives_for_render` so the selected star(s) still
    /// visibly track the slider.
    pub fn preview_poly_star_ratio(&mut self, ratio: InnerRatio) {
        self.poly_star.preview_ratio(ratio);
    }

    /// Commits whatever `preview_poly_star_ratio` has accumulated, as
    /// one commit for the whole selection — call once, when the slider
    /// drag ends.
    pub fn commit_poly_star_ratio(&mut self) {
        self.poly_star
            .commit_ratio_preview(&self.document, &self.selection);
    }

    /// "Object to path" (acceptance criteria 17, 21, 22): converts every
    /// currently selected primitive to a path, in one
    /// [`vecmanf_document_core::Document::convert_to_paths`] call
    /// (the anchor geometry itself is built by `vecmanf-ui-core`'s own
    /// `build_primitive_conversions` — architect review: this facade
    /// must hold no editing logic of its own, ADR 0001 §1), then
    /// switches to the node tool. The shape handles/tool-options bar
    /// disappear outright because the object is no longer a primitive
    /// at all (`specification.md`'s "Primitive vs. path: handles don't
    /// coexist") — nothing re-renders them since
    /// `shape_decoration_input` only ever looks at primitives.
    ///
    /// When exactly one primitive was selected, every one of its new
    /// anchors is selected in the node tool afterward, so it reads as
    /// "immediately editable" (acceptance criterion 17). The converted
    /// ids stay in `self.selection` — unlike the pre-`canvas-navigation-
    /// and-selection` behaviour, which cleared them — because "object to
    /// path" keeps the `NodeId` (`specs/0003-primitive-shapes/adrs.md`):
    /// they are still the same objects, now sharing their id space with
    /// the Select tool's own selection
    /// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "a multi-
    /// object conversion therefore stays selected together at object
    /// level"). A multi-object conversion (acceptance criterion 22)
    /// additionally selects the first converted path's anchors the same
    /// way — `vecmanf-ui-core`'s [`vecmanf_ui_core::NodeSelection`] has no
    /// representation for "these anchors across several different paths
    /// are selected together", so that part remains an approximation of
    /// AC22's "remain selected together" wording, not a literal one — a
    /// known, narrowed scope (see this crate's own report).
    pub fn convert_selected_to_paths(&mut self) {
        let ids = self.selection.ids().to_vec();
        if ids.is_empty() {
            return;
        }
        let conversions = build_primitive_conversions(&self.document, &mut self.minter, &ids);
        if conversions.is_empty() {
            return;
        }
        let first_converted = conversions[0].0;
        if self.document.convert_to_paths(&conversions).is_ok() {
            self.tool = Tool::Node;
            if let Some(path) = self.document.path(first_converted) {
                self.node.select_all_anchors(&path);
            }
        }
    }

    /// The active shape tool's live, uncommitted preview — `None`
    /// outside a shape tool, or when it has nothing to show (idle, or a
    /// still-degenerate create-drag).
    fn live_preview(&self) -> Option<LiveShape> {
        match self.tool {
            Tool::Rectangle => self.rectangle.live_shape(),
            Tool::Ellipse => self.ellipse.live_shape(),
            Tool::PolygonStar => self.poly_star.live_shape(),
            Tool::Select | Tool::Pen | Tool::Node => None,
        }
    }

    /// The shape to preview on canvas right now — `Session::draw_list`'s
    /// own hook into `live_preview` (ux-engineer review: "a maker
    /// dragging out a rectangle sees a rectangle updating live").
    ///
    /// Paired with the rotation to draw it at: a create-drag is always
    /// unrotated; a resize/radius/ratio adjustment keeps the single
    /// selected primitive's own `rotation` (a handle drag only ever
    /// starts on a single selection), so the preview outline turns with
    /// the shape instead of snapping unrotated for the length of the
    /// drag (`object-transform` acceptance criterion 25).
    pub(super) fn live_preview_shape(&self) -> Option<(Shape, vecmanf_document_core::Angle)> {
        let live = self.live_preview()?;
        let rotation = match live {
            LiveShape::Creating(..) => vecmanf_document_core::Angle::from_radians(0.0),
            LiveShape::Adjusting(_) => match self.selection.ids() {
                [only] => self
                    .primitives()
                    .into_iter()
                    .find(|p| p.id == *only)
                    .map_or(vecmanf_document_core::Angle::from_radians(0.0), |p| {
                        p.rotation
                    }),
                _ => vecmanf_document_core::Angle::from_radians(0.0),
            },
        };
        Some((*live.shape(), rotation))
    }

    /// The numeric readout for an in-progress create-drag
    /// (`specification.md`'s "Live creation feedback": "W × H" for a
    /// rectangle, "rx × ry" for an ellipse, the outer radius — and, for
    /// a star, its fixed ratio — for a polygon/star). `None` outside a
    /// create-drag (ux-engineer review item 2).
    #[must_use]
    pub fn live_readout(&self) -> Option<LiveReadout> {
        if self.tool == Tool::Select {
            return self.select_live_readout();
        }
        let LiveShape::Creating(shape, anchor) = self.live_preview()? else {
            return None;
        };
        let text = match shape {
            Shape::Rect { bounds, .. } => {
                format!(
                    "{:.1} × {:.1} mm",
                    bounds.width.as_mm(),
                    bounds.height.as_mm()
                )
            }
            Shape::Ellipse { frame } => {
                format!("{:.1} × {:.1} mm", frame.rx.as_mm(), frame.ry.as_mm())
            }
            Shape::Polygon { frame, .. } => format!("r {:.1} mm", frame.radius.as_mm()),
            Shape::Star {
                frame, inner_ratio, ..
            } => format!(
                "r {:.1} mm, ratio {:.2}",
                frame.radius.as_mm(),
                inner_ratio.get()
            ),
        };
        Some(LiveReadout { text, anchor })
    }

    /// Builds this frame's primitive-shape decoration input
    /// (`specs/0003-primitive-shapes/specification.md`'s "Tool mismatch"
    /// rule): empty unless a shape tool is active, and even then only
    /// for primitives of that tool's own kind.
    ///
    /// The "filled solid while being dragged" handle-glyph state
    /// (`docs/design-system.md`) is a flagged simplification: every
    /// handle here always reports `dragging: false`. Wiring a live
    /// per-handle "currently being dragged" flag through from each
    /// shape tool's own (private) drag state would need a new public
    /// accessor on each tool returning which exact handle (kind *and*,
    /// for a resize handle, which of the eight directions) is in
    /// flight; nothing in acceptance criteria 1-22 pins that visual
    /// detail, so it is left for a follow-up rather than guessed at
    /// here.
    pub(super) fn shape_decoration_input(&self) -> ShapeDecorationInput {
        if !matches!(
            self.tool,
            Tool::Rectangle | Tool::Ellipse | Tool::PolygonStar
        ) {
            return ShapeDecorationInput::default();
        }
        let primitives = self.primitives_for_render();
        let selected: Vec<NodeId> = self
            .selection
            .ids()
            .iter()
            .copied()
            .filter(|id| {
                primitives
                    .iter()
                    .find(|p| p.id == *id)
                    .is_some_and(|p| self.shape_matches_active_tool(&p.shape))
            })
            .collect();
        let handles = selected
            .iter()
            .filter_map(|&id| {
                let snapshot = primitives.iter().find(|p| p.id == id)?;
                let rendered: Vec<RenderShapeHandle> = vecmanf_ui_core::handles_for(snapshot)
                    .into_iter()
                    .map(|h| RenderShapeHandle {
                        kind: match h.kind {
                            HandleKind::Resize(_) => ShapeHandleKind::Resize,
                            HandleKind::CornerRadius => ShapeHandleKind::CornerRadius,
                            HandleKind::CornerRadiusEcho => ShapeHandleKind::CornerRadiusEcho,
                            HandleKind::InnerRadius => ShapeHandleKind::InnerRadius,
                        },
                        position: h.position,
                        draggable: h.draggable,
                        dragging: false,
                    })
                    .collect();
                Some((id, rendered))
            })
            .collect();
        let hovered = self.hovered_primitive.filter(|&id| {
            primitives
                .iter()
                .find(|p| p.id == id)
                .is_some_and(|p| self.shape_matches_active_tool(&p.shape))
        });
        ShapeDecorationInput {
            selected,
            hovered,
            handles,
        }
    }
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::Shape;

    use super::super::{Session, Tool};
    use vecmanf_document_core::Point;

    /// AC1: drawing a rectangle with the rectangle tool, from an empty
    /// canvas, with no UI setup beyond switching tools.
    #[test]
    fn ac1_rectangle_tool_creates_a_rectangle() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);

        let primitives = session.primitives();
        assert_eq!(primitives.len(), 1);
        assert!(matches!(primitives[0].shape, Shape::Rect { .. }));
    }

    /// AC7: same for the ellipse tool.
    #[test]
    fn ac7_ellipse_tool_creates_an_ellipse() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Ellipse);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);

        let primitives = session.primitives();
        assert_eq!(primitives.len(), 1);
        assert!(matches!(primitives[0].shape, Shape::Ellipse { .. }));
    }

    /// AC11: same for the polygon/star tool, in polygon mode by default.
    #[test]
    fn ac11_polygon_star_tool_creates_a_polygon_by_default() {
        let mut session = Session::new(1);
        session.set_tool(Tool::PolygonStar);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 0.0), false, false);

        let primitives = session.primitives();
        assert_eq!(primitives.len(), 1);
        assert!(matches!(primitives[0].shape, Shape::Polygon { .. }));
    }

    /// ux-engineer review item 1: during a create-drag, before release,
    /// `draw_list` already includes the live preview outline — not just
    /// a placeholder that snaps to shape on release.
    #[test]
    fn draw_list_includes_the_live_preview_during_a_create_drag() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        let empty = session.draw_list().triangle_count();

        session.pointer_down(Point::new(0.0, 0.0), false);
        let still_degenerate = session.draw_list().triangle_count();
        assert_eq!(still_degenerate, empty, "no movement yet: no preview");

        session.pointer_hover(Point::new(20.0, 10.0), false, false);
        let with_preview = session.draw_list().triangle_count();
        assert!(
            with_preview > empty,
            "the live rectangle preview must draw before release"
        );

        session.pointer_up(Point::new(20.0, 10.0), false, false);
        assert_eq!(session.primitives().len(), 1, "and it still commits once");
    }

    /// ux-engineer review item 2: the numeric readout is present during
    /// a create-drag and anchored at the live pointer position, and
    /// absent otherwise (idle, or a resize/radius/ratio adjustment).
    #[test]
    fn live_readout_only_applies_to_a_create_drag() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        assert!(
            session.live_readout().is_none(),
            "idle: nothing to read out"
        );

        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_hover(Point::new(20.0, 10.0), false, false);
        let readout = session.live_readout().expect("a create-drag is in flight");
        assert_eq!(readout.anchor, Point::new(20.0, 10.0));
        assert!(readout.text.contains("20.0"));
        assert!(readout.text.contains("10.0"));

        session.pointer_up(Point::new(20.0, 10.0), false, false);
        let id = session.primitives()[0].id;
        session.pointer_down(Point::new(0.0, 0.0), false); // select it
        session.pointer_hover(Point::new(0.0, 0.0), false, false);
        let _ = id;
        // A plain resize/radius drag (not a create-drag) never shows a
        // readout, even while its own live preview is visible.
        assert!(
            session.live_readout().is_none(),
            "no readout outside a create-drag"
        );
    }

    /// AC17, AC21: "object to path" only runs when explicitly invoked —
    /// switching tools and selecting a different object leave the
    /// primitive as it is; `convert_selected_to_paths` then replaces it
    /// with a path, keeping its id, and selects it in the node tool.
    #[test]
    fn ac17_ac21_object_to_path_is_explicit_and_keeps_the_id() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);
        let id = session.primitives()[0].id;

        // Switching tools and back does not convert it.
        session.set_tool(Tool::Node);
        session.set_tool(Tool::Rectangle);
        assert!(session.document.primitive(id).is_some());

        // Re-select it (a fresh `Session::pointer_down` on its outline)
        // before converting, since switching tools does not itself
        // clear the shared primitive selection, but this test does not
        // rely on that — it re-selects explicitly for clarity.
        session.pointer_down(Point::new(5.0, 0.0), false);
        session.convert_selected_to_paths();

        assert!(session.document.primitive(id).is_none());
        let path = session.document.path(id).expect("same id, now a path");
        assert!(path.closed);
        assert_eq!(path.anchors.len(), 4);
        assert_eq!(session.tool(), Tool::Node);
        assert_eq!(
            session.node.selection().nodes().len(),
            4,
            "AC17: immediately editable"
        );
    }

    /// AC22: two different primitive kinds selected together both
    /// convert, independently, in one call.
    #[test]
    fn ac22_multi_object_conversion() {
        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);
        let rect_id = session.primitives()[0].id;

        session.set_tool(Tool::Ellipse);
        session.pointer_down(Point::new(50.0, 50.0), false);
        session.pointer_up(Point::new(60.0, 60.0), false, false);
        let ellipse_id = session
            .primitives()
            .into_iter()
            .find(|p| p.id != rect_id)
            .expect("ellipse exists")
            .id;

        session.selection.select_single(rect_id);
        session.selection.toggle(ellipse_id);
        session.convert_selected_to_paths();

        assert!(session.document.path(rect_id).is_some());
        assert!(session.document.path(ellipse_id).is_some());
    }

    /// Architect review: dragging the ratio slider (preview on every
    /// tick, commit once) writes the document exactly once, and the
    /// rendered primitive tracks the preview on every tick in between.
    #[test]
    fn ratio_slider_preview_renders_live_but_commits_once() {
        use vecmanf_document_core::{InnerRatio, Length, PointCount, Shape, StarFrame};

        let mut session = Session::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: vecmanf_document_core::Angle::from_radians(0.0),
        };
        let id = session.document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        session.set_tool(Tool::PolygonStar);
        session.selection.select_single(id);

        for tick in [0.3, 0.6, 0.8] {
            session.preview_poly_star_ratio(InnerRatio::new(tick).unwrap());
            let Shape::Star { inner_ratio, .. } = session.document.primitive(id).unwrap().shape
            else {
                panic!("expected star");
            };
            assert!(
                (inner_ratio.get() - 0.5).abs() < 1e-9,
                "no tick writes to the document"
            );
            // The rendered primitive, however, already reflects the
            // in-progress preview.
            let rendered = session.primitives_for_render();
            let Shape::Star {
                inner_ratio: rendered_ratio,
                ..
            } = rendered.iter().find(|p| p.id == id).unwrap().shape
            else {
                panic!("expected star");
            };
            assert!((rendered_ratio.get() - tick).abs() < 1e-9);
        }

        session.commit_poly_star_ratio();
        let Shape::Star { inner_ratio, .. } = session.document.primitive(id).unwrap().shape else {
            panic!("expected star");
        };
        assert!((inner_ratio.get() - 0.8).abs() < 1e-9, "committed once");
    }

    /// Re-verification (architect, ratio-preview flush bug): a slider
    /// drag released *outside* the slider element never fires the
    /// slider's own `pointerup`/`blur` — common browser behavior. The
    /// next canvas click must not let `Session::pointer_down` change
    /// the selection *before* the pending preview flushes, or the
    /// previewed ratio would commit to whatever got newly selected
    /// instead of the star it was actually previewed against.
    #[test]
    fn ratio_preview_flushes_against_the_previewed_star_not_a_newly_selected_one() {
        use vecmanf_document_core::{InnerRatio, Length, PointCount, Shape, StarFrame};

        let mut session = Session::new(1);
        let frame_a = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: vecmanf_document_core::Angle::from_radians(0.0),
        };
        let frame_b = StarFrame {
            center: Point::new(100.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: vecmanf_document_core::Angle::from_radians(0.0),
        };
        let star_a = session.document.create_star(
            frame_a,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let star_b = session.document.create_star(
            frame_b,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        session.set_tool(Tool::PolygonStar);
        session.selection.select_single(star_a);

        // A slider drag against star A, in progress — never committed
        // (simulating the mouse being released off the slider, so
        // neither the slider's `pointerup` nor its `blur` ever fires).
        session.preview_poly_star_ratio(InnerRatio::new(0.9).unwrap());

        // The maker then clicks directly on star B's own outline (its
        // first outer vertex, at `frame_b.center + (radius, 0)`), which
        // changes the selection — `pointer_down` must flush the pending
        // preview first, against the selection as it was *before* this
        // click (star A), not after.
        let star_b_vertex = Point::new(110.0, 0.0);
        session.pointer_down(star_b_vertex, false);
        assert_eq!(
            session.selection.ids(),
            &[star_b],
            "the click did select the new star"
        );

        let Shape::Star {
            inner_ratio: a_ratio,
            ..
        } = session.document.primitive(star_a).unwrap().shape
        else {
            panic!("expected star");
        };
        assert!(
            (a_ratio.get() - 0.9).abs() < 1e-9,
            "the previewed ratio must commit to star A, the star it was previewed against"
        );

        let Shape::Star {
            inner_ratio: b_ratio,
            ..
        } = session.document.primitive(star_b).unwrap().shape
        else {
            panic!("expected star");
        };
        assert!(
            (b_ratio.get() - 0.5).abs() < 1e-9,
            "star B (only just selected, never previewed) must be untouched"
        );
    }

    /// Re-verification (tester, item 2): dragging the corner-radius
    /// handle through the full `Session` pointer API (not just the
    /// `RectangleTool` unit) must render a live preview mid-drag —
    /// before release, before any document commit — and that preview's
    /// radius must change continuously as the pointer moves, not only
    /// snap to a value on `pointer_up`.
    #[test]
    fn corner_radius_drag_renders_a_live_preview_through_the_session() {
        use vecmanf_ui_core::handles_for;

        let mut session = Session::new(1);
        session.set_tool(Tool::Rectangle);
        session.pointer_down(Point::new(0.0, 0.0), false);
        session.pointer_up(Point::new(10.0, 10.0), false, false);
        let id = session.primitives()[0].id;

        // Select it, then locate the corner-radius handle at its
        // current (zero-radius) position.
        session.pointer_down(Point::new(5.0, 0.0), false);
        let snapshot = session.document.primitive(id).unwrap();
        let handle = handles_for(&snapshot)
            .into_iter()
            .find(|h| matches!(h.kind, vecmanf_ui_core::HandleKind::CornerRadius))
            .expect("corner-radius handle exists even at zero radius (AC1 UX note)");

        let before_drag = session.draw_list().triangle_count();
        session.pointer_down(handle.position, false);
        // The drag has started but the pointer has not moved yet: the
        // live preview already exists (an "Adjusting" preview, unlike
        // a create-drag's "Creating" preview, shows up the instant the
        // handle is grabbed) and still reports the starting radius.
        let Some((
            Shape::Rect {
                corner_radius: start_radius,
                ..
            },
            _,
        )) = session.live_preview_shape()
        else {
            panic!("expected a live rect preview as soon as the handle is grabbed");
        };
        assert!(
            start_radius.as_mm().abs() < f64::EPSILON,
            "starting radius must be zero, matching the committed shape"
        );

        // Drag the handle inward along the diagonal, in two steps, and
        // confirm the live radius increases monotonically and nothing
        // is committed to the document until release.
        let first_drag = Point::new(handle.position.x - 1.0, handle.position.y + 1.0);
        session.pointer_hover(first_drag, false, false);
        let Some((
            Shape::Rect {
                corner_radius: first_radius,
                ..
            },
            _,
        )) = session.live_preview_shape()
        else {
            panic!("expected a live rect preview mid-drag");
        };
        assert!(
            first_radius.as_mm() > 0.0,
            "radius must already be live-visible before release"
        );
        let mid_draw_list = session.draw_list().triangle_count();
        assert!(
            mid_draw_list > before_drag,
            "the live preview outline must add geometry to the draw list mid-drag"
        );
        let Shape::Rect {
            corner_radius: still_uncommitted,
            ..
        } = session.document.primitive(id).unwrap().shape
        else {
            panic!("expected rect");
        };
        assert!(
            still_uncommitted.as_mm().abs() < f64::EPSILON,
            "mid-drag must not have written to the document yet"
        );

        let second_drag = Point::new(handle.position.x - 2.0, handle.position.y + 2.0);
        session.pointer_hover(second_drag, false, false);
        let Some((
            Shape::Rect {
                corner_radius: second_radius,
                ..
            },
            _,
        )) = session.live_preview_shape()
        else {
            panic!("expected a live rect preview mid-drag");
        };
        assert!(
            second_radius.as_mm() > first_radius.as_mm(),
            "the live radius must track the drag continuously, not jump only on release"
        );

        session.pointer_up(second_drag, false, false);
        assert!(
            session.live_preview_shape().is_none(),
            "no live preview once the drag is committed"
        );
        let Shape::Rect {
            corner_radius: committed,
            ..
        } = session.document.primitive(id).unwrap().shape
        else {
            panic!("expected rect");
        };
        assert!((committed.as_mm() - second_radius.as_mm()).abs() < 1e-9);
    }
}
