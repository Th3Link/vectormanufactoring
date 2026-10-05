//! `Session`'s Select-tool glue (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23): dispatching
//! pointer events to [`vecmanf_ui_core::SelectTool`], its hover/decoration
//! input, and the double-click handoff. Split out of `session/mod.rs` the
//! same way `session/shapes.rs` is for the shape tools (architect review:
//! one responsibility per file) — a child module of `session`, so it
//! shares `Session`'s privacy boundary and its methods below join the
//! same type's `impl Session` `session/mod.rs` itself defines.

use vecmanf_document_core::{ObjectSnapshot, Point, Shape};
use vecmanf_render_core::SelectDecorationInput;
use vecmanf_ui_core::{SelectDoubleClickOutcome, double_click, hit_test_object, object_bounds};

use super::{Session, Tool};

/// Which tool a primitive of this shape is created/edited with — the one
/// mapping both directions funnel through
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`: "`Session` maps
/// kind to `Tool` with **one** function, `tool_for`... The existing
/// `shape_matches_active_tool` is then derived from it, so the two
/// directions of the mapping cannot drift apart").
pub(super) fn tool_for_shape(shape: &Shape) -> Tool {
    match shape {
        Shape::Rect { .. } => Tool::Rectangle,
        Shape::Ellipse { .. } => Tool::Ellipse,
        Shape::Polygon { .. } | Shape::Star { .. } => Tool::PolygonStar,
    }
}

/// Which tool a double-clicked object hands off to (acceptance criteria
/// 22, 23): a path to the Node tool, a primitive to its own creation
/// tool via [`tool_for_shape`].
pub(super) fn tool_for(object: &ObjectSnapshot) -> Tool {
    match object {
        ObjectSnapshot::Path(_) => Tool::Node,
        ObjectSnapshot::Primitive(primitive) => tool_for_shape(&primitive.shape),
    }
}

impl Session {
    /// Acceptance criteria 14-18: dispatches a press to the Select tool.
    pub(super) fn select_pointer_down(&mut self, point: Point, shift: bool) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        self.select
            .pointer_down(&objects, &mut self.selection, point, tolerance, shift);
    }

    /// Acceptance criterion 20: commits whatever move-drag
    /// [`Session::select_pointer_down`] began.
    pub(super) fn select_pointer_up(&mut self, point: Point) {
        let objects = self.objects();
        self.select
            .pointer_up(&self.document, &objects, &mut self.selection, point);
    }

    /// Updates the Select tool's own hover state (UX notes: "Hover, tool
    /// not yet clicked into selection: same box at `--accent-hover`").
    pub(super) fn select_hover(&mut self, point: Point) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        self.hovered_object = hit_test_object(&objects, point, tolerance);
    }

    /// Acceptance criteria 22, 23: a double-click while the Select tool
    /// is active hands off to the hit object's own tool — clearing the
    /// node tool's selection for a path (so it is "ready for node
    /// editing with no nodes selected", `adrs.md`), or selecting the
    /// primitive (so its own shape tool shows its handles at once).
    pub(super) fn select_double_click(&mut self, point: Point) {
        let objects = self.objects();
        let tolerance = self.segment_tolerance();
        let outcome = double_click(&objects, point, tolerance);
        let SelectDoubleClickOutcome::Hit(object) = outcome else {
            return;
        };
        let target = tool_for(&object);
        match &object {
            ObjectSnapshot::Path(_) => {
                self.node.escape();
            }
            ObjectSnapshot::Primitive(primitive) => {
                self.selection.select_single(primitive.id);
            }
        }
        self.tool = target;
    }

    /// Builds the Select tool's decoration input for this frame: every
    /// selected object's own box, plus a hovered-but-unselected one — each
    /// box computed from the *live* (possibly drag-translated) geometry,
    /// via [`vecmanf_ui_core::object_bounds`], the same bounds rule
    /// `vecmanf-render-core`'s own primitive-selection box used to compute
    /// privately.
    pub(super) fn select_decoration_input(&self) -> SelectDecorationInput {
        if self.tool != Tool::Select {
            return SelectDecorationInput::default();
        }
        let mut objects: Vec<ObjectSnapshot> = self
            .live_node_drag_paths()
            .into_iter()
            .map(ObjectSnapshot::Path)
            .collect();
        objects.extend(
            self.primitives_for_render()
                .into_iter()
                .map(ObjectSnapshot::Primitive),
        );
        let selected = self
            .selection
            .ids()
            .iter()
            .filter_map(|&id| {
                objects
                    .iter()
                    .find(|object| object.id() == id)
                    .map(|object| (id, object_bounds(object)))
            })
            .collect();
        let hovered = self
            .hovered_object
            .filter(|&id| !self.selection.contains(id))
            .and_then(|id| {
                objects
                    .iter()
                    .find(|object| object.id() == id)
                    .map(|object| (id, object_bounds(object)))
            });
        SelectDecorationInput { selected, hovered }
    }
}
