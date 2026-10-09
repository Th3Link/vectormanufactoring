//! The Close path command (`specs/0034-pen-path-extension` criteria 18 to 21): which open paths it
//! closes, and the one [`PathGrowth`] per path that [`curvyo_document_core::Document::close_paths`]
//! writes in a single commit. The Node bar's buttons use [`closable_counts`] for their enablement
//! and tooltips and [`plan_close_paths`] for the press, so a button and the command cannot
//! disagree.

use curvyo_document_core::{
    AnchorSnapshot, COINCIDENT_MM, HandleSlot, PathEnd, PathGrowth, PathSnapshot, merged_junction,
};

use crate::closing_join::{JoinType, resolve_closing_node};

/// What the command does to a set of paths.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosePlan {
    /// One growth per path that gets closed, for `Document::close_paths`.
    pub growths: Vec<PathGrowth>,
    /// How many paths are closed.
    pub closed: usize,
    /// How many open ordinary paths are skipped for having fewer than three nodes.
    pub skipped: usize,
}

/// Whether a path is an open ordinary path (a candidate): not closed, not compound.
fn is_open_ordinary(path: &PathSnapshot) -> bool {
    !path.closed && !path.is_compound() && !path.anchors.is_empty()
}

/// Whether the ends of `path` coincide and are merged into one node before closing (`0006`
/// criterion 10), which leaves one node fewer.
fn merges_ends(path: &PathSnapshot) -> bool {
    path.anchors.len() >= 3
        && path.anchors[0]
            .point
            .vector_to(path.anchors[path.anchors.len() - 1].point)
            .length()
            < COINCIDENT_MM
}

/// How many nodes `path` has once closed: the merge of coincident ends counts.
fn nodes_when_closed(path: &PathSnapshot) -> usize {
    path.anchors.len() - usize::from(merges_ends(path))
}

/// `(closable, skipped)` among `paths`: open ordinary paths that would be closed, and those
/// skipped for having fewer than three nodes. Closed paths, compound paths and anything else are
/// not counted (criterion 21).
#[must_use]
pub fn closable_counts(paths: &[&PathSnapshot]) -> (usize, usize) {
    let mut closable = 0;
    let mut skipped = 0;
    for path in paths.iter().filter(|path| is_open_ordinary(path)) {
        if nodes_when_closed(path) >= 3 {
            closable += 1;
        } else {
            skipped += 1;
        }
    }
    (closable, skipped)
}

/// Closes every open ordinary path of `paths` with three or more nodes: one segment from its last
/// node to its first, the first node being the closing node with `join` applied by
/// [`resolve_closing_node`]. A path whose first and last node coincide has them merged first.
#[must_use]
pub fn plan_close_paths(paths: &[&PathSnapshot], join: JoinType) -> ClosePlan {
    let (closed, skipped) = closable_counts(paths);
    let mut growths = Vec::with_capacity(closed);
    for path in paths.iter().filter(|path| is_open_ordinary(path)) {
        if nodes_when_closed(path) < 3 {
            continue;
        }
        let len = path.anchors.len();
        let first = &path.anchors[0];
        let (closing, drop, before): (AnchorSnapshot, Vec<_>, _) = if merges_ends(path) {
            let last = &path.anchors[len - 1];
            (
                merged_junction(first, false, last, true),
                vec![last.id],
                path.anchors[len - 2].point,
            )
        } else {
            (*first, Vec::new(), path.anchors[len - 1].point)
        };
        let after = path.anchors[1].point;
        let resolved = resolve_closing_node(&closing, before, after, HandleSlot::In, join);
        growths.push(PathGrowth {
            path: path.id,
            end: PathEnd::Last,
            added: Vec::new(),
            absorb: None,
            replace: vec![resolved],
            drop,
        });
    }
    ClosePlan {
        growths,
        closed,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, AnchorKind, Document, NewAnchor, Point};

    use super::*;

    fn open(document: &Document, base: u64, points: &[(f64, f64)], closed: bool) -> PathSnapshot {
        let anchors: Vec<NewAnchor> = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                NewAnchor::corner(AnchorId::new(1, base + i as u64), Point::new(x, y))
            })
            .collect();
        let id = document.create_path(&anchors, closed);
        document.path(id).unwrap()
    }

    /// Criteria 19 and 21: open ordinary paths of three nodes or more close, shorter ones are
    /// counted as skipped, closed paths are not counted at all.
    #[test]
    fn the_plan_closes_the_long_open_paths_and_counts_the_short_ones() {
        let document = Document::new(1);
        let a = open(
            &document,
            1,
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
            false,
        );
        let b = open(&document, 10, &[(0.0, 0.0), (10.0, 0.0)], false);
        let c = open(
            &document,
            20,
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)],
            true,
        );
        let plan = plan_close_paths(&[&a, &b, &c], JoinType::Sharp);
        assert_eq!(plan.closed, 1);
        assert_eq!(plan.skipped, 1);
        assert_eq!(plan.growths.len(), 1);
        assert_eq!(plan.growths[0].path, a.id);
        assert_eq!(closable_counts(&[&b, &c]), (0, 1));
    }

    /// Criterion 19: Smooth makes the first node Asymmetric along the tangent from the last node to
    /// the second.
    #[test]
    fn smooth_makes_the_first_node_asymmetric() {
        let document = Document::new(1);
        let a = open(
            &document,
            1,
            &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
            false,
        );
        let plan = plan_close_paths(&[&a], JoinType::Smooth);
        assert_eq!(plan.growths[0].replace[0].kind, AnchorKind::Asymmetric);
        let sharp = plan_close_paths(&[&a], JoinType::Sharp);
        assert_eq!(sharp.growths[0].replace[0].kind, AnchorKind::Corner);
    }

    /// Criterion 19: coincident first and last nodes are merged into one first.
    #[test]
    fn coincident_ends_are_merged_before_closing() {
        let document = Document::new(1);
        let a = open(
            &document,
            1,
            &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 0.0)],
            false,
        );
        let plan = plan_close_paths(&[&a], JoinType::Sharp);
        assert_eq!(plan.closed, 1);
        assert_eq!(plan.growths[0].drop, vec![a.anchors[3].id]);
        // Three distinct nodes remain: a triangle.
        document.close_paths(&plan.growths).unwrap();
        let closed = document.path(a.id).unwrap();
        assert!(closed.closed);
        assert_eq!(closed.anchors.len(), 3);
        // Fewer than three distinct nodes after the merge: skipped.
        let b = open(&document, 10, &[(0.0, 0.0), (20.0, 0.0), (0.0, 0.0)], false);
        assert_eq!(closable_counts(&[&b]), (0, 1));
    }
}
