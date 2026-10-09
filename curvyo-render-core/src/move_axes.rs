//! Draws the two origin axis lines of an axis-locked move
//! (`specs/0010-edit-interaction-polish/specification.md`, criterion 27; the
//! "Move axis guide" row of `docs/design-system.md`).

use curvyo_document_core::{Point, ViewTransform};

use crate::glyphs::{DrawList, thick_line};
use crate::select_box::snap_guide_line;
use crate::theme;

/// The axis a move is locked to: the offset runs along it and is zero on the
/// other. Mirrors the Select tool's own axis, so this crate depends on no
/// `curvyo-ui-core` type (ADR 0011 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockedAxis {
    /// The horizontal axis: the horizontal line is the active one.
    Horizontal,
    /// The vertical axis: the vertical line is the active one.
    Vertical,
}

/// The two origin axes of a locked move: one horizontal and one vertical line
/// through the selection's start centre, given as end points that span the
/// viewport (this crate does not know the canvas size).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveAxes {
    /// The end points of the horizontal line, document space.
    pub horizontal: (Point, Point),
    /// The end points of the vertical line, document space.
    pub vertical: (Point, Point),
    /// The axis the move is locked to; its line is drawn in `--axis-guide`,
    /// the other in `--axis-guide-idle`.
    pub locked: LockedAxis,
    /// `window.devicePixelRatio`, so the 1 px lines sit on whole device
    /// pixels like the selection box does.
    pub device_pixel_ratio: f64,
}

/// The axes as a draw list: the idle line first, the active one over it.
#[must_use]
pub fn build_move_axes(view: ViewTransform, axes: &MoveAxes) -> DrawList {
    let (active, idle) = match axes.locked {
        LockedAxis::Horizontal => (axes.horizontal, axes.vertical),
        LockedAxis::Vertical => (axes.vertical, axes.horizontal),
    };
    let mut list = DrawList::default();
    for (line, color) in [(idle, theme::AXIS_GUIDE_IDLE), (active, theme::AXIS_GUIDE)] {
        let (from, to, width_px) = snap_guide_line(view, line.0, line.1, axes.device_pixel_ratio);
        list.extend(thick_line(from, to, width_px / view.scale(), color));
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::RgbaColor;

    fn axes(locked: LockedAxis) -> MoveAxes {
        MoveAxes {
            horizontal: (Point::new(-100.0, 20.0), Point::new(300.0, 20.0)),
            vertical: (Point::new(50.0, -100.0), Point::new(50.0, 300.0)),
            locked,
            device_pixel_ratio: 1.0,
        }
    }

    fn colored(list: &DrawList, color: RgbaColor) -> Vec<Point> {
        list.triangles
            .iter()
            .filter(|v| v.color == color)
            .map(|v| v.position)
            .collect()
    }

    /// The locked axis's line is `--axis-guide`, the other
    /// `--axis-guide-idle`, and each line is one quad.
    #[test]
    fn the_locked_axis_is_drawn_in_the_stronger_colour() {
        let view = ViewTransform::identity();
        for (locked, along_x) in [
            (LockedAxis::Horizontal, true),
            (LockedAxis::Vertical, false),
        ] {
            let list = build_move_axes(view, &axes(locked));
            assert_eq!(list.triangle_count(), 4, "two quads");
            let strong = colored(&list, theme::AXIS_GUIDE);
            let faint = colored(&list, theme::AXIS_GUIDE_IDLE);
            assert_eq!((strong.len(), faint.len()), (6, 6));
            // A horizontal line is as thin as a line in y and long in x.
            let span = |points: &[Point], x: bool| {
                let values = points.iter().map(|p| if x { p.x } else { p.y });
                let low = values.clone().fold(f64::MAX, f64::min);
                values.fold(f64::MIN, f64::max) - low
            };
            assert!(span(&strong, along_x) > 100.0 && span(&strong, !along_x) < 2.0);
            assert!(span(&faint, !along_x) > 100.0 && span(&faint, along_x) < 2.0);
        }
    }

    /// The idle line is drawn first, so the active one lies over the crossing.
    #[test]
    fn the_active_line_is_drawn_over_the_idle_one() {
        let list = build_move_axes(ViewTransform::identity(), &axes(LockedAxis::Vertical));
        assert_eq!(list.triangles[0].color, theme::AXIS_GUIDE_IDLE);
        assert_eq!(list.triangles[6].color, theme::AXIS_GUIDE);
    }

    /// A 1 px line stays 1 screen pixel wide at any zoom and sits on whole
    /// device pixels at ratio 1.
    #[test]
    fn the_lines_are_one_pixel_wide_whatever_the_zoom() {
        for scale in [0.25, 1.0, 4.0] {
            let view = ViewTransform::new(scale, Point::new(0.0, 0.0));
            let list = build_move_axes(view, &axes(LockedAxis::Horizontal));
            let ys: Vec<f64> = colored(&list, theme::AXIS_GUIDE)
                .iter()
                .map(|p| p.y * scale)
                .collect();
            let (low, high) = ys
                .iter()
                .fold((f64::MAX, f64::MIN), |(l, h), y| (l.min(*y), h.max(*y)));
            assert!(
                (high - low - 1.0).abs() < 1e-9,
                "scale {scale}: {}",
                high - low
            );
            assert!(
                (low.fract() - 0.0).abs() < 1e-9,
                "on a pixel boundary: {low}"
            );
        }
    }
}
