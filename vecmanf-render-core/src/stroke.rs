//! Tessellating a path's (or one segment's) cubic Bézier geometry into a
//! stroke, via `lyon` (ADR 0001 §4). The curve math itself is `lyon`'s
//! job here, same as `vecmanf-geometry-core`'s hit-testing — this crate
//! just needs a different, coarser tolerance for display
//! (ADR 0003 §7: "a separate, coarser display tolerance in
//! `vecmanf-render-core` must not be reused" for hit-testing).

use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, StrokeOptions, StrokeTessellator, StrokeVertex, StrokeVertexConstructor,
    VertexBuffers,
};
use vecmanf_document_core::{AnchorSnapshot, Point, Vec2};

use crate::color::RgbaColor;
use crate::primitives::{DrawList, Vertex};

/// `lyon`'s own display tolerance for approximating a curve with line
/// segments during stroking — unrelated to, and coarser than,
/// `vecmanf-geometry-core`'s hit-testing [`vecmanf_document_core::Tolerance`]
/// (ADR 0003 §7).
const DISPLAY_TOLERANCE_MM: f32 = 0.05;

struct WithColor(RgbaColor);

impl StrokeVertexConstructor<Vertex> for WithColor {
    fn new_vertex(&mut self, vertex: StrokeVertex) -> Vertex {
        let p = vertex.position();
        Vertex {
            position: Point::new(f64::from(p.x), f64::from(p.y)),
            color: self.0,
        }
    }
}

fn to_lyon(point_mm: Point) -> lyon::math::Point {
    #[allow(clippy::cast_possible_truncation)]
    point(point_mm.x as f32, point_mm.y as f32)
}

fn cubic_bezier_to(
    builder: &mut lyon::path::path::Builder,
    from: &AnchorSnapshot,
    to: &AnchorSnapshot,
) {
    let c1 = from.point.translated(from.handle_out);
    let c2 = to.point.translated(to.handle_in);
    builder.cubic_bezier_to(to_lyon(c1), to_lyon(c2), to_lyon(to.point));
}

/// Builds one whole path's geometry (every anchor, in traversal order,
/// closing the loop with its own curved handles rather than `lyon`'s
/// straight-line `close()` when `closed` is set). `None` when there are
/// fewer than two anchors — nothing to stroke.
fn build_path(anchors: &[AnchorSnapshot], closed: bool) -> Option<Path> {
    if anchors.len() < 2 {
        return None;
    }
    let mut builder = Path::builder();
    builder.begin(to_lyon(anchors[0].point));
    for i in 1..anchors.len() {
        cubic_bezier_to(&mut builder, &anchors[i - 1], &anchors[i]);
    }
    if closed {
        // invariant: `anchors.len() >= 2` was checked above, so both
        // `last()` and `[0]` are present.
        #[allow(clippy::unwrap_used)]
        let last = anchors.last().unwrap();
        cubic_bezier_to(&mut builder, last, &anchors[0]);
        builder.end(true);
    } else {
        builder.end(false);
    }
    Some(builder.build())
}

fn stroke(path: &Path, width_mm: f64, color: RgbaColor) -> DrawList {
    let mut buffers: VertexBuffers<Vertex, u16> = VertexBuffers::new();
    let mut tessellator = StrokeTessellator::new();
    #[allow(clippy::cast_possible_truncation)]
    let options = StrokeOptions::default()
        .with_line_width(width_mm as f32)
        .with_tolerance(DISPLAY_TOLERANCE_MM);
    let mut output = BuffersBuilder::new(&mut buffers, WithColor(color));
    if tessellator
        .tessellate_path(path, &options, &mut output)
        .is_err()
    {
        // Degrades to "nothing drawn" rather than panicking: this is
        // rendering, not a correctness-critical write, and a malformed
        // path (e.g. NaN coordinates from upstream corruption) should
        // not be able to crash the editor.
        return DrawList::default();
    }

    let mut list = DrawList::default();
    let (triangles, _remainder) = buffers.indices.as_chunks::<3>();
    for &[a, b, c] in triangles {
        list.triangles.push(buffers.vertices[usize::from(a)]);
        list.triangles.push(buffers.vertices[usize::from(b)]);
        list.triangles.push(buffers.vertices[usize::from(c)]);
    }
    list
}

/// Tessellates a whole path's stroke (acceptance criterion 6: this
/// slice's one placeholder stroke, regardless of node types or
/// curvature).
#[must_use]
pub fn path_stroke(
    anchors: &[AnchorSnapshot],
    closed: bool,
    width_mm: f64,
    color: RgbaColor,
) -> DrawList {
    build_path(anchors, closed)
        .map_or_else(DrawList::default, |path| stroke(&path, width_mm, color))
}

/// Tessellates one segment's stroke in isolation — the selected-segment
/// overlay (acceptance criterion 14), drawn on top of the path's own
/// stroke rather than replacing it.
#[must_use]
pub fn segment_stroke(
    start: Point,
    start_handle_out: Vec2,
    end_handle_in: Vec2,
    end: Point,
    width_mm: f64,
    color: RgbaColor,
) -> DrawList {
    let mut builder = Path::builder();
    builder.begin(to_lyon(start));
    let c1 = start.translated(start_handle_out);
    let c2 = end.translated(end_handle_in);
    builder.cubic_bezier_to(to_lyon(c1), to_lyon(c2), to_lyon(end));
    builder.end(false);
    stroke(&builder.build(), width_mm, color)
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{AnchorId, AnchorKind, NewAnchor};

    use super::*;

    fn corner(id: u64, x: f64, y: f64) -> AnchorSnapshot {
        NewAnchor::corner(AnchorId::new(1, id), Point::new(x, y))
    }

    #[test]
    fn a_two_node_open_path_produces_a_non_empty_stroke() {
        let anchors = vec![corner(1, 0.0, 0.0), corner(2, 10.0, 0.0)];
        let list = path_stroke(&anchors, false, 0.25, RgbaColor::BLACK);
        assert!(!list.triangles.is_empty());
        assert_eq!(
            list.triangles.len() % 3,
            0,
            "a flat triangle list is always a multiple of 3"
        );
    }

    #[test]
    fn fewer_than_two_anchors_produces_nothing() {
        let anchors = vec![corner(1, 0.0, 0.0)];
        let list = path_stroke(&anchors, false, 0.25, RgbaColor::BLACK);
        assert!(list.triangles.is_empty());
    }

    #[test]
    fn a_closed_path_strokes_the_wraparound_segment_too() {
        let anchors = vec![
            corner(1, 0.0, 0.0),
            corner(2, 10.0, 0.0),
            corner(3, 5.0, 10.0),
        ];
        let open = path_stroke(&anchors, false, 0.25, RgbaColor::BLACK);
        let closed = path_stroke(&anchors, true, 0.25, RgbaColor::BLACK);
        assert!(
            closed.triangle_count() > open.triangle_count(),
            "closing adds the third segment's geometry"
        );
    }

    #[test]
    fn stroked_geometry_stays_near_the_control_polygon() {
        let anchors = vec![
            NewAnchor {
                id: AnchorId::new(1, 1),
                point: Point::new(0.0, 0.0),
                handle_in: Vec2::ZERO,
                handle_out: Vec2::new(0.0, 5.0),
                kind: AnchorKind::Smooth,
            },
            corner(2, 10.0, 0.0),
        ];
        let list = path_stroke(&anchors, false, 0.25, RgbaColor::BLACK);
        for vertex in &list.triangles {
            // A generous bound: the curve plus half the stroke width
            // must stay well within the control polygon's bounding box
            // padded by a few mm.
            assert!(vertex.position.x >= -1.0 && vertex.position.x <= 11.0);
            assert!(vertex.position.y >= -1.0 && vertex.position.y <= 6.0);
        }
    }

    #[test]
    fn segment_stroke_produces_geometry_for_a_single_curve() {
        let list = segment_stroke(
            Point::new(0.0, 0.0),
            Vec2::new(0.0, 5.0),
            Vec2::new(0.0, 5.0),
            Point::new(10.0, 0.0),
            2.0,
            RgbaColor::opaque(0x2F, 0x6F, 0xEE),
        );
        assert!(!list.triangles.is_empty());
    }
}
