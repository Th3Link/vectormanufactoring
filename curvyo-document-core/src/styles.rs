//! `Document`'s style commands (`specs/0007-stroke-and-fill-styling/adrs.md`,
//! "register granularity", "the gradient stop model" and "interaction
//! commits"): one style-field edit over several objects, the fill-mode
//! switch, and add, remove and edit for gradient stops. Every command
//! resolves all its ids before the first write, so one stale id refuses the
//! whole call, writes nothing only where a value actually changes, and ends
//! in at most one commit.

use std::collections::HashSet;

use loro::LoroMap;

use crate::document::{Document, OBJECTS_TREE};
use crate::path_codec::node_exists;
use crate::path_model::{Color, NodeId};
use crate::paths::tree_id_of;
use crate::style_codec::{
    ensure_stops_list, insert_stop_at, read_stop_map, read_style, stop_index, stop_map_at,
    stops_list, write_changes, write_stop_color, write_stop_opacity, write_stop_position,
    write_stops,
};
use crate::style_model::{
    DashPattern, GradientStop, LineCap, LineJoin, Opacity, StopId, StopPosition, Style,
};
use crate::units::Length;

/// The most stops the add command lets a gradient reach (acceptance
/// criterion 16: an edit-time limit, not a stored invariant).
pub const MAX_GRADIENT_STOPS: usize = 16;
/// The fewest stops the remove command leaves a gradient with.
pub const MIN_GRADIENT_STOPS: usize = 2;

/// Why a style command refused to apply. Every variant describes a caller
/// error against a snapshot that is already stale (ADR 0009 §2), or an edit
/// outside the limits the editor enforces, not a defect in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StyleEditError {
    /// No object with this id exists any more.
    #[error("no such object")]
    NoSuchObject,
    /// The object has no stop with this id.
    #[error("no such gradient stop")]
    NoSuchStop,
    /// A stroke width must be finite and not negative (zero turns the
    /// stroke off).
    #[error("stroke width must be zero or greater")]
    InvalidWidth,
    /// Adding would pass [`MAX_GRADIENT_STOPS`].
    #[error("A gradient holds at most 16 stops")]
    TooManyStops,
    /// Removing would leave fewer than [`MIN_GRADIENT_STOPS`].
    #[error("A gradient keeps at least 2 stops")]
    TooFewStops,
    /// The object already has a stop with the id being added.
    #[error("a stop with this id already exists")]
    DuplicateStop,
}

/// One style property change. Applied to each target object on its own, so
/// every other property of each object stays as it was (acceptance criterion
/// 24). Editing the stroke colour, opacity or width turns a stroke that is
/// off back on in the same commit.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleEdit {
    /// The Paint switch of the stroke. Off keeps every other stroke value.
    StrokeEnabled(bool),
    /// The stroke width. Exactly zero switches the stroke off and leaves the
    /// stored width alone; a positive width is stored and switches it on.
    StrokeWidth(Length),
    /// The stroke colour; also switches the stroke on.
    StrokeColor(Color),
    /// The stroke opacity; also switches the stroke on.
    StrokeOpacity(Opacity),
    /// The dash pattern, as multiples of the width.
    StrokeDash(DashPattern),
    /// The join style.
    StrokeJoin(LineJoin),
    /// The cap style.
    StrokeCap(LineCap),
    /// The solid fill colour.
    FillColor(Color),
    /// The solid fill opacity.
    FillOpacity(Opacity),
}

impl StyleEdit {
    fn apply(&self, style: &mut Style) -> Result<(), StyleEditError> {
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
            Self::StrokeOpacity(opacity) => {
                stroke.opacity = *opacity;
                stroke.enabled = true;
            }
            Self::StrokeDash(dash) => stroke.dash = dash.clone(),
            Self::StrokeJoin(join) => stroke.join = *join,
            Self::StrokeCap(cap) => stroke.cap = *cap,
            Self::FillColor(color) => style.fill.color = *color,
            Self::FillOpacity(opacity) => style.fill.opacity = *opacity,
        }
        Ok(())
    }
}

/// What the fill-mode row of the panel offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    /// No fill; the stored colour, kind and stops stay.
    None,
    /// The stored solid colour.
    Solid,
    /// A linear gradient on the stored stops.
    Linear,
    /// A radial gradient on the stored stops.
    Radial,
}

/// One object of a [`Document::set_fill_mode`] call, with the stops it gets
/// if it has none yet (acceptance criterion 17; each object of a
/// multi-selection takes its own, built from its own stored colour with
/// [`GradientStop::default_pair`]). Ignored for `None` and `Solid` and for an
/// object that already holds stops.
#[derive(Debug, Clone, PartialEq)]
pub struct FillModeTarget {
    /// The object.
    pub id: NodeId,
    /// The stops to create when the object has none.
    pub seed_stops: Vec<GradientStop>,
}

/// The one value of a stop a [`StopEdit`] changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StopChange {
    /// Move the stop along the gradient. The list order is not touched; the
    /// render order is a stable sort by position.
    Position(StopPosition),
    /// Recolour the stop.
    Color(Color),
    /// Change the stop's own opacity.
    Opacity(Opacity),
}

/// One stop edit: object, stop and change. A multi-selection edit passes one
/// per object, each with that object's own `StopId` (the editor maps rank to
/// id; this crate never addresses a stop by rank).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopEdit {
    /// The object holding the stop.
    pub id: NodeId,
    /// The stop within that object's list.
    pub stop: StopId,
    /// What changes.
    pub change: StopChange,
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
                edit.apply(&mut new)?;
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

    /// Switches the fill mode of every target, in **one commit** (acceptance
    /// criteria 13, 17, 24). Switching keeps every stored colour, opacity and
    /// stop. A gradient mode on an object without stops creates its
    /// `seed_stops`; linear and radial share one stop list.
    ///
    /// # Errors
    /// [`StyleEditError::NoSuchObject`] if any id is gone, refusing the whole
    /// call.
    pub fn set_fill_mode(
        &self,
        mode: FillMode,
        targets: &[FillModeTarget],
    ) -> Result<(), StyleEditError> {
        let ids: Vec<NodeId> = targets.iter().map(|t| t.id).collect();
        let metas = self.style_metas(&ids)?;
        let mut seen = HashSet::new();
        let mut wrote = false;
        for (target, meta) in targets.iter().filter(|t| seen.insert(t.id)).zip(&metas) {
            let old = read_style(meta);
            let mut new = old.clone();
            match mode {
                FillMode::None => new.fill.enabled = false,
                FillMode::Solid => {
                    new.fill.enabled = true;
                    new.fill.kind = crate::style_model::FillKind::Solid;
                }
                FillMode::Linear | FillMode::Radial => {
                    new.fill.enabled = true;
                    new.fill.kind = if mode == FillMode::Linear {
                        crate::style_model::FillKind::Linear
                    } else {
                        crate::style_model::FillKind::Radial
                    };
                }
            }
            wrote |= write_changes(meta, &old, &new);
            let is_gradient = matches!(mode, FillMode::Linear | FillMode::Radial);
            if is_gradient && old.fill.stops.is_empty() && !target.seed_stops.is_empty() {
                write_stops(meta, &target.seed_stops);
                wrote = true;
            }
        }
        if wrote {
            self.commit_with_label("set_fill_mode");
        }
        Ok(())
    }

    /// Adds `stop` to the object's gradient, in **one commit**, inserted
    /// before the first stop whose position is greater, so a tie goes after
    /// the existing equal stops (acceptance criterion 18). The caller chose
    /// the position, colour and opacity (the ramp's own at that position).
    ///
    /// # Errors
    /// [`StyleEditError::NoSuchObject`]; [`StyleEditError::TooManyStops`] at
    /// 16 or more stops; [`StyleEditError::DuplicateStop`] if the id is
    /// already in the list.
    pub fn add_stop(&self, id: NodeId, stop: GradientStop) -> Result<(), StyleEditError> {
        let meta = self.style_meta(id)?;
        let list = ensure_stops_list(&meta);
        if list.len() >= MAX_GRADIENT_STOPS {
            return Err(StyleEditError::TooManyStops);
        }
        if stop_index(&list, stop.id).is_some() {
            return Err(StyleEditError::DuplicateStop);
        }
        let index = (0..list.len())
            .find(|&index| {
                stop_map_at(&list, index)
                    .and_then(|map| read_stop_map(&map))
                    .is_some_and(|existing| existing.position.get() > stop.position.get())
            })
            .unwrap_or_else(|| list.len());
        insert_stop_at(&list, index, &stop);
        self.commit_with_label("add_stop");
        Ok(())
    }

    /// Removes one stop, in **one commit** (acceptance criterion 19).
    ///
    /// # Errors
    /// [`StyleEditError::NoSuchObject`]; [`StyleEditError::NoSuchStop`];
    /// [`StyleEditError::TooFewStops`] when the gradient holds 2 stops or
    /// fewer.
    ///
    /// # Panics
    /// Does not panic in practice: the index deleted was just read from the
    /// same list.
    pub fn remove_stop(&self, id: NodeId, stop: StopId) -> Result<(), StyleEditError> {
        let meta = self.style_meta(id)?;
        let list = stops_list(&meta).ok_or(StyleEditError::NoSuchStop)?;
        let index = stop_index(&list, stop).ok_or(StyleEditError::NoSuchStop)?;
        if list.len() <= MIN_GRADIENT_STOPS {
            return Err(StyleEditError::TooFewStops);
        }
        // invariant: `index` was just read from this same list.
        #[allow(clippy::unwrap_used)]
        list.delete(index, 1).unwrap();
        self.commit_with_label("remove_stop");
        Ok(())
    }

    /// Edits stop values, in **one commit** for the whole batch (acceptance
    /// criteria 20, 34): only the named value of each named stop is written,
    /// and only if it differs from the stored one. A batch that changes
    /// nothing makes no commit.
    ///
    /// # Errors
    /// [`StyleEditError::NoSuchObject`] or [`StyleEditError::NoSuchStop`] if
    /// any target is gone, refusing the whole call.
    pub fn edit_stops(&self, edits: &[StopEdit]) -> Result<(), StyleEditError> {
        let resolved: Vec<(LoroMap, &StopEdit)> = edits
            .iter()
            .map(|edit| {
                let meta = self.style_meta(edit.id)?;
                let list = stops_list(&meta).ok_or(StyleEditError::NoSuchStop)?;
                let index = stop_index(&list, edit.stop).ok_or(StyleEditError::NoSuchStop)?;
                let map = stop_map_at(&list, index).ok_or(StyleEditError::NoSuchStop)?;
                Ok((map, edit))
            })
            .collect::<Result<_, StyleEditError>>()?;
        let mut wrote = false;
        for (map, edit) in &resolved {
            let Some(current) = read_stop_map(map) else {
                continue;
            };
            match edit.change {
                StopChange::Position(position) if current.position != position => {
                    write_stop_position(map, position);
                    wrote = true;
                }
                StopChange::Color(color) if current.color != color => {
                    write_stop_color(map, color);
                    wrote = true;
                }
                StopChange::Opacity(opacity) if current.opacity != opacity => {
                    write_stop_opacity(map, opacity);
                    wrote = true;
                }
                _ => {}
            }
        }
        if wrote {
            self.commit_with_label("edit_stops");
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

    #[test]
    fn set_fill_mode_and_the_stop_commands_are_one_commit_each() {
        let document = Document::new(1);
        let a = rect(&document);
        let stops = GradientStop::default_pair(Color::BLACK, StopId::new(1, 1), StopId::new(1, 2));
        let (_, changes) = counters(&document);
        document
            .set_fill_mode(
                FillMode::Linear,
                &[FillModeTarget {
                    id: a,
                    seed_stops: stops.to_vec(),
                }],
            )
            .unwrap();
        assert_eq!(counters(&document).1 - changes, 1);
        let before = counters(&document);
        document
            .set_fill_mode(
                FillMode::Linear,
                &[FillModeTarget {
                    id: a,
                    seed_stops: stops.to_vec(),
                }],
            )
            .unwrap();
        assert_eq!(
            counters(&document),
            before,
            "same mode again writes nothing"
        );
    }

    #[test]
    fn a_failed_stop_command_writes_nothing() {
        let document = Document::new(1);
        let a = rect(&document);
        let stops = GradientStop::default_pair(Color::BLACK, StopId::new(1, 1), StopId::new(1, 2));
        document
            .set_fill_mode(
                FillMode::Linear,
                &[FillModeTarget {
                    id: a,
                    seed_stops: stops.to_vec(),
                }],
            )
            .unwrap();
        let before = counters(&document);
        assert_eq!(
            document.remove_stop(a, StopId::new(1, 1)),
            Err(StyleEditError::TooFewStops)
        );
        assert_eq!(
            document.remove_stop(a, StopId::new(9, 9)),
            Err(StyleEditError::NoSuchStop)
        );
        assert_eq!(
            document.add_stop(a, stops[0]),
            Err(StyleEditError::DuplicateStop)
        );
        let edits = [
            StopEdit {
                id: a,
                stop: StopId::new(1, 1),
                change: StopChange::Color(red()),
            },
            StopEdit {
                id: a,
                stop: StopId::new(9, 9),
                change: StopChange::Color(red()),
            },
        ];
        assert_eq!(document.edit_stops(&edits), Err(StyleEditError::NoSuchStop));
        assert_eq!(counters(&document), before);
    }
}
