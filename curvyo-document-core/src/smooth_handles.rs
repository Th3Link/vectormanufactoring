//! The Corner to Asymmetric rule of a node conversion, as one pure function
//! (`specs/0006-path-merge-split-and-node-types` criterion 2): two handles collinear through the
//! node along the local tangent, each keeping its length if it had one and taking the default
//! otherwise. [`crate::Document::convert_anchor_kind`] writes with it; the Pen's and the Close
//! path command's "smooth" join resolve their closing node with the same function
//! (`specs/0034-pen-path-extension` criterion 15).

use crate::paths::DEFAULT_HANDLE_LENGTH_MM;
use crate::units::Vec2;

/// Below this length, a handle counts as "no existing length to keep" for
/// acceptance criterion 2's "its own current length, if that side already
/// had a non-zero handle; otherwise the slice's existing default handle
/// length" — not a geometric [`crate::units::Tolerance`] (`CLAUDE.md` §5):
/// this is a plain zero/non-zero classification of a stored value, the
/// same kind of exact check `specs/0002-path-node-editing/adrs.md` already
/// uses for "a retracted handle is the exact zero vector".
const ZERO_HANDLE_EPSILON: f64 = f64::EPSILON;

/// The `(handle_in, handle_out)` a Corner node takes when it becomes Asymmetric: the outgoing
/// handle along `tangent` (the direction from the node before it to the node after it), the
/// incoming one opposite, each at its own current length if that is not zero and at the default
/// handle length otherwise.
#[must_use]
pub fn smooth_corner_handles(tangent: Vec2, handle_in: Vec2, handle_out: Vec2) -> (Vec2, Vec2) {
    let unit = tangent.normalized_to(1.0);
    (
        unit.negated().scaled(kept_length_or_default(handle_in)),
        unit.scaled(kept_length_or_default(handle_out)),
    )
}

/// Acceptance criterion 2's "its own current length, if that side already
/// had a non-zero handle; otherwise the slice's existing default handle
/// length" — one side of a Corner→Asymmetric conversion.
#[must_use]
fn kept_length_or_default(existing_handle: Vec2) -> f64 {
    let length = existing_handle.length();
    if length > ZERO_HANDLE_EPSILON {
        length
    } else {
        DEFAULT_HANDLE_LENGTH_MM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_keeps_its_length_and_a_missing_one_takes_the_default() {
        let (handle_in, handle_out) =
            smooth_corner_handles(Vec2::new(30.0, 0.0), Vec2::new(0.0, 4.0), Vec2::ZERO);
        assert_eq!(handle_out, Vec2::new(DEFAULT_HANDLE_LENGTH_MM, 0.0));
        assert_eq!(handle_in, Vec2::new(-4.0, 0.0));
    }
}
