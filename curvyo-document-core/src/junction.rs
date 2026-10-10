//! The merge of two path ends into one node (`specs/0006-path-merge-split-and-node-types`
//! criterion 9): the midpoint, a Corner, each side keeping its own handle that faces into its
//! path. [`crate::Document::join_endpoints`] writes with it; the Pen's connect and the Close path
//! command use it when two ends lie within [`COINCIDENT_MM`] of each other
//! (`specs/0034-pen-path-extension` criteria 9 and 19).

use crate::path_model::{AnchorKind, AnchorSnapshot};
use crate::units::{Point, Vec2};

/// Two path ends closer than this (millimetres) are merged into one node instead of getting a
/// zero-length segment between them.
pub const COINCIDENT_MM: f64 = 0.001;

/// The handle of an end node that faces into its own path: its incoming handle if it is the last
/// node, its outgoing handle if it is the first.
#[must_use]
pub fn interior_handle(anchor: &AnchorSnapshot, is_last: bool) -> Vec2 {
    if is_last {
        anchor.handle_in
    } else {
        anchor.handle_out
    }
}

/// The one node that replaces the ends `a` and `b`: at their midpoint, a Corner, with `a`'s id.
/// `a_is_last` says whether `a` is the last node of its path (else the first), likewise `b`. The
/// handle of `a` that faces into its path stays; `b`'s interior-facing handle becomes the
/// handle on the other side.
#[must_use]
pub fn merged_junction(
    a: &AnchorSnapshot,
    a_is_last: bool,
    b: &AnchorSnapshot,
    b_is_last: bool,
) -> AnchorSnapshot {
    let point = Point::new(
        f64::midpoint(a.point.x, b.point.x),
        f64::midpoint(a.point.y, b.point.y),
    );
    let (a_interior, b_interior) = (interior_handle(a, a_is_last), interior_handle(b, b_is_last));
    let (handle_in, handle_out) = if a_is_last {
        (a_interior, b_interior)
    } else {
        (b_interior, a_interior)
    };
    AnchorSnapshot {
        id: a.id,
        point,
        handle_in,
        handle_out,
        kind: AnchorKind::Corner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path_model::{AnchorId, NewAnchor};

    #[test]
    fn the_merged_node_is_a_corner_at_the_midpoint_keeping_each_interior_handle() {
        let mut a = NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0));
        a.handle_in = Vec2::new(-1.0, 0.0);
        let mut b = NewAnchor::corner(AnchorId::new(1, 2), Point::new(4.0, 2.0));
        b.handle_out = Vec2::new(0.0, 3.0);
        // `a` ends its path, `b` starts its own: a's handle_in stays, b's handle_out follows.
        let merged = merged_junction(&a, true, &b, false);
        assert_eq!(merged.id, a.id);
        assert_eq!(merged.point, Point::new(2.0, 1.0));
        assert_eq!(merged.kind, AnchorKind::Corner);
        assert_eq!(merged.handle_in, Vec2::new(-1.0, 0.0));
        assert_eq!(merged.handle_out, Vec2::new(0.0, 3.0));
    }
}
