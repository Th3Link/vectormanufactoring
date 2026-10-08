//! Tessellating a closed outline's interior with the non-zero rule
//! (acceptance criterion 14), via `lyon`.

use curvyo_document_core::Point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, FillVertexConstructor,
    VertexBuffers,
};

use crate::color::RgbaColor;
use crate::glyphs::{DrawList, Vertex};
use crate::stroke::triangles_of;

struct WithColor(RgbaColor);

impl FillVertexConstructor<Vertex> for WithColor {
    fn new_vertex(&mut self, vertex: FillVertex) -> Vertex {
        let p = vertex.position();
        Vertex {
            position: Point::new(f64::from(p.x), f64::from(p.y)),
            color: self.0,
        }
    }
}

/// Tessellates `path`'s interior by the non-zero fill rule. Degrades to
/// "nothing drawn" on a malformed path, like the stroke does.
pub(crate) fn fill(path: &Path, color: RgbaColor, tolerance_mm: f64) -> DrawList {
    let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    #[allow(clippy::cast_possible_truncation)]
    let options = FillOptions::default()
        .with_fill_rule(FillRule::NonZero)
        .with_tolerance(tolerance_mm as f32);
    let mut output = BuffersBuilder::new(&mut buffers, WithColor(color));
    if tessellator
        .tessellate_path(path, &options, &mut output)
        .is_err()
    {
        return DrawList::default();
    }
    triangles_of(&buffers)
}
