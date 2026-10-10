//! Which closed outlines touch or cross (`specs/0035-combine-and-break-apart` criteria 10 and
//! 10a). Every outline is flattened with the kernel's own deterministic [`flatten_closed`] to
//! [`TOUCH_FLATTEN_MM`], so each polyline lies within that distance of its curve. Pairs of
//! outlines whose bounding boxes are far apart are skipped; for the others the chords are swept
//! by their minimum x, so disjoint inputs cost about a sort and nested ones about their
//! box-overlapping pairs.
//!
//! **Guarantee.** Every pair whose true curves are within [`TOUCH_DISTANCE_MM`] (crossing
//! included) is reported, and no pair whose true curves are 0.003 mm or more apart is: a chord is
//! within 0.0005 mm of its curve, so chords 0.002 mm apart or closer are curves within 0.003 mm,
//! and curves within 0.001 mm have chords within 0.002 mm. Straight lines are exact: two squares
//! 0.01 mm apart pass, and a shared edge is reported. An outline touches itself when two chords
//! that are not neighbours are within reach; chords are neighbours when the polyline between them
//! is at most 0.002 mm long, so zero-length segments (a clamped corner radius, a duplicated node)
//! are no self-touch.

use curvyo_document_core::Point;

use crate::Outline;
use crate::flatten::flatten_closed;

/// Outlines this close (millimetres), or crossing, touch (the kernel grid, `0016` criterion 39).
pub const TOUCH_DISTANCE_MM: f64 = 0.001;

/// The flattening tolerance of the test (millimetres).
const TOUCH_FLATTEN_MM: f64 = 0.0005;

/// Chords closer than this are reported: two flattening tolerances beyond [`TOUCH_DISTANCE_MM`].
const CHORD_REACH_MM: f64 = 0.002;

/// One outline as a polyline with its bounding box.
pub(crate) struct Flat {
    pub(crate) points: Vec<Point>,
    pub(crate) min: Point,
    pub(crate) max: Point,
}

impl Flat {
    pub(crate) fn of(outline: &Outline<'_>) -> Self {
        let points = flatten_closed(outline.anchors, TOUCH_FLATTEN_MM);
        let mut min = Point::new(f64::INFINITY, f64::INFINITY);
        let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for point in &points {
            min = Point::new(min.x.min(point.x), min.y.min(point.y));
            max = Point::new(max.x.max(point.x), max.y.max(point.y));
        }
        Self { points, min, max }
    }

    fn chord(&self, index: usize) -> (Point, Point) {
        (
            self.points[index],
            self.points[(index + 1) % self.points.len()],
        )
    }
}

/// The pairs `(i, j)` with `i <= j` of `outlines` that touch or cross, sorted. `(i, i)` means the
/// outline touches or crosses itself (non-adjacent chords within reach). Open outlines are not
/// expected; they are treated as closed.
#[must_use]
pub fn touching_outlines(outlines: &[Outline<'_>]) -> Vec<(usize, usize)> {
    let flats: Vec<Flat> = outlines.iter().map(Flat::of).collect();
    let mut pairs = Vec::new();
    for (index, flat) in flats.iter().enumerate() {
        if self_touches(flat) {
            pairs.push((index, index));
        }
    }
    // Candidate pairs by a sweep over the bounding boxes, sorted by minimum x.
    let mut order: Vec<usize> = (0..flats.len()).collect();
    order.sort_by(|&a, &b| flats[a].min.x.total_cmp(&flats[b].min.x));
    let mut active: Vec<usize> = Vec::new();
    for &index in &order {
        let flat = &flats[index];
        active.retain(|&other| flats[other].max.x + CHORD_REACH_MM >= flat.min.x);
        for &other in &active {
            let held = &flats[other];
            let apart = flat.min.y > held.max.y + CHORD_REACH_MM
                || held.min.y > flat.max.y + CHORD_REACH_MM;
            if !apart && pair_touches(held, flat) {
                pairs.push((other.min(index), other.max(index)));
            }
        }
        active.push(index);
    }
    pairs.sort_unstable();
    pairs
}

/// A chord's bounding box, widened by the reach.
fn chord_box(a: Point, b: Point) -> (f64, f64, f64, f64) {
    (
        a.x.min(b.x) - CHORD_REACH_MM,
        a.x.max(b.x) + CHORD_REACH_MM,
        a.y.min(b.y) - CHORD_REACH_MM,
        a.y.max(b.y) + CHORD_REACH_MM,
    )
}

/// Whether any chord of `a` is within reach of any chord of `b`: a sweep over the chords of both
/// restricted to the overlap of their bounding boxes.
fn pair_touches(a: &Flat, b: &Flat) -> bool {
    let (low_x, high_x) = (
        a.min.x.max(b.min.x) - CHORD_REACH_MM,
        a.max.x.min(b.max.x) + CHORD_REACH_MM,
    );
    let (low_y, high_y) = (
        a.min.y.max(b.min.y) - CHORD_REACH_MM,
        a.max.y.min(b.max.y) + CHORD_REACH_MM,
    );
    let near = |flat: &Flat| -> Vec<(f64, f64, f64, f64, Point, Point)> {
        let mut chords: Vec<_> = (0..flat.points.len())
            .filter_map(|index| {
                let (p, q) = flat.chord(index);
                let (x0, x1, y0, y1) = chord_box(p, q);
                (x1 >= low_x && x0 <= high_x && y1 >= low_y && y0 <= high_y)
                    .then_some((x0, x1, y0, y1, p, q))
            })
            .collect();
        chords.sort_by(|left, right| left.0.total_cmp(&right.0));
        chords
    };
    let (chords_a, chords_b) = (near(a), near(b));
    sweep(&chords_a, &chords_b, |left, right| {
        segment_distance(left.4, left.5, right.4, right.5) <= CHORD_REACH_MM
    })
}

/// Whether two chords of the same outline that are not neighbours are within reach. Two chords are
/// neighbours when the polyline between them, the shorter way round, is no longer than
/// [`CHORD_REACH_MM`]: adjacent chords, and chords separated only by a zero-length or tiny run (the
/// two arcs of a slot meet at a point, a node dragged onto its neighbour). A real crossing has a
/// longer loop between its chords.
fn self_touches(flat: &Flat) -> bool {
    let count = flat.points.len();
    if count < 3 {
        return false;
    }
    // `along[k]` is the length of the polyline from point 0 to point k; `along[count]` is the whole.
    let mut along = Vec::with_capacity(count + 1);
    along.push(0.0_f64);
    for index in 0..count {
        let (p, q) = flat.chord(index);
        along.push(along[index] + (q.x - p.x).hypot(q.y - p.y));
    }
    let whole = along[count];
    let mut chords: Vec<(f64, f64, f64, f64, Point, Point, usize)> = (0..count)
        .map(|index| {
            let (p, q) = flat.chord(index);
            let (x0, x1, y0, y1) = chord_box(p, q);
            (x0, x1, y0, y1, p, q, index)
        })
        .collect();
    chords.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut active: Vec<usize> = Vec::new();
    for current in 0..chords.len() {
        active.retain(|&held| chords[held].1 >= chords[current].0);
        for &held in &active {
            let (left, right) = (&chords[held], &chords[current]);
            let (low, high) = (left.6.min(right.6), left.6.max(right.6));
            // From the end of chord `low` to the start of chord `high`, and from the end of `high`
            // round to the start of `low`.
            let forward = along[high] - along[low + 1];
            let backward = whole - along[high + 1] + along[low];
            let neighbours = forward.min(backward) <= CHORD_REACH_MM;
            if !neighbours
                && left.2 <= right.3
                && right.2 <= left.3
                && segment_distance(left.4, left.5, right.4, right.5) <= CHORD_REACH_MM
            {
                return true;
            }
        }
        active.push(current);
    }
    false
}

type Chord = (f64, f64, f64, f64, Point, Point);

/// Whether `test` holds for any pair of one chord of `a` and one of `b` whose boxes overlap; both
/// lists are sorted by their minimum x.
fn sweep(a: &[Chord], b: &[Chord], test: impl Fn(&Chord, &Chord) -> bool) -> bool {
    let mut active_a: Vec<usize> = Vec::new();
    let mut active_b: Vec<usize> = Vec::new();
    let (mut next_a, mut next_b) = (0, 0);
    while next_a < a.len() || next_b < b.len() {
        let take_a = next_b >= b.len() || (next_a < a.len() && a[next_a].0 <= b[next_b].0);
        if take_a {
            let chord = &a[next_a];
            active_b.retain(|&held| b[held].1 >= chord.0);
            for &held in &active_b {
                if b[held].2 <= chord.3 && chord.2 <= b[held].3 && test(chord, &b[held]) {
                    return true;
                }
            }
            active_a.push(next_a);
            next_a += 1;
        } else {
            let chord = &b[next_b];
            active_a.retain(|&held| a[held].1 >= chord.0);
            for &held in &active_a {
                if a[held].2 <= chord.3 && chord.2 <= a[held].3 && test(&a[held], chord) {
                    return true;
                }
            }
            active_b.push(next_b);
            next_b += 1;
        }
    }
    false
}

/// The distance between the segments `a0 a1` and `b0 b1`: zero when they cross.
pub(crate) fn segment_distance(a0: Point, a1: Point, b0: Point, b1: Point) -> f64 {
    if segments_cross(a0, a1, b0, b1) {
        return 0.0;
    }
    point_segment_distance(a0, b0, b1)
        .min(point_segment_distance(a1, b0, b1))
        .min(point_segment_distance(b0, a0, a1))
        .min(point_segment_distance(b1, a0, a1))
}

fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn segments_cross(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    let d1 = cross(a0, a1, b0);
    let d2 = cross(a0, a1, b1);
    let d3 = cross(b0, b1, a0);
    let d4 = cross(b0, b1, a1);
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}

/// The distance from `point` to the segment `a b`.
pub(crate) fn point_segment_distance(point: Point, a: Point, b: Point) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let length_sq = abx * abx + aby * aby;
    let t = if length_sq <= 0.0 {
        0.0
    } else {
        (((point.x - a.x) * abx + (point.y - a.y) * aby) / length_sq).clamp(0.0, 1.0)
    };
    (point.x - (a.x + abx * t)).hypot(point.y - (a.y + aby * t))
}
