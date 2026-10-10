//! The selection the Select tool and the three shape tools
//! (`RectangleTool`, `EllipseTool`, `PolygonStarTool`) all read and write
//! — any object, any kind, one id space
//! (`specs/0004-canvas-navigation-and-selection/adrs.md`: "one object
//! selection, shared by the Select tool and the shape tools... renamed to
//! `ObjectSelection`, and its doc comment says 'any object'"). Originally
//! `PrimitiveSelection` (`specs/0003-primitive-shapes/specification.md`'s
//! "Selection and hover convention for primitives": "the whole object is
//! the selection unit") — the type already held plain [`NodeId`]s and knew
//! nothing primitive-specific, so this slice's rename is the only change.
//! One type rather than one per tool/mode, so selecting a rectangle then
//! shift-selecting a path (or an ellipse) stays selected together
//! (acceptance criteria 17, 22) — the same reasoning
//! `curvyo_document_core::AnchorIdMinter` is shared across
//! `PenTool`/`NodeTool` rather than duplicated.
//!
//! Ephemeral, per ADR 0009 §2 — never written to the document, never
//! saved. Holds [`NodeId`]s and resolves them lazily against whatever
//! snapshot list the caller hands in, the same stance
//! [`crate::NodeSelection`] already takes for anchors. Each id's kind
//! (path or which primitive) comes from `Document::object(id)` at the
//! point of use — no stored kind here.

use std::collections::HashMap;

use curvyo_document_core::{NodeId, ObjectSnapshot};

/// How the result of a marquee or lasso combines with the current selection
/// (`specs/0014-advanced-selection/specification.md`, "Modifier scheme"): Shift
/// adds, Ctrl strictly removes, neither replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionCombine {
    /// The selection becomes exactly the result.
    Replace,
    /// The result joins the selection; nothing is deselected.
    Add,
    /// The result leaves the selection; an object of the result that was not
    /// selected is ignored (never a toggle).
    Remove,
}

impl SelectionCombine {
    /// The combine mode of the Shift and Ctrl state: Ctrl wins when both are
    /// down (the key pressed last in the realistic sequence of a held Shift
    /// and a Ctrl added for one drag), else Shift adds, else replace.
    #[must_use]
    pub const fn from_modifiers(shift: bool, ctrl: bool) -> Self {
        if ctrl {
            Self::Remove
        } else if shift {
            Self::Add
        } else {
            Self::Replace
        }
    }
}

/// The current set of selected objects, any kind (acceptance criteria 17,
/// 22: two or more, possibly of different kinds, can be selected
/// together). Order is selection order, not z-order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectSelection {
    ids: Vec<NodeId>,
}

impl ObjectSelection {
    /// No selection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The currently selected primitives, in selection order.
    #[must_use]
    pub fn ids(&self) -> &[NodeId] {
        &self.ids
    }

    /// Whether `id` is currently selected.
    #[must_use]
    pub fn contains(&self, id: NodeId) -> bool {
        self.ids.contains(&id)
    }

    /// Whether nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Selects exactly `id`, replacing whatever was selected before.
    pub fn select_single(&mut self, id: NodeId) {
        self.ids.clear();
        self.ids.push(id);
    }

    /// Adds `id` to the selection, keeping every other id: a no-op when it is
    /// already selected.
    pub fn add(&mut self, id: NodeId) {
        if !self.ids.contains(&id) {
            self.ids.push(id);
        }
    }

    /// Selects exactly `ids`, in the order given, replacing whatever was
    /// selected before (the copies a copy commit just made).
    pub fn set(&mut self, ids: &[NodeId]) {
        self.ids.clear();
        self.ids.extend_from_slice(ids);
    }

    /// Shift-click: toggles `id`'s membership in the selection, keeping
    /// every other currently-selected id untouched — including ones from
    /// a different shape kind (acceptance criterion 22).
    pub fn toggle(&mut self, id: NodeId) {
        if let Some(position) = self.ids.iter().position(|&existing| existing == id) {
            self.ids.remove(position);
        } else {
            self.ids.push(id);
        }
    }

    /// Combines a gesture's `result` with the selection (`result` in the
    /// order the gesture found it): [`SelectionCombine::Replace`] selects
    /// exactly `result`, [`SelectionCombine::Add`] appends every id not yet
    /// selected and keeps the order, [`SelectionCombine::Remove`] deselects
    /// every id of `result` and leaves the rest as it was. An empty `result`
    /// therefore clears on Replace and changes nothing otherwise.
    pub fn apply(&mut self, combine: SelectionCombine, result: &[NodeId]) {
        match combine {
            SelectionCombine::Replace => self.set(result),
            SelectionCombine::Add => {
                for &id in result {
                    self.add(id);
                }
            }
            SelectionCombine::Remove => self.ids.retain(|id| !result.contains(id)),
        }
    }

    /// Clears the selection.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Drops every id that no longer names a live object in `objects`
    /// (ADR 0009 §2: a selection resolves lazily, dropping ids a
    /// collaborator — or this same peer's own earlier action, e.g.
    /// deleting a path's anchors down to nothing with the Node tool —
    /// has since removed). `adrs.md`: "`ui-core` filters the selection
    /// against the current snapshot first", so a single stale id can
    /// never refuse an otherwise-valid `translate_objects`/
    /// `delete_objects` batch for every *other* still-live id alongside
    /// it.
    pub fn retain_existing(&mut self, objects: &[ObjectSnapshot]) {
        self.ids
            .retain(|&id| objects.iter().any(|object| object.id() == id));
    }
}

/// Up to this many ids are looked up with a scan of `objects`; more are looked up in an index.
const SCAN_MAX_IDS: usize = 8;

/// The objects named by `ids`, in the order of `ids`; an id no object holds is dropped, an id
/// listed twice is found twice, and with two objects of one id the first wins. One pass over
/// `objects` however many ids there are: a panel read with thousands selected must not scan all
/// objects once per id.
pub(crate) fn objects_with_ids<'a>(
    objects: &'a [ObjectSnapshot],
    ids: &[NodeId],
) -> Vec<&'a ObjectSnapshot> {
    if ids.len() <= SCAN_MAX_IDS {
        return ids
            .iter()
            .filter_map(|id| objects.iter().find(|object| object.id() == *id))
            .collect();
    }
    let mut index: HashMap<NodeId, &ObjectSnapshot> = HashMap::with_capacity(objects.len());
    for object in objects {
        index.entry(object.id()).or_insert(object);
    }
    ids.iter().filter_map(|id| index.get(id).copied()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::Document;

    fn two_ids() -> (NodeId, NodeId) {
        let document = Document::new(1);
        let a = document.create_rect(curvyo_document_core::RectBounds {
            origin: curvyo_document_core::Point::new(0.0, 0.0),
            width: curvyo_document_core::Length::from_mm(1.0),
            height: curvyo_document_core::Length::from_mm(1.0),
        });
        let b = document.create_rect(curvyo_document_core::RectBounds {
            origin: curvyo_document_core::Point::new(5.0, 5.0),
            width: curvyo_document_core::Length::from_mm(1.0),
            height: curvyo_document_core::Length::from_mm(1.0),
        });
        (a, b)
    }

    #[test]
    fn select_single_replaces_prior_selection() {
        let (a, b) = two_ids();
        let mut selection = ObjectSelection::new();
        selection.select_single(a);
        selection.select_single(b);
        assert_eq!(selection.ids(), &[b]);
    }

    #[test]
    fn toggle_builds_then_shrinks_a_multi_selection() {
        let (a, b) = two_ids();
        let mut selection = ObjectSelection::new();
        selection.toggle(a);
        selection.toggle(b);
        assert_eq!(selection.ids(), &[a, b]);
        selection.toggle(a);
        assert_eq!(selection.ids(), &[b]);
    }

    fn three_ids() -> (NodeId, NodeId, NodeId) {
        let document = Document::new(1);
        let make = |x: f64| {
            document.create_rect(curvyo_document_core::RectBounds {
                origin: curvyo_document_core::Point::new(x, 0.0),
                width: curvyo_document_core::Length::from_mm(1.0),
                height: curvyo_document_core::Length::from_mm(1.0),
            })
        };
        (make(0.0), make(5.0), make(10.0))
    }

    /// Ctrl wins over Shift (the realistic sequence: Shift stays down, Ctrl is
    /// added for one drag), Shift alone adds, nothing replaces.
    #[test]
    fn the_combine_mode_follows_the_modifiers() {
        assert_eq!(
            SelectionCombine::from_modifiers(false, false),
            SelectionCombine::Replace
        );
        assert_eq!(
            SelectionCombine::from_modifiers(true, false),
            SelectionCombine::Add
        );
        assert_eq!(
            SelectionCombine::from_modifiers(false, true),
            SelectionCombine::Remove
        );
        assert_eq!(
            SelectionCombine::from_modifiers(true, true),
            SelectionCombine::Remove
        );
    }

    #[test]
    fn replace_selects_exactly_the_result_in_its_order() {
        let (a, b, c) = three_ids();
        let mut selection = ObjectSelection::new();
        selection.select_single(a);
        selection.apply(SelectionCombine::Replace, &[c, b]);
        assert_eq!(selection.ids(), &[c, b]);
    }

    #[test]
    fn replace_with_an_empty_result_clears() {
        let (a, ..) = three_ids();
        let mut selection = ObjectSelection::new();
        selection.select_single(a);
        selection.apply(SelectionCombine::Replace, &[]);
        assert!(selection.is_empty());
    }

    #[test]
    fn add_appends_only_new_ids_and_keeps_the_order() {
        let (a, b, c) = three_ids();
        let mut selection = ObjectSelection::new();
        selection.set(&[b, a]);
        selection.apply(SelectionCombine::Add, &[a, c]);
        assert_eq!(selection.ids(), &[b, a, c]);
    }

    /// Remove is a set difference, never a toggle: `c` was not selected and
    /// stays unselected.
    #[test]
    fn remove_deselects_the_result_and_never_adds() {
        let (a, b, c) = three_ids();
        let mut selection = ObjectSelection::new();
        selection.set(&[a, b]);
        selection.apply(SelectionCombine::Remove, &[b, c]);
        assert_eq!(selection.ids(), &[a]);
    }

    #[test]
    fn add_and_remove_with_an_empty_result_change_nothing() {
        let (a, b, _) = three_ids();
        let mut selection = ObjectSelection::new();
        selection.set(&[a, b]);
        selection.apply(SelectionCombine::Add, &[]);
        selection.apply(SelectionCombine::Remove, &[]);
        assert_eq!(selection.ids(), &[a, b]);
    }

    #[test]
    fn retain_existing_drops_only_ids_no_longer_in_the_object_list() {
        let document = Document::new(1);
        let a = document.create_rect(curvyo_document_core::RectBounds {
            origin: curvyo_document_core::Point::new(0.0, 0.0),
            width: curvyo_document_core::Length::from_mm(1.0),
            height: curvyo_document_core::Length::from_mm(1.0),
        });
        let b = document.create_rect(curvyo_document_core::RectBounds {
            origin: curvyo_document_core::Point::new(5.0, 5.0),
            width: curvyo_document_core::Length::from_mm(1.0),
            height: curvyo_document_core::Length::from_mm(1.0),
        });
        let mut selection = ObjectSelection::new();
        selection.toggle(a);
        selection.toggle(b);

        // `a` deleted; only `b` remains in the current object list.
        document.delete_objects(&[a]).expect("delete a");
        let objects: Vec<ObjectSnapshot> = document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.object(id))
            .collect();

        selection.retain_existing(&objects);
        assert_eq!(
            selection.ids(),
            &[b],
            "the stale id must be dropped, the live one kept"
        );
    }

    fn ten_rects() -> (Vec<ObjectSnapshot>, Vec<NodeId>) {
        let document = Document::new(1);
        let ids: Vec<NodeId> = (0..10_u32)
            .map(|i| {
                document.create_rect(curvyo_document_core::RectBounds {
                    origin: curvyo_document_core::Point::new(f64::from(i) * 20.0, 0.0),
                    width: curvyo_document_core::Length::from_mm(1.0),
                    height: curvyo_document_core::Length::from_mm(1.0),
                })
            })
            .collect();
        let objects = ids.iter().filter_map(|id| document.object(*id)).collect();
        (objects, ids)
    }

    /// The scan (few ids) and the index (many ids) give the same objects in the same order: the
    /// order of the ids, a stale id dropped, an id listed twice found twice.
    #[test]
    fn objects_with_ids_keeps_the_order_of_the_ids_with_few_ids_and_with_many() {
        let (objects, ids) = ten_rects();
        let stale = Document::new(2).create_rect(curvyo_document_core::RectBounds {
            origin: curvyo_document_core::Point::new(0.0, 0.0),
            width: curvyo_document_core::Length::from_mm(1.0),
            height: curvyo_document_core::Length::from_mm(1.0),
        });
        let few = [ids[3], stale, ids[1], ids[3]];
        let many: Vec<NodeId> = ids.iter().rev().copied().chain([stale, ids[0]]).collect();
        assert!(few.len() <= SCAN_MAX_IDS && many.len() > SCAN_MAX_IDS);
        let found = |ids: &[NodeId]| -> Vec<NodeId> {
            objects_with_ids(&objects, ids)
                .into_iter()
                .map(ObjectSnapshot::id)
                .collect()
        };
        assert_eq!(found(&few), vec![ids[3], ids[1], ids[3]]);
        let expected: Vec<NodeId> = ids.iter().rev().copied().chain([ids[0]]).collect();
        assert_eq!(found(&many), expected);
    }
}
