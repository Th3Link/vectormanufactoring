//! `Document`'s style command (`specs/0007-stroke-and-fill-styling/adrs.md`,
//! "register granularity" and "interaction commits"): one style-field edit over
//! several objects. It resolves all its ids before the first write, so one stale
//! id refuses the whole call, writes only where a value actually changes, and
//! ends in at most one commit.

use std::collections::HashSet;

use loro::LoroMap;

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::node_exists;
use crate::path_model::{Color, NodeId};
use crate::paths::tree_id_of;
use crate::style_codec::{read_style, write_changes};
use crate::style_model::{DashPattern, LineCap, LineJoin, Opacity, Style};
use crate::units::Length;

/// Why a style command refused to apply. Every variant describes a caller
/// error against a snapshot that is already stale (ADR 0009 §2), or an edit
/// outside the limits the editor enforces, not a defect in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StyleEditError {
    /// No object with this id exists any more.
    #[error("no such object")]
    NoSuchObject,
    /// A stroke width must be finite and not negative (zero turns the
    /// stroke off).
    #[error("stroke width must be zero or greater")]
    InvalidWidth,
}

/// One style property change. Applied to each target object on its own, so
/// every other property of each object stays as it was (`0007` acceptance
/// criterion 24). Editing the stroke colour, opacity or width turns a stroke
/// that is off back on in the same commit; a fill is turned on only by
/// [`StyleEdit::FillEnabled`] (`0017` criteria 9 and 15).
#[derive(Debug, Clone, PartialEq)]
pub enum StyleEdit {
    /// The Paint switch of the stroke. Off keeps every other stroke value.
    StrokeEnabled(bool),
    /// The stroke width. Exactly zero switches the stroke off and leaves the
    /// stored width alone; a positive width is stored and switches it on.
    StrokeWidth(Length),
    /// The stroke colour; also switches the stroke on.
    StrokeColor(Color),
    /// The stroke colour and opacity in one write (an 8-digit hex, a pick from
    /// the drawing); also switches the stroke on.
    StrokeRgba(Color, Opacity),
    /// The stroke opacity; also switches the stroke on.
    StrokeOpacity(Opacity),
    /// The dash pattern, as multiples of the width.
    StrokeDash(DashPattern),
    /// The join style.
    StrokeJoin(LineJoin),
    /// The cap style.
    StrokeCap(LineCap),
    /// The Paint switch of the fill. Off keeps the colour and opacity.
    FillEnabled(bool),
    /// The solid fill colour. Never turns the fill on.
    FillColor(Color),
    /// The solid fill colour and opacity in one write. Never turns the fill on.
    FillRgba(Color, Opacity),
    /// The solid fill opacity. Never turns the fill on.
    FillOpacity(Opacity),
}

impl StyleEdit {
    /// Applies the change to `style` in memory, exactly as
    /// [`Document::edit_style`] would write it: the editor's ephemeral panel
    /// preview shows an edit through this, so the preview and the commit can
    /// never disagree (acceptance criterion 36).
    ///
    /// # Errors
    /// [`StyleEditError::InvalidWidth`] for a negative or non-finite width;
    /// `style` is then unchanged.
    pub fn apply_to(&self, style: &mut Style) -> Result<(), StyleEditError> {
        let stroke = &mut style.stroke;
        match self {
            Self::StrokeEnabled(on) => stroke.enabled = *on,
            Self::StrokeWidth(width) => {
                let mm = width.as_mm();
                if !mm.is_finite() || mm < 0.0 {
                    return Err(StyleEditError::InvalidWidth);
                }
                if mm == 0.0 {
                    stroke.enabled = false;
                } else {
                    stroke.width = *width;
                    stroke.enabled = true;
                }
            }
            Self::StrokeColor(color) => {
                stroke.color = *color;
                stroke.enabled = true;
            }
            Self::StrokeRgba(color, opacity) => {
                stroke.color = *color;
                stroke.opacity = *opacity;
                stroke.enabled = true;
            }
            Self::StrokeOpacity(opacity) => {
                stroke.opacity = *opacity;
                stroke.enabled = true;
            }
            Self::StrokeDash(dash) => stroke.dash = dash.clone(),
            Self::StrokeJoin(join) => stroke.join = *join,
            Self::StrokeCap(cap) => stroke.cap = *cap,
            Self::FillEnabled(on) => style.fill.enabled = *on,
            Self::FillColor(color) => style.fill.color = *color,
            Self::FillRgba(color, opacity) => {
                style.fill.color = *color;
                style.fill.opacity = *opacity;
            }
            Self::FillOpacity(opacity) => style.fill.opacity = *opacity,
        }
        Ok(())
    }
}

impl Document {
    /// Applies one style property change to every named object, in **one
    /// commit** for the whole batch (acceptance criteria 24, 2 to 6, 9). An
    /// object whose stored value already equals the new one is not written,
    /// and a batch that changes nothing makes no commit.
    ///
    /// # Errors
    /// [`StyleEditError::NoSuchObject`] if any id is gone;
    /// [`StyleEditError::InvalidWidth`] for a negative or non-finite width.
    /// Either refuses the whole call.
    pub fn edit_style(&self, ids: &[NodeId], edit: &StyleEdit) -> Result<(), StyleEditError> {
        let metas = self.style_metas(ids)?;
        let planned: Vec<(LoroMap, Style, Style)> = metas
            .into_iter()
            .map(|meta| {
                let old = read_style(&meta);
                let mut new = old.clone();
                edit.apply_to(&mut new)?;
                Ok((meta, old, new))
            })
            .collect::<Result<_, StyleEditError>>()?;
        let mut wrote = false;
        for (meta, old, new) in &planned {
            wrote |= write_changes(meta, old, new);
        }
        if wrote {
            self.commit_with_label("edit_style");
        }
        Ok(())
    }

    /// Resolves one id to its meta map.
    fn style_meta(&self, id: NodeId) -> Result<LoroMap, StyleEditError> {
        let tree = self.loro().get_tree(OBJECTS_TREE);
        let tree_id = tree_id_of(id);
        if !node_exists(&tree, tree_id) {
            return Err(StyleEditError::NoSuchObject);
        }
        tree.get_meta(tree_id)
            .map_err(|_| StyleEditError::NoSuchObject)
    }

    /// Resolves each distinct id to its meta map, refusing on the first
    /// stale one.
    fn style_metas(&self, ids: &[NodeId]) -> Result<Vec<LoroMap>, StyleEditError> {
        let mut seen = HashSet::with_capacity(ids.len());
        ids.iter()
            .filter(|id| seen.insert(**id))
            .map(|&id| self.style_meta(id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive_model::{EllipseFrame, RectBounds};
    use crate::units::Point;

    fn rect(document: &Document) -> NodeId {
        document.create_rect(RectBounds {
            origin: Point::new(0.0, 0.0),
            width: Length::from_mm(10.0),
            height: Length::from_mm(5.0),
        })
    }

    fn ellipse(document: &Document) -> NodeId {
        document.create_ellipse(EllipseFrame {
            center: Point::new(30.0, 30.0),
            rx: Length::from_mm(4.0),
            ry: Length::from_mm(4.0),
        })
    }

    fn style_of(document: &Document, id: NodeId) -> Style {
        match document.object(id).unwrap() {
            crate::ObjectSnapshot::Path(p) => p.style,
            crate::ObjectSnapshot::Primitive(p) => p.style,
        }
    }

    fn red() -> Color {
        Color { r: 255, g: 0, b: 0 }
    }

    /// Operations and changes recorded so far: a command that writes nothing
    /// must leave both alone; one that writes ends in a single change.
    fn counters(document: &Document) -> (usize, usize) {
        (document.loro().len_ops(), document.loro().len_changes())
    }

    #[test]
    fn a_batch_edit_is_one_commit_for_every_object() {
        let document = Document::new(1);
        let (a, b) = (rect(&document), ellipse(&document));
        let (ops, changes) = counters(&document);
        document
            .edit_style(&[a, b], &StyleEdit::StrokeColor(red()))
            .unwrap();
        let (new_ops, new_changes) = counters(&document);
        assert_eq!(new_ops - ops, 2, "one register per object");
        assert_eq!(new_changes - changes, 1, "one commit for the batch");
        assert_eq!(style_of(&document, a).stroke.color, red());
        assert_eq!(style_of(&document, b).stroke.color, red());
    }

    #[test]
    fn an_edit_that_changes_nothing_writes_and_commits_nothing() {
        let document = Document::new(1);
        let a = rect(&document);
        document
            .edit_style(&[a], &StyleEdit::StrokeJoin(LineJoin::Bevel))
            .unwrap();
        let before = counters(&document);
        for edit in [
            StyleEdit::StrokeJoin(LineJoin::Bevel),
            StyleEdit::StrokeCap(LineCap::Butt),
            StyleEdit::StrokeEnabled(true),
            StyleEdit::StrokeColor(Color::BLACK),
            StyleEdit::StrokeWidth(Length::from_mm(0.25)),
            StyleEdit::StrokeDash(DashPattern::solid()),
            StyleEdit::FillOpacity(Opacity::OPAQUE),
        ] {
            document.edit_style(&[a], &edit).unwrap();
        }
        assert_eq!(counters(&document), before);
    }

    #[test]
    fn a_stale_id_refuses_the_whole_batch_and_writes_nothing() {
        let document = Document::new(1);
        let (a, b) = (rect(&document), ellipse(&document));
        document.delete_objects(&[b]).unwrap();
        let before = counters(&document);
        assert_eq!(
            document.edit_style(&[a, b], &StyleEdit::StrokeColor(red())),
            Err(StyleEditError::NoSuchObject)
        );
        assert_eq!(counters(&document), before);
        assert_eq!(style_of(&document, a), Style::default());
    }

    #[test]
    fn an_invalid_width_refuses_and_writes_nothing() {
        let document = Document::new(1);
        let a = rect(&document);
        let before = counters(&document);
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                document.edit_style(&[a], &StyleEdit::StrokeWidth(Length::from_mm(bad))),
                Err(StyleEditError::InvalidWidth)
            );
        }
        assert_eq!(counters(&document), before);
    }

    /// `0017` criteria 9, 15: a stroke edit of colour, opacity or width turns an
    /// off stroke on; a fill edit never turns a fill on; only Paint does.
    #[test]
    fn only_the_stroke_turns_itself_on_by_an_edit() {
        let document = Document::new(1);
        let a = rect(&document);
        document
            .edit_style(&[a], &StyleEdit::StrokeEnabled(false))
            .unwrap();
        document
            .edit_style(
                &[a],
                &StyleEdit::StrokeRgba(red(), Opacity::new(0.5).unwrap()),
            )
            .unwrap();
        let stroke = style_of(&document, a).stroke;
        assert!(stroke.enabled);
        assert_eq!((stroke.color, stroke.opacity.get()), (red(), 0.5));

        document
            .edit_style(&[a], &StyleEdit::FillColor(red()))
            .unwrap();
        document
            .edit_style(
                &[a],
                &StyleEdit::FillRgba(Color::BLACK, Opacity::new(0.25).unwrap()),
            )
            .unwrap();
        document
            .edit_style(&[a], &StyleEdit::FillOpacity(Opacity::new(0.75).unwrap()))
            .unwrap();
        let fill = style_of(&document, a).fill;
        assert!(!fill.enabled, "a colour edit leaves the fill off");
        assert_eq!(fill.opacity.get(), 0.75);
        document
            .edit_style(&[a], &StyleEdit::FillEnabled(true))
            .unwrap();
        let fill = style_of(&document, a).fill;
        assert!(fill.enabled);
        assert_eq!((fill.color, fill.opacity.get()), (Color::BLACK, 0.75));
    }

    #[test]
    fn a_duplicate_id_in_the_batch_is_applied_once() {
        let document = Document::new(1);
        let a = rect(&document);
        let (ops, _) = counters(&document);
        document
            .edit_style(&[a, a], &StyleEdit::StrokeColor(red()))
            .unwrap();
        assert_eq!(counters(&document).0 - ops, 1);
    }
}
