//! "Object to path" (acceptance criteria 17, 21, 22): building the
//! resolved anchor geometry a
//! [`vecmanf_document_core::Document::convert_to_paths`] call needs,
//! for a whole primitive selection at once. Lives in this crate, not
//! `vecmanf-editor-wasm`'s facade (architect review: ADR 0001 §1's
//! "the facade has no editing logic of its own" rule) — `Session` just
//! calls [`build_primitive_conversions`] and passes the result straight
//! to `Document::convert_to_paths`.

use vecmanf_document_core::{Document, NewAnchor, NodeId, outline_of_rotated};

use crate::AnchorIdMinter;

/// For every id in `ids` that still names a live primitive, mints fresh
/// [`vecmanf_document_core::AnchorId`]s for its own, rotation-aware
/// outline ([`vecmanf_document_core::outline_of_rotated`]) and pairs
/// them with that id — exactly the shape `Document::convert_to_paths`
/// takes. A rotated primitive's anchors come out already baked into
/// document space (`specs/0005-object-transform/adrs.md`: "a path bakes
/// its rotation directly into its anchor points"); the primitive's own
/// `rotation` register itself is carried over unchanged by
/// `convert_to_paths` (it is never one of the keys "object to path"
/// strips), so the converted path's selection box keeps the original's
/// orientation (acceptance criterion 17 of `specs/0005-object-transform/
/// specification.md`) without this function doing anything extra. An id
/// that no longer resolves to a primitive (already deleted, or already
/// converted) is silently skipped, the same lazy-resolution stance
/// `vecmanf-ui-core`'s other multi-id operations take (ADR 0009 §2);
/// `convert_to_paths` itself still refuses the whole call on any id it
/// cannot resolve, so a genuinely stale id surviving to that point is
/// reported there, not swallowed twice.
#[must_use]
pub fn build_primitive_conversions(
    document: &Document,
    minter: &mut AnchorIdMinter,
    ids: &[NodeId],
) -> Vec<(NodeId, Vec<NewAnchor>)> {
    ids.iter()
        .filter_map(|&id| {
            let primitive = document.primitive(id)?;
            let outline = outline_of_rotated(&primitive.shape, primitive.rotation);
            let anchors: Vec<NewAnchor> = outline
                .into_iter()
                .map(|anchor| NewAnchor {
                    id: minter.mint(),
                    point: anchor.point,
                    handle_in: anchor.handle_in,
                    handle_out: anchor.handle_out,
                    kind: anchor.kind,
                })
                .collect();
            Some((id, anchors))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vecmanf_document_core::{AnchorKind, EllipseFrame, Length, Point, RectBounds};

    /// AC17: one primitive's conversion carries its outline's exact
    /// anchor count/kind, with freshly minted, distinct ids.
    #[test]
    fn build_primitive_conversions_mints_fresh_ids_for_one_rect() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let mut minter = AnchorIdMinter::new(1);
        let conversions = build_primitive_conversions(&document, &mut minter, &[id]);
        assert_eq!(conversions.len(), 1);
        let (converted_id, anchors) = &conversions[0];
        assert_eq!(*converted_id, id);
        assert_eq!(anchors.len(), 4);
        assert!(anchors.iter().all(|a| a.kind == AnchorKind::Corner));
        let mut ids: Vec<_> = anchors.iter().map(|a| a.id).collect();
        ids.sort_by_key(|id| id.to_hex());
        ids.dedup();
        assert_eq!(ids.len(), 4, "every minted anchor id is distinct");
    }

    /// AC22: multiple primitives convert independently in the returned
    /// list, each with its own outline.
    #[test]
    fn build_primitive_conversions_handles_a_multi_object_selection() {
        let document = Document::new(1);
        let rect_id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        let ellipse_id = document.create_ellipse(EllipseFrame {
            center: Point::new(0.0, 0.0),
            rx: Length::from_mm(5.0),
            ry: Length::from_mm(5.0),
        });
        let mut minter = AnchorIdMinter::new(1);
        let conversions =
            build_primitive_conversions(&document, &mut minter, &[rect_id, ellipse_id]);
        assert_eq!(conversions.len(), 2);
        assert_eq!(conversions[0].1.len(), 4, "rect: 4 corners");
        assert_eq!(conversions[1].1.len(), 4, "ellipse: 4 smooth nodes");
    }

    /// Acceptance criterion 17/21: converting a rotated primitive bakes
    /// the rotated outline into the new path's anchors, and the
    /// converted path keeps the original's `rotation` register.
    #[test]
    fn build_primitive_conversions_bakes_rotation_and_keeps_the_register() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(-5.0, -5.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(10.0),
        });
        document
            .rotate_object(
                id,
                Point::new(0.0, 0.0),
                vecmanf_document_core::Angle::from_radians(std::f64::consts::FRAC_PI_2),
            )
            .expect("rotate");
        let mut minter = AnchorIdMinter::new(1);
        let conversions = build_primitive_conversions(&document, &mut minter, &[id]);
        document.convert_to_paths(&conversions).expect("convert");

        let path = document.path(id).expect("now a path, same id");
        assert!(
            (path.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "rotation register carried over"
        );
        // The unrotated top-left corner was (-5, -5); after a 90-degree
        // rotation about the origin it is now (5, -5).
        assert!(
            path.anchors
                .iter()
                .any(|a| (a.point.x - 5.0).abs() < 1e-6 && (a.point.y - (-5.0)).abs() < 1e-6),
            "anchors are baked into the rotated, absolute positions: {:?}",
            path.anchors
        );
    }

    /// A stale id (no longer a primitive) is skipped, not included.
    #[test]
    fn build_primitive_conversions_skips_a_stale_id() {
        let document = Document::new(1);
        let id = document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(1.0),
            height: Length::from_mm(1.0),
        });
        let mut minter = AnchorIdMinter::new(1);
        document
            .convert_to_paths(&build_primitive_conversions(&document, &mut minter, &[id]))
            .expect("convert once");

        let conversions = build_primitive_conversions(&document, &mut minter, &[id]);
        assert!(conversions.is_empty(), "already a path, not a primitive");
    }
}
