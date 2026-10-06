//! The Select tool's state machine (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23; extended by
//! `specs/0005-object-transform/specification.md`, acceptance criteria
//! 1-23): click/shift-toggle selection over [`hit_test_object`], a
//! single-object drag-to-move tracked as a screen-independent document-
//! space offset, Delete/Backspace, the double-click handoff outcome —
//! and now, for a single-object selection only (acceptance criterion
//! 2), the 8 resize + 1 rotate transform handles
//! [`crate::transform_handle_layout`] lays out on that object's own
//! [`OrientedBox`]. Follows [`crate::RectangleTool`]'s established
//! pattern — ephemeral in-progress drag state (ADR 0009 §2), one
//! [`vecmanf_document_core::Document`] commit on release, a press-and-
//! release with no movement writes nothing (`specs/0002-path-node-
//! editing/adrs.md`'s rule, extended here to a resize/rotate too).

use vecmanf_document_core::{
    Document, NodeId, ObjectSnapshot, Point, PrimitiveSnapshot, Shape, Tolerance, Vec2,
};

use crate::ResizeDirection;
use crate::hit_test_object::hit_test_object;
use crate::object_selection::ObjectSelection;
use crate::oriented_box::{OrientedBox, oriented_bounds};
use crate::transform_drag::{commit_resize, compute_resize, compute_rotate};
use crate::transform_handle_layout::{
    ALL_EIGHT, CORNERS_FOUR, TransformHandle, hit_test_transform_handle,
    resize_anchor_local_position, resize_handle_hit, rotate_pivot, transform_handles,
};

/// The three tolerances the Select tool's own transform handles need —
/// mirrors [`crate::ShapeHitTolerances`]'s split for the shape tools.
#[derive(Debug, Clone, Copy)]
pub struct TransformHandleTolerances {
    /// Bounds a hit against one of the 8 resize handles.
    pub resize: Tolerance,
    /// Bounds a hit against the rotate handle.
    pub rotate: Tolerance,
    /// The rotate handle's screen-space offset above the top-edge
    /// resize handle, already converted to document millimetres by the
    /// caller at the current zoom (the same convention every other
    /// handle's screen-space hit size already uses).
    pub rotate_offset_mm: f64,
}

#[derive(Debug, Clone, Default)]
enum SelectDrag {
    #[default]
    None,
    /// A move in progress: the document point the drag started at.
    Moving { down_at: Point },
    /// A resize-handle drag in progress.
    Resizing {
        id: NodeId,
        start: ObjectSnapshot,
        start_box: OrientedBox,
        direction: ResizeDirection,
        down_at: Point,
    },
    /// A rotate-handle drag in progress.
    Rotating {
        start: ObjectSnapshot,
        start_box: OrientedBox,
        down_at: Point,
    },
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
    /// A transform handle was hit; a resize or rotate drag began
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criterion 1).
    Handle,
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
/// (`specification.md`: "the Select tool shows no shape handles... only
/// the bounding box") for two-or-more selected objects — but, since
/// `object-transform`, a single selected object's own 8 resize + 1
/// rotate transform handles — plus a plain object selection (shared
/// with the shape tools, [`ObjectSelection`]) and whichever single-
/// object drag is in flight.
#[derive(Debug, Default)]
pub struct SelectTool {
    drag: SelectDrag,
}

/// Whether `object` is a polygon or a star — the one kind whose
/// transform handles are corner-only and always-uniform (acceptance
/// criterion 11).
fn is_polygon_or_star(object: &ObjectSnapshot) -> bool {
    matches!(
        object,
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Polygon { .. } | Shape::Star { .. },
            ..
        })
    )
}

/// Every resize-handle direction `object`'s own transform-handle set
/// shows (acceptance criterion 11: corner-only for a polygon/star).
fn resize_directions_for(object: &ObjectSnapshot) -> &'static [ResizeDirection] {
    if is_polygon_or_star(object) {
        &CORNERS_FOUR
    } else {
        &ALL_EIGHT
    }
}

impl SelectTool {
    /// A tool with no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every transform handle the current single-object selection shows
    /// right now, in document space — `empty` for no selection or a
    /// multi-selection (acceptance criterion 2). Independent of whether
    /// a drag is in flight; `Session`'s own decoration input calls this
    /// every frame, the same way `vecmanf-ui-core::handles_for` already
    /// works for a shape tool's own handles.
    #[must_use]
    pub fn transform_handles(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        tolerances: TransformHandleTolerances,
    ) -> Vec<(TransformHandle, Point)> {
        let [only_id] = selection.ids() else {
            return Vec::new();
        };
        let Some(object) = objects.iter().find(|o| o.id() == *only_id) else {
            return Vec::new();
        };
        let box_ = oriented_bounds(object);
        transform_handles(
            &box_,
            resize_directions_for(object),
            tolerances.rotate_offset_mm,
        )
    }

    /// Which handle, if any, is currently being dragged — for the
    /// renderer's own "solid fill while dragging" state
    /// (`docs/design-system.md`).
    #[must_use]
    pub const fn dragging_handle(&self) -> Option<TransformHandle> {
        match &self.drag {
            SelectDrag::Resizing { direction, .. } => Some(TransformHandle::Resize(*direction)),
            SelectDrag::Rotating { .. } => Some(TransformHandle::Rotate),
            SelectDrag::None | SelectDrag::Moving { .. } => None,
        }
    }

    /// The active scale/rotate pivot, shown only while a drag is in
    /// flight (`docs/design-system.md`'s "Transform pivot marker") —
    /// `shift`'s live state decides which point, re-evaluated every
    /// frame so the marker jumps the instant the modifier changes.
    #[must_use]
    pub fn live_pivot(&self, shift: bool) -> Option<Point> {
        match &self.drag {
            SelectDrag::Resizing {
                start,
                start_box,
                direction,
                ..
            } => {
                let local = if is_polygon_or_star(start) {
                    start_box.local_center()
                } else {
                    resize_anchor_local_position(start_box.min, start_box.max, *direction, shift)
                };
                Some(start_box.to_document(local))
            }
            SelectDrag::Rotating { start_box, .. } => Some(rotate_pivot(start_box, shift)),
            SelectDrag::None | SelectDrag::Moving { .. } => None,
        }
    }

    /// The transform handle of the current single-object selection under
    /// `point`, if any, with that object and its oriented box — the one
    /// hit test [`SelectTool::pointer_down`] (to start a drag) and a
    /// hover cursor query (`Session`'s cursor hint) both use, so what
    /// the cursor promises and what a press does can never disagree.
    /// `None` for no selection or a multi-selection (acceptance
    /// criterion 2).
    #[must_use]
    pub fn handle_at<'a>(
        objects: &'a [ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
    ) -> Option<(&'a ObjectSnapshot, OrientedBox, TransformHandle)> {
        let [only_id] = selection.ids() else {
            return None;
        };
        let object = objects.iter().find(|o| o.id() == *only_id)?;
        let box_ = oriented_bounds(object);
        let handles = transform_handles(
            &box_,
            resize_directions_for(object),
            tolerances.rotate_offset_mm,
        );
        let (rotate, resize): (Vec<_>, Vec<_>) = handles
            .into_iter()
            .partition(|(h, _)| matches!(h, TransformHandle::Rotate));
        let hit = hit_test_transform_handle(&rotate, point, tolerances.rotate.as_mm())
            .or_else(|| resize_handle_hit(&resize, &box_, point, tolerances.resize))?;
        Some((object, box_, hit))
    }

    /// Acceptance criteria 1, 4-18: hit-tests `point` first against the
    /// current single-object selection's own transform handles, then
    /// (same as before `object-transform`) against every object's own
    /// body — updates `selection` and starts whichever drag matches.
    pub fn pointer_down(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        tolerance: Tolerance,
        handle_tolerances: TransformHandleTolerances,
        shift: bool,
    ) -> SelectPointerDownOutcome {
        // `adrs.md`: "ui-core filters the selection against the current
        // snapshot first" — drops any id a prior action (this peer's own
        // edit in a different tool, or a collaborator) has since removed,
        // before this click can act on it.
        selection.retain_existing(objects);

        if let Some((object, box_, handle)) =
            Self::handle_at(objects, selection, point, handle_tolerances)
        {
            self.drag = match handle {
                TransformHandle::Rotate => SelectDrag::Rotating {
                    start: object.clone(),
                    start_box: box_,
                    down_at: point,
                },
                TransformHandle::Resize(direction) => SelectDrag::Resizing {
                    id: object.id(),
                    start: object.clone(),
                    start_box: box_,
                    direction,
                    down_at: point,
                },
            };
            return SelectPointerDownOutcome::Handle;
        }

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
            SelectDrag::None | SelectDrag::Resizing { .. } | SelectDrag::Rotating { .. } => None,
        }
    }

    /// The live, uncommitted resize preview (acceptance criterion 14):
    /// the object as it would commit right now, re-evaluated from the
    /// drag-start snapshot every call so `shift`/`ctrl`'s live state is
    /// always reflected. `None` unless a resize is in flight.
    #[must_use]
    pub fn live_resize(&self, current: Point, shift: bool, ctrl: bool) -> Option<ObjectSnapshot> {
        match &self.drag {
            SelectDrag::Resizing {
                start,
                start_box,
                direction,
                down_at,
                ..
            } => Some(compute_resize(
                start, start_box, *direction, *down_at, current, shift, ctrl,
            )),
            SelectDrag::None | SelectDrag::Moving { .. } | SelectDrag::Rotating { .. } => None,
        }
    }

    /// The live, uncommitted rotate preview (acceptance criterion 22).
    /// `None` unless a rotate is in flight.
    #[must_use]
    pub fn live_rotate(&self, current: Point, shift: bool, ctrl: bool) -> Option<ObjectSnapshot> {
        match &self.drag {
            SelectDrag::Rotating {
                start,
                start_box,
                down_at,
                ..
            } => Some(compute_rotate(
                start, start_box, *down_at, current, shift, ctrl,
            )),
            SelectDrag::None | SelectDrag::Moving { .. } | SelectDrag::Resizing { .. } => None,
        }
    }

    /// Commits whatever drag is in flight — a move (as one
    /// [`vecmanf_document_core::Document::translate_objects`] call for
    /// the whole selection, acceptance criteria 18, 20), a resize, or a
    /// rotate (one [`vecmanf_document_core::Document::rotate_object`]
    /// call, acceptance criteria 15-18) — a no-op (writes nothing) if no
    /// drag was in flight, or it moved nowhere (acceptance criterion 3).
    /// `shift`/`ctrl` are the modifiers' state at release.
    pub fn pointer_up(
        &mut self,
        document: &Document,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        shift: bool,
        ctrl: bool,
    ) {
        match std::mem::take(&mut self.drag) {
            SelectDrag::None => {}
            SelectDrag::Moving { down_at } => {
                if down_at == point {
                    return;
                }
                let offset = down_at.vector_to(point);
                selection.retain_existing(objects);
                if selection.is_empty() {
                    return;
                }
                let _ = document.translate_objects(selection.ids(), offset);
            }
            SelectDrag::Resizing {
                id,
                start,
                start_box,
                direction,
                down_at,
            } => {
                if down_at == point {
                    return;
                }
                let resized =
                    compute_resize(&start, &start_box, direction, down_at, point, shift, ctrl);
                if resized != start {
                    commit_resize(document, id, &resized);
                }
            }
            SelectDrag::Rotating {
                start,
                start_box,
                down_at,
                ..
            } => {
                if down_at == point {
                    return;
                }
                let rotated = compute_rotate(&start, &start_box, down_at, point, shift, ctrl);
                if rotated != start {
                    let _ = document.rotate_object(&rotated);
                }
            }
        }
    }

    /// Cancels whichever drag is in flight, writing nothing.
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
    use vecmanf_document_core::{
        AnchorId, EllipseFrame, Length, NewAnchor, NodeId, RectBounds, StarFrame,
    };

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
    const HANDLE_TOLERANCES: TransformHandleTolerances = TransformHandleTolerances {
        resize: Tolerance::from_mm(1.0),
        rotate: Tolerance::from_mm(1.0),
        rotate_offset_mm: 5.0,
    };

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
            HANDLE_TOLERANCES,
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
            HANDLE_TOLERANCES,
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
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(selection.ids(), &[a]);
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(55.0, 0.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
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
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(55.0, 0.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
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
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(
            selection.ids(),
            &[a, b],
            "clicking a selected member keeps the group selected"
        );

        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(8.0, 3.0),
            false,
            false,
        );

        let shape_a = document.primitive(a).expect("exists").shape;
        let Shape::Rect { bounds, .. } = shape_a else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(3.0, 3.0));
        let shape_b = document.primitive(b).expect("exists").shape;
        let Shape::Rect { bounds, .. } = shape_b else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(53.0, 3.0));
    }

    #[test]
    fn ac19_delete_removes_every_selected_object() {
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
            HANDLE_TOLERANCES,
            false,
        );

        let offset = tool
            .live_offset(Point::new(5.0, 9.0))
            .expect("a drag in flight");
        assert_eq!(offset, Vec2::new(5.0, 4.0));
        // Not committed yet.
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(0.0, 0.0));

        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(5.0, 9.0),
            false,
            false,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
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
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(
            selection.ids(),
            &[id],
            "sanity check: the press did hit and select it"
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(0.0, 5.0),
            false,
            false,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
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
    /// every other, still-live selected object.
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
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(selection.ids(), &[live]);

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
            false,
            false,
        );

        assert_eq!(
            selection.ids(),
            &[live],
            "the stale id must be dropped by pointer_up itself"
        );
        let Shape::Rect { bounds, .. } = document.primitive(live).expect("exists").shape else {
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

    // --- object-transform (slice 5) ---

    /// Acceptance criterion 1: with exactly one object selected, its 9
    /// handles (8 resize + 1 rotate) appear; clicking one starts a
    /// resize/rotate drag rather than a move.
    #[test]
    fn ac1_a_single_selection_shows_nine_handles() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES);
        assert_eq!(handles.len(), 9);
    }

    /// Acceptance criterion 2: a multi-selection shows no transform
    /// handles at all.
    #[test]
    fn ac2_a_multi_selection_shows_no_transform_handles() {
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
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES);
        assert_eq!(handles.len(), 0);
    }

    /// Acceptance criterion 4: a free corner-handle drag resizes the
    /// rectangle, anchored at the opposite corner, committed on release.
    #[test]
    fn ac4_free_corner_resize_commits_on_release() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        // The Se resize handle sits at (10, 10).
        let outcome = tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 10.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(outcome, SelectPointerDownOutcome::Handle);

        let live = tool
            .live_resize(Point::new(15.0, 13.0), false, false)
            .expect("resize in progress");
        let ObjectSnapshot::Primitive(p) = &live else {
            panic!("expected a primitive");
        };
        let Shape::Rect { bounds, .. } = p.shape else {
            panic!("expected rect");
        };
        assert!((bounds.width.as_mm() - 15.0).abs() < 1e-9);
        assert!((bounds.height.as_mm() - 13.0).abs() < 1e-9);

        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(15.0, 13.0),
            false,
            false,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected rect");
        };
        assert_eq!(bounds.origin, Point::new(0.0, 0.0));
        assert!((bounds.width.as_mm() - 15.0).abs() < 1e-9);
        assert!((bounds.height.as_mm() - 13.0).abs() < 1e-9);
    }

    /// Acceptance criterion 5: holding Ctrl on a corner handle resizes
    /// proportionally.
    #[test]
    fn ac5_ctrl_corner_resize_is_proportional() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 10.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(20.0, 11.0),
            false,
            true,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected rect");
        };
        assert!((bounds.width.as_mm() - 20.0).abs() < 1e-9);
        assert!(
            (bounds.height.as_mm() - 20.0).abs() < 1e-9,
            "forced to match the dominant axis"
        );
    }

    /// Acceptance criterion 7: Shift anchors a resize at the center.
    #[test]
    fn ac7_shift_resize_anchors_at_the_center() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        // The E resize handle sits at (10, 5).
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 5.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(14.0, 5.0),
            true,
            false,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected rect");
        };
        assert!((bounds.origin.x - (-4.0)).abs() < 1e-9);
        assert!((bounds.width.as_mm() - 18.0).abs() < 1e-9);
    }

    /// Acceptance criterion 8: a proportional resize scales the stroke
    /// width by the same factor.
    #[test]
    fn ac8_proportional_resize_scales_stroke_width() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 10.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(15.0, 15.0),
            false,
            true,
        );
        let snapshot = document.primitive(id).expect("exists");
        // 1.5x proportional resize -> stroke width also 1.5x (0.25 -> 0.375).
        assert!((snapshot.stroke_width.as_mm() - 0.375).abs() < 1e-9);
    }

    /// Acceptance criterion 9: a rectangle's corner radius scales by the
    /// same factor as a proportional resize.
    #[test]
    fn ac9_proportional_resize_scales_corner_radius() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        document
            .set_corner_radius(&[id], Length::from_mm(2.0))
            .expect("set radius");
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 10.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(20.0, 20.0),
            false,
            true,
        );
        let Shape::Rect { corner_radius, .. } = document.primitive(id).expect("exists").shape
        else {
            panic!("expected rect");
        };
        assert!((corner_radius.as_mm() - 4.0).abs() < 1e-9, "2x factor");
    }

    /// Acceptance criterion 11: a star's corner-handle drag is always a
    /// uniform scale, point count and ratio untouched.
    #[test]
    fn ac11_star_corner_handle_is_uniform_scale() {
        use vecmanf_document_core::{InnerRatio, PointCount};
        let document = Document::new(1);
        let frame = StarFrame {
            center: Point::new(0.0, 0.0),
            radius: Length::from_mm(10.0),
            angle: vecmanf_document_core::Angle::from_radians(0.0),
        };
        let id = document.create_star(
            frame,
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        // The Ne corner handle sits along the (1,-1) diagonal.
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES);
        assert_eq!(handles.len(), 5, "4 corners + rotate, no edges");
        let (_, ne_position) = handles
            .iter()
            .find(|(h, _)| matches!(h, TransformHandle::Resize(ResizeDirection::Ne)))
            .expect("Ne handle exists");
        tool.pointer_down(
            &objects,
            &mut selection,
            *ne_position,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        let drag_to = ne_position.translated(Vec2::new(1.0, -1.0));
        tool.pointer_up(&document, &objects, &mut selection, drag_to, false, false);
        let Shape::Star {
            frame: new_frame,
            point_count,
            inner_ratio,
        } = document.primitive(id).expect("exists").shape
        else {
            panic!("expected star");
        };
        assert!(new_frame.radius.as_mm() > 10.0, "radius grew");
        assert_eq!(point_count.get(), 5);
        assert!((inner_ratio.get() - 0.5).abs() < 1e-9);
    }

    /// Acceptance criterion 12: resizing a path scales every anchor's
    /// point and handle vectors by the drag's per-axis factors.
    #[test]
    fn ac12_path_resize_scales_anchors_and_handles() {
        let document = Document::new(1);
        let id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 3), Point::new(10.0, 10.0)),
                NewAnchor::corner(AnchorId::new(1, 4), Point::new(0.0, 10.0)),
            ],
            true,
        );
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        // Se resize handle sits at (10, 10).
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 10.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(20.0, 10.0),
            false,
            false,
        );
        let snapshot = document.path(id).expect("exists");
        // Anchored at (0,0); X doubled, Y unchanged.
        let far_corner = snapshot
            .anchors
            .iter()
            .find(|a| (a.point.y - 10.0).abs() < 1e-6 && a.point.x > 15.0)
            .expect("the (10,10) corner scaled in X to (20,10)");
        assert!((far_corner.point.x - 20.0).abs() < 1e-6);
    }

    /// Acceptance criterion 13: a resize drag past the opposite edge
    /// clamps the dimension to zero rather than flipping negative.
    #[test]
    fn ac13_resize_clamps_at_zero() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 5.0), // E handle
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(-100.0, 5.0),
            false,
            false,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected rect");
        };
        assert!(bounds.width.as_mm().abs() < 1e-9, "clamped to zero");
    }

    /// Acceptance criterion 15: dragging the rotate handle rotates about
    /// the object's own center, writing `rotation`.
    #[test]
    fn ac15_rotate_handle_drag_rotates_about_the_center() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES);
        let (_, rotate_position) = handles
            .iter()
            .find(|(h, _)| matches!(h, TransformHandle::Rotate))
            .expect("rotate handle exists");
        let outcome = tool.pointer_down(
            &objects,
            &mut selection,
            *rotate_position,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(outcome, SelectPointerDownOutcome::Handle);
        // Swing the rotate handle a quarter turn around the center (5,5).
        let center = Point::new(5.0, 5.0);
        let current = center.translated(Vec2::new(-5.0, 0.0));
        tool.pointer_up(&document, &objects, &mut selection, current, false, false);
        let snapshot = document.primitive(id).expect("exists");
        assert!(
            snapshot.rotation.as_radians().abs() > 0.1,
            "rotation written"
        );
        let Shape::Rect { bounds, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        let new_center = Point::new(
            bounds.origin.x + bounds.width.as_mm() / 2.0,
            bounds.origin.y + bounds.height.as_mm() / 2.0,
        );
        assert!((new_center.x - 5.0).abs() < 1e-6, "center unmoved");
        assert!((new_center.y - 5.0).abs() < 1e-6);
    }

    /// Acceptance criterion 17: Ctrl snaps the rotate drag to 15°
    /// increments.
    #[test]
    fn ac17_ctrl_snaps_rotation_to_15_degrees() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES);
        let (_, rotate_position) = handles
            .iter()
            .find(|(h, _)| matches!(h, TransformHandle::Rotate))
            .expect("rotate handle exists");
        tool.pointer_down(
            &objects,
            &mut selection,
            *rotate_position,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        let center = Point::new(5.0, 5.0);
        let to_rotate_handle = center.vector_to(*rotate_position);
        let base_angle = to_rotate_handle.y.atan2(to_rotate_handle.x);
        let radius = to_rotate_handle.length();
        let ten_degrees_from_start = {
            let angle = base_angle + 10.0_f64.to_radians();
            center.translated(Vec2::new(radius * angle.cos(), radius * angle.sin()))
        };
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            ten_degrees_from_start,
            false,
            true,
        );
        let snapshot = document.primitive(id).expect("exists");
        let degrees = snapshot.rotation.as_radians().to_degrees();
        assert!(
            (degrees - 15.0).abs() < 1e-6,
            "snapped to 15 degrees, got {degrees}"
        );
    }

    /// Acceptance criteria 4, 18: resizing a *rotated* primitive along its
    /// own axes keeps the opposite corner fixed on screen too — the
    /// frame's own center moves with the resize, and the rotation turns
    /// about that center, so without pinning the anchor it would swing.
    #[test]
    fn ac4_ac18_resizing_a_rotated_rect_keeps_the_opposite_corner_fixed_in_document_space() {
        use vecmanf_document_core::{Angle, outline_of_rotated};
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let angle = Angle::from_radians(30.0_f64.to_radians());
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(5.0, 5.0), angle),
            )
            .expect("rotate");
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let nw_before = {
            let p = document.primitive(id).expect("exists");
            outline_of_rotated(&p.shape, p.rotation)[0].point
        };
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES);
        let se = handles
            .iter()
            .find(|(h, _)| matches!(h, TransformHandle::Resize(ResizeDirection::Se)))
            .expect("Se handle")
            .1;
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            se,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        // Drag 5 mm along the object's own local X axis.
        let drag_to = se.translated(Vec2::new(5.0, 0.0).rotated(angle));
        tool.pointer_up(&document, &objects, &mut selection, drag_to, false, false);
        let p = document.primitive(id).expect("exists");
        let Shape::Rect { bounds, .. } = p.shape else {
            panic!("expected rect");
        };
        assert!(
            (bounds.width.as_mm() - 15.0).abs() < 1e-9,
            "grew along local X"
        );
        assert!((bounds.height.as_mm() - 10.0).abs() < 1e-9);
        let nw_after = outline_of_rotated(&p.shape, p.rotation)[0].point;
        assert!(
            (nw_after.x - nw_before.x).abs() < 1e-9,
            "{nw_after:?} vs {nw_before:?}"
        );
        assert!((nw_after.y - nw_before.y).abs() < 1e-9);
        assert!((p.rotation.as_radians() - angle.as_radians()).abs() < 1e-9);
    }

    fn handle_pos(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        wanted: TransformHandle,
    ) -> Point {
        SelectTool::transform_handles(objects, selection, HANDLE_TOLERANCES)
            .into_iter()
            .find(|(h, _)| *h == wanted)
            .expect("handle exists")
            .1
    }

    /// Acceptance criterion 3: pressing any transform handle and
    /// releasing without moving writes nothing.
    #[test]
    fn ac3_a_press_and_release_on_a_handle_writes_nothing() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let before = document.object(id);
        for wanted in [
            TransformHandle::Rotate,
            TransformHandle::Resize(ResizeDirection::Se),
            TransformHandle::Resize(ResizeDirection::N),
        ] {
            let at = handle_pos(&objects, &selection, wanted);
            let mut tool = SelectTool::new();
            let outcome = tool.pointer_down(
                &objects,
                &mut selection,
                at,
                TOLERANCE,
                HANDLE_TOLERANCES,
                false,
            );
            assert_eq!(outcome, SelectPointerDownOutcome::Handle);
            tool.pointer_up(&document, &objects, &mut selection, at, true, true);
            assert_eq!(document.object(id), before, "{wanted:?}");
        }
    }

    /// Acceptance criterion 6: an edge handle changes only its own
    /// perpendicular dimension; Ctrl adds nothing.
    #[test]
    fn ac6_edge_handle_changes_one_dimension_and_ignores_ctrl() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let n = handle_pos(
            &objects,
            &selection,
            TransformHandle::Resize(ResizeDirection::N),
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            n,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(50.0, -6.0),
            false,
            true,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("expected rect");
        };
        assert!(
            (bounds.width.as_mm() - 10.0).abs() < 1e-9,
            "width untouched"
        );
        assert!((bounds.height.as_mm() - 16.0).abs() < 1e-9);
        assert!(
            (bounds.origin.y - (-6.0)).abs() < 1e-9,
            "bottom edge anchored"
        );
    }

    /// Acceptance criterion 8: a non-proportional resize scales the
    /// stroke width by √(sx·sy) (4 × 1 → 2).
    #[test]
    fn ac8_non_proportional_resize_scales_stroke_width_by_the_geometric_mean() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let e = handle_pos(
            &objects,
            &selection,
            TransformHandle::Resize(ResizeDirection::E),
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            e,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(40.0, 5.0),
            false,
            false,
        );
        let snapshot = document.primitive(id).expect("exists");
        assert!(
            (snapshot.stroke_width.as_mm() - 0.5).abs() < 1e-9,
            "0.25 × √4"
        );
    }

    /// Acceptance criterion 8: a drag toward zero never takes the stroke
    /// width to zero or below — it stays at the smallest positive value.
    #[test]
    fn ac8_stroke_width_is_floored_above_zero_when_a_resize_collapses_the_object() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let e = handle_pos(
            &objects,
            &selection,
            TransformHandle::Resize(ResizeDirection::E),
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            e,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(-80.0, 5.0),
            false,
            false,
        );
        let snapshot = document.primitive(id).expect("exists");
        assert!(
            snapshot.stroke_width.as_mm() > 0.0,
            "never zero or negative"
        );
        assert!(snapshot.stroke_width.as_mm() < 0.25);
    }

    /// Acceptance criterion 10: an ellipse resizes through the Select
    /// tool's handles — rx/ry follow the box.
    #[test]
    fn ac10_ellipse_resizes_through_the_select_tools_handles() {
        let document = Document::new(1);
        let id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(3.0),
        });
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let se = handle_pos(
            &objects,
            &selection,
            TransformHandle::Resize(ResizeDirection::Se),
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            se,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            se.translated(Vec2::new(4.0, 2.0)),
            false,
            false,
        );
        let Shape::Ellipse { frame } = document.primitive(id).expect("exists").shape else {
            panic!("still an ellipse");
        };
        assert!((frame.rx.as_mm() - 7.0).abs() < 1e-9);
        assert!((frame.ry.as_mm() - 4.0).abs() < 1e-9);
        // Anchored at the opposite (Nw) corner (-5, -3).
        assert!((frame.center.x - (-5.0 + 7.0)).abs() < 1e-9);
        assert!((frame.center.y - (-3.0 + 4.0)).abs() < 1e-9);
    }

    /// Acceptance criterion 16: Shift pivots a rotate to the bottom-edge
    /// midpoint, so the object's center swings around it.
    #[test]
    fn ac16_shift_rotates_about_the_bottom_edge_midpoint() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let r = handle_pos(&objects, &selection, TransformHandle::Rotate);
        tool.pointer_down(
            &objects,
            &mut selection,
            r,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        // Bottom-edge midpoint is (5, 10); the handle sits straight above
        // it at (5, -5), so a quarter turn clockwise (+90° in Y-down)
        // swings it to (20, 10).
        let current = Point::new(5.0 + 15.0, 10.0);
        assert_eq!(tool.live_pivot(true), Some(Point::new(5.0, 10.0)));
        assert_eq!(tool.live_pivot(false), Some(Point::new(5.0, 5.0)));
        tool.pointer_up(&document, &objects, &mut selection, current, true, false);
        let snapshot = document.primitive(id).expect("exists");
        assert!((snapshot.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let Shape::Rect { bounds, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        // The old center (5, 5) is 5 above the pivot; a clockwise quarter
        // turn about (5, 10) puts it at (10, 10).
        let center = Point::new(
            bounds.origin.x + bounds.width.as_mm() / 2.0,
            bounds.origin.y + bounds.height.as_mm() / 2.0,
        );
        assert!(
            (center.x - 10.0).abs() < 1e-9 && (center.y - 10.0).abs() < 1e-9,
            "{center:?}"
        );
    }

    /// Acceptance criteria 20, 21: rotating a path bakes its anchors and
    /// stores the angle; rotating a primitive keeps it the same kind.
    #[test]
    fn ac20_ac21_rotate_bakes_a_path_and_never_converts_a_primitive() {
        let document = Document::new(1);
        let path_id = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 2), Point::new(10.0, 0.0)),
            ],
            false,
        );
        let rect_id = rect(&document, 100.0);
        for id in [path_id, rect_id] {
            let objects = vec![document.object(id).expect("exists")];
            let mut selection = ObjectSelection::new();
            selection.select_single(id);
            let mut tool = SelectTool::new();
            let r = handle_pos(&objects, &selection, TransformHandle::Rotate);
            tool.pointer_down(
                &objects,
                &mut selection,
                r,
                TOLERANCE,
                HANDLE_TOLERANCES,
                false,
            );
            let b = oriented_bounds(&objects[0]);
            let pivot = b.to_document(b.local_center());
            let to = pivot.translated(
                pivot
                    .vector_to(r)
                    .rotated(vecmanf_document_core::Angle::from_radians(0.6)),
            );
            tool.pointer_up(&document, &objects, &mut selection, to, false, false);
        }
        let path = document.path(path_id).expect("still a path");
        assert!((path.rotation.as_radians() - 0.6).abs() < 1e-9);
        assert!(
            path.anchors[1].point.y.abs() > 0.1,
            "anchors baked, not just the register"
        );
        assert!(
            document.primitive(rect_id).is_some(),
            "rectangle stays a rectangle"
        );
        assert!(document.path(rect_id).is_none());
    }

    /// UX review item 1: on a small object the handles' hit radii must not
    /// swallow the body. A 10 × 5.5 rectangle (a 40 × 22 px box at the
    /// usual 16 px radius, all lengths divided by 4) with 4-unit hit
    /// radii: points on the top outline away from a handle, and the
    /// middle of the box, are *not* handle hits (so the body — the move —
    /// gets them), while the handles themselves and points just outside
    /// them still are.
    #[test]
    fn a_small_rectangles_body_stays_reachable_despite_the_handle_radii() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(5.5),
        });
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let wide = TransformHandleTolerances {
            resize: Tolerance::from_mm(4.0),
            rotate: Tolerance::from_mm(4.0),
            rotate_offset_mm: 8.0,
        };
        let handle_at = |point: Point| {
            SelectTool::handle_at(&objects, &selection, point, wide).map(|(_, _, h)| h)
        };
        // 2.5 from the NW corner and from the N handle: both full 4-unit
        // radii would claim it; the shrunk radius (a third of 5.5) does
        // not, so the outline press there moves the object.
        assert_eq!(handle_at(Point::new(2.5, 0.0)), None);
        // The middle is deep inside: body, not resize.
        assert_eq!(handle_at(Point::new(5.0, 2.75)), None);
        // The handles still work, from just outside as well.
        assert_eq!(
            handle_at(Point::new(10.0, 5.5)),
            Some(TransformHandle::Resize(ResizeDirection::Se))
        );
        assert_eq!(
            handle_at(Point::new(11.0, 6.5)),
            Some(TransformHandle::Resize(ResizeDirection::Se))
        );
        // And just inside a handle, within the thin inner band, too.
        assert_eq!(
            handle_at(Point::new(9.5, 5.0)),
            Some(TransformHandle::Resize(ResizeDirection::Se))
        );
    }

    /// UX review item 1, through the tool itself, at the real 40 × 22 px
    /// size with 16 px hit radii (this needed the `nearest_point_on_segment`
    /// fix to be testable): a press on the top outline away from any
    /// handle is a *move* (the object follows the drag), and a press at
    /// the centre is not a resize. An unfilled object's centre is not a
    /// body hit either (slice 4: outline only), so it just deselects.
    #[test]
    fn a_40_by_22_rectangle_moves_from_its_outline_and_is_not_resized_from_its_centre() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(40.0),
            height: Length::from_mm(22.0),
        });
        let objects = vec![document.object(id).expect("exists")];
        let wide = TransformHandleTolerances {
            resize: Tolerance::from_mm(16.0),
            rotate: Tolerance::from_mm(16.0),
            rotate_offset_mm: 32.0,
        };
        let mut selection = ObjectSelection::new();
        selection.select_single(id);

        let mut tool = SelectTool::new();
        let centre = tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(20.0, 11.0),
            TOLERANCE,
            wide,
            false,
        );
        assert_ne!(centre, SelectPointerDownOutcome::Handle);
        assert!(tool.dragging_handle().is_none());

        selection.select_single(id);
        let on_outline = Point::new(10.0, 0.0);
        assert_eq!(
            tool.pointer_down(&objects, &mut selection, on_outline, TOLERANCE, wide, false),
            SelectPointerDownOutcome::Selected,
            "an outline press away from every handle starts a move"
        );
        assert!(
            tool.dragging_handle().is_none(),
            "a move, not a handle drag"
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            on_outline.translated(Vec2::new(5.0, 3.0)),
            false,
            false,
        );
        let Shape::Rect { bounds, .. } = document.primitive(id).expect("exists").shape else {
            panic!("rect");
        };
        assert_eq!(bounds.origin, Point::new(5.0, 3.0), "moved by the drag");
        assert!((bounds.width.as_mm() - 40.0).abs() < 1e-9, "not resized");
    }

    /// A zero-height path (a line) keeps grabbable handles: the radius
    /// floor stops the box-size scaling from shrinking it to nothing.
    #[test]
    fn a_line_paths_handles_stay_grabbable() {
        let document = Document::new(1);
        let id = path(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let outcome = tool.pointer_down(
            &objects,
            &mut selection,
            Point::new(10.0, 0.0),
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(outcome, SelectPointerDownOutcome::Handle);
    }

    /// Tester finding: the polygon/star corner handle follows the pointer
    /// exactly — dragging the NE corner out by (+5, -5) puts the new
    /// corner at (r+5, -(r+5)), i.e. the radius grows by 5, not by the
    /// 7.07 of raw diagonal displacement.
    #[test]
    fn a_polygon_corner_handle_ends_up_under_the_pointer() {
        use vecmanf_document_core::PointCount;
        let document = Document::new(1);
        let id = document.create_polygon(
            StarFrame {
                center: Point::new(0.0, 0.0),
                radius: Length::from_mm(10.0),
                angle: vecmanf_document_core::Angle::from_radians(0.0),
            },
            PointCount::new(6).unwrap(),
        );
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let ne = handle_pos(
            &objects,
            &selection,
            TransformHandle::Resize(ResizeDirection::Ne),
        );
        assert_eq!(ne, Point::new(10.0, -10.0));
        let mut tool = SelectTool::new();
        tool.pointer_down(
            &objects,
            &mut selection,
            ne,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            Point::new(15.0, -15.0),
            false,
            false,
        );
        let Shape::Polygon { frame, .. } = document.primitive(id).expect("exists").shape else {
            panic!("polygon");
        };
        assert!(
            (frame.radius.as_mm() - 15.0).abs() < 1e-9,
            "{}",
            frame.radius.as_mm()
        );
    }

    /// Tester/ADR: a NaN, infinite or absurd pointer never writes
    /// non-finite geometry — the drag resolves to "no change".
    #[test]
    fn hostile_pointer_values_resolve_to_no_change() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let before = document.object(id);
        let objects = vec![before.clone().expect("exists")];
        // NaN/infinity leave every drag unchanged; a finite but absurd
        // 1e300 only a *resize* would blow up (a rotation by any finite
        // pointer is a legal angle), so it is checked on resizes alone.
        let cases = [
            (Point::new(f64::NAN, f64::NAN), true),
            (Point::new(f64::INFINITY, 0.0), true),
            (Point::new(1e300, -1e300), false),
        ];
        for (bad, include_rotate) in cases {
            for wanted in [
                TransformHandle::Rotate,
                TransformHandle::Resize(ResizeDirection::Se),
                TransformHandle::Resize(ResizeDirection::E),
            ] {
                if wanted == TransformHandle::Rotate && !include_rotate {
                    continue;
                }
                let mut selection = ObjectSelection::new();
                selection.select_single(id);
                let at = handle_pos(&objects, &selection, wanted);
                let mut tool = SelectTool::new();
                tool.pointer_down(
                    &objects,
                    &mut selection,
                    at,
                    TOLERANCE,
                    HANDLE_TOLERANCES,
                    false,
                );
                tool.pointer_up(&document, &objects, &mut selection, bad, true, true);
                assert_eq!(document.object(id), before, "{wanted:?} to {bad:?}");
            }
        }
    }

    /// Architect item 1: the live rotate preview and the committed
    /// rotation are the same snapshot — one rule.
    #[test]
    fn rotate_preview_and_commit_resolve_to_the_same_snapshot() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let r = handle_pos(&objects, &selection, TransformHandle::Rotate);
        tool.pointer_down(
            &objects,
            &mut selection,
            r,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        let to = Point::new(-3.0, 4.0);
        let preview = tool.live_rotate(to, true, false).expect("rotating");
        tool.pointer_up(&document, &objects, &mut selection, to, true, false);
        assert_eq!(document.object(id), Some(preview));
    }

    /// Acceptance criterion 23: a plain body drag still moves the object
    /// and never touches its rotation or size, even for an already-
    /// rotated object.
    #[test]
    fn ac23_move_never_changes_rotation_or_size() {
        use vecmanf_document_core::Angle;
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        document
            .rotate_object(
                &document
                    .object(id)
                    .expect("object exists")
                    .rotated(Point::new(5.0, 5.0), Angle::from_radians(0.3)),
            )
            .expect("rotate");
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        let mut tool = SelectTool::new();
        // The rotated E edge-midpoint, on the outline — nothing is
        // selected yet, so this press is necessarily a plain body hit,
        // never a handle check (that only runs once an object is
        // already the sole selection).
        let body_point =
            Point::new(5.0, 5.0).translated(Vec2::new(5.0, 0.0).rotated(Angle::from_radians(0.3)));
        let outcome = tool.pointer_down(
            &objects,
            &mut selection,
            body_point,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(outcome, SelectPointerDownOutcome::Selected);
        tool.pointer_up(
            &document,
            &objects,
            &mut selection,
            body_point.translated(Vec2::new(2.0, 3.0)),
            false,
            false,
        );
        let snapshot = document.primitive(id).expect("exists");
        assert!(
            (snapshot.rotation.as_radians() - 0.3).abs() < 1e-9,
            "a move never touches rotation"
        );
        let Shape::Rect { bounds, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        assert!(
            (bounds.width.as_mm() - 10.0).abs() < 1e-9,
            "a move never touches size"
        );
    }
}
