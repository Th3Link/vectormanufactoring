//! `Document`'s commands that write a rectangle's corner radius, split out of
//! [`crate::shapes`] so that module stays under the size limit
//! (`specs/rectangle-corner-radii/adrs.md`, decision 1).

use crate::document::Document;
use crate::path_model::NodeId;
use crate::primitive_model::RectBounds;
use crate::shape_codec::{self, SHAPE_RECT};
use crate::shapes::{ShapeEditError, write_stroke_width_if_changed};
use crate::units::Length;

impl Document {
    /// Sets a corner radius as entered — clamped only where it is later
    /// evaluated, never here (acceptance criteria 4, 5, 6) — on every
    /// named rectangle, as **one commit for the whole batch** (architect
    /// review: a resize-handle drag always names exactly one id, but
    /// "remove rounding" can name an entire multi-rectangle selection,
    /// and that must not cost one commit per object). Every id is
    /// resolved and confirmed to be a rectangle before any of them is
    /// written, so one unknown or non-rectangle id anywhere in `ids`
    /// refuses the whole call — the same contract
    /// [`Document::convert_to_paths`] already uses. A negative value is
    /// floored to zero defensively; nothing in this slice's UI can
    /// produce one.
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer
    /// exists; [`ShapeEditError::NotAPrimitive`] /
    /// [`ShapeEditError::WrongShape`] if any named id is not a
    /// rectangle.
    pub fn set_corner_radius(&self, ids: &[NodeId], radius: Length) -> Result<(), ShapeEditError> {
        let radius = Length::from_mm(radius.as_mm().max(0.0));
        let metas: Vec<_> = ids
            .iter()
            .map(|&id| self.require_shape(id, SHAPE_RECT))
            .collect::<Result<_, _>>()?;
        for meta in metas {
            shape_codec::write_corner_radius(&meta, radius);
        }
        self.commit_with_label("set_corner_radius");
        Ok(())
    }

    /// Sets each named rectangle's corner radius to its own value as **one
    /// commit for the whole batch**: the Select bar's "Radius" field over
    /// rectangles of different sizes (`specs/unified-object-editing/`
    /// criterion 21a), where each value is limited to half of that
    /// rectangle's own shorter side. A rectangle whose stored radius already
    /// equals its value is not rewritten (an LWW rewrite of an unchanged value
    /// could beat a concurrent radius edit), and nothing is committed when no
    /// rectangle changes. Every id is resolved and confirmed to be a rectangle
    /// before any is written. A negative value is floored to zero.
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] if any named id no longer exists;
    /// [`ShapeEditError::NotAPrimitive`] / [`ShapeEditError::WrongShape`] if
    /// any named id is not a rectangle.
    pub fn set_corner_radii(&self, radii: &[(NodeId, Length)]) -> Result<(), ShapeEditError> {
        let metas: Vec<_> = radii
            .iter()
            .map(|&(id, radius)| {
                self.require_shape(id, SHAPE_RECT)
                    .map(|meta| (meta, Length::from_mm(radius.as_mm().max(0.0))))
            })
            .collect::<Result<_, _>>()?;
        let mut changed = false;
        for (meta, radius) in metas {
            if shape_codec::read_corner_radius(&meta) != Some(radius) {
                shape_codec::write_corner_radius(&meta, radius);
                changed = true;
            }
        }
        if changed {
            self.commit_with_label("set_corner_radii");
        }
        Ok(())
    }

    /// Resizes a rectangle's bounding box, corner radius and (optionally)
    /// stroke width together as **one commit** (`specs/0005-object-transform/
    /// adrs.md`'s resize-writes table: "frame, `corner_radius` (rect),
    /// `stroke_width`" — a Select-tool resize-handle drag, acceptance
    /// criteria 8, 9). `curvyo-ui-core` computes all three values
    /// (including the local-frame mapping for a rotated object and the
    /// √(sx·sy) stroke/radius factor) before calling this — this method
    /// `stroke_width: None` leaves the stored stroke width untouched.
    /// This method is purely "write what was computed", the same split
    /// [`Document::set_rect_bounds`] already follows for a plain resize.
    ///
    /// # Errors
    /// [`ShapeEditError::NoSuchObject`] / [`ShapeEditError::NotAPrimitive`]
    /// / [`ShapeEditError::WrongShape`] if `id` is not a rectangle.
    pub fn resize_rect(
        &self,
        id: NodeId,
        bounds: RectBounds,
        corner_radius: Length,
        stroke_width: Option<Length>,
    ) -> Result<(), ShapeEditError> {
        let meta = self.require_shape(id, SHAPE_RECT)?;
        shape_codec::write_rect_bounds(&meta, bounds);
        // An unchanged radius is not rewritten (a radius of 0 scales to
        // 0): an LWW rewrite of an unchanged value would be a new
        // operation that could beat a concurrent radius edit.
        if shape_codec::read_corner_radius(&meta) != Some(corner_radius) {
            shape_codec::write_corner_radius(&meta, corner_radius);
        }
        write_stroke_width_if_changed(&meta, stroke_width);
        self.commit_with_label("resize_rect");
        Ok(())
    }
}
