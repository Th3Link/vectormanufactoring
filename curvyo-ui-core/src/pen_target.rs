//! What a Pen press at a point would do, as one value (`specs/0034-pen-path-extension`): the hover
//! cue (ring, cursor, chip) and the press are both derived from [`pen_target`], so the cursor, the
//! node and the chip always agree with the click (criterion 22).
//!
//! The ends of open paths are found through an [`EndNodeIndex`], a flat list built from the
//! document's paths and rebuilt only when the document changes (criterion 24): a query is a linear
//! scan over the ends, not a read of the document.

use curvyo_document_core::{AnchorSnapshot, Length, NodeId, PathEnd, PathSnapshot, Point};

use crate::closing_join::JoinType;
use crate::pen_tool::{Continuation, PenTool};

/// One end node of an open ordinary path.
#[derive(Debug, Clone, PartialEq)]
pub struct EndNode {
    /// The path the end belongs to.
    pub path: NodeId,
    /// Which end of it.
    pub end: PathEnd,
    /// The end node as stored.
    pub anchor: AnchorSnapshot,
    /// The path's place in the stacking order (higher is on top).
    pub z: usize,
}

/// The end nodes of every open ordinary path (closed paths, compound paths and paths of fewer than
/// two nodes have none).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EndNodeIndex {
    ends: Vec<EndNode>,
}

impl EndNodeIndex {
    /// The index of `paths`, given in stacking order, bottom first.
    #[must_use]
    pub fn from_paths(paths: &[PathSnapshot]) -> Self {
        let mut ends = Vec::new();
        for (z, path) in paths.iter().enumerate() {
            if path.closed || path.is_compound() || path.anchors.len() < 2 {
                continue;
            }
            ends.push(EndNode {
                path: path.id,
                end: PathEnd::First,
                anchor: path.anchors[0],
                z,
            });
            ends.push(EndNode {
                path: path.id,
                end: PathEnd::Last,
                anchor: path.anchors[path.anchors.len() - 1],
                z,
            });
        }
        Self { ends }
    }

    /// The end node nearest to `point` within `radius`, not of `exclude`'s path. A tie goes to
    /// the topmost object, then to the last node (`0034` criteria 7 and 12).
    #[must_use]
    pub fn nearest(
        &self,
        point: Point,
        radius: Length,
        exclude: Option<NodeId>,
    ) -> Option<&EndNode> {
        let mut best: Option<(f64, &EndNode)> = None;
        for end in &self.ends {
            if Some(end.path) == exclude {
                continue;
            }
            let distance = end.anchor.point.vector_to(point).length();
            if distance > radius.as_mm() {
                continue;
            }
            let better = best.is_none_or(|(best_distance, held)| {
                distance < best_distance
                    || ((distance - best_distance).abs() < 1e-9
                        && (end.z, end.end == PathEnd::Last) > (held.z, held.end == PathEnd::Last))
            });
            if better {
                best = Some((distance, end));
            }
        }
        best.map(|(_, end)| end)
    }

    /// Whether the index holds no end at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }
}

/// What a press at a point does.
#[derive(Debug, Clone, PartialEq)]
pub enum PenTarget {
    /// Place a node (or start a new path); nothing is under the pointer.
    Place,
    /// Shift over an end node while the Pen is idle: start a new path there instead of
    /// continuing.
    NewPathAt,
    /// Continue the open path from this end.
    Continue(EndNode),
    /// Join the path in progress to this end of another path.
    Join(EndNode),
    /// Shift over a join target: place a plain node there instead of joining.
    PlaceOverEnd,
    /// Close the path in progress onto its closing node.
    Close {
        /// The join the press applies (as drawn, flipped while Shift is held).
        join: JoinType,
        /// The join with no Shift held.
        as_drawn: JoinType,
        /// Whether Shift is held.
        shift: bool,
    },
}

/// What a press at `point` would do now, with `radius` the node hit radius (16 px) and `shift`
/// the live Shift state. Priority: the path in progress's own close target; then end nodes of
/// other open paths (both ends of a continued path are excluded: the far one is its close target,
/// the near one is the node being drawn from); then a plain node.
#[must_use]
pub fn pen_target(
    pen: &PenTool,
    index: &EndNodeIndex,
    point: Point,
    radius: Length,
    shift: bool,
) -> PenTarget {
    if let Some(closing) = pen.closing_node()
        && closing.point.vector_to(point).length() <= radius.as_mm()
    {
        let as_drawn = JoinType::as_drawn(closing.kind);
        return PenTarget::Close {
            join: JoinType::resolve(closing.kind, shift),
            as_drawn,
            shift,
        };
    }
    let exclude = pen.continuation().map(|continuation| continuation.path);
    match index.nearest(point, radius, exclude) {
        Some(end) if pen.is_placing() => {
            if shift {
                PenTarget::PlaceOverEnd
            } else {
                PenTarget::Join(end.clone())
            }
        }
        Some(end) => {
            if shift {
                PenTarget::NewPathAt
            } else {
                PenTarget::Continue(end.clone())
            }
        }
        None => PenTarget::Place,
    }
}

/// The continuation a press on `end` starts, read from `path`, the full snapshot of its path.
#[must_use]
pub fn continuation_of(end: &EndNode, path: &PathSnapshot) -> Continuation {
    Continuation {
        path: end.path,
        end: end.end,
        original: path.anchors.clone(),
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, Document, NewAnchor};

    use super::*;
    use crate::AnchorIdMinter;
    use crate::pen_tool::PointerUpOutcome;

    fn open_path(document: &Document, base: u64, points: &[(f64, f64)]) -> NodeId {
        let anchors: Vec<NewAnchor> = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                NewAnchor::corner(AnchorId::new(1, base + i as u64), Point::new(x, y))
            })
            .collect();
        document.create_path(&anchors, false)
    }

    fn paths(document: &Document) -> Vec<PathSnapshot> {
        document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.path(id))
            .collect()
    }

    const RADIUS: Length = Length::from_mm(2.0);

    /// Criteria 1 and 12: idle, only end nodes of open ordinary paths are targets; Shift asks for
    /// a new path.
    #[test]
    fn idle_over_an_end_continues_and_shift_starts_a_new_path() {
        let document = Document::new(1);
        let _ = open_path(&document, 1, &[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)]);
        let _ = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 50), Point::new(100.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 51), Point::new(110.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 52), Point::new(110.0, 10.0)),
            ],
            true,
        );
        let index = EndNodeIndex::from_paths(&paths(&document));
        let pen = PenTool::new();
        let end = Point::new(20.5, 0.0);
        assert!(matches!(
            pen_target(&pen, &index, end, RADIUS, false),
            PenTarget::Continue(EndNode {
                end: PathEnd::Last,
                ..
            })
        ));
        assert_eq!(
            pen_target(&pen, &index, end, RADIUS, true),
            PenTarget::NewPathAt
        );
        assert_eq!(
            pen_target(&pen, &index, Point::new(10.0, 0.0), RADIUS, false),
            PenTarget::Place,
            "an interior node is no target"
        );
        assert_eq!(
            pen_target(&pen, &index, Point::new(100.0, 0.0), RADIUS, false),
            PenTarget::Place,
            "a closed path has no end"
        );
    }

    /// Criterion 12: a tie goes to the topmost object.
    #[test]
    fn a_tie_goes_to_the_topmost_path() {
        let document = Document::new(1);
        let _ = open_path(&document, 1, &[(0.0, 0.0), (10.0, 0.0)]);
        let top = open_path(&document, 10, &[(10.0, 0.0), (20.0, 0.0)]);
        let index = EndNodeIndex::from_paths(&paths(&document));
        let target = index.nearest(Point::new(10.0, 0.0), RADIUS, None).unwrap();
        assert_eq!(target.path, top);
    }

    /// Criteria 7, 13: while drawing, another path's end is a join target, the first node of a new
    /// path is the close target, and the close target wins.
    #[test]
    fn drawing_joins_other_ends_and_closes_onto_its_own_start() {
        let document = Document::new(1);
        let other = open_path(&document, 1, &[(100.0, 0.0), (110.0, 0.0)]);
        let index = EndNodeIndex::from_paths(&paths(&document));
        let mut minter = AnchorIdMinter::new(7);
        let mut pen = PenTool::new();
        for at in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)] {
            pen.pointer_down(Point::new(at.0, at.1), RADIUS);
            let outcome = pen.pointer_up(&mut minter, &document, Point::new(at.0, at.1), RADIUS);
            assert_eq!(outcome, PointerUpOutcome::Placed);
        }
        match pen_target(&pen, &index, Point::new(100.0, 0.5), RADIUS, false) {
            PenTarget::Join(end) => assert_eq!(end.path, other),
            target => panic!("{target:?}"),
        }
        assert_eq!(
            pen_target(&pen, &index, Point::new(100.0, 0.5), RADIUS, true),
            PenTarget::PlaceOverEnd
        );
        assert_eq!(
            pen_target(&pen, &index, Point::new(0.5, 0.0), RADIUS, false),
            PenTarget::Close {
                join: JoinType::Sharp,
                as_drawn: JoinType::Sharp,
                shift: false
            }
        );
        assert_eq!(
            pen_target(&pen, &index, Point::new(0.5, 0.0), RADIUS, true),
            PenTarget::Close {
                join: JoinType::Smooth,
                as_drawn: JoinType::Sharp,
                shift: true
            }
        );
    }
}
