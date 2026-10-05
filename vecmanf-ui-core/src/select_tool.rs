//! The Select tool's state machine (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23): click/shift-
//! toggle selection over [`hit_test_object`], a single-object drag-to-move
//! tracked as a screen-independent document-space offset, Delete/
//! Backspace, and the double-click handoff outcome. Follows
//! [`crate::RectangleTool`]'s established pattern — ephemeral in-progress
//! drag state (ADR 0009 §2), one [`vecmanf_document_core::Document`]
//! commit on release, a press-and-release with no movement writes nothing
//! (`specs/0002-path-node-editing/adrs.md`'s rule, extended here to a
//! whole-object move).

use vecmanf_document_core::{Document, ObjectSnapshot, Point, Tolerance, Vec2};

use crate::hit_test_object::hit_test_object;
use crate::object_selection::ObjectSelection;

#[derive(Debug, Clone, Copy, Default)]
enum SelectDrag {
    #[default]
    None,
    /// A move in progress: the document point the drag started at.
    Moving { down_at: Point },
}

/// What [`SelectTool::pointer_down`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectPointerDownOutcome {
    /// An object was hit — selected (acceptance criterion 14), toggled
    /// into a multi-selection (acceptance criterion 17), or left alone
    /// because it was already part of one (so the whole group can be
    /// dragged, acceptance criterion 18) — and a move-drag began.
    Selected,
    /// Nothing was hit; the selection was cleared (acceptance criterion
    /// 15) unless Shift was held.
    Cleared,
}

/// What a double-click on the Select tool hit (`adrs.md`: "the Select
/// tool returns an outcome (the hit `NodeId` and its kind, or a miss)").
#[derive(Debug, Clone, PartialEq)]
pub enum SelectDoubleClickOutcome {
    /// Nothing was hit; no handoff.
    Miss,
    /// This object was hit — the caller (`Session`) selects it and maps
    /// its kind to the tool to hand off to (acceptance criteria 22, 23).
    Hit(ObjectSnapshot),
}

/// The Select tool's state: no shape handles, no path nodes
/// (`specification.md`: "the Select tool shows no shape handles and no
/// path nodes, only the bounding box") — just a plain object selection
/// (shared with the shape tools, [`ObjectSelection`]) and whichever
/// single-object move-drag is in flight.
#[derive(Debug, Default)]
pub struct SelectTool {
    drag: SelectDrag,
}

impl SelectTool {
    /// A tool with no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acceptance criteria 14-18: hit-tests `point` against every object,
    /// updates `selection` (plain click selects/replaces, Shift-click
    /// toggles, a miss clears unless Shift is held), and — when something
    /// is now part of the selection — starts a move-drag.
    pub fn pointer_down(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        tolerance: Tolerance,
        shift: bool,
    ) -> SelectPointerDownOutcome {
        // `adrs.md`: "ui-core filters the selection against the current
        // snapshot first" — drops any id a prior action (this peer's own
        // edit in a different tool, or a collaborator) has since removed,
        // before this click can act on it.
        selection.retain_existing(objects);
        let Some(hit) = hit_test_object(objects, point, tolerance) else {
            if !shift {
                selection.clear();
            }
            self.drag = SelectDrag::None;
            return SelectPointerDownOutcome::Cleared;
        };

        if shift {
            selection.toggle(hit);
        } else if !selection.contains(hit) {
            // A plain click on a *different* object is single-select,
            // not additive (acceptance criterion 16). A plain click on an
            // object already part of a multi-selection leaves the whole
            // selection as it is, so the group can be dragged together
            // (acceptance criterion 18) — matching every reference tool's
            // own "click one of several selected objects to drag them
            // all" convention.
            selection.select_single(hit);
        }
        self.drag = SelectDrag::Moving { down_at: point };
        SelectPointerDownOutcome::Selected
    }

    /// The live, uncommitted move offset while a drag is in flight — the
    /// Select tool's own counterpart to the shape tools' `live_shape`
    /// (acceptance criterion 20's "live"). `None` when idle, or once the
    /// drag has moved nowhere yet — a press-and-release with no movement
    /// must write nothing (`specs/0002-path-node-editing/adrs.md`'s rule).
    #[must_use]
    pub fn live_offset(&self, current_point: Point) -> Option<Vec2> {
        match self.drag {
            SelectDrag::Moving { down_at } => {
                let offset = down_at.vector_to(current_point);
                if offset == Vec2::ZERO {
                    None
                } else {
                    Some(offset)
                }
            }
            SelectDrag::None => None,
        }
    }

    /// Commits whatever move-drag is in flight as one
    /// [`vecmanf_document_core::Document::translate_objects`] call for the
    /// whole selection (acceptance criteria 18, 20) — a no-op (writes
    /// nothing) if no drag was in flight, or it moved nowhere. `objects`
    /// is the current object list, used to drop any id `selection` still
    /// names that no longer exists (`adrs.md`: "ui-core filters the
    /// selection against the current snapshot first") *before* calling
    /// `translate_objects`, so one stale id (e.g. a path a Node-tool
    /// Delete removed down to nothing after this drag started) cannot
    /// refuse the whole move for every other, still-live selected object.
    pub fn pointer_up(
        &mut self,
        document: &Document,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
    ) {
        let offset = self.live_offset(point);
        self.drag = SelectDrag::None;
        let Some(offset) = offset else {
            return;
        };
        selection.retain_existing(objects);
        if selection.is_empty() {
            return;
        }
        let _ = document.translate_objects(selection.ids(), offset);
    }

    /// Cancels whichever move-drag is in flight, writing nothing.
    pub fn escape(&mut self) {
        self.drag = SelectDrag::None;
    }

    /// Acceptance criteria 19, 21: deletes every selected object as one
    /// commit, then clears the selection (every id it named no longer
    /// exists). `objects` filters out any already-stale id first, the
    /// same way [`SelectTool::pointer_up`] does.
    pub fn delete_selected(
        &mut self,
        document: &Document,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
    ) {
        self.drag = SelectDrag::None;
        selection.retain_existing(objects);
        if selection.is_empty() {
            return;
        }
        if document.delete_objects(selection.ids()).is_ok() {
            selection.clear();
        }
    }
}

/// Acceptance criteria 22, 23: what a double-click hit, for `Session` to
/// map to the object's own tool (`tool_for`) and hand off to.
#[must_use]
pub fn double_click(
    objects: &[ObjectSnapshot],
    point: Point,
    tolerance: Tolerance,
) -> SelectDoubleClickOutcome {
    let Some(hit) = hit_test_object(objects, point, tolerance) else {
        return SelectDoubleClickOutcome::Miss;
    };
    objects
        .iter()
        .find(|object| object.id() == hit)
        .cloned()
        .map_or(
            SelectDoubleClickOutcome::Miss,
            SelectDoubleClickOutcome::Hit,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{AnchorId, Length, NewAnchor, NodeId, RectBounds};

    fn rect(document: &Document, x: f64) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(x, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        })
    }

    /// Called at most once per test's `Document`, so a fixed pair of
    /// anchor ids never collides within one test.
    fn path(document: &Document, x: f64) -> NodeId {
        document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(x, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(x + 10.0, 0.0)),
            ],
            false,
        )
    }

    const TOLERANCE: Tolerance = Tolerance::from_mm(1.0);

    #[test]
    fn ac14_clicking_an_object_selects_it() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut tool = SelectTool::new();
        let mut selection = ObjectSelection::new();
        let outcome = tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(5.0, 0.0),
            TOLERANCE,
            false,
        );
        assert_eq!(outcome, SelectPointerDownOutcome::Selected);
        assert_eq!(selection.ids(), &[id]);
    }

    #[test]
    fn ac15_clicking_empty_canvas_clears_the_selection() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut tool = SelectTool::new();
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(500.0, 500.0),
            TOLERANCE,
            false,
        );
        assert!(selection.is_empty());
    }

    #[test]
    fn ac16_plain_click_on_a_different_object_replaces_the_selection() {
        let document = Document::new(1);
        let a = rect(&document, 0.0);
        let b = rect(&document, 50.0);
        let objects = vec![
            document.object(a).expect("exists"),
            document.object(b).expect("exists"),
        ];
        let mut tool = SelectTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(5.0, 0.0),
            TOLERANCE,
            false,
        );
        assert_eq!(selection.ids(), &[a]);
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(55.0, 0.0),
            TOLERANCE,
            false,
        );
        assert_eq!(
            selection.ids(),
            &[b],
            "plain click is single-select, not additive"
        );
    }

    #[test]
    fn ac17_shift_click_adds_a_second_object_a_path_this_time() {
        let document = Document::new(1);
        let a = rect(&document, 0.0);
        let b = path(&document, 50.0);
        let objects = vec![
            document.object(a).expect("exists"),
            document.object(b).expect("exists"),
        ];
        let mut tool = SelectTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(5.0, 0.0),
            TOLERANCE,
            false,
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(55.0, 0.0),
            TOLERANCE,
            true,
        );
        assert_eq!(selection.ids(), &[a, b]);
    }

    #[test]
    fn ac18_dragging_any_selected_member_moves_the_whole_multi_selection() {
        let document = Document::new(1);
        let a = rect(&document, 0.0);
        let b = rect(&document, 50.0);
        let objects = vec![
            document.object(a).expect("exists"),
            document.object(b).expect("exists"),
        ];
        let mut selection = ObjectSelection::new();
        selection.toggle(a);
        selection.toggle(b);

        let mut tool = SelectTool::new();
        // Click (no shift) on A, already part of the multi-selection:
        // the whole selection must stay intact.
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(5.0, 0.0),
            TOLERANCE,
            false,
        );
        assert_eq!(
            selection.ids(),
            &[a, b],
            "clicking a selected member keeps the group selected"
        );

        tool.pointer_up(&document, &objects, &mut selection, Point::new(8.0, 3.0));

        let shape_a = document.primitive(a).expect("exists").shape;
        let vecmanf_document_core::Shape::Rect { bounds, .. } = shape_a else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(3.0, 3.0));
        let shape_b = document.primitive(b).expect("exists").shape;
        let vecmanf_document_core::Shape::Rect { bounds, .. } = shape_b else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(53.0, 3.0));
    }

    #[test]
    fn ac19_delete_removes_every_selected_object() {
        // The one-commit-per-batch property itself is pinned directly on
        // `Document::delete_objects` in `vecmanf-document-core::objects`'s
        // own tests (that crate's `Loro` handle is `pub(crate)`, not
        // visible from here) — this test is the Select-tool-level
        // behavioural check.
        let document = Document::new(1);
        let a = rect(&document, 0.0);
        let b = path(&document, 50.0);
        let objects = vec![
            document.object(a).expect("exists"),
            document.object(b).expect("exists"),
        ];
        let mut selection = ObjectSelection::new();
        selection.toggle(a);
        selection.toggle(b);

        let mut tool = SelectTool::new();
        tool.delete_selected(&document, &objects, &mut selection);

        assert!(selection.is_empty());
        assert_eq!(document.object_ids(), Vec::new());
    }

    #[test]
    fn ac20_a_single_object_drag_moves_it_live_and_commits_once_on_release() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut tool = SelectTool::new();
        let mut selection = ObjectSelection::new();
        // (0.0, 5.0) sits on the rect's left edge — the interior is
        // unfilled and so not hittable (same rule `hit_test_primitive`
        // already has), the edge is.
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(0.0, 5.0),
            TOLERANCE,
            false,
        );

        let offset = tool
            .live_offset(Point::new(5.0, 9.0))
            .expect("a drag in flight");
        assert_eq!(offset, Vec2::new(5.0, 4.0));
        // Not committed yet.
        let vecmanf_document_core::Shape::Rect { bounds, .. } =
            document.primitive(id).expect("exists").shape
        else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(0.0, 0.0));

        tool.pointer_up(&document, &objects, &mut selection, Point::new(5.0, 9.0));
        let vecmanf_document_core::Shape::Rect { bounds, .. } =
            document.primitive(id).expect("exists").shape
        else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(5.0, 4.0));
    }

    #[test]
    fn a_press_and_release_with_no_movement_writes_nothing() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut tool = SelectTool::new();
        let mut selection = ObjectSelection::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(0.0, 5.0),
            TOLERANCE,
            false,
        );
        assert_eq!(
            selection.ids(),
            &[id],
            "sanity check: the press did hit and select it"
        );
        tool.pointer_up(&document, &objects, &mut selection, Point::new(0.0, 5.0));
        let vecmanf_document_core::Shape::Rect { bounds, .. } =
            document.primitive(id).expect("exists").shape
        else {
            panic!("expected rect");
        };
        assert_eq!(
            bounds.origin,
            Point::new(0.0, 0.0),
            "no movement must write nothing"
        );
    }

    /// Architect review: a stale id (e.g. a path the Node tool deleted
    /// down to nothing, or any other action that removed an object the
    /// Select tool once selected) must not block committing the move of
    /// every other, still-live selected object. The stale id enters the
    /// selection *after* the drag's own `pointer_down` already ran (the
    /// same shape as the real repro: select A with Select, switch tools,
    /// delete A, switch back to Select, shift-click B, then drag B —
    /// `pointer_down` for the drag only ever sees the object it hit, not
    /// the already-stale A still sitting in `selection`), so this test
    /// exercises `pointer_up`'s own independent `retain_existing` call,
    /// not `pointer_down`'s.
    #[test]
    fn pointer_up_drops_a_stale_id_so_it_cannot_block_moving_the_rest() {
        let document = Document::new(1);
        let live = rect(&document, 0.0);
        let doomed = rect(&document, 50.0);
        let objects_at_press = vec![
            document.object(live).expect("exists"),
            document.object(doomed).expect("exists"),
        ];

        let mut selection = ObjectSelection::new();
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects_at_press,
            &mut selection,
            Point::new(0.0, 5.0),
            TOLERANCE,
            false,
        );
        assert_eq!(selection.ids(), &[live]);

        // `doomed` joins the selection, then is removed from the
        // document entirely — both after `pointer_down` already ran, so
        // nothing has pruned it from `selection` yet.
        selection.toggle(doomed);
        document.delete_objects(&[doomed]).expect("delete doomed");
        let objects_at_release: Vec<ObjectSnapshot> = document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect();

        tool.pointer_up(
            &document,
            &objects_at_release,
            &mut selection,
            Point::new(5.0, 9.0),
        );

        assert_eq!(
            selection.ids(),
            &[live],
            "the stale id must be dropped by pointer_up itself"
        );
        let vecmanf_document_core::Shape::Rect { bounds, .. } =
            document.primitive(live).expect("exists").shape
        else {
            panic!("expected rect");
        };
        assert_eq!(
            bounds.origin,
            Point::new(5.0, 4.0),
            "the live object must still move even though a stale id shared its batch"
        );
    }

    /// Same defense, for `delete_selected` (architect review): a stale id
    /// sitting alongside a live one must not stop the live object from
    /// being deleted.
    #[test]
    fn delete_selected_drops_a_stale_id_so_it_cannot_block_deleting_the_rest() {
        let document = Document::new(1);
        let live = rect(&document, 0.0);
        let doomed = rect(&document, 50.0);
        let mut selection = ObjectSelection::new();
        selection.toggle(live);
        selection.toggle(doomed);

        document
            .delete_objects(&[doomed])
            .expect("delete doomed early");
        let objects: Vec<ObjectSnapshot> = document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect();

        let mut tool = SelectTool::new();
        tool.delete_selected(&document, &objects, &mut selection);

        assert!(selection.is_empty());
        assert_eq!(
            document.object_ids(),
            Vec::new(),
            "the live object must still be deleted despite the stale id"
        );
    }

    #[test]
    fn ac22_double_click_on_a_path_reports_that_path() {
        let document = Document::new(1);
        let id = path(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let outcome = double_click(&objects, Point::new(5.0, 0.0), TOLERANCE);
        assert_eq!(
            outcome,
            SelectDoubleClickOutcome::Hit(document.object(id).expect("exists"))
        );
    }

    #[test]
    fn ac23_double_click_on_a_primitive_reports_that_primitive() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let outcome = double_click(&objects, Point::new(5.0, 0.0), TOLERANCE);
        assert_eq!(
            outcome,
            SelectDoubleClickOutcome::Hit(document.object(id).expect("exists"))
        );
    }

    #[test]
    fn double_click_on_empty_canvas_is_a_miss() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let outcome = double_click(&objects, Point::new(500.0, 500.0), TOLERANCE);
        assert_eq!(outcome, SelectDoubleClickOutcome::Miss);
    }
}
