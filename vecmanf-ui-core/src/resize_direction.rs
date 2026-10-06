//! The eight directions of a resize handle on an oriented box: a corner or an
//! edge midpoint (`specs/0005-object-transform/specification.md`).

use vecmanf_document_core::Vec2;

/// One of a box's eight resize directions: a corner or an edge midpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    /// North (top edge midpoint).
    N,
    /// North-east (top-right corner).
    Ne,
    /// East (right edge midpoint).
    E,
    /// South-east (bottom-right corner).
    Se,
    /// South (bottom edge midpoint).
    S,
    /// South-west (bottom-left corner).
    Sw,
    /// West (left edge midpoint).
    W,
    /// North-west (top-left corner).
    Nw,
}

impl ResizeDirection {
    /// Every direction of a box.
    pub const ALL_EIGHT: [Self; 8] = [
        Self::N,
        Self::Ne,
        Self::E,
        Self::Se,
        Self::S,
        Self::Sw,
        Self::W,
        Self::Nw,
    ];
    /// The unit vector from a shape's center toward this direction, in
    /// document (Y-down) space.
    #[must_use]
    pub const fn unit_vector(self) -> Vec2 {
        let (x, y) = match self {
            Self::N => (0.0, -1.0),
            Self::Ne => (1.0, -1.0),
            Self::E => (1.0, 0.0),
            Self::Se => (1.0, 1.0),
            Self::S => (0.0, 1.0),
            Self::Sw => (-1.0, 1.0),
            Self::W => (-1.0, 0.0),
            Self::Nw => (-1.0, -1.0),
        };
        Vec2::new(x, y)
    }
}
