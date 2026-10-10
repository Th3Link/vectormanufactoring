//! How the node a path closes onto joins the closing segment (`specs/0034-pen-path-extension`
//! criteria 15 and 17): Sharp or Smooth, as drawn or flipped by Shift. One pure function resolves
//! the closing node for the Pen's preview, the Pen's commit and the Close path command, so the
//! three cannot disagree.

use curvyo_document_core::{
    AnchorKind, AnchorSnapshot, HandleSlot, Point, Vec2, smooth_corner_handles,
};

/// The join of a closing node with its closing segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    /// A cusp: the closing node is a Corner and has no handle on the closing side.
    Sharp,
    /// Tangent-continuous: the closing node is Symmetric or Asymmetric.
    Smooth,
}

impl JoinType {
    /// The join "as drawn": a Corner node closes sharp, a Symmetric or Asymmetric node smooth.
    #[must_use]
    pub const fn as_drawn(kind: AnchorKind) -> Self {
        match kind {
            AnchorKind::Corner => Self::Sharp,
            AnchorKind::Symmetric | AnchorKind::Asymmetric => Self::Smooth,
        }
    }

    /// The join Shift asks for: as drawn, flipped while Shift is held.
    #[must_use]
    pub const fn resolve(kind: AnchorKind, shift: bool) -> Self {
        match (Self::as_drawn(kind), shift) {
            (join, false) => join,
            (Self::Sharp, true) => Self::Smooth,
            (Self::Smooth, true) => Self::Sharp,
        }
    }

    /// The other join.
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Sharp => Self::Smooth,
            Self::Smooth => Self::Sharp,
        }
    }
}

/// `closing` with `join` applied. `before` and `after` are the nodes on either side of it in the
/// closed path (for a path closing onto its first node: the last node and the second node), and
/// `side` is the slot of `closing` that faces the closing segment (its incoming handle when the
/// closing segment arrives at it, its outgoing handle when it leaves it).
///
/// - Sharp: a Symmetric or Asymmetric node becomes a Corner whose handle on the closing side is
///   retracted to zero; the other handle stays. Without that retraction two collinear handles on a
///   Corner node would draw the same curve as a Symmetric node and Sharp would look like Smooth.
///   A Corner node is unchanged.
/// - Smooth: a Corner node becomes Asymmetric with two handles along the tangent from `before` to
///   `after`, each keeping its length or taking the default (`0006` criterion 2). A Symmetric or
///   Asymmetric node is unchanged.
#[must_use]
pub fn resolve_closing_node(
    closing: &AnchorSnapshot,
    before: Point,
    after: Point,
    side: HandleSlot,
    join: JoinType,
) -> AnchorSnapshot {
    match (join, closing.kind) {
        (JoinType::Sharp, AnchorKind::Corner)
        | (JoinType::Smooth, AnchorKind::Symmetric | AnchorKind::Asymmetric) => *closing,
        (JoinType::Sharp, _) => {
            let mut node = *closing;
            node.kind = AnchorKind::Corner;
            match side {
                HandleSlot::In => node.handle_in = Vec2::ZERO,
                HandleSlot::Out => node.handle_out = Vec2::ZERO,
            }
            node
        }
        (JoinType::Smooth, AnchorKind::Corner) => {
            let (handle_in, handle_out) = smooth_corner_handles(
                before.vector_to(after),
                closing.handle_in,
                closing.handle_out,
            );
            AnchorSnapshot {
                handle_in,
                handle_out,
                kind: AnchorKind::Asymmetric,
                ..*closing
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use curvyo_document_core::AnchorId;

    use super::*;

    fn node(kind: AnchorKind, handle_in: Vec2, handle_out: Vec2) -> AnchorSnapshot {
        AnchorSnapshot {
            id: AnchorId::new(1, 1),
            point: Point::new(0.0, 0.0),
            handle_in,
            handle_out,
            kind,
        }
    }

    #[test]
    fn the_default_is_as_drawn_and_shift_flips_it() {
        assert_eq!(
            JoinType::resolve(AnchorKind::Corner, false),
            JoinType::Sharp
        );
        assert_eq!(
            JoinType::resolve(AnchorKind::Corner, true),
            JoinType::Smooth
        );
        assert_eq!(
            JoinType::resolve(AnchorKind::Symmetric, false),
            JoinType::Smooth
        );
        assert_eq!(
            JoinType::resolve(AnchorKind::Asymmetric, true),
            JoinType::Sharp
        );
    }

    /// Criterion 15: Sharp on a drawn-with-a-drag node retracts the closing-side handle and keeps
    /// the other, so the closing segment arrives without a tangent.
    #[test]
    fn sharp_retracts_the_handle_on_the_closing_side() {
        let drawn = node(
            AnchorKind::Symmetric,
            Vec2::new(-3.0, 0.0),
            Vec2::new(3.0, 0.0),
        );
        let near = Point::new(0.0, 5.0);
        let sharp_in = resolve_closing_node(&drawn, near, near, HandleSlot::In, JoinType::Sharp);
        assert_eq!(sharp_in.kind, AnchorKind::Corner);
        assert_eq!(sharp_in.handle_in, Vec2::ZERO);
        assert_eq!(sharp_in.handle_out, Vec2::new(3.0, 0.0));
        let sharp_out = resolve_closing_node(&drawn, near, near, HandleSlot::Out, JoinType::Sharp);
        assert_eq!(sharp_out.handle_out, Vec2::ZERO);
        assert_eq!(sharp_out.handle_in, Vec2::new(-3.0, 0.0));
    }

    /// Criterion 15: Smooth on a Corner node makes it Asymmetric along the tangent from the node
    /// before to the node after; an already smooth node and a Corner closing Sharp are unchanged.
    #[test]
    fn smooth_turns_a_corner_into_collinear_handles_and_unchanged_cases_stay() {
        let corner = node(AnchorKind::Corner, Vec2::ZERO, Vec2::ZERO);
        // D (0, 20) before, B (20, 0) after: the tangent runs along (20, -20).
        let smooth = resolve_closing_node(
            &corner,
            Point::new(0.0, 20.0),
            Point::new(20.0, 0.0),
            HandleSlot::In,
            JoinType::Smooth,
        );
        assert_eq!(smooth.kind, AnchorKind::Asymmetric);
        let along = Vec2::new(1.0, -1.0).normalized_to(1.0);
        assert!((smooth.handle_out.x - along.x * 10.0).abs() < 1e-9);
        assert!((smooth.handle_out.y - along.y * 10.0).abs() < 1e-9);
        assert!((smooth.handle_in.x + along.x * 10.0).abs() < 1e-9);
        let origin = Point::new(0.0, 0.0);
        assert_eq!(
            resolve_closing_node(&corner, origin, origin, HandleSlot::In, JoinType::Sharp),
            corner
        );
        let drawn = node(
            AnchorKind::Symmetric,
            Vec2::new(-3.0, 0.0),
            Vec2::new(3.0, 0.0),
        );
        assert_eq!(
            resolve_closing_node(&drawn, origin, origin, HandleSlot::In, JoinType::Smooth),
            drawn
        );
    }
}
