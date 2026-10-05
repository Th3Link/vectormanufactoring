//! A primitive shape's own draw-list geometry
//! (`specs/0003-primitive-shapes/specification.md`, acceptance criterion 16;
//! "Selection and hover convention for primitives"): its placeholder
//! stroke (reusing [`crate::stroke::path_stroke`] on
//! [`vecmanf_document_core::outline_of`]'s output, so AC16's identical
//! stroke holds by construction and AC17's "visually identical"
//! conversion holds exactly, not just within a tolerance — `adrs.md`),
//! its bounding-box selection/hover outline, and its shape handles.
//!
//! [`ShapeDecorationInput`] mirrors [`crate::DecorationInput`]'s own
//! reason for existing: this crate cannot read `vecmanf-ui-core`'s
//! selection or its handle-layout module directly (ADR 0011 §3), so
//! `vecmanf-editor-wasm` builds this input from them each frame.

use vecmanf_document_core::{NodeId, Point, PrimitiveSnapshot, Shape, ViewTransform, outline_of};

use crate::color::RgbaColor;
use crate::glyphs::{self, DrawList};
use crate::stroke;
use crate::theme;

/// A shape handle's kind, duplicated (narrowly) from
/// `vecmanf_ui_core::handle_layout::HandleKind` — this crate cannot
/// depend on `vecmanf-ui-core` (ADR 0011 §3), and the only thing
/// rendering needs to know is "does this one get the corner-radius
/// guide line", everything else is handled by `position`/`draggable`
/// alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeHandleKind {
    /// Resizes the shape.
    Resize,
    /// The rectangle's one draggable corner-radius handle — gets the
    /// dashed connecting guide (acceptance criteria 4, 5, 6).
    CornerRadius,
    /// A rectangle's other three corners, once rounded — display-only.
    CornerRadiusEcho,
    /// A star's inner-radius handle (acceptance criterion 14).
    InnerRadius,
}

/// One shape handle to draw, for one primitive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderShapeHandle {
    /// What this handle does.
    pub kind: ShapeHandleKind,
    /// Where it is, in document space.
    pub position: Point,
    /// Whether it can be dragged at all (a `CornerRadiusEcho` cannot).
    pub draggable: bool,
    /// Whether it is currently being dragged — filled solid
    /// `--accent` while so (`docs/design-system.md`).
    pub dragging: bool,
}

/// What to decorate, built from `vecmanf-ui-core`'s
/// `vecmanf_ui_core::ObjectSelection` and `handle_layout` module by whoever owns both
/// it and this crate (`vecmanf-editor-wasm`).
#[derive(Debug, Clone, Default)]
pub struct ShapeDecorationInput {
    /// Primitives shown selected: bounding box plus handles
    /// (acceptance criterion 22 can select more than one at once).
    pub selected: Vec<NodeId>,
    /// A primitive currently hovered, not yet selected — bounding box
    /// only, `--accent-hover`, no handles.
    pub hovered: Option<NodeId>,
    /// Each selected primitive's own handles, keyed by id. Only
    /// consulted for an id also present in `selected`.
    pub handles: Vec<(NodeId, Vec<RenderShapeHandle>)>,
}

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// A dummy identity used only to satisfy
/// [`crate::stroke::path_stroke`]'s `AnchorSnapshot` parameter type — an
/// outline anchor has no real id (`vecmanf_document_core::OutlineAnchor`
/// is deliberately id-less), and nothing here ever reads it back.
const UNUSED_ANCHOR_ID: vecmanf_document_core::AnchorId =
    vecmanf_document_core::AnchorId::new(0, 0);

fn outline_to_anchors(
    outline: &[vecmanf_document_core::OutlineAnchor],
) -> Vec<vecmanf_document_core::AnchorSnapshot> {
    outline
        .iter()
        .map(|a| vecmanf_document_core::NewAnchor {
            id: UNUSED_ANCHOR_ID,
            point: a.point,
            handle_in: a.handle_in,
            handle_out: a.handle_out,
            kind: a.kind,
        })
        .collect()
}

/// A primitive's own placeholder stroke (acceptance criterion 16):
/// identical weight/color/no-fill to a path's, built from the exact
/// same outline "object to path" would convert. `view` feeds the
/// screen-space display tolerance and the minimum on-screen stroke width
/// (`specs/0004-canvas-navigation-and-selection/adrs.md`), the same way
/// [`crate::build_draw_list`] does for a path's own stroke.
fn primitive_stroke(snapshot: &PrimitiveSnapshot, view: ViewTransform) -> DrawList {
    let outline = outline_of(&snapshot.shape);
    let anchors = outline_to_anchors(&outline);
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    let min_width_mm = screen_px_to_mm(view, theme::MIN_DISPLAY_STROKE_WIDTH_PX);
    stroke::path_stroke(
        &anchors,
        true,
        snapshot.stroke_width.as_mm().max(min_width_mm),
        snapshot.stroke.into(),
        tolerance_mm,
    )
}

/// A shape tool's live, uncommitted create/resize/radius/ratio preview
/// (`specs/0003-primitive-shapes/specification.md`'s "Live creation
/// feedback": "a maker dragging out a rectangle sees a rectangle
/// updating live, not a placeholder box that snaps to shape on
/// release" — ux-engineer review: this applies to every drag kind, not
/// only create). Hollow, `--accent` outline, screen-space-constant
/// stroke weight — distinct from this module's own placeholder stroke
/// (acceptance criterion 16's black, document-mm-weighted one), since
/// nothing has committed yet. `vecmanf-ui-core`'s own shape tools
/// decide *whether* there is a live shape to preview right now
/// (`vecmanf_ui_core::LiveShape`); this function only draws the
/// [`Shape`] it is given.
#[must_use]
pub fn build_shape_live_preview(shape: &Shape, view: ViewTransform) -> DrawList {
    let outline = outline_of(shape);
    let anchors = outline_to_anchors(&outline);
    let width = screen_px_to_mm(view, theme::LIVE_PREVIEW_STROKE_PX);
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    stroke::path_stroke(&anchors, true, width, theme::ACCENT, tolerance_mm)
}

fn bounding_box_outline(shape: &Shape, width_mm: f64, color: RgbaColor) -> DrawList {
    // The frame-bounds rule itself moved to `vecmanf-document-core`
    // (`specs/0004-canvas-navigation-and-selection/adrs.md`: "the
    // primitive box... moves to `document-core` as plain arithmetic on
    // its own types") so `vecmanf-ui-core`'s `object_bounds` can share
    // it for the Select tool's own bounding box; the rectangle-outline
    // *drawing* itself is `glyphs::box_outline`, shared with
    // `select_decoration.rs` (architect review: the four-`thick_line`
    // loop was duplicated between the two).
    let (min, max) = vecmanf_document_core::shape_frame_bounds(shape);
    glyphs::box_outline(min, max, width_mm, color)
}

/// A hollow shape-handle glyph: `--accent` outline, white idle fill or
/// solid `--accent` fill while being dragged (`docs/design-system.md`).
fn shape_handle_glyph(center: Point, size_mm: f64, outline_mm: f64, dragging: bool) -> DrawList {
    let mut list = glyphs::square(center, size_mm, theme::SHAPE_HANDLE_STROKE);
    let fill = if dragging {
        theme::SHAPE_HANDLE_STROKE
    } else {
        RgbaColor::WHITE
    };
    list.extend(glyphs::square(
        center,
        (size_mm - 2.0 * outline_mm).max(0.0),
        fill,
    ));
    list
}

/// The corner-radius handle's dashed connecting guide (`docs/design-
/// system.md`: "dashed `--accent-hover` line — visually distinct from
/// the node tool's *solid* handle line"). A flagged simplification like
/// `pen_preview.rs`'s rubber-band line: built from short solid segments
/// rather than a real stippled-stroke primitive, since this crate has
/// none yet.
fn dashed_guide(
    a: Point,
    b: Point,
    width_mm: f64,
    color: RgbaColor,
    dash_mm: f64,
    gap_mm: f64,
) -> DrawList {
    let mut list = DrawList::default();
    let total = a.vector_to(b).length();
    if total <= f64::EPSILON || dash_mm <= 0.0 {
        return list;
    }
    let direction = a.vector_to(b).normalized_to(1.0);
    let step = dash_mm + gap_mm;
    let mut travelled = 0.0;
    while travelled < total {
        let dash_end = (travelled + dash_mm).min(total);
        let start = a.translated(direction.scaled(travelled));
        let end = a.translated(direction.scaled(dash_end));
        list.extend(glyphs::thick_line(start, end, width_mm, color));
        travelled += step;
    }
    list
}

/// A rectangle's own top-right corner — the corner-radius handle's
/// guide always runs from here (`specification.md`'s "Placement").
fn rect_top_right_corner(shape: &Shape) -> Option<Point> {
    match *shape {
        Shape::Rect { bounds, .. } => Some(Point::new(
            bounds.origin.x + bounds.width.as_mm(),
            bounds.origin.y,
        )),
        Shape::Ellipse { .. } | Shape::Polygon { .. } | Shape::Star { .. } => None,
    }
}

/// Builds every primitive's own stroke, plus selection/hover/handle
/// decorations, from `input`.
#[must_use]
pub fn build(
    primitives: &[PrimitiveSnapshot],
    view: ViewTransform,
    input: &ShapeDecorationInput,
) -> DrawList {
    let mut list = DrawList::default();
    let outline_width = screen_px_to_mm(view, theme::BOUNDING_BOX_OUTLINE_PX);
    let handle_size = screen_px_to_mm(view, theme::SHAPE_HANDLE_SIZE_PX);
    let handle_outline = screen_px_to_mm(view, theme::SHAPE_HANDLE_OUTLINE_PX);
    let dash = screen_px_to_mm(view, theme::GUIDE_DASH_PX);
    let gap = screen_px_to_mm(view, theme::GUIDE_GAP_PX);
    let guide_width = screen_px_to_mm(view, 1.0);

    for snapshot in primitives {
        list.extend(primitive_stroke(snapshot, view));
        let selected = input.selected.contains(&snapshot.id);
        let hovered = input.hovered == Some(snapshot.id);
        if selected {
            list.extend(bounding_box_outline(
                &snapshot.shape,
                outline_width,
                theme::ACCENT,
            ));
        } else if hovered {
            list.extend(bounding_box_outline(
                &snapshot.shape,
                outline_width,
                theme::ACCENT_HOVER,
            ));
        }
    }

    for (id, handles) in &input.handles {
        if !input.selected.contains(id) {
            continue;
        }
        let corner = primitives
            .iter()
            .find(|p| p.id == *id)
            .and_then(|p| rect_top_right_corner(&p.shape));
        for handle in handles {
            if handle.kind == ShapeHandleKind::CornerRadius
                && let Some(corner) = corner
            {
                list.extend(dashed_guide(
                    corner,
                    handle.position,
                    guide_width,
                    theme::ACCENT_HOVER,
                    dash,
                    gap,
                ));
            }
            list.extend(shape_handle_glyph(
                handle.position,
                handle_size,
                handle_outline,
                handle.dragging,
            ));
        }
    }

    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{Document, Length, RectBounds};

    fn rect_snapshot(document: &Document) -> (NodeId, PrimitiveSnapshot) {
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        (id, document.primitive(id).expect("exists"))
    }

    /// ux-engineer review item 1: a live preview draws a non-empty
    /// outline for an in-progress shape, independent of any committed
    /// primitive.
    #[test]
    fn build_shape_live_preview_draws_a_rect_outline() {
        let shape = Shape::Rect {
            bounds: RectBounds {
                origin: Point::new(0.0, 0.0),
                width: Length::from_mm(10.0),
                height: Length::from_mm(10.0),
            },
            corner_radius: Length::from_mm(0.0),
        };
        let list = build_shape_live_preview(&shape, ViewTransform::identity());
        assert_ne!(list.triangles, Vec::<glyphs::Vertex>::new());
    }

    #[test]
    fn an_unselected_primitive_draws_only_its_stroke() {
        let document = Document::new(1);
        let (_, snapshot) = rect_snapshot(&document);
        let list = build(
            &[snapshot],
            ViewTransform::identity(),
            &ShapeDecorationInput::default(),
        );
        assert!(!list.triangles.is_empty(), "the stroke itself still draws");
    }

    #[test]
    fn a_selected_primitive_adds_bounding_box_and_handle_geometry() {
        let document = Document::new(1);
        let (id, snapshot) = rect_snapshot(&document);
        let without = build(
            &[snapshot],
            ViewTransform::identity(),
            &ShapeDecorationInput::default(),
        );

        let input = ShapeDecorationInput {
            selected: vec![id],
            hovered: None,
            handles: vec![(
                id,
                vec![RenderShapeHandle {
                    kind: ShapeHandleKind::CornerRadius,
                    position: Point::new(10.0, 0.0),
                    draggable: true,
                    dragging: false,
                }],
            )],
        };
        let with = build(&[snapshot], ViewTransform::identity(), &input);
        assert!(with.triangle_count() > without.triangle_count());
    }

    #[test]
    fn a_hovered_unselected_primitive_draws_a_bounding_box_but_no_handles() {
        let document = Document::new(1);
        let (id, snapshot) = rect_snapshot(&document);
        let without = build(
            &[snapshot],
            ViewTransform::identity(),
            &ShapeDecorationInput::default(),
        );
        let input = ShapeDecorationInput {
            selected: vec![],
            hovered: Some(id),
            handles: vec![],
        };
        let with = build(&[snapshot], ViewTransform::identity(), &input);
        assert!(with.triangle_count() > without.triangle_count());
    }
}
