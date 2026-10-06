//! The Select tool's state machine (`specs/0004-canvas-navigation-and-
//! selection/specification.md`, acceptance criteria 14-23; extended by
//! `specs/0005-object-transform/specification.md` and
//! `specs/object-transform-refinements/specification.md`): click/shift-
//! toggle selection over [`hit_test_object`], a drag-to-move tracked as a
//! screen-independent document-space offset, Delete/Backspace, the double-
//! click handoff outcome — and, for a single-object selection only, the
//! transform handles [`crate::transform_handle_layout`] lays out on that
//! object's own [`OrientedBox`]: resize, rotate (corner and Shift-revealed
//! side), skew (paths) and the centre move handle, plus the typed numeric
//! entry [`crate::transform_entry`]. Follows [`crate::RectangleTool`]'s
//! established pattern — ephemeral in-progress drag state (ADR 0009 §2), one
//! [`vecmanf_document_core::Document`] commit on release, a press-and-
//! release inside the 3 px dead zone writes nothing.

use vecmanf_document_core::{Document, ObjectSnapshot, Point, Tolerance};

use crate::hit_test_object::hit_test_object;
use crate::object_selection::ObjectSelection;
use crate::oriented_box::{OrientedBox, oriented_bounds};
use crate::transform_drag::{DragOrigin, StrokeScaling, TransformDrag, is_polygon_or_star};
use crate::transform_entry::TransformEntry;
pub use crate::transform_handle_layout::TransformHandleTolerances;
use crate::transform_handle_layout::{
    ALL_EIGHT, CORNERS_FOUR, EditHandle, HandleSpec, hit_transform_handle, transform_handles,
};

mod entry;
mod preview;

pub use entry::double_click;

#[derive(Debug, Clone, Default)]
enum SelectDrag {
    #[default]
    None,
    /// A move in progress (body or centre handle).
    Moving {
        origin: DragOrigin,
        /// Whether the press was on the centre handle (it then shows its
        /// dragging look).
        from_center: bool,
    },
    /// A resize, rotate or skew drag in progress.
    Transforming(TransformDrag),
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
    /// A transform handle was hit; a resize, rotate or skew drag began
    /// (`specs/0005-object-transform/specification.md`, acceptance
    /// criterion 1).
    Handle,
}

/// What a double-click on the Select tool did (`adrs.md`, "double-click on
/// a handle").
#[derive(Debug, Clone, PartialEq)]
pub enum SelectDoubleClickOutcome {
    /// Nothing was hit; no handoff.
    Miss,
    /// This object was hit — the caller (`Session`) selects it and maps
    /// its kind to the tool to hand off to (acceptance criteria 22, 23 of
    /// slice 4; criterion 3 of `object-transform-refinements` for the
    /// inside of the selected box).
    Hit(ObjectSnapshot),
    /// A rotate or resize handle was double-clicked: the numeric entry is
    /// open (criteria 18, 25, 26), and there is no handoff (criteria 23,
    /// 32).
    EntryOpened,
    /// A skew handle was double-clicked: nothing happens, and there is no
    /// handoff (criterion 49).
    Ignored,
}

/// The Select tool's state: no shape handles, no path nodes
/// (`specification.md`: "the Select tool shows no shape handles... only
/// the bounding box") for two-or-more selected objects — but, since
/// `object-transform`, a single selected object's own transform handles —
/// plus a plain object selection (shared with the shape tools,
/// [`ObjectSelection`]), whichever drag is in flight and the open numeric
/// entry, if any.
#[derive(Debug, Default)]
pub struct SelectTool {
    drag: SelectDrag,
    stroke_scaling: StrokeScaling,
    entry: Option<TransformEntry>,
    /// The handle the most recent press landed on (`None` for a press
    /// anywhere else): a double-click only acts on a handle the *first*
    /// press already grabbed, so double-clicking the outline of an object
    /// that is not selected yet (whose handles appear after the first click)
    /// still hands off.
    last_press_handle: Option<EditHandle>,
}

/// The handle kinds `object` shows (criteria 11, 37, 50): corner resize
/// only for a polygon or star; skew handles only for a path; the side
/// rotate handles when `side_rotate`.
fn handle_spec_for(object: &ObjectSnapshot, side_rotate: bool) -> HandleSpec {
    HandleSpec {
        resize_directions: if is_polygon_or_star(object) {
            &CORNERS_FOUR
        } else {
            &ALL_EIGHT
        },
        skew: matches!(object, ObjectSnapshot::Path(_)),
        side_rotate,
    }
}

/// The sole selected object in `objects`, if the selection is exactly one.
fn sole_selected<'a>(
    objects: &'a [ObjectSnapshot],
    selection: &ObjectSelection,
) -> Option<&'a ObjectSnapshot> {
    let [only_id] = selection.ids() else {
        return None;
    };
    objects.iter().find(|o| o.id() == *only_id)
}

impl SelectTool {
    /// A tool with no drag in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether resizes scale the stroke width — the "Scale stroke width"
    /// switch (AC 26-31). Tool state, never written to the document; the
    /// default is [`StrokeScaling::Keep`] and a new tool (every new
    /// session) starts there (AC 27).
    #[must_use]
    pub const fn stroke_scaling(&self) -> StrokeScaling {
        self.stroke_scaling
    }

    /// Sets the stroke-scaling mode for the *next* resize drag: a drag
    /// already in flight keeps the value it was pressed with (AC 28).
    pub fn set_stroke_scaling(&mut self, stroke_scaling: StrokeScaling) {
        self.stroke_scaling = stroke_scaling;
    }

    /// Whether the four side rotate handles show right now (criterion 6):
    /// the live Shift state while idle; frozen as it was at the press during
    /// a drag, and as it was when a numeric entry opened while one is open,
    /// so pressing Shift mid-drag reveals nothing and a dragged side handle
    /// stays until the drag ends.
    #[must_use]
    pub fn side_rotate_revealed(&self, live_shift: bool) -> bool {
        match &self.drag {
            SelectDrag::Moving { origin, .. } => origin.shift_at_press,
            SelectDrag::Transforming(drag) => drag.origin.shift_at_press,
            SelectDrag::None => self
                .entry
                .as_ref()
                .map_or(live_shift, TransformEntry::side_rotate_revealed),
        }
    }

    /// Every transform handle the current single-object selection shows
    /// right now, in document space — `empty` for no selection or a
    /// multi-selection (acceptance criterion 2 of slice 5). Independent of
    /// whether a drag is in flight; `side_rotate` is
    /// [`SelectTool::side_rotate_revealed`].
    #[must_use]
    pub fn transform_handles(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        tolerances: TransformHandleTolerances,
        side_rotate: bool,
    ) -> Vec<(EditHandle, Point)> {
        let Some(object) = sole_selected(objects, selection) else {
            return Vec::new();
        };
        transform_handles(
            &oriented_bounds(object),
            handle_spec_for(object, side_rotate),
            &tolerances,
        )
    }

    /// The transform handle of the current single-object selection under
    /// `point`, if any, with that object and its oriented box — the one hit
    /// rule [`SelectTool::pointer_down`] (to start a drag), a hover cursor
    /// query and a double-click all use, so what the cursor promises and
    /// what a press does can never disagree (criterion 9). `shift` is the
    /// live Shift state: it decides whether the side rotate handles exist.
    /// The centre handle is never returned (it is a body press); see
    /// [`SelectTool::hover_handle_at`]. `None` for no selection or a
    /// multi-selection.
    #[must_use]
    pub fn handle_at<'a>(
        objects: &'a [ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        shift: bool,
    ) -> Option<(&'a ObjectSnapshot, OrientedBox, EditHandle)> {
        Self::query_handle(objects, selection, point, tolerances, shift, false)
    }

    /// [`SelectTool::handle_at`] for hover, cursor and hint: also reports the
    /// centre move handle where no other handle is hit.
    #[must_use]
    pub fn hover_handle_at<'a>(
        objects: &'a [ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        shift: bool,
    ) -> Option<(&'a ObjectSnapshot, OrientedBox, EditHandle)> {
        Self::query_handle(objects, selection, point, tolerances, shift, true)
    }

    fn query_handle<'a>(
        objects: &'a [ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
        tolerances: TransformHandleTolerances,
        shift: bool,
        include_move: bool,
    ) -> Option<(&'a ObjectSnapshot, OrientedBox, EditHandle)> {
        let object = sole_selected(objects, selection)?;
        let box_ = oriented_bounds(object);
        let handles = transform_handles(&box_, handle_spec_for(object, shift), &tolerances);
        let hit = hit_transform_handle(&handles, &box_, point, &tolerances, include_move)?;
        Some((object, box_, hit))
    }

    /// Whether `point` lies inside the oriented box of the sole selected
    /// object (edges included). `false` for no selection or several.
    fn is_inside_selected_box(
        objects: &[ObjectSnapshot],
        selection: &ObjectSelection,
        point: Point,
    ) -> bool {
        let Some(object) = sole_selected(objects, selection) else {
            return false;
        };
        let box_ = oriented_bounds(object);
        let local = box_.to_local(point);
        local.x >= box_.min.x
            && local.x <= box_.max.x
            && local.y >= box_.min.y
            && local.y <= box_.max.y
    }

    /// Acceptance criteria 1, 4-18 of slice 5 and 1-11 here: hit-tests
    /// `point` first against the current single-object selection's own
    /// transform handles, then (same as before) against every object's own
    /// body — updates `selection` and starts whichever drag matches. Also
    /// closes an open numeric entry (the press itself is processed as
    /// usual, criterion 20).
    pub fn pointer_down(
        &mut self,
        objects: &[ObjectSnapshot],
        selection: &mut ObjectSelection,
        point: Point,
        tolerance: Tolerance,
        handle_tolerances: TransformHandleTolerances,
        shift: bool,
    ) -> SelectPointerDownOutcome {
        self.entry = None;
        self.last_press_handle = None;
        // `adrs.md`: "ui-core filters the selection against the current
        // snapshot first" — drops any id a prior action (this peer's own
        // edit in a different tool, or a collaborator) has since removed,
        // before this click can act on it.
        selection.retain_existing(objects);
        let origin = DragOrigin::new(point, handle_tolerances.drag_threshold_mm, shift);

        if let Some((object, box_, handle)) =
            Self::handle_at(objects, selection, point, handle_tolerances, shift)
        {
            self.drag = SelectDrag::Transforming(TransformDrag {
                origin,
                start: object.clone(),
                start_box: box_,
                handle,
                stroke_scaling: self.stroke_scaling,
            });
            self.last_press_handle = Some(handle);
            return SelectPointerDownOutcome::Handle;
        }
        let from_center = matches!(
            Self::hover_handle_at(objects, selection, point, handle_tolerances, shift),
            Some((_, _, EditHandle::Move))
        );

        let Some(hit) = hit_test_object(objects, point, tolerance) else {
            // A press inside the sole selected object's own (oriented) box
            // that is not on a handle is a grab of that object — its body
            // (acceptance criterion 23 of slice 5). Without it a small
            // unfilled object could not be moved at all: the handle radii
            // tile its whole outline and its interior hits nothing.
            // Unselected objects still hit only on their outline.
            if !shift && Self::is_inside_selected_box(objects, selection, point) {
                self.drag = SelectDrag::Moving {
                    origin,
                    from_center,
                };
                return SelectPointerDownOutcome::Selected;
            }
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
        self.drag = SelectDrag::Moving {
            origin,
            from_center: false,
        };
        SelectPointerDownOutcome::Selected
    }

    /// Records the pointer's position for the drag in flight, so the 3 px
    /// dead zone, once left, stays left (criterion 41's "a drag"). A no-op
    /// when idle.
    pub fn pointer_moved(&mut self, point: Point) {
        match &mut self.drag {
            SelectDrag::Moving { origin, .. } => origin.note(point),
            SelectDrag::Transforming(drag) => drag.origin.note(point),
            SelectDrag::None => {}
        }
    }

    /// Commits whatever drag is in flight — a move (as one
    /// [`vecmanf_document_core::Document::translate_objects`] call for
    /// the whole selection), a resize, a rotate or a skew (one commit
    /// each) — a no-op (writes nothing) if no drag was in flight, or the
    /// pointer never left the dead zone, or the result equals the start.
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
            SelectDrag::Moving { origin, .. } => {
                if !origin.is_active_at(point) {
                    return;
                }
                let offset = origin.down_at.vector_to(point);
                selection.retain_existing(objects);
                if selection.is_empty() {
                    return;
                }
                let _ = document.translate_objects(selection.ids(), offset);
            }
            SelectDrag::Transforming(drag) => {
                if !drag.origin.is_active_at(point) {
                    return;
                }
                let result = drag.resolve(point, shift, ctrl);
                if result != drag.start {
                    drag.commit(document, &result);
                }
            }
        }
    }

    /// Cancels whichever drag is in flight, writing nothing.
    pub fn escape(&mut self) {
        self.drag = SelectDrag::None;
        self.last_press_handle = None;
    }

    /// Forgets which handle the last press grabbed: the Select tool was left
    /// or re-entered, so a later double-click's first press may never have
    /// reached it.
    pub fn forget_press(&mut self) {
        self.last_press_handle = None;
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
        self.entry = None;
        self.last_press_handle = None;
        selection.retain_existing(objects);
        if selection.is_empty() {
            return;
        }
        if document.delete_objects(selection.ids()).is_ok() {
            selection.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResizeDirection;
    use vecmanf_document_core::Shape;
    use vecmanf_document_core::Vec2;
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
        skew: Tolerance::from_mm(1.0),
        center_hover: Tolerance::from_mm(1.0),
        rotate_offset_mm: 5.0,
        skew_offset_mm: 2.5,
        edge_handle_min_side_mm: 0.0,
        skew_min_side_mm: 0.0,
        // No centre handle and no dead zone in these unit-scale tests;
        // the refinements' own tests set them.
        center_min_side_mm: 1e9,
        drag_threshold_mm: 0.0,
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

    /// Acceptance criterion 1: with exactly one object selected, its
    /// handles appear: 8 resize and the 4 corner rotate handles on a
    /// rectangle (the side rotate handles need Shift, the centre handle a
    /// big enough box, skew a path).
    #[test]
    fn ac1_a_single_selection_shows_twelve_handles() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES, false);
        assert_eq!(handles.len(), 12);
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
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES, false);
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
            .live_transform(Point::new(15.0, 13.0), false, false)
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
        tool.set_stroke_scaling(StrokeScaling::Proportional);
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

    // --- "Scale stroke width" switch (AC 8, 26-31) ---

    /// Presses `wanted` on a fresh single selection of `id`, drags to `to`
    /// and releases; the tool is configured by `configure`.
    fn resize_drag(
        document: &Document,
        id: NodeId,
        wanted: EditHandle,
        to: Point,
        configure: impl FnOnce(&mut SelectTool),
    ) {
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        configure(&mut tool);
        let at = handle_pos(&objects, &selection, wanted);
        tool.pointer_down(
            &objects,
            &mut selection,
            at,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.pointer_up(document, &objects, &mut selection, to, false, false);
    }

    /// AC 8 + 27: a new tool keeps the stroke width, for every resize
    /// handle, on a rectangle, an ellipse, a star and a path (anchors).
    #[test]
    fn ac8_ac27_default_keeps_the_stroke_width_for_every_handle_and_kind() {
        use vecmanf_document_core::{InnerRatio, PointCount};
        assert_eq!(SelectTool::new().stroke_scaling(), StrokeScaling::Keep);
        for direction in ResizeDirection::ALL_EIGHT {
            let document = Document::new(1);
            let rect_id = rect(&document, 0.0);
            let ellipse_id = document.create_ellipse(EllipseFrame {
                center: Point::new(30.0, 5.0),
                rx: Length::from_mm(5.0),
                ry: Length::from_mm(3.0),
            });
            let path_id = document.create_path(
                &[
                    NewAnchor::corner(AnchorId::new(1, 1), Point::new(60.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(1, 2), Point::new(70.0, 8.0)),
                ],
                false,
            );
            for id in [rect_id, ellipse_id, path_id] {
                let before = document.object(id).expect("exists");
                let at = {
                    let objects = vec![before.clone()];
                    let mut selection = ObjectSelection::new();
                    selection.select_single(id);
                    handle_pos(&objects, &selection, EditHandle::Resize(direction))
                };
                resize_drag(
                    &document,
                    id,
                    EditHandle::Resize(direction),
                    at.translated(Vec2::new(6.0, 4.0)),
                    |_| {},
                );
                let after = document.object(id).expect("exists");
                assert_ne!(after, before, "{direction:?}: the resize happened");
                let width = |o: &ObjectSnapshot| match o {
                    ObjectSnapshot::Primitive(p) => p.stroke_width,
                    ObjectSnapshot::Path(p) => p.stroke_width,
                };
                assert_eq!(width(&after), width(&before), "{direction:?} on {id:?}");
            }
        }
        // A star's corner handles (uniform scale) keep it too.
        let document = Document::new(1);
        let star = document.create_star(
            StarFrame {
                center: Point::new(0.0, 0.0),
                radius: Length::from_mm(10.0),
                angle: vecmanf_document_core::Angle::from_radians(0.0),
            },
            PointCount::new(5).unwrap(),
            InnerRatio::new(0.5).unwrap(),
        );
        resize_drag(
            &document,
            star,
            EditHandle::Resize(ResizeDirection::Ne),
            Point::new(20.0, -20.0),
            |_| {},
        );
        assert!((document.primitive(star).unwrap().stroke_width.as_mm() - 0.25).abs() < 1e-12);
    }

    /// AC 26: with the switch on a proportional resize scales the width by
    /// the factor, a single-axis one by √(sx·sy), floored at 0.01 mm.
    #[test]
    fn ac26_proportional_mode_scales_by_the_geometric_mean_with_the_floor() {
        let on = |t: &mut SelectTool| t.set_stroke_scaling(StrokeScaling::Proportional);
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        resize_drag(
            &document,
            id,
            EditHandle::Resize(ResizeDirection::E),
            Point::new(40.0, 5.0),
            on,
        );
        assert!(
            (document.primitive(id).unwrap().stroke_width.as_mm() - 0.5).abs() < 1e-9,
            "0.25 × √4"
        );
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        resize_drag(
            &document,
            id,
            EditHandle::Resize(ResizeDirection::E),
            Point::new(-80.0, 5.0),
            on,
        );
        let w = document.primitive(id).unwrap().stroke_width.as_mm();
        assert!((w - 0.01).abs() < 1e-12, "floored at 0.01 mm, got {w}");
    }

    /// AC 28: a drag uses the mode it was pressed with; a toggle during
    /// the drag changes neither its live preview nor its commit, and
    /// applies to the next drag.
    #[test]
    fn ac28_a_toggle_mid_drag_applies_to_the_next_drag_only() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let se = handle_pos(
            &objects,
            &selection,
            EditHandle::Resize(ResizeDirection::Se),
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            se,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        tool.set_stroke_scaling(StrokeScaling::Proportional); // mid-drag
        let to = Point::new(20.0, 20.0);
        let ObjectSnapshot::Primitive(preview) = tool.live_transform(to, false, false).unwrap()
        else {
            panic!("primitive");
        };
        assert!(
            (preview.stroke_width.as_mm() - 0.25).abs() < 1e-12,
            "preview unchanged"
        );
        tool.pointer_up(&document, &objects, &mut selection, to, false, false);
        assert!((document.primitive(id).unwrap().stroke_width.as_mm() - 0.25).abs() < 1e-12);

        // The next drag picks up the new mode: 2x -> stroke 0.5.
        let objects = vec![document.object(id).expect("exists")];
        let se = handle_pos(
            &objects,
            &selection,
            EditHandle::Resize(ResizeDirection::Se),
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
            Point::new(40.0, 40.0),
            false,
            true,
        );
        assert!((document.primitive(id).unwrap().stroke_width.as_mm() - 0.5).abs() < 1e-9);
    }

    /// AC 29: toggling writes nothing — the saved bytes are identical.
    #[test]
    fn ac29_toggling_the_switch_writes_nothing() {
        let document = Document::new(1);
        let _ = rect(&document, 0.0);
        let before = document.export_loro_snapshot().unwrap();
        let mut tool = SelectTool::new();
        tool.set_stroke_scaling(StrokeScaling::Proportional);
        tool.set_stroke_scaling(StrokeScaling::Keep);
        tool.set_stroke_scaling(StrokeScaling::Proportional);
        assert_eq!(document.export_loro_snapshot().unwrap(), before);
    }

    /// AC 31: the corner radius scales identically in both modes.
    #[test]
    fn ac31_the_switch_does_not_affect_the_corner_radius() {
        let radius_after = |mode: StrokeScaling| {
            let document = Document::new(1);
            let id = rect(&document, 0.0);
            document
                .set_corner_radius(&[id], Length::from_mm(2.0))
                .unwrap();
            resize_drag(
                &document,
                id,
                EditHandle::Resize(ResizeDirection::Se),
                Point::new(20.0, 20.0),
                |t| t.set_stroke_scaling(mode),
            );
            let Shape::Rect { corner_radius, .. } = document.primitive(id).unwrap().shape else {
                panic!("rect");
            };
            corner_radius.as_mm()
        };
        assert!((radius_after(StrokeScaling::Keep) - 4.0).abs() < 1e-9);
        assert!((radius_after(StrokeScaling::Proportional) - 4.0).abs() < 1e-9);
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
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES, false);
        assert_eq!(handles.len(), 8, "4 corners + 4 corner rotate, no edges");
        let (_, ne_position) = handles
            .iter()
            .find(|(h, _)| matches!(h, EditHandle::Resize(ResizeDirection::Ne)))
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
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES, false);
        let (_, rotate_position) = handles
            .iter()
            .find(|(h, _)| matches!(h, EditHandle::Rotate(_)))
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
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES, false);
        let (_, rotate_position) = handles
            .iter()
            .find(|(h, _)| matches!(h, EditHandle::Rotate(_)))
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
        let handles = SelectTool::transform_handles(&objects, &selection, HANDLE_TOLERANCES, false);
        let se = handles
            .iter()
            .find(|(h, _)| matches!(h, EditHandle::Resize(ResizeDirection::Se)))
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
        wanted: EditHandle,
    ) -> Point {
        SelectTool::transform_handles(objects, selection, HANDLE_TOLERANCES, false)
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
            EditHandle::Rotate(ResizeDirection::Ne),
            EditHandle::Resize(ResizeDirection::Se),
            EditHandle::Resize(ResizeDirection::N),
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
        let n = handle_pos(&objects, &selection, EditHandle::Resize(ResizeDirection::N));
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
        tool.set_stroke_scaling(StrokeScaling::Proportional);
        let e = handle_pos(&objects, &selection, EditHandle::Resize(ResizeDirection::E));
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
        tool.set_stroke_scaling(StrokeScaling::Proportional);
        let e = handle_pos(&objects, &selection, EditHandle::Resize(ResizeDirection::E));
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
            EditHandle::Resize(ResizeDirection::Se),
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

    /// Criteria 12, 13: no modifier rotates about the box center; Shift
    /// pivots a corner rotate on the opposite corner, so the object's
    /// center swings around it.
    #[test]
    fn ac13_shift_rotates_a_corner_handle_about_the_opposite_corner() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        selection.select_single(id);
        let mut tool = SelectTool::new();
        let r = handle_pos(
            &objects,
            &selection,
            EditHandle::Rotate(ResizeDirection::Ne),
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            r,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        assert_eq!(tool.live_pivot(true), Some(Point::new(0.0, 10.0)));
        assert_eq!(tool.live_pivot(false), Some(Point::new(5.0, 5.0)));
        // The Ne rotate handle is (13.54, -3.54): from the Sw corner
        // (0, 10) the vector is (13.54, -13.54); a clockwise quarter turn
        // (+90 degrees in Y-down) gives (13.54, 13.54).
        let offset = 5.0 / std::f64::consts::SQRT_2;
        let current = Point::new(10.0 + offset, 10.0 + 10.0 + offset);
        tool.pointer_up(&document, &objects, &mut selection, current, true, false);
        let snapshot = document.primitive(id).expect("exists");
        assert!((snapshot.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        let Shape::Rect { bounds, .. } = snapshot.shape else {
            panic!("expected rect");
        };
        // The old center (5, 5) is (5, -5) from the pivot; a clockwise
        // quarter turn about (0, 10) puts it at (5, 15).
        let center = Point::new(
            bounds.origin.x + bounds.width.as_mm() / 2.0,
            bounds.origin.y + bounds.height.as_mm() / 2.0,
        );
        assert!(
            (center.x - 5.0).abs() < 1e-9 && (center.y - 15.0).abs() < 1e-9,
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
            let r = handle_pos(
                &objects,
                &selection,
                EditHandle::Rotate(ResizeDirection::Ne),
            );
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
            ..HANDLE_TOLERANCES
        };
        let handle_at = |point: Point| {
            SelectTool::handle_at(&objects, &selection, point, wide, false).map(|(_, _, h)| h)
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
            Some(EditHandle::Resize(ResizeDirection::Se))
        );
        assert_eq!(
            handle_at(Point::new(11.0, 6.5)),
            Some(EditHandle::Resize(ResizeDirection::Se))
        );
        // And just inside a handle, within the thin inner band, too.
        assert_eq!(
            handle_at(Point::new(9.5, 5.0)),
            Some(EditHandle::Resize(ResizeDirection::Se))
        );
    }

    /// A selected unfilled object of any small size can be moved by
    /// pressing inside its box (its body, acceptance criterion 23): the
    /// handle radii tile a small box's outline, so without this nothing
    /// could move it. Squares of 10, 22, 40 and 60 units with 16-unit
    /// radii, unrotated and rotated: a press at the centre starts a move
    /// that changes neither size nor rotation; the rotate handle and a
    /// resize handle still win where they are; an empty-canvas press
    /// still deselects.
    #[test]
    fn a_selected_small_square_moves_from_inside_its_box_rotated_or_not() {
        use vecmanf_document_core::Angle;
        let wide = TransformHandleTolerances {
            resize: Tolerance::from_mm(16.0),
            rotate: Tolerance::from_mm(16.0),
            rotate_offset_mm: 32.0,
            ..HANDLE_TOLERANCES
        };
        for (size, height) in [
            (10.0, 10.0),
            (22.0, 22.0),
            (40.0, 40.0),
            (60.0, 60.0),
            (40.0, 22.0),
        ] {
            for turn in [0.0, 0.6] {
                let document = Document::new(1);
                let id = document.create_rect(RectBounds {
                    origin: Point::new(0.0, 0.0),
                    width: Length::from_mm(size),
                    height: Length::from_mm(height),
                });
                let centre = Point::new(size / 2.0, height / 2.0);
                if turn != 0.0 {
                    document
                        .rotate_object(
                            &document
                                .object(id)
                                .expect("exists")
                                .rotated(centre, Angle::from_radians(turn)),
                        )
                        .expect("rotate");
                }
                let objects = vec![document.object(id).expect("exists")];
                let mut selection = ObjectSelection::new();
                selection.select_single(id);

                // Inside press: a move, not a handle drag.
                let mut tool = SelectTool::new();
                let outcome =
                    tool.pointer_down(&objects, &mut selection, centre, TOLERANCE, wide, false);
                assert_eq!(outcome, SelectPointerDownOutcome::Selected, "{size} {turn}");
                assert!(tool.dragging_handle().is_none(), "{size} {turn}");
                assert_eq!(selection.ids(), &[id], "still selected");
                tool.pointer_up(
                    &document,
                    &objects,
                    &mut selection,
                    centre.translated(Vec2::new(7.0, 3.0)),
                    false,
                    false,
                );
                let moved = document.primitive(id).expect("exists");
                assert!((moved.rotation.as_radians() - turn).abs() < 1e-9);
                let Shape::Rect { bounds, .. } = moved.shape else {
                    panic!("rect");
                };
                assert!((bounds.width.as_mm() - size).abs() < 1e-9, "size kept");
                assert!((bounds.origin.x - 7.0).abs() < 1e-9, "{size} {turn}: moved");
                assert!((bounds.origin.y - 3.0).abs() < 1e-9);

                // The handles still win where they are.
                let objects = vec![document.object(id).expect("exists")];
                let at = |wanted: EditHandle| {
                    SelectTool::transform_handles(&objects, &selection, wide, false)
                        .into_iter()
                        .find(|(h, _)| *h == wanted)
                        .expect("handle exists")
                        .1
                };
                let rotate = at(EditHandle::Rotate(ResizeDirection::Ne));
                let se = at(EditHandle::Resize(ResizeDirection::Se));
                let mut tool = SelectTool::new();
                assert_eq!(
                    tool.pointer_down(&objects, &mut selection, rotate, TOLERANCE, wide, false),
                    SelectPointerDownOutcome::Handle
                );
                tool.escape();
                assert_eq!(
                    tool.pointer_down(&objects, &mut selection, se, TOLERANCE, wide, false),
                    SelectPointerDownOutcome::Handle
                );
                tool.escape();

                // Far outside the box: deselects.
                assert_eq!(
                    tool.pointer_down(
                        &objects,
                        &mut selection,
                        Point::new(900.0, 900.0),
                        TOLERANCE,
                        wide,
                        false
                    ),
                    SelectPointerDownOutcome::Cleared
                );
                assert!(selection.is_empty());
            }
        }
    }

    /// An *unselected* object still hits only on its outline: its centre
    /// is empty canvas.
    #[test]
    fn an_unselected_objects_centre_is_still_empty_canvas() {
        let document = Document::new(1);
        let id = rect(&document, 0.0);
        let objects = vec![document.object(id).expect("exists")];
        let mut selection = ObjectSelection::new();
        let mut tool = SelectTool::new();
        assert_eq!(
            tool.pointer_down(
                &objects,
                &mut selection,
                Point::new(5.0, 5.0),
                TOLERANCE,
                HANDLE_TOLERANCES,
                false
            ),
            SelectPointerDownOutcome::Cleared
        );
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
            EditHandle::Resize(ResizeDirection::Ne),
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
                EditHandle::Rotate(ResizeDirection::Ne),
                EditHandle::Resize(ResizeDirection::Se),
                EditHandle::Resize(ResizeDirection::E),
            ] {
                if matches!(wanted, EditHandle::Rotate(_)) && !include_rotate {
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
        let r = handle_pos(
            &objects,
            &selection,
            EditHandle::Rotate(ResizeDirection::Ne),
        );
        tool.pointer_down(
            &objects,
            &mut selection,
            r,
            TOLERANCE,
            HANDLE_TOLERANCES,
            false,
        );
        let to = Point::new(-3.0, 4.0);
        let preview = tool.live_transform(to, true, false).expect("rotating");
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
