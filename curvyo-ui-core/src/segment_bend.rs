//! A segment bend in progress (`specs/0031-segment-drag-bending`): the state a press on a segment
//! of the Node tool starts, what a release at a pointer position would commit, and the live
//! shape the preview draws. [`SegmentBend::resolve`] is the one resolving function the preview
//! and the release both call, so they cannot disagree.
//!
//! The work per pointer move is constant: the press keeps a window of at most four anchors (the
//! two ends of the dragged segment and their outer neighbours), never the whole path.

use curvyo_document_core::{
    AnchorId, AnchorSnapshot, Document, HandleSlot, NodeId, PathSnapshot, Point, Tolerance, Vec2,
    resolve_handle_pair,
};
use curvyo_geometry_core::{BentHandles, bend_segment_handles, nearest_point_on_segment};

use crate::select_tool::{Axis, lock_axis};
use crate::transform_drag::DragOrigin;

/// A net displacement shorter than this (millimetres) writes nothing (`0031` criterion 19).
const NO_DISPLACEMENT_MM: f64 = 1e-9;

/// The anchors a bend can change or must draw, as they were at the press.
#[derive(Debug, Clone)]
struct Window {
    start: AnchorSnapshot,
    end: AnchorSnapshot,
    /// The anchor before `start` (wrapping when the path is closed), if any.
    before: Option<AnchorSnapshot>,
    /// The anchor after `end` (wrapping when the path is closed), if any.
    after: Option<AnchorSnapshot>,
}

/// A bend started by a press on a segment.
#[derive(Debug, Clone)]
pub(crate) struct SegmentBend {
    origin: DragOrigin,
    last_axis: Option<Axis>,
    /// Shift as of the latest pointer event, for the release.
    shift: bool,
    path: NodeId,
    /// The grab parameter: where on the segment the press landed.
    t0: f64,
    window: Window,
}

/// What a bend at a pointer position shows and would commit.
#[derive(Debug, Clone, PartialEq)]
pub struct BendResolution {
    /// The bent path.
    pub path: NodeId,
    /// The displacement from the press point, after any axis lock.
    pub displacement: Vec2,
    /// The axis the displacement is locked to; `None` while Shift is up.
    pub axis: Option<Axis>,
    /// What [`Document::bend_segment`] is given for the two handles that face the segment.
    pub written: BentHandles,
    /// The two end anchors of the dragged segment with their live handles (both sides: the far
    /// side of a Symmetric or Asymmetric node follows).
    pub ends: [AnchorSnapshot; 2],
    /// Every segment whose shape the bend changes, each as its two anchors in path order: the
    /// dragged one first, then a neighbour only where a far handle changed.
    pub changed_segments: Vec<[AnchorSnapshot; 2]>,
}

/// Whether the segment between `start` and `end` of `path` can be bent: it has a grab point (it
/// is not a zero-length line). Used by the hover, so the band shows only what a press would bend.
#[must_use]
pub fn segment_is_bendable(path: &PathSnapshot, start: AnchorId, end: AnchorId) -> bool {
    let find = |id: AnchorId| path.anchors.iter().find(|anchor| anchor.id == id);
    match (find(start), find(end)) {
        (Some(start), Some(end)) => bend_segment_handles(
            start.point,
            start.handle_out,
            end.handle_in,
            end.point,
            0.5,
            Vec2::ZERO,
        )
        .is_some(),
        _ => false,
    }
}

impl SegmentBend {
    /// A bend of the segment `start` to `end` of `path`, pressed at `down_at` with the drag
    /// threshold `dead_zone`, or `None` if the anchors do not resolve or the segment has no grab
    /// point (a zero-length line).
    pub(crate) fn begin(
        path: &PathSnapshot,
        start: AnchorId,
        end: AnchorId,
        down_at: Point,
        dead_zone: Tolerance,
        tolerance: Tolerance,
    ) -> Option<Self> {
        let count = path.anchors.len();
        let start_index = path.anchors.iter().position(|a| a.id == start)?;
        let end_index = path.anchors.iter().position(|a| a.id == end)?;
        let (s, e) = (path.anchors[start_index], path.anchors[end_index]);
        bend_segment_handles(s.point, s.handle_out, e.handle_in, e.point, 0.5, Vec2::ZERO)?;
        let before = (start_index > 0)
            .then(|| start_index - 1)
            .or_else(|| (path.closed && count > 1).then(|| count - 1))
            .map(|i| path.anchors[i]);
        let after = (end_index + 1 < count)
            .then(|| end_index + 1)
            .or_else(|| (path.closed && count > 1).then_some(0))
            .map(|i| path.anchors[i]);
        let (t0, _, _) = nearest_point_on_segment(
            s.point,
            s.handle_out,
            e.handle_in,
            e.point,
            down_at,
            tolerance,
        );
        Some(Self {
            origin: DragOrigin::new(down_at, dead_zone.as_mm(), false),
            last_axis: None,
            shift: false,
            path: path.id,
            t0,
            window: Window {
                start: s,
                end: e,
                before,
                after,
            },
        })
    }

    /// Records a pointer position: the drag threshold, once left, stays left (criterion 3), and
    /// the axis of a lock is remembered to break an exact tie.
    pub(crate) fn pointer_moved(&mut self, point: Point, shift: bool) {
        self.origin.note(point);
        self.shift = shift;
        if shift && self.origin.is_active_at(point) {
            let raw = self.origin.down_at.vector_to(point);
            self.last_axis = Some(lock_axis(raw, self.last_axis).1);
        }
    }

    /// What the bend is at `cursor` with Shift as given; `None` while the pointer has never left
    /// the drag threshold.
    pub(crate) fn resolve(&self, cursor: Point, shift: bool) -> Option<BendResolution> {
        if !self.origin.is_active_at(cursor) {
            return None;
        }
        let raw = self.origin.down_at.vector_to(cursor);
        let (displacement, axis) = if shift {
            let (locked, axis) = lock_axis(raw, self.last_axis);
            (locked, Some(axis))
        } else {
            (raw, None)
        };
        let Window { start, end, .. } = self.window;
        let written = bend_segment_handles(
            start.point,
            start.handle_out,
            end.handle_in,
            end.point,
            self.t0,
            displacement,
        )?;
        // The same per-kind rule `Document::bend_segment` writes with.
        let (start_in, start_out) = resolve_handle_pair(
            start.kind,
            HandleSlot::Out,
            written.start_handle_out,
            start.handle_in,
            start.handle_out,
        );
        let (end_in, end_out) = resolve_handle_pair(
            end.kind,
            HandleSlot::In,
            written.end_handle_in,
            end.handle_in,
            end.handle_out,
        );
        let live_start = AnchorSnapshot {
            handle_in: start_in,
            handle_out: start_out,
            ..start
        };
        let live_end = AnchorSnapshot {
            handle_in: end_in,
            handle_out: end_out,
            ..end
        };
        let live = |anchor: AnchorSnapshot| {
            if anchor.id == live_start.id {
                live_start
            } else if anchor.id == live_end.id {
                live_end
            } else {
                anchor
            }
        };
        let mut changed_segments = vec![[live_start, live_end]];
        if start_in != start.handle_in
            && let Some(before) = self.window.before
        {
            push_new(&mut changed_segments, [live(before), live_start]);
        }
        if end_out != end.handle_out
            && let Some(after) = self.window.after
        {
            push_new(&mut changed_segments, [live_end, live(after)]);
        }
        Some(BendResolution {
            path: self.path,
            displacement,
            axis,
            written,
            ends: [live_start, live_end],
            changed_segments,
        })
    }

    /// Commits the bend released at `point` as one `bend_segment`, unless nothing would change:
    /// the pointer never left the drag threshold, or the net displacement is zero
    /// (`0031` criteria 2 and 19). Returns whether anything was written.
    pub(crate) fn commit(&mut self, document: &Document, point: Point) -> bool {
        self.pointer_moved(point, self.shift);
        let Some(resolution) = self.resolve(point, self.shift) else {
            return false;
        };
        if resolution.displacement.length() < NO_DISPLACEMENT_MM {
            return false;
        }
        document
            .bend_segment(
                self.path,
                self.window.start.id,
                self.window.end.id,
                resolution.written.start_handle_out,
                resolution.written.end_handle_in,
            )
            .is_ok()
    }
}

/// Pushes `segment` unless the list already holds it (a two-node closed path reaches its other
/// segment from both sides).
fn push_new(segments: &mut Vec<[AnchorSnapshot; 2]>, segment: [AnchorSnapshot; 2]) {
    if !segments.contains(&segment) {
        segments.push(segment);
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorKind, Document, NewAnchor};

    use super::*;

    fn id(n: u64) -> AnchorId {
        AnchorId::new(1, n)
    }

    fn anchor(n: u64, x: f64, kind: AnchorKind) -> NewAnchor {
        NewAnchor {
            id: id(n),
            point: Point::new(x, 0.0),
            handle_in: Vec2::ZERO,
            handle_out: Vec2::ZERO,
            kind,
        }
    }

    fn snapshot(anchors: &[NewAnchor], closed: bool) -> PathSnapshot {
        let document = Document::new(1);
        let path = document.create_path(anchors, closed);
        document.path(path).unwrap()
    }

    const DEAD_ZONE: Tolerance = Tolerance::from_mm(3.0);
    const TOLERANCE: Tolerance = Tolerance::from_mm(4.0);

    fn begin(path: &PathSnapshot, start: u64, end: u64, at: Point) -> SegmentBend {
        SegmentBend::begin(path, id(start), id(end), at, DEAD_ZONE, TOLERANCE).unwrap()
    }

    /// Criterion 3: the threshold, once left, stays left, and the displacement is from the press
    /// point, not from where the threshold was passed.
    #[test]
    fn the_threshold_latches_and_the_displacement_is_from_the_press() {
        let path = snapshot(
            &[
                anchor(1, 0.0, AnchorKind::Corner),
                anchor(2, 90.0, AnchorKind::Corner),
            ],
            false,
        );
        let mut bend = begin(&path, 1, 2, Point::new(45.0, 0.0));
        assert!(
            bend.resolve(Point::new(45.0, -2.0), false).is_none(),
            "inside 3 mm"
        );
        bend.pointer_moved(Point::new(45.0, -10.0), false);
        let back = bend.resolve(Point::new(45.0, -1.0), false).unwrap();
        assert!(
            (back.displacement.y + 1.0).abs() < 1e-12,
            "latched: d = (0, -1)"
        );
        let far = bend.resolve(Point::new(45.0, -30.0), false).unwrap();
        // The worked example of criterion 8.
        assert!((far.written.start_handle_out.x - 30.0).abs() < 1e-9);
        assert!((far.written.start_handle_out.y + 40.0).abs() < 1e-9);
    }

    /// Criterion 13: Shift keeps the axis with the larger displacement; the other is zero.
    #[test]
    fn shift_locks_the_axis_of_the_larger_displacement() {
        let path = snapshot(
            &[
                anchor(1, 0.0, AnchorKind::Corner),
                anchor(2, 90.0, AnchorKind::Corner),
            ],
            false,
        );
        let mut bend = begin(&path, 1, 2, Point::new(45.0, 0.0));
        bend.pointer_moved(Point::new(70.0, -20.0), true);
        let locked = bend.resolve(Point::new(70.0, -20.0), true).unwrap();
        assert_eq!(locked.axis, Some(Axis::X));
        assert_eq!(locked.displacement, Vec2::new(25.0, 0.0));
        let free = bend.resolve(Point::new(70.0, -20.0), false).unwrap();
        assert_eq!(free.axis, None);
    }

    /// Criterion 16: a neighbouring segment is in the preview only where the node kind moved the
    /// far handle; a Corner end changes the dragged segment only.
    #[test]
    fn the_preview_has_a_neighbour_only_through_a_smooth_node() {
        let corners = snapshot(
            &[
                anchor(1, 0.0, AnchorKind::Corner),
                anchor(2, 90.0, AnchorKind::Corner),
                anchor(3, 180.0, AnchorKind::Corner),
            ],
            false,
        );
        let mut bend = begin(&corners, 1, 2, Point::new(45.0, 0.0));
        bend.pointer_moved(Point::new(45.0, -30.0), false);
        assert_eq!(
            bend.resolve(Point::new(45.0, -30.0), false)
                .unwrap()
                .changed_segments
                .len(),
            1
        );

        let smooth = snapshot(
            &[
                anchor(1, 0.0, AnchorKind::Corner),
                anchor(2, 90.0, AnchorKind::Symmetric),
                anchor(3, 180.0, AnchorKind::Corner),
            ],
            false,
        );
        let mut bend = begin(&smooth, 1, 2, Point::new(45.0, 0.0));
        bend.pointer_moved(Point::new(45.0, -30.0), false);
        let resolution = bend.resolve(Point::new(45.0, -30.0), false).unwrap();
        assert_eq!(resolution.changed_segments.len(), 2, "AB and BC");
        assert_eq!(resolution.changed_segments[1][0].id, id(2));
        assert_eq!(resolution.changed_segments[1][1].id, id(3));
    }

    /// A closed path of two nodes reaches its other segment from both ends: it is listed once.
    #[test]
    fn a_two_node_closed_path_lists_its_other_segment_once() {
        let path = snapshot(
            &[
                anchor(1, 0.0, AnchorKind::Symmetric),
                anchor(2, 90.0, AnchorKind::Symmetric),
            ],
            true,
        );
        // The closing segment B to A.
        let mut bend = begin(&path, 2, 1, Point::new(45.0, 0.0));
        bend.pointer_moved(Point::new(45.0, -30.0), false);
        let resolution = bend.resolve(Point::new(45.0, -30.0), false).unwrap();
        assert_eq!(
            resolution.changed_segments.len(),
            2,
            "B to A and A to B, once each"
        );
    }

    /// Criterion 6: a zero-length line has no grab point and starts no bend.
    #[test]
    fn a_zero_length_segment_cannot_be_bent() {
        let path = snapshot(
            &[
                NewAnchor::corner(id(1), Point::new(5.0, 5.0)),
                NewAnchor::corner(id(2), Point::new(5.0, 5.0)),
            ],
            false,
        );
        assert!(!segment_is_bendable(&path, id(1), id(2)));
        assert!(
            SegmentBend::begin(
                &path,
                id(1),
                id(2),
                Point::new(5.0, 5.0),
                DEAD_ZONE,
                TOLERANCE
            )
            .is_none()
        );
    }

    /// Criterion 18/19: a release writes one `bend_segment` unless the net displacement is zero.
    #[test]
    fn a_release_writes_unless_the_net_displacement_is_zero() {
        let document = Document::new(1);
        let path_id = document.create_path(
            &[
                anchor(1, 0.0, AnchorKind::Corner),
                anchor(2, 90.0, AnchorKind::Corner),
            ],
            false,
        );
        let path = document.path(path_id).unwrap();
        let mut bend = begin(&path, 1, 2, Point::new(45.0, 0.0));
        bend.pointer_moved(Point::new(45.0, -30.0), false);
        assert!(
            !bend.commit(&document, Point::new(45.0, 0.0)),
            "back at the press: nothing"
        );
        assert_eq!(
            document.path(path_id).unwrap().anchors[0].handle_out,
            Vec2::ZERO
        );
        let mut bend = begin(&path, 1, 2, Point::new(45.0, 0.0));
        assert!(
            !bend.commit(&document, Point::new(45.0, -1.0)),
            "inside the threshold: a click"
        );
        let mut bend = begin(&path, 1, 2, Point::new(45.0, 0.0));
        assert!(bend.commit(&document, Point::new(45.0, -30.0)));
        let bent = document.path(path_id).unwrap();
        assert!((bent.anchors[0].handle_out.y + 40.0).abs() < 1e-9);
    }
}
