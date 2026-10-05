//! Hit-testing one document-space point against every path in the
//! document (acceptance criteria 7, 10, 12, 14).
//!
//! Priority order, matching the visual stacking
//! `specs/0002-path-node-editing/specification.md`'s UX notes describe: a
//! handle (only hittable when its own node is selected — an unselected
//! node shows no handles at all), then a node, then a segment. Within
//! each category the nearest candidate within its own tolerance wins.

use vecmanf_document_core::{AnchorId, HandleSlot, NodeId, PathSnapshot, Point, Tolerance, Vec2};
use vecmanf_geometry_core::nearest_point_on_segment;

use crate::NodeSelection;

/// What one point hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// One of a selected node's two handle endpoints.
    Handle {
        /// The handle's path.
        path: NodeId,
        /// The handle's own anchor.
        anchor: AnchorId,
        /// Which of the anchor's two handles.
        slot: HandleSlot,
    },
    /// A node, on any path (nodes are hittable whether or not they are
    /// currently selected).
    Node {
        /// The node's path.
        path: NodeId,
        /// The node itself.
        anchor: AnchorId,
    },
    /// A point on a segment, strictly between its two endpoint anchors.
    Segment {
        /// The segment's path.
        path: NodeId,
        /// The segment's earlier anchor in traversal order (the one whose
        /// `handle_out` faces this segment).
        start: AnchorId,
        /// The segment's later anchor (whose `handle_in` faces it).
        end: AnchorId,
    },
}

fn distance(a: Point, b: Point) -> f64 {
    a.vector_to(b).length()
}

/// Hit-tests `point` against every path in `paths`.
///
/// `node_tolerance` bounds node hits; `handle_tolerance` bounds handle
/// hits (wider than `node_tolerance` since 2026-10-05: the handle
/// endpoint's own doubled visual size needs a doubled clickable radius
/// to match, or it would look bigger without actually being easier to
/// hit); `segment_tolerance` bounds segment hits — kept as separate
/// parameters per `docs/design-system.md`'s own distinct values,
/// converted to document millimetres by the caller before this is
/// called.
#[must_use]
pub fn hit_test(
    paths: &[PathSnapshot],
    selection: &NodeSelection,
    point: Point,
    node_tolerance: Tolerance,
    handle_tolerance: Tolerance,
    segment_tolerance: Tolerance,
) -> Option<Hit> {
    hit_test_handle(paths, selection, point, handle_tolerance)
        .or_else(|| hit_test_node(paths, point, node_tolerance))
        .or_else(|| hit_test_segment(paths, point, segment_tolerance))
}

fn better(
    best: Option<(f64, Hit)>,
    distance: f64,
    tolerance: Tolerance,
    hit: impl FnOnce() -> Hit,
) -> Option<(f64, Hit)> {
    if distance > tolerance.as_mm() {
        return best;
    }
    match best {
        Some((best_distance, _)) if best_distance <= distance => best,
        _ => Some((distance, hit())),
    }
}

fn hit_test_handle(
    paths: &[PathSnapshot],
    selection: &NodeSelection,
    point: Point,
    tolerance: Tolerance,
) -> Option<Hit> {
    let path_id = selection.path()?;
    let snapshot = paths.iter().find(|p| p.id == path_id)?;
    let mut best: Option<(f64, Hit)> = None;
    for anchor in &snapshot.anchors {
        if !selection.contains_node(anchor.id) {
            continue;
        }
        for (slot, handle) in [
            (HandleSlot::Out, anchor.handle_out),
            (HandleSlot::In, anchor.handle_in),
        ] {
            if handle == Vec2::ZERO {
                continue;
            }
            let handle_point = anchor.point.translated(handle);
            let d = distance(handle_point, point);
            best = better(best, d, tolerance, || Hit::Handle {
                path: path_id,
                anchor: anchor.id,
                slot,
            });
        }
    }
    best.map(|(_, hit)| hit)
}

fn hit_test_node(paths: &[PathSnapshot], point: Point, tolerance: Tolerance) -> Option<Hit> {
    let mut best: Option<(f64, Hit)> = None;
    for snapshot in paths {
        for anchor in &snapshot.anchors {
            let d = distance(anchor.point, point);
            best = better(best, d, tolerance, || Hit::Node {
                path: snapshot.id,
                anchor: anchor.id,
            });
        }
    }
    best.map(|(_, hit)| hit)
}

/// Every path-adjacent pair of anchors in traversal order, including the
/// wraparound closing segment when the path is closed and has more than
/// two anchors.
fn segments(snapshot: &PathSnapshot) -> impl Iterator<Item = (usize, usize)> + '_ {
    let len = snapshot.anchors.len();
    let adjacent = (0..len.saturating_sub(1)).map(|i| (i, i + 1));
    let wraparound = (snapshot.closed && len > 2).then_some((len - 1, 0));
    adjacent.chain(wraparound)
}

fn hit_test_segment(paths: &[PathSnapshot], point: Point, tolerance: Tolerance) -> Option<Hit> {
    let mut best: Option<(f64, Hit)> = None;
    for snapshot in paths {
        for (i, j) in segments(snapshot) {
            let start = &snapshot.anchors[i];
            let end = &snapshot.anchors[j];
            let (_, segment_distance, _) = nearest_point_on_segment(
                start.point,
                start.handle_out,
                end.handle_in,
                end.point,
                point,
                tolerance,
            );
            best = better(best, segment_distance.as_mm(), tolerance, || Hit::Segment {
                path: snapshot.id,
                start: start.id,
                end: end.id,
            });
        }
    }
    best.map(|(_, hit)| hit)
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{AnchorId, AnchorKind, Document, NewAnchor};

    use super::*;

    const POINT_TOLERANCE: Tolerance = Tolerance::from_mm(2.0);
    const HANDLE_TOLERANCE: Tolerance = Tolerance::from_mm(4.0);
    const SEGMENT_TOLERANCE: Tolerance = Tolerance::from_mm(1.0);

    /// A closed triangle, returned together with the `Document` that owns
    /// it — a `NodeId`/`AnchorId` only resolves against the document that
    /// minted it, so tests that need to read a snapshot back keep the
    /// document alive alongside the ids.
    fn triangle(peer: u64) -> (Document, NodeId, [AnchorId; 3]) {
        let document = Document::new(peer);
        let ids = [
            AnchorId::new(peer, 1),
            AnchorId::new(peer, 2),
            AnchorId::new(peer, 3),
        ];
        let path = document.create_path(
            &[
                NewAnchor::corner(ids[0], Point::new(0.0, 0.0)),
                NewAnchor::corner(ids[1], Point::new(20.0, 0.0)),
                NewAnchor::corner(ids[2], Point::new(10.0, 20.0)),
            ],
            true,
        );
        (document, path, ids)
    }

    /// A single open two-node segment from `a` to `b`, same deal.
    fn open_segment(peer: u64, a: AnchorId, b: AnchorId) -> (Document, NodeId) {
        let document = Document::new(peer);
        let path = document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        (document, path)
    }

    #[test]
    fn hits_the_nearest_node_within_tolerance() {
        let (document, path, ids) = triangle(1);
        let paths = vec![document.path(path).expect("exists")];
        let selection = NodeSelection::new();

        let hit = hit_test(
            &paths,
            &selection,
            Point::new(0.5, 0.2),
            POINT_TOLERANCE,
            HANDLE_TOLERANCE,
            SEGMENT_TOLERANCE,
        );
        assert_eq!(
            hit,
            Some(Hit::Node {
                path,
                anchor: ids[0]
            })
        );
    }

    #[test]
    fn misses_everything_far_from_the_path() {
        let (document, path) = open_segment(1, AnchorId::new(1, 1), AnchorId::new(1, 2));
        let paths = vec![document.path(path).expect("exists")];
        let selection = NodeSelection::new();
        let hit = hit_test(
            &paths,
            &selection,
            Point::new(1000.0, 1000.0),
            POINT_TOLERANCE,
            HANDLE_TOLERANCE,
            SEGMENT_TOLERANCE,
        );
        assert_eq!(hit, None);
    }

    #[test]
    fn hits_a_segment_between_two_nodes() {
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let (document, path) = open_segment(1, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let selection = NodeSelection::new();
        let hit = hit_test(
            &paths,
            &selection,
            Point::new(10.0, 0.3),
            POINT_TOLERANCE,
            HANDLE_TOLERANCE,
            SEGMENT_TOLERANCE,
        );
        assert_eq!(
            hit,
            Some(Hit::Segment {
                path,
                start: a,
                end: b
            })
        );
    }

    #[test]
    fn handles_are_only_hittable_on_a_selected_node() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Smooth,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let handle_point = Point::new(5.0, 0.0);

        let unselected = NodeSelection::new();
        assert_eq!(
            hit_test(
                &paths,
                &unselected,
                handle_point,
                POINT_TOLERANCE,
                HANDLE_TOLERANCE,
                SEGMENT_TOLERANCE
            ),
            None,
            "an unselected node's handle is not hittable"
        );

        let mut selected = NodeSelection::new();
        selected.select_single_node(path, a);
        assert_eq!(
            hit_test(
                &paths,
                &selected,
                handle_point,
                POINT_TOLERANCE,
                HANDLE_TOLERANCE,
                SEGMENT_TOLERANCE
            ),
            Some(Hit::Handle {
                path,
                anchor: a,
                slot: HandleSlot::Out
            })
        );
    }

    /// The bug this run fixes: the handle's own hit-test radius must be
    /// independently wider than the node radius (2026-10-05, matching
    /// the handle glyph's own doubled visual size) — a click that lands
    /// outside `POINT_TOLERANCE` but within `HANDLE_TOLERANCE` of a
    /// selected node's handle must still hit it. `HANDLE_TOLERANCE` is
    /// double `POINT_TOLERANCE` in this test module, same ratio as
    /// `docs/design-system.md`'s real 8px/16px tokens.
    #[test]
    fn handle_tolerance_is_wider_than_point_tolerance() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::new(5.0, 0.0),
                    kind: AnchorKind::Smooth,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let mut selected = NodeSelection::new();
        selected.select_single_node(path, a);

        // 3mm off the handle endpoint (5.0, 0.0): outside POINT_TOLERANCE
        // (2mm) but within HANDLE_TOLERANCE (4mm).
        let just_past_point_tolerance = Point::new(8.0, 0.0);
        assert_eq!(
            hit_test(
                &paths,
                &selected,
                just_past_point_tolerance,
                POINT_TOLERANCE,
                POINT_TOLERANCE,
                SEGMENT_TOLERANCE
            ),
            None,
            "sanity check: using the node radius for handles too would miss this click"
        );
        assert_eq!(
            hit_test(
                &paths,
                &selected,
                just_past_point_tolerance,
                POINT_TOLERANCE,
                HANDLE_TOLERANCE,
                SEGMENT_TOLERANCE
            ),
            Some(Hit::Handle {
                path,
                anchor: a,
                slot: HandleSlot::Out
            }),
            "the real, wider handle tolerance must hit"
        );
    }

    #[test]
    fn a_zero_handle_is_never_hittable() {
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let (document, path) = open_segment(1, a, b);
        let paths = vec![document.path(path).expect("exists")];
        let mut selection = NodeSelection::new();
        selection.select_single_node(path, a);
        // The handle sits exactly on the node itself (zero vector); it
        // must not be reported as a handle hit distinct from the node.
        let hit = hit_test(
            &paths,
            &selection,
            Point::new(0.0, 0.0),
            POINT_TOLERANCE,
            HANDLE_TOLERANCE,
            SEGMENT_TOLERANCE,
        );
        assert_eq!(hit, Some(Hit::Node { path, anchor: a }));
    }

    #[test]
    fn a_closed_path_hit_tests_its_wraparound_segment() {
        let (document, path, ids) = triangle(1);
        let paths = vec![document.path(path).expect("exists")];
        let selection = NodeSelection::new();
        let midpoint = Point::new(5.0, 10.0);
        let hit = hit_test(
            &paths,
            &selection,
            midpoint,
            POINT_TOLERANCE,
            HANDLE_TOLERANCE,
            SEGMENT_TOLERANCE,
        );
        assert_eq!(
            hit,
            Some(Hit::Segment {
                path,
                start: ids[2],
                end: ids[0]
            })
        );
    }
}
