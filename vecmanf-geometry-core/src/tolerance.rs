//! An explicit geometric tolerance (ADR 0002 §3: "every geometric
//! comparison takes an explicit `Tolerance`. There is no global epsilon").

use vecmanf_document_core::Length;

/// A tolerance for one geometric comparison, in document millimetres.
///
/// Every function in this crate that compares positions or flattens a
/// curve takes one of these explicitly rather than assuming a shared
/// constant — `vecmanf-render-core`'s own, coarser display tolerance
/// (ADR 0003 §7) is a different value entirely and must never be reused
/// for hit-testing or subdivision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance(Length);

impl Tolerance {
    /// Builds a [`Tolerance`] from a value already in millimetres.
    #[must_use]
    pub const fn from_mm(millimetres: f64) -> Self {
        Self(Length::from_mm(millimetres))
    }

    /// Returns the tolerance as a plain millimetre value, for handing to
    /// `kurbo`'s own accuracy-as-`f64` parameters.
    #[must_use]
    pub const fn as_mm(self) -> f64 {
        self.0.as_mm()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_mm() {
        let tolerance = Tolerance::from_mm(0.1);
        assert!((tolerance.as_mm() - 0.1).abs() < f64::EPSILON);
    }
}
