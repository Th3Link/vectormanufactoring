//! Node/handle/segment decoration geometry (acceptance criteria 7, 9, 10,
//! 14; `docs/design-system.md`'s node/handle visual convention).
//!
//! Built from a [`DecorationInput`] of [`AnchorId`]s and flags —
//! `vecmanf-editor-wasm` builds that input from `vecmanf-ui-core`'s
//! selection, since this crate cannot read `vecmanf-ui-core` directly
//! (`specs/path-node-editing/adrs.md`, "the path/node crate boundary").

use vecmanf_document_core::{AnchorKind, HandleSlot, NodeId, PathSnapshot, Vec2, ViewTransform};

use crate::color::RgbaColor;
use crate::primitives::{self, DrawList};
use crate::theme;

/// One node or handle currently under the pointer, for the hover ring
/// (`docs/design-system.md`'s UX notes — not itself a numbered
/// acceptance criterion, but the stated baseline every later tool's
/// selection feedback follows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hovered {
    /// A node is hovered.
    Node(NodeId, vecmanf_document_core::AnchorId),
    /// One of a node's handles is hovered.
    Handle(NodeId, vecmanf_document_core::AnchorId, HandleSlot),
}

/// What to decorate, built from `vecmanf-ui-core`'s selection by whoever
/// owns both it and this crate (`vecmanf-editor-wasm`).
#[derive(Debug, Clone, Default)]
pub struct DecorationInput {
    /// Nodes shown as selected, each tagged with its own path (acceptance
    /// criteria 7, 10).
    pub selected_nodes: Vec<(NodeId, vecmanf_document_core::AnchorId)>,
    /// The one segment shown as selected, if any (acceptance criterion
    /// 14).
    pub selected_segment: Option<(
        NodeId,
        vecmanf_document_core::AnchorId,
        vecmanf_document_core::AnchorId,
    )>,
    /// What is currently under the pointer, if anything.
    pub hovered: Option<Hovered>,
}

impl DecorationInput {
    fn is_node_selected(&self, path: NodeId, anchor: vecmanf_document_core::AnchorId) -> bool {
        self.selected_nodes
            .iter()
            .any(|&(p, a)| p == path && a == anchor)
    }
}

fn screen_px_to_mm(view: ViewTransform, px: f64) -> f64 {
    px / view.scale()
}

/// Builds every node/handle decoration across every path.
#[must_use]
pub fn build(paths: &[PathSnapshot], view: ViewTransform, input: &DecorationInput) -> DrawList {
    let node_size = screen_px_to_mm(view, theme::NODE_SIZE_PX);
    let node_outline = screen_px_to_mm(view, theme::NODE_OUTLINE_PX);
    let handle_diameter = screen_px_to_mm(view, theme::HANDLE_DIAMETER_PX);
    let handle_line_width = screen_px_to_mm(view, theme::HANDLE_LINE_WIDTH_PX);
    let hover_ring_diameter = screen_px_to_mm(view, theme::HOVER_RING_DIAMETER_PX);
    let hover_ring_thickness = screen_px_to_mm(view, 1.0);

    let mut list = DrawList::default();
    for snapshot in paths {
        for anchor in &snapshot.anchors {
            let selected = input.is_node_selected(snapshot.id, anchor.id);

            if selected {
                for (slot, handle) in [
                    (HandleSlot::Out, anchor.handle_out),
                    (HandleSlot::In, anchor.handle_in),
                ] {
                    if handle == Vec2::ZERO {
                        continue;
                    }
                    let endpoint = anchor.point.translated(handle);
                    list.extend(primitives::thick_line(
                        anchor.point,
                        endpoint,
                        handle_line_width,
                        theme::ACCENT,
                    ));
                    if input.hovered == Some(Hovered::Handle(snapshot.id, anchor.id, slot)) {
                        list.extend(primitives::ring(
                            endpoint,
                            hover_ring_diameter,
                            hover_ring_thickness,
                            theme::ACCENT_HOVER,
                        ));
                    }
                    // Idle handle style: accent outline, white fill
                    // (`docs/design-system.md`).
                    list.extend(primitives::circle(endpoint, handle_diameter, theme::ACCENT));
                    list.extend(primitives::circle(
                        endpoint,
                        (handle_diameter - 2.0 * hover_ring_thickness).max(0.0),
                        RgbaColor::WHITE,
                    ));
                }
            }

            let glyph = match anchor.kind {
                AnchorKind::Corner => primitives::square,
                AnchorKind::Smooth => primitives::diamond,
            };
            if selected {
                list.extend(glyph(anchor.point, node_size, theme::ACCENT));
            } else {
                list.extend(glyph(anchor.point, node_size, theme::NODE_STROKE));
                list.extend(glyph(
                    anchor.point,
                    (node_size - 2.0 * node_outline).max(0.0),
                    RgbaColor::WHITE,
                ));
            }

            if input.hovered == Some(Hovered::Node(snapshot.id, anchor.id)) {
                list.extend(primitives::ring(
                    anchor.point,
                    hover_ring_diameter,
                    hover_ring_thickness,
                    theme::ACCENT_HOVER,
                ));
            }
        }
    }

    if let Some(overlay) = selected_segment_overlay(paths, view, input.selected_segment) {
        list.extend(overlay);
    }

    list
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
        vecmanf_document_core::AnchorId,
        vecmanf_document_core::AnchorId,
    )>,
) -> Option<DrawList> {
    let (path, start, end) = selected_segment?;
    let snapshot = paths.iter().find(|p| p.id == path)?;
    let start_anchor = snapshot.anchors.iter().find(|a| a.id == start)?;
    let end_anchor = snapshot.anchors.iter().find(|a| a.id == end)?;
    let overlay_width =
        snapshot.stroke_width.as_mm() + screen_px_to_mm(view, theme::SEGMENT_OVERLAY_EXTRA_PX);
    Some(crate::stroke::segment_stroke(
        start_anchor.point,
        start_anchor.handle_out,
        end_anchor.handle_in,
        end_anchor.point,
        overlay_width,
        theme::ACCENT,
    ))
}

#[cfg(test)]
mod tests {
    use vecmanf_document_core::{AnchorId, Document, NewAnchor, Point};

    use super::*;

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

    #[test]
    fn an_unselected_node_draws_a_glyph_but_no_handles() {
        let (document, path, _a, _b) = two_node_path();
        let paths = vec![document.path(path).expect("exists")];
        let input = DecorationInput::default();
        let list = build(&paths, ViewTransform::identity(), &input);
        // Two node glyphs, each as outline+fill (2 quads = 4 triangles each).
        assert_eq!(list.triangle_count(), 8);
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
                    kind: AnchorKind::Smooth,
                },
                NewAnchor::corner(b, Point::new(20.0, 0.0)),
            ],
            false,
        );
        let paths = vec![document.path(path).expect("exists")];
        let unselected = DecorationInput::default();
        let without = build(&paths, ViewTransform::identity(), &unselected);

        let input = DecorationInput {
            selected_nodes: vec![(path, a)],
            ..DecorationInput::default()
        };
        let with = build(&paths, ViewTransform::identity(), &input);

        assert!(with.triangle_count() > without.triangle_count());
    }

    #[test]
    fn a_selected_segment_adds_overlay_geometry() {
        let (document, path, a, b) = two_node_path();
        let paths = vec![document.path(path).expect("exists")];
        let without = build(
            &paths,
            ViewTransform::identity(),
            &DecorationInput::default(),
        );
        let input = DecorationInput {
            selected_segment: Some((path, a, b)),
            ..DecorationInput::default()
        };
        let with = build(&paths, ViewTransform::identity(), &input);
        assert!(with.triangle_count() > without.triangle_count());
    }

    #[test]
    fn decoration_sizes_shrink_in_document_space_as_zoom_increases() {
        let (document, path, _a, _b) = two_node_path();
        let paths = vec![document.path(path).expect("exists")];
        let input = DecorationInput::default();

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
}
