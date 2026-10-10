//! Node/handle/segment decoration geometry (acceptance criteria 7, 9, 10,
//! 14; `docs/design-system.md`'s node/handle visual convention).
//!
//! Built from a [`DecorationInput`] of [`AnchorId`]s and flags —
//! `curvyo-editor-wasm` builds that input from `curvyo-ui-core`'s
//! selection, since this crate cannot read `curvyo-ui-core` directly
//! (`specs/0002-path-node-editing/adrs.md`, "the path/node crate boundary").

use curvyo_document_core::{AnchorKind, HandleSlot, NodeId, PathSnapshot, Vec2, ViewTransform};

use crate::color::RgbaColor;
use crate::glyphs::{self, DrawList};
use crate::theme;

/// One node or handle currently under the pointer, for the hover ring
/// (`docs/design-system.md`'s UX notes — not itself a numbered
/// acceptance criterion, but the stated baseline every later tool's
/// selection feedback follows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hovered {
    /// A node is hovered.
    Node(NodeId, curvyo_document_core::AnchorId),
    /// One of a node's handles is hovered.
    Handle(NodeId, curvyo_document_core::AnchorId, HandleSlot),
    /// A segment a press would bend is hovered (`0031-segment-drag-bending` criterion 15): its
    /// path and its two end anchors in path order.
    Segment(
        NodeId,
        curvyo_document_core::AnchorId,
        curvyo_document_core::AnchorId,
    ),
}

/// What to decorate, built from `curvyo-ui-core`'s selection by whoever
/// owns both it and this crate (`curvyo-editor-wasm`).
#[derive(Debug, Clone, Default)]
pub struct DecorationInput {
    /// Whether to draw path nodes (and the handles of selected ones) at
    /// all. Only the Node tool shows them — `docs/design-system.md`: no
    /// path nodes in the Select tool or any other tool.
    pub show_nodes: bool,
    /// Nodes shown as selected, each tagged with its own path (acceptance
    /// criteria 7, 10).
    pub selected_nodes: Vec<(NodeId, curvyo_document_core::AnchorId)>,
    /// The one segment shown as selected, if any (acceptance criterion
    /// 14).
    pub selected_segment: Option<(
        NodeId,
        curvyo_document_core::AnchorId,
        curvyo_document_core::AnchorId,
    )>,
    /// What is currently under the pointer, if anything.
    pub hovered: Option<Hovered>,
    /// Nodes whose handles are drawn in the idle look without the node being selected: the two
    /// ends of a segment being bent (`0031` criterion 16).
    pub handle_nodes: Vec<(NodeId, curvyo_document_core::AnchorId)>,
}

impl DecorationInput {
    fn is_node_selected(&self, path: NodeId, anchor: curvyo_document_core::AnchorId) -> bool {
        self.selected_nodes
            .iter()
            .any(|&(p, a)| p == path && a == anchor)
    }

    fn shows_handles(&self, path: NodeId, anchor: curvyo_document_core::AnchorId) -> bool {
        self.is_node_selected(path, anchor)
            || self
                .handle_nodes
                .iter()
                .any(|&(p, a)| p == path && a == anchor)
    }
}

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// The screen-pixel sizes of the node decorations at one zoom, in millimetres.
struct Sizes {
    node: f64,
    node_outline: f64,
    handle_diameter: f64,
    handle_line_width: f64,
    hover_ring_diameter: f64,
    handle_hover_ring_diameter: f64,
    ring_thickness: f64,
}

impl Sizes {
    fn at(view: ViewTransform) -> Self {
        Self {
            node: screen_px_to_mm(view, theme::NODE_SIZE_PX),
            node_outline: screen_px_to_mm(view, theme::NODE_OUTLINE_PX),
            handle_diameter: screen_px_to_mm(view, theme::HANDLE_DIAMETER_PX),
            handle_line_width: screen_px_to_mm(view, theme::HANDLE_LINE_WIDTH_PX),
            hover_ring_diameter: screen_px_to_mm(view, theme::HOVER_RING_DIAMETER_PX),
            handle_hover_ring_diameter: screen_px_to_mm(view, theme::HANDLE_HOVER_RING_DIAMETER_PX),
            ring_thickness: screen_px_to_mm(view, 1.0),
        }
    }
}

/// Builds every node/handle decoration across every path.
#[must_use]
pub fn build(paths: &[PathSnapshot], view: ViewTransform, input: &DecorationInput) -> DrawList {
    let sizes = Sizes::at(view);
    let mut list = DrawList::default();
    let drawn_paths = if input.show_nodes { paths } else { &[] };
    // The hover band lies over the path's stroke and under every node and handle.
    if input.show_nodes
        && let Some(band) = hovered_segment_band(paths, view, input)
    {
        list.extend(band);
    }
    // Unselected nodes first, then selected ones: where two nodes lie on the
    // same spot (after a Split) the selected glyph is drawn above the other
    // and its accent fill stays visible (`specs/0010-edit-interaction-polish/`
    // criterion 50).
    for draw_selected in [false, true] {
        for path in drawn_paths {
            for anchor in &path.anchors {
                let selected = input.is_node_selected(path.id, anchor.id);
                if selected == draw_selected {
                    push_node(&mut list, &sizes, input, path.id, anchor, selected);
                }
            }
        }
    }

    if let Some(overlay) = selected_segment_overlay(paths, view, input.selected_segment) {
        list.extend(overlay);
    }

    list
}

/// One node: for a selected one its handles first, then its glyph, then the
/// hover ring.
fn push_node(
    list: &mut DrawList,
    sizes: &Sizes,
    input: &DecorationInput,
    path: NodeId,
    anchor: &curvyo_document_core::AnchorSnapshot,
    selected: bool,
) {
    if input.shows_handles(path, anchor.id) {
        for (slot, handle) in [
            (HandleSlot::Out, anchor.handle_out),
            (HandleSlot::In, anchor.handle_in),
        ] {
            if handle == Vec2::ZERO {
                continue;
            }
            let endpoint = anchor.point.translated(handle);
            list.extend(glyphs::cased(
                sizes.handle_line_width,
                theme::ACCENT,
                theme::SELECTION_CASING,
                |width, color| glyphs::thick_line(anchor.point, endpoint, width, color),
            ));
            // Idle handle style: accent outline, white fill
            // (`docs/design-system.md`).
            list.extend(glyphs::circle(
                endpoint,
                sizes.handle_diameter,
                theme::ACCENT,
            ));
            list.extend(glyphs::circle(
                endpoint,
                (sizes.handle_diameter - 2.0 * sizes.ring_thickness).max(0.0),
                RgbaColor::WHITE,
            ));
            // Drawn *after* the handle's own glyph, and sized
            // from it (`theme::HANDLE_HOVER_RING_DIAMETER_PX`),
            // not the shared node-ring token — either alone would
            // keep the ring visible once `HANDLE_DIAMETER_PX`
            // doubled past the old shared ring size, but a
            // smaller glyph drawn on top of a wider ring is the
            // only ordering that reads as "a ring around a
            // glyph" regardless of their relative sizes, so both
            // are kept (`theme::HANDLE_HOVER_RING_DIAMETER_PX`'s
            // own doc comment).
            if input.hovered == Some(Hovered::Handle(path, anchor.id, slot)) {
                list.extend(glyphs::ring(
                    endpoint,
                    sizes.handle_hover_ring_diameter,
                    sizes.ring_thickness,
                    theme::ACCENT_HOVER,
                ));
            }
        }
    }

    let glyph = match anchor.kind {
        AnchorKind::Corner => glyphs::square,
        AnchorKind::Symmetric => glyphs::diamond,
        AnchorKind::Asymmetric => glyphs::triangle,
    };
    if selected {
        list.extend(glyph(anchor.point, sizes.node, theme::ACCENT));
    } else {
        list.extend(glyph(anchor.point, sizes.node, theme::NODE_STROKE));
        list.extend(glyph(
            anchor.point,
            (sizes.node - 2.0 * sizes.node_outline).max(0.0),
            RgbaColor::WHITE,
        ));
    }

    if input.hovered == Some(Hovered::Node(path, anchor.id)) {
        list.extend(glyphs::ring(
            anchor.point,
            sizes.hover_ring_diameter,
            sizes.ring_thickness,
            theme::ACCENT_HOVER,
        ));
    }
}

/// The hover band of the hovered segment (`0031` criterion 15), or `None` when no segment is
/// hovered, the hovered one is the selected one (it keeps the selected look) or it no longer
/// resolves.
fn hovered_segment_band(
    paths: &[PathSnapshot],
    view: ViewTransform,
    input: &DecorationInput,
) -> Option<DrawList> {
    let Some(Hovered::Segment(path, start, end)) = input.hovered else {
        return None;
    };
    if input.selected_segment == Some((path, start, end)) {
        return None;
    }
    let snapshot = paths.iter().find(|p| p.id == path)?;
    let start_anchor = snapshot.anchors.iter().find(|a| a.id == start)?;
    let end_anchor = snapshot.anchors.iter().find(|a| a.id == end)?;
    Some(crate::stroke::segment_band(
        start_anchor.point,
        start_anchor.handle_out,
        end_anchor.handle_in,
        end_anchor.point,
        screen_px_to_mm(view, theme::SEGMENT_HOVER_WIDTH_PX),
        theme::SEGMENT_HOVER,
        screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX),
    ))
}

/// The selected-segment overlay (acceptance criterion 14), or `None`
/// when nothing is selected or the selection no longer resolves (ADR
/// 0009 §2: a stale selection degrades to "nothing to draw", not a
/// panic).
fn selected_segment_overlay(
    paths: &[PathSnapshot],
    view: ViewTransform,
    selected_segment: Option<(
        NodeId,
        curvyo_document_core::AnchorId,
        curvyo_document_core::AnchorId,
    )>,
) -> Option<DrawList> {
    let (path, start, end) = selected_segment?;
    let snapshot = paths.iter().find(|p| p.id == path)?;
    let start_anchor = snapshot.anchors.iter().find(|a| a.id == start)?;
    let end_anchor = snapshot.anchors.iter().find(|a| a.id == end)?;
    let min_width_mm = screen_px_to_mm(view, theme::MIN_DISPLAY_STROKE_WIDTH_PX);
    let overlay_width = snapshot.style.stroke.width.as_mm().max(min_width_mm)
        + screen_px_to_mm(view, theme::SEGMENT_OVERLAY_EXTRA_PX);
    let tolerance_mm = screen_px_to_mm(view, theme::DISPLAY_TOLERANCE_PX);
    Some(glyphs::cased(
        overlay_width,
        theme::ACCENT,
        theme::SELECTION_CASING,
        |width, color| {
            crate::stroke::segment_stroke(
                start_anchor.point,
                start_anchor.handle_out,
                end_anchor.handle_in,
                end_anchor.point,
                width,
                color,
                tolerance_mm,
            )
        },
    ))
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::{AnchorId, Document, NewAnchor, Point};

    use super::*;

    fn nodes_on() -> DecorationInput {
        DecorationInput {
            show_nodes: true,
            ..DecorationInput::default()
        }
    }

    fn two_node_path() -> (Document, NodeId, AnchorId, AnchorId) {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        (document, path, a, b)
    }

    /// Path nodes belong to the Node tool only: with `show_nodes` off no
    /// glyph is drawn for any anchor, selected or hovered or not.
    #[test]
    fn no_node_glyphs_are_drawn_unless_show_nodes_is_set() {
        let (document, path, a, _b) = two_node_path();
        let paths = vec![document.path(path).expect("exists")];
        let input = DecorationInput {
            selected_nodes: vec![(path, a)],
            hovered: Some(Hovered::Node(path, a)),
            ..DecorationInput::default()
        };
        let list = build(&paths, ViewTransform::identity(), &input);
        assert_eq!(list.triangle_count(), 0);
    }

    #[test]
    fn an_unselected_node_draws_a_glyph_but_no_handles() {
        let (document, path, _a, _b) = two_node_path();
        let paths = vec![document.path(path).expect("exists")];
        let input = nodes_on();
        let list = build(&paths, ViewTransform::identity(), &input);
        // Two node glyphs, each as outline+fill (2 quads = 4 triangles each).
        assert_eq!(list.triangle_count(), 8);
    }

    /// An Asymmetric node draws with the triangle glyph — one outline
    /// triangle plus one fill triangle, unlike Corner/Symmetric's
    /// four-sided (2-quad) glyphs.
    #[test]
    fn an_asymmetric_node_draws_the_triangle_glyph() {
        let document = Document::new(1);
        let a = AnchorId::new(1, 1);
        let b = AnchorId::new(1, 2);
        let path = document.create_path(
            &[
                NewAnchor {
                    id: a,
                    point: Point::new(0.0, 0.0),
                    handle_in: Vec2::ZERO,
                    handle_out: Vec2::ZERO,
                    kind: AnchorKind::Asymmetric,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let list = build(&paths, ViewTransform::identity(), &nodes_on());
        // A's own glyph is one outline triangle + one fill triangle (2
        // total); B's is a Corner square (2 quads = 4 triangles) — 6 in
        // all.
        assert_eq!(list.triangle_count(), 6);
    }

    #[test]
    fn a_selected_node_with_pulled_handles_draws_handle_geometry_too() {
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
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let unselected = nodes_on();
        let without = build(&paths, ViewTransform::identity(), &unselected);

        let input = DecorationInput {
            selected_nodes: vec![(path, a)],
            ..nodes_on()
        };
        let with = build(&paths, ViewTransform::identity(), &input);

        assert!(with.triangle_count() > without.triangle_count());
    }

    /// The bug this run fixes: once `theme::HANDLE_DIAMETER_PX` doubled
    /// past the old shared hover-ring size, a hovered handle's ring drew
    /// fully behind (and so fully hidden by) the handle's own opaque
    /// fill — the same triangle-count-grows assertion the other hover
    /// tests use could not have caught that (geometry was still being
    /// *emitted*, just invisibly, under the glyph). This test instead
    /// checks the actual pixel footprint: at least one ring vertex must
    /// sit strictly outside the handle glyph's own radius, proving the
    /// ring extends past the glyph rather than nesting entirely inside
    /// it.
    #[test]
    fn a_hovered_handles_ring_extends_past_the_handles_own_glyph() {
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
                    kind: AnchorKind::Symmetric,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let input = DecorationInput {
            selected_nodes: vec![(path, a)],
            hovered: Some(Hovered::Handle(path, a, HandleSlot::Out)),
            ..nodes_on()
        };
        let view = ViewTransform::identity();
        let list = build(&paths, view, &input);

        // The handle endpoint sits at document (5.0, 0.0) (anchor at
        // (0,0) + handle_out (5,0)); the glyph's own radius at identity
        // view scale is theme::HANDLE_DIAMETER_PX / 2 document mm.
        //
        // Filtered to the ring's own color (`theme::ACCENT_HOVER`) —
        // architect review: the unfiltered whole-draw-list version of
        // this test passed even with the ring branch disabled entirely,
        // satisfied instead by unrelated geometry already farther than
        // the glyph radius (node A's own selected glyph/handle line,
        // and node B 15mm away) — nothing in that version actually
        // exercised the ring. No other glyph `build` draws uses this
        // color (every other handle/node glyph uses `theme::ACCENT`,
        // `theme::NODE_STROKE`, or white), so a vertex of this color
        // existing at all already proves the ring was drawn; requiring
        // one farther than the glyph radius additionally proves it is
        // not nested entirely inside (or behind) the glyph.
        let endpoint = Point::new(5.0, 0.0);
        let glyph_radius = theme::HANDLE_DIAMETER_PX / 2.0;
        let max_ring_vertex_distance = list
            .triangles
            .iter()
            .filter(|v| v.color == theme::ACCENT_HOVER)
            .map(|v| v.position.vector_to(endpoint).length())
            .fold(0.0_f64, f64::max);
        assert!(
            max_ring_vertex_distance > glyph_radius,
            "no ACCENT_HOVER-colored vertex drawn farther than the glyph's own radius \
             ({glyph_radius}mm) from the handle endpoint (farthest ring vertex found: \
             {max_ring_vertex_distance}mm, 0.0 if none at all) — the hover ring must exist and \
             extend past the glyph, not nest entirely inside (or behind) it"
        );
    }

    #[test]
    fn a_selected_segment_adds_overlay_geometry() {
        let (document, path, a, b) = two_node_path();
        let paths = vec![document.path(path).expect("exists")];
        let without = build(&paths, ViewTransform::identity(), &nodes_on());
        let input = DecorationInput {
            selected_segment: Some((path, a, b)),
            ..nodes_on()
        };
        let with = build(&paths, ViewTransform::identity(), &input);
        assert!(with.triangle_count() > without.triangle_count());
    }

    #[test]
    fn decoration_sizes_shrink_in_document_space_as_zoom_increases() {
        let (document, path, _a, _b) = two_node_path();
        let paths = vec![document.path(path).expect("exists")];
        let input = nodes_on();

        let zoomed_out = build(
            &paths,
            ViewTransform::new(1.0, Point::new(0.0, 0.0)),
            &input,
        );
        let zoomed_in = build(
            &paths,
            ViewTransform::new(10.0, Point::new(0.0, 0.0)),
            &input,
        );

        let extent = |list: &DrawList| -> f64 {
            list.triangles
                .iter()
                .map(|v| v.position.x.abs())
                .fold(0.0, f64::max)
        };
        assert!(
            extent(&zoomed_in) < extent(&zoomed_out),
            "higher zoom means a smaller document-space glyph"
        );
    }

    /// Two coincident end nodes on two paths, as a Split leaves them.
    fn coincident_ends() -> (Document, [(NodeId, AnchorId); 2]) {
        let document = Document::new(1);
        let (a, b) = (AnchorId::new(1, 1), AnchorId::new(1, 2));
        let first = document.create_path(
            &[
                NewAnchor::corner(AnchorId::new(1, 10), Point::new(-20.0, 0.0)),
                NewAnchor::corner(a, Point::new(0.0, 0.0)),
            ],
            false,
        );
        let second = document.create_path(
            &[
                NewAnchor::corner(b, Point::new(0.0, 0.0)),
                NewAnchor::corner(AnchorId::new(1, 11), Point::new(20.0, 0.0)),
            ],
            false,
        );
        (document, [(first, a), (second, b)])
    }

    /// `edit-interaction-polish` criterion 50: where two nodes lie on the same
    /// spot the selected one is drawn above the unselected one, whichever
    /// comes first in document order, so its accent fill is not hidden by the
    /// other glyph's white fill.
    #[test]
    fn a_selected_node_glyph_is_drawn_above_a_coincident_unselected_one() {
        let (document, ends) = coincident_ends();
        let paths: Vec<_> = document
            .object_ids()
            .into_iter()
            .filter_map(|id| document.path(id))
            .collect();
        for selected in ends {
            let input = DecorationInput {
                selected_nodes: vec![selected],
                ..nodes_on()
            };
            let list = build(&paths, ViewTransform::identity(), &input);
            let last_white = list
                .triangles
                .iter()
                .rposition(|v| v.color == RgbaColor::WHITE)
                .expect("an unselected glyph's white fill");
            let first_accent = list
                .triangles
                .iter()
                .position(|v| v.color == theme::ACCENT)
                .expect("the selected glyph");
            assert!(
                first_accent > last_white,
                "the selected node ({selected:?}) must be drawn after every unselected glyph"
            );
        }
    }

    /// `0007` criterion 40: a Bézier handle line and the selected-segment
    /// overlay each sit on a white casing three times as wide, under them.
    #[test]
    fn a_handle_line_and_the_segment_overlay_sit_on_a_white_casing() {
        let (document, path, a, b) = two_node_path();
        document
            .set_handle(path, a, HandleSlot::Out, Vec2::new(10.0, 10.0))
            .expect("pull a handle");
        let paths = vec![document.path(path).unwrap()];
        let list = build(
            &paths,
            ViewTransform::identity(),
            &DecorationInput {
                show_nodes: true,
                selected_nodes: vec![(path, a)],
                selected_segment: Some((path, a, b)),
                ..DecorationInput::default()
            },
        );
        assert!(
            list.triangles
                .iter()
                .any(|v| v.color == theme::SELECTION_CASING),
            "a white casing is drawn"
        );
        let first_accent = list
            .triangles
            .iter()
            .position(|v| v.color == theme::ACCENT)
            .unwrap();
        let first_casing = list
            .triangles
            .iter()
            .position(|v| v.color == theme::SELECTION_CASING)
            .unwrap();
        assert!(
            first_casing < first_accent,
            "a casing comes before the line it sits under"
        );
    }
}
