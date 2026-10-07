//! The text a corner radius knob shows: live readout and hint-chip lines
//! (`specs/rectangle-corner-radii/` criteria 7 and 23).

use curvyo_document_core::{
    ObjectSnapshot, PrimitiveSnapshot, SHARP_CORNER_EPSILON_MM, Shape, effective_corner_radii,
};
use curvyo_ui_core::{EditHandle, ParamDragInfo, ParamHandle};

use super::{Session, Tool};

impl Session {
    /// The lines of the hint chip of a corner radius knob under the pointer
    /// (`specs/rectangle-corner-radii/` criterion 23), empty on any other
    /// handle. Which lines show depends on the "Link corners" switch (Shift is
    /// not tracked live): linked, "Corner radius, all four" and "Shift: this
    /// corner only"; unlinked, "Corner radius, this corner" and "Shift: all four
    /// corners"; always "Double-click: type a value". A corner whose stored
    /// radius is above its effective one adds a first line "Limited by the
    /// size. Stored 30 mm, shown 20 mm." and, when the next drag would be
    /// unlinked, "Editing one corner fixes the other three at their shown
    /// size." Those two lines are muted; the host recognises them by their
    /// start.
    #[must_use]
    pub fn corner_hint_lines(&self) -> Vec<String> {
        if self.tool != Tool::Select {
            return Vec::new();
        }
        let objects = self.objects();
        let Some((object, _, EditHandle::Param(ParamHandle::CornerRadius(corner)))) =
            self.select_hovered_handle(&objects)
        else {
            return Vec::new();
        };
        let ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Rect {
                bounds,
                corner_radii,
            },
            ..
        }) = object
        else {
            return Vec::new();
        };
        let linked = self.link_corners();
        let stored = corner_radii.get(corner).as_mm();
        let shown = effective_corner_radii(bounds, corner_radii)
            .get(corner)
            .as_mm();
        let mut lines = Vec::new();
        if stored - shown > SHARP_CORNER_EPSILON_MM {
            lines.push(format!(
                "Limited by the size. Stored {} mm, shown {} mm.",
                trimmed_mm(stored),
                trimmed_mm(shown)
            ));
            if !linked {
                lines.push("Editing one corner fixes the other three at their shown size.".into());
            }
        }
        lines.extend(
            if linked {
                [
                    "Corner radius, all four",
                    "Shift: this corner only",
                    "Double-click: type a value",
                ]
            } else {
                [
                    "Corner radius, this corner",
                    "Shift: all four corners",
                    "Double-click: type a value",
                ]
            }
            .map(String::from),
        );
        lines
    }
}

/// `r 3.5 mm` for the dragged corner's effective radius of a rectangle,
/// `ratio 0.45` for a star's inner ratio. A corner radius readout says " max"
/// while a limit stops the drag and, for a linked drag that overwrites unequal
/// radii, " · all corners" after it (`specs/rectangle-corner-radii/` criterion
/// 7): "r 12.0 mm max · all corners".
pub(super) fn param_readout(
    object: &ObjectSnapshot,
    handle: ParamHandle,
    info: Option<ParamDragInfo>,
) -> Option<String> {
    let ObjectSnapshot::Primitive(PrimitiveSnapshot { shape, .. }) = object else {
        return None;
    };
    match (*shape, handle) {
        (
            Shape::Rect {
                bounds,
                corner_radii,
            },
            ParamHandle::CornerRadius(corner),
        ) => {
            let radius = effective_corner_radii(bounds, corner_radii)
                .get(corner)
                .as_mm();
            let info = info.unwrap_or(ParamDragInfo {
                limited: false,
                overwrites_unequal: false,
            });
            Some(format!(
                "r {radius:.1} mm{}{}",
                if info.limited { " max" } else { "" },
                if info.overwrites_unequal {
                    " \u{b7} all corners"
                } else {
                    ""
                }
            ))
        }
        (Shape::Star { inner_ratio, .. }, ParamHandle::InnerRadius) => {
            Some(format!("ratio {:.2}", inner_ratio.get()))
        }
        _ => None,
    }
}

/// A length in millimetres with up to two decimals and no trailing zeros
/// ("30", "3.5", "0.25"), the bar field's precision.
fn trimmed_mm(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}
