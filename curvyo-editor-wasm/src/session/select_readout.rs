//! The Select tool's live numeric readout of a drag in flight (`specs/0005-
//! object-transform`, `specs/0008-object-transform-refinements`,
//! `specs/0009-unified-object-editing`). Split out of `session/select_view.rs`.

use curvyo_document_core::{ObjectSnapshot, Shape};
use curvyo_ui_core::{EditHandle, Side, format_degrees, oriented_bounds};

use super::{Session, Tool};

impl Session {
    /// The Select tool's live resolved object for the drag in flight, even
    /// where it equals the committed one (a readout still shows the value at
    /// the start of a drag that has left the dead zone). `None` outside such
    /// a drag.
    fn select_live_transform(&self) -> Option<ObjectSnapshot> {
        if self.tool != Tool::Select {
            return None;
        }
        let cursor = self.pointer_position?;
        self.select
            .live_transform(cursor, self.held.shift, self.held.ctrl)
    }

    /// The on-canvas numeric readout for an in-flight Select-tool resize,
    /// rotate, skew or parameter drag (acceptance criteria 14, 22 of slice 5;
    /// 35, 40 of the refinements; 20 of `unified-object-editing`): the
    /// object's live size in millimetres ("W × H mm"; a polygon/star's single
    /// outer radius, "r R mm"), its live rotation ("37.4°", one decimal at
    /// most, "22.5°" on a snap stop), the skew ("Skew x +12.5°", a real minus
    /// sign when negative), a rectangle's corner radius ("r 3.5 mm") or a
    /// star's inner ratio ("ratio 0.45"), anchored at the pointer. `None`
    /// outside such a drag.
    pub(super) fn select_live_readout(&self) -> Option<super::shapes::LiveReadout> {
        if self.tool != Tool::Select {
            return None;
        }
        if let Some(legend) = self.gesture_readout() {
            return Some(legend);
        }
        if self.select.move_in_flight() {
            return self.move_readout();
        }
        if self.select.group_drag_in_flight() {
            return self.group_live_readout();
        }
        let Some(handle) = self.select.dragging_handle() else {
            // No drag: the "max" notice of a limited typed radius, if any.
            return self.limit_notice.clone();
        };
        let anchor = self.pointer_position?;
        let text = match handle {
            EditHandle::Skew(side) => {
                let angle = self
                    .select
                    .live_skew_angle(anchor, self.held.shift, self.held.ctrl)?;
                skew_readout(side, angle.as_radians().to_degrees())
            }
            EditHandle::Rotate(_) => {
                let live = self.select_live_transform()?;
                format_degrees(live.orientation().as_radians().to_degrees())
            }
            EditHandle::Resize(_) => match &self.select_live_transform()? {
                ObjectSnapshot::Primitive(p)
                    if matches!(p.shape, Shape::Polygon { .. } | Shape::Star { .. }) =>
                {
                    let (Shape::Polygon { frame, .. } | Shape::Star { frame, .. }) = p.shape else {
                        return None;
                    };
                    format!("r {:.1} mm", frame.radius.as_mm())
                }
                other => {
                    let b = oriented_bounds(other);
                    format!("{:.1} × {:.1} mm", b.width(), b.height())
                }
            },
            EditHandle::Param(param) => super::corner_readout::param_readout(
                &self.select_live_transform()?,
                param,
                self.select.live_param_drag(anchor),
            )?,
            EditHandle::Move => return None,
        };
        Some(super::shapes::LiveReadout { text, anchor })
    }
}

/// `Skew x +12.5°`: the axis (x for the top and bottom handles, y for left
/// and right), the sign (a real minus, U+2212) and one decimal at most.
pub(super) fn skew_readout(side: Side, degrees: f64) -> String {
    let axis = if side.skews_along_u() { 'x' } else { 'y' };
    let magnitude = format_degrees(degrees.abs());
    let sign = if magnitude == "0°" {
        ""
    } else if degrees < 0.0 {
        "\u{2212}"
    } else {
        "+"
    };
    format!("Skew {axis} {sign}{magnitude}")
}
