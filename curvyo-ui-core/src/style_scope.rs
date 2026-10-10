//! Which objects the Style panel edits for the active tool, and the line that
//! says so (`specs/0007-stroke-and-fill-styling` criterion 37).

use curvyo_document_core::{NodeId, ObjectSnapshot, PrimitiveSnapshot, Shape, StyleEdit};

use crate::object_selection::{ObjectSelection, objects_with_ids};
use crate::selection::NodeSelection;

/// The tool the panel is scoped by (criterion 37). The creation tools and
/// Select all edit the object selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleTool {
    /// The pen: the path being drawn does not exist yet; nothing to edit.
    Pen,
    /// The node tool: the paths that own the selected nodes.
    Node,
    /// Select and the rectangle, ellipse and polygon/star tools.
    Other,
}

/// The objects the panel edits and the line that says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleScope {
    /// The objects an edit goes to; empty when the panel has nothing to edit.
    pub ids: Vec<NodeId>,
    /// A kind ("Rectangle") or a count ("3 rectangles", "4 objects", "2
    /// paths"); empty when there is nothing to edit.
    pub subject: String,
}

impl StyleScope {
    /// The objects `edit` goes to: all of the scope, or for a marker edit only
    /// its paths, since a primitive has no markers (`specs/0018-stroke-markers`
    /// criterion 22).
    #[must_use]
    pub fn targets(&self, objects: &[ObjectSnapshot], edit: &StyleEdit) -> Vec<NodeId> {
        if edit.is_marker_edit() {
            paths_among(&self.ids, objects)
        } else {
            self.ids.clone()
        }
    }
}

/// The ids in `ids` that are paths, in order.
fn paths_among(ids: &[NodeId], objects: &[ObjectSnapshot]) -> Vec<NodeId> {
    objects_with_ids(objects, ids)
        .into_iter()
        .filter(|object| matches!(object, ObjectSnapshot::Path(_)))
        .map(ObjectSnapshot::id)
        .collect()
}

/// The scope of the panel for `tool` (criterion 37). Ids the document no
/// longer holds are dropped.
#[must_use]
pub fn style_scope(
    tool: StyleTool,
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    nodes: &NodeSelection,
) -> StyleScope {
    if tool == StyleTool::Pen {
        return StyleScope {
            ids: Vec::new(),
            subject: String::new(),
        };
    }
    let wanted: Vec<NodeId> = if tool == StyleTool::Node {
        let owners = node_owners(nodes);
        if owners.is_empty() {
            // The path being edited: the paths of the object selection.
            paths_among(selection.ids(), objects)
        } else {
            owners
        }
    } else {
        selection.ids().to_vec()
    };
    let held = objects_with_ids(objects, &wanted);
    StyleScope {
        ids: held.iter().map(|object| object.id()).collect(),
        subject: subject_line(&held),
    }
}

/// The paths owning the selected nodes or the selected segment, in selection
/// order, once each.
fn node_owners(nodes: &NodeSelection) -> Vec<NodeId> {
    let mut owners: Vec<NodeId> = Vec::new();
    let paths = nodes.node_pairs().iter().map(|&(path, _)| path);
    let segment = nodes.segment_with_path().map(|(path, _, _)| path);
    for path in paths.chain(segment) {
        if !owners.contains(&path) {
            owners.push(path);
        }
    }
    owners
}

fn kind_name(object: &ObjectSnapshot) -> (&'static str, &'static str) {
    match object {
        ObjectSnapshot::Path(path) if path.is_compound() => ("Compound path", "compound paths"),
        ObjectSnapshot::Path(_) => ("Path", "paths"),
        ObjectSnapshot::Primitive(PrimitiveSnapshot { shape, .. }) => match shape {
            Shape::Rect { .. } => ("Rectangle", "rectangles"),
            Shape::Ellipse { .. } => ("Ellipse", "ellipses"),
            Shape::Polygon { .. } => ("Polygon", "polygons"),
            Shape::Star { .. } => ("Star", "stars"),
        },
    }
}

fn subject_line(objects: &[&ObjectSnapshot]) -> String {
    let Some(first) = objects.first() else {
        return String::new();
    };
    let (singular, plural) = kind_name(first);
    let same_kind = objects.iter().all(|object| kind_name(object).0 == singular);
    // Compound paths are paths: a mix of them and ordinary paths is "N paths".
    let all_paths = objects
        .iter()
        .all(|object| matches!(object, ObjectSnapshot::Path(_)));
    match (objects.len(), same_kind) {
        (1, _) => singular.to_string(),
        (count, true) => format!("{count} {plural}"),
        (count, false) if all_paths => format!("{count} paths"),
        (count, false) => format!("{count} objects"),
    }
}
