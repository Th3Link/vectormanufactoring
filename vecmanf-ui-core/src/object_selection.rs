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
//! `vecmanf_document_core::AnchorIdMinter` is shared across
//! `PenTool`/`NodeTool` rather than duplicated.
//!
//! Ephemeral, per ADR 0009 §2 — never written to the document, never
//! saved. Holds [`NodeId`]s and resolves them lazily against whatever
//! snapshot list the caller hands in, the same stance
//! [`crate::NodeSelection`] already takes for anchors. Each id's kind
//! (path or which primitive) comes from `Document::object(id)` at the
//! point of use — no stored kind here.

use vecmanf_document_core::NodeId;

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

    /// Clears the selection.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Drops every id in `converted` — called once "object to path" has
    /// replaced them, since a primitive selection has nothing left to
    /// name once its object is a path (`specification.md`'s "handles
    /// don't coexist").
    pub fn remove_all(&mut self, converted: &[NodeId]) {
        self.ids.retain(|id| !converted.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::Document;

    fn two_ids() -> (NodeId, NodeId) {
        let document = Document::new(1);
        let a = document.create_rect(vecmanf_document_core::RectBounds {
            origin: vecmanf_document_core::Point::new(0.0, 0.0),
            width: vecmanf_document_core::Length::from_mm(1.0),
            height: vecmanf_document_core::Length::from_mm(1.0),
        });
        let b = document.create_rect(vecmanf_document_core::RectBounds {
            origin: vecmanf_document_core::Point::new(5.0, 5.0),
            width: vecmanf_document_core::Length::from_mm(1.0),
            height: vecmanf_document_core::Length::from_mm(1.0),
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

    #[test]
    fn remove_all_drops_only_the_named_ids() {
        let (a, b) = two_ids();
        let mut selection = ObjectSelection::new();
        selection.toggle(a);
        selection.toggle(b);
        selection.remove_all(&[a]);
        assert_eq!(selection.ids(), &[b]);
    }
}
