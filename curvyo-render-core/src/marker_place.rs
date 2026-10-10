//! Where the markers of one outline go and which way they face
//! (`specs/0018-stroke-markers` criteria 7 to 16, `adrs.md` decision 2): a pure
//! function of the anchors and the marker settings. Ends and nodes come from
//! the anchors' control polygon; evenly spaced markers come from `lyon`'s
//! arc-length measure of the undashed path, the same measure `dash.rs` uses.

use curvyo_document_core::{AnchorSnapshot, MarkerPlace, MarkerShape, Markers, Point, Vec2};
use lyon::algorithms::measure::{PathMeasurements, SampleType};
use lyon::path::Path;

use crate::stroke::to_lyon;

/// The most markers one slot of one outline draws.
pub(crate) const MAX_MARKERS_PER_SLOT: usize = 500;

/// A path shorter than this has no markers (criterion 11), millimetres.
const MIN_LENGTH_MM: f64 = 0.001;

/// A handle shorter than this has no direction, millimetres.
const MIN_HANDLE_MM: f64 = 1e-9;

/// A spaced marker this close to a node sits on it, millimetres.
const NODE_SNAP_MM: f64 = 1e-3;

/// Two tangents within this many degrees of opposite are a reversal.
const REVERSAL_DEGREES: f64 = 1.0;

/// One marker to draw: the shape on its anchor, facing `direction` (a unit
/// vector; the direction of travel, or for a Start arrow its opposite).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MarkerPlacement {
    pub(crate) shape: MarkerShape,
    pub(crate) point: Point,
    pub(crate) direction: Vec2,
}

fn unit(vector: Vec2) -> Option<Vec2> {
    let length = vector.length();
    (length > MIN_HANDLE_MM && length.is_finite()).then(|| vector.scaled(1.0 / length))
}

fn between(from: Point, to: Point) -> Vec2 {
    Vec2::new(to.x - from.x, to.y - from.y)
}

/// The control points met walking away from anchor `i` along the control
/// polygon (forward: handle out, next handle in, next node, next handle out,
/// ...; backward the mirror image), as far as the path goes. A closed path
/// wraps around once.
fn polygon_points(anchors: &[AnchorSnapshot], closed: bool, i: usize, forward: bool) -> Vec<Point> {
    let n = anchors.len();
    let own = &anchors[i];
    let mut points = vec![if forward {
        own.point.translated(own.handle_out)
    } else {
        own.point.translated(own.handle_in)
    }];
    for step in 1..n {
        let j = if forward {
            i + step
        } else if step <= i || closed {
            i + n - step
        } else {
            break;
        };
        if forward && !closed && j >= n {
            break;
        }
        let j = j % n;
        let a = &anchors[j];
        let (near, far) = if forward {
            (a.handle_in, a.handle_out)
        } else {
            (a.handle_out, a.handle_in)
        };
        points.push(a.point.translated(near));
        points.push(a.point);
        points.push(a.point.translated(far));
    }
    points
}

/// The outgoing tangent of anchor `i` (a unit vector), or `None` when every
/// control point after it coincides with it. A zero-length handle uses the
/// direction to the next control point or node that differs.
fn outgoing(anchors: &[AnchorSnapshot], closed: bool, i: usize) -> Option<Vec2> {
    let origin = anchors[i].point;
    polygon_points(anchors, closed, i, true)
        .into_iter()
        .find_map(|target| unit(between(origin, target)))
}

/// The incoming tangent of anchor `i`: the direction of travel arriving at it.
fn incoming(anchors: &[AnchorSnapshot], closed: bool, i: usize) -> Option<Vec2> {
    let target = anchors[i].point;
    polygon_points(anchors, closed, i, false)
        .into_iter()
        .find_map(|source| unit(between(source, target)))
}

/// The direction of travel at an inner node: the bisector of the incoming and
/// outgoing tangents; within a degree of a reversal, the outgoing tangent.
fn at_node(anchors: &[AnchorSnapshot], closed: bool, i: usize) -> Option<Vec2> {
    let out = outgoing(anchors, closed, i);
    let inc = incoming(anchors, closed, i);
    match (inc, out) {
        (Some(inc), Some(out)) => {
            let dot = inc.x * out.x + inc.y * out.y;
            if dot < -(REVERSAL_DEGREES.to_radians()).cos() {
                return Some(out);
            }
            unit(Vec2::new(inc.x + out.x, inc.y + out.y)).or(Some(out))
        }
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

/// The path of one outline as `lyon` measures it.
fn measured_path(anchors: &[AnchorSnapshot], closed: bool) -> Path {
    let mut builder = Path::builder();
    builder.begin(to_lyon(anchors[0].point));
    let curve =
        |builder: &mut lyon::path::path::Builder, from: &AnchorSnapshot, to: &AnchorSnapshot| {
            builder.cubic_bezier_to(
                to_lyon(from.point.translated(from.handle_out)),
                to_lyon(to.point.translated(to.handle_in)),
                to_lyon(to.point),
            );
        };
    for pair in anchors.windows(2) {
        curve(&mut builder, &pair[0], &pair[1]);
    }
    if closed {
        curve(&mut builder, &anchors[anchors.len() - 1], &anchors[0]);
    }
    builder.end(closed);
    builder.build()
}

/// The markers of one outline. `tolerance_mm` is the measuring tolerance of
/// the arc length (`adrs.md`: at most 0.01 mm, so positions do not move with
/// zoom). Empty for fewer than two anchors or a path shorter than a
/// thousandth of a millimetre.
pub(crate) fn marker_placements(
    anchors: &[AnchorSnapshot],
    closed: bool,
    markers: Markers,
    tolerance_mm: f64,
) -> Vec<MarkerPlacement> {
    let slots_used = markers.start != MarkerShape::None
        || markers.mid != MarkerShape::None
        || markers.end != MarkerShape::None;
    if anchors.len() < 2 || !slots_used {
        return Vec::new();
    }
    let path = measured_path(anchors, closed);
    #[allow(clippy::cast_possible_truncation)]
    let measurements = PathMeasurements::from_path(&path, tolerance_mm.min(0.01) as f32);
    let length = f64::from(measurements.length());
    if !length.is_finite() || length < MIN_LENGTH_MM {
        return Vec::new();
    }
    let mut placements = Vec::new();
    let last = anchors.len() - 1;
    if !closed {
        if let (true, Some(direction)) = (
            markers.start != MarkerShape::None,
            outgoing(anchors, false, 0),
        ) {
            placements.push(MarkerPlacement {
                shape: markers.start,
                point: anchors[0].point,
                // The Start arrow points outward: against the travel.
                direction: direction.negated(),
            });
        }
        if let (true, Some(direction)) = (
            markers.end != MarkerShape::None,
            incoming(anchors, false, last),
        ) {
            placements.push(MarkerPlacement {
                shape: markers.end,
                point: anchors[last].point,
                direction,
            });
        }
    }
    if markers.mid == MarkerShape::None {
        return placements;
    }
    match markers.mid_place {
        MarkerPlace::AtNodes => {
            let nodes = if closed {
                0..=last
            } else {
                1..=last.saturating_sub(1)
            };
            for i in nodes.take(MAX_MARKERS_PER_SLOT) {
                if let Some(direction) = at_node(anchors, closed, i) {
                    placements.push(MarkerPlacement {
                        shape: markers.mid,
                        point: anchors[i].point,
                        direction,
                    });
                }
            }
        }
        MarkerPlace::Spaced => {
            let count = (markers.mid_count.get() as usize).min(MAX_MARKERS_PER_SLOT);
            let mut sampler = measurements.create_sampler(&path, SampleType::Distance);
            // The direction comes from two points on the outline either side of
            // the marker, not from the sampler's tangent, which is not a number
            // at the end of a degenerate curve (a corner anchor's zero handles).
            let step = (length * 0.001).clamp(1e-4, 0.05);
            for k in 0..count {
                // Open: k / (N + 1) for k = 1..N. Closed: k / N for k = 0..N-1.
                #[allow(clippy::cast_precision_loss)]
                let fraction = if closed {
                    k as f64 / count as f64
                } else {
                    (k + 1) as f64 / (count + 1) as f64
                };
                let at = length * fraction;
                #[allow(clippy::cast_possible_truncation)]
                let mut position_at = |distance: f64| {
                    // A closed outline wraps, so the first marker looks back
                    // along the closing segment and takes the bisector.
                    let distance = if closed {
                        distance.rem_euclid(length)
                    } else {
                        distance.clamp(0.0, length)
                    };
                    let p = sampler.sample(distance as f32).position();
                    Point::new(f64::from(p.x), f64::from(p.y))
                };
                let point = position_at(at);
                // A marker that lands on a node takes the node's direction
                // (the bisector, criterion 14); the chord is skewed there by the
                // flattening of the curve.
                let on_node = anchors
                    .iter()
                    .position(|a| between(a.point, point).length() <= NODE_SNAP_MM);
                let direction = on_node
                    .and_then(|i| at_node(anchors, closed, i))
                    .or_else(|| unit(between(position_at(at - step), position_at(at + step))))
                    .unwrap_or(Vec2::new(1.0, 0.0));
                placements.push(MarkerPlacement {
                    shape: markers.mid,
                    point,
                    direction,
                });
            }
        }
    }
    placements
}
