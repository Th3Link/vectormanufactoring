//! The gradient stop editor's state and the rules of its commands
//! (`specs/0007-stroke-and-fill-styling` criteria 16 to 20, 34, 35): what the
//! stop list and the gradient bar show for the edited objects, which stops a
//! rank names, and the values a new stop gets. Pure functions of the
//! snapshots, so the DOM holds no editing logic.

use curvyo_document_core::{
    Color, Fill, FillMode, FillModeTarget, GradientStop, MAX_GRADIENT_STOPS, MIN_GRADIENT_STOPS,
    NodeId, ObjectSnapshot, Opacity, PrimitiveSnapshot, Shape, StopChange, StopEdit, StopId,
    StopPosition, ramp_at, sorted_stops,
};

use crate::anchor_id_minter::AnchorIdMinter;
use crate::select_bar::BarValue;
use crate::style_panel::fill_mode;

/// A position typed or clicked is stored to 0.1 %: 1000 steps per unit.
const POSITION_STEPS_PER_UNIT: f64 = 1000.0;

/// One row of the stop list: the stop of one rank in every edited object.
#[derive(Debug, Clone, PartialEq)]
pub struct StopRowView {
    /// The position.
    pub position: BarValue<StopPosition>,
    /// The colour.
    pub color: BarValue<Color>,
    /// The stop's own opacity.
    pub opacity: BarValue<Opacity>,
}

/// One stop of the gradient bar, in position order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarStop {
    /// The position.
    pub position: StopPosition,
    /// The colour.
    pub color: Color,
    /// The opacity.
    pub opacity: Opacity,
}

/// What the stop editor shows when it is shown.
#[derive(Debug, Clone, PartialEq)]
pub struct StopEditorView {
    /// One row per stop, in rank order (position, ties in list order). Empty
    /// for a gradient with no stops.
    pub rows: Vec<StopRowView>,
    /// How many objects are edited.
    pub objects: usize,
    /// The ramp and its thumbs, when every edited object's list is equal in
    /// value; `None` is a neutral hatched track with no thumbs.
    pub bar: Option<Vec<BarStop>>,
    /// Add stop and a bar click work: one object, fewer than the maximum.
    pub can_add: bool,
    /// Remove works: one object, more than the minimum.
    pub can_remove: bool,
    /// The selection holds a polygon or star, whose gradient box is the square
    /// around it, not a tight box (criterion 21).
    pub box_note: bool,
}

/// The state of the stop editor.
#[derive(Debug, Clone, PartialEq)]
pub enum StopsPanel {
    /// No editor: not every edited object is in the same gradient mode.
    Hidden,
    /// Every edited object is in the same gradient mode, with different stop
    /// counts: the editor is replaced by a message.
    DifferentCounts,
    /// The editor.
    Editor(StopEditorView),
}

fn edited<'a>(objects: &'a [ObjectSnapshot], ids: &[NodeId]) -> Vec<&'a ObjectSnapshot> {
    ids.iter()
        .filter_map(|id| objects.iter().find(|object| object.id() == *id))
        .collect()
}

fn sorted_of(object: &ObjectSnapshot) -> Vec<GradientStop> {
    sorted_stops(&object.style().fill.stops)
}

fn shared<T: Copy + PartialEq>(values: impl Iterator<Item = T>) -> BarValue<T> {
    let values: Vec<T> = values.collect();
    match values.split_first() {
        Some((first, rest)) if rest.iter().all(|v| v == first) => BarValue::Uniform(*first),
        _ => BarValue::Mixed,
    }
}

fn is_polygon_or_star(object: &ObjectSnapshot) -> bool {
    matches!(
        object,
        ObjectSnapshot::Primitive(PrimitiveSnapshot {
            shape: Shape::Polygon { .. } | Shape::Star { .. },
            ..
        })
    )
}

/// The stop editor's state for the objects `ids` out of `objects`.
#[must_use]
pub fn stops_panel(objects: &[ObjectSnapshot], ids: &[NodeId]) -> StopsPanel {
    let objects_edited = edited(objects, ids);
    let Some(first) = objects_edited.first() else {
        return StopsPanel::Hidden;
    };
    let mode = fill_mode(first.style());
    let same_gradient_mode = matches!(mode, FillMode::Linear | FillMode::Radial)
        && objects_edited
            .iter()
            .all(|object| fill_mode(object.style()) == mode);
    if !same_gradient_mode {
        return StopsPanel::Hidden;
    }
    let lists: Vec<Vec<GradientStop>> = objects_edited.iter().map(|o| sorted_of(o)).collect();
    let count = lists[0].len();
    if lists.iter().any(|list| list.len() != count) {
        return StopsPanel::DifferentCounts;
    }
    let rows = (0..count)
        .map(|rank| StopRowView {
            position: shared(lists.iter().map(|list| list[rank].position)),
            color: shared(lists.iter().map(|list| list[rank].color)),
            opacity: shared(lists.iter().map(|list| list[rank].opacity)),
        })
        .collect();
    let identical = lists.iter().all(|list| {
        list.iter()
            .zip(&lists[0])
            .all(|(a, b)| a.position == b.position && a.color == b.color && a.opacity == b.opacity)
    });
    let bar = identical.then(|| {
        lists[0]
            .iter()
            .map(|stop| BarStop {
                position: stop.position,
                color: stop.color,
                opacity: stop.opacity,
            })
            .collect()
    });
    let single = objects_edited.len() == 1;
    StopsPanel::Editor(StopEditorView {
        rows,
        objects: objects_edited.len(),
        bar,
        can_add: single && count < MAX_GRADIENT_STOPS,
        can_remove: single && count > MIN_GRADIENT_STOPS,
        box_note: objects_edited.iter().any(|o| is_polygon_or_star(o)),
    })
}

/// The fill-mode targets for `ids`: every object, with the two default stops
/// (criterion 17) for those that hold none, each built from the object's own
/// stored fill colour and two fresh ids.
pub fn fill_targets(
    minter: &mut AnchorIdMinter,
    objects: &[ObjectSnapshot],
    ids: &[NodeId],
) -> Vec<FillModeTarget> {
    edited(objects, ids)
        .into_iter()
        .map(|object| {
            let fill = &object.style().fill;
            let seed_stops = if fill.stops.is_empty() {
                GradientStop::default_pair(fill.color, minter.mint_stop(), minter.mint_stop())
                    .to_vec()
            } else {
                Vec::new()
            };
            FillModeTarget {
                id: object.id(),
                seed_stops,
            }
        })
        .collect()
}

/// The values of a stop about to be added.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NewStop {
    /// Where it goes.
    pub position: StopPosition,
    /// Its colour.
    pub color: Color,
    /// Its opacity.
    pub opacity: Opacity,
}

fn rounded_position(value: f64) -> StopPosition {
    let clamped = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let stepped = (clamped * POSITION_STEPS_PER_UNIT).round() / POSITION_STEPS_PER_UNIT;
    // Rounding a value within 0 to 1 stays within 0 to 1 up to float noise.
    StopPosition::new(stepped.clamp(0.0, 1.0)).unwrap_or(StopPosition::START)
}

/// The midpoint of the widest gap between consecutive positions of `sorted`
/// (the gaps being 0 to the first stop, between neighbours, and the last stop to
/// 1); a tie goes to the gap nearest the start.
fn widest_gap_midpoint(sorted: &[GradientStop]) -> f64 {
    let mut edges = vec![0.0];
    edges.extend(sorted.iter().map(|stop| stop.position.get()));
    edges.push(1.0);
    let mut best = (f64::NEG_INFINITY, 0.5);
    for pair in edges.windows(2) {
        let width = pair[1] - pair[0];
        if width > best.0 {
            best = (width, f64::midpoint(pair[0], pair[1]));
        }
    }
    best.1
}

/// The stop the Add button (`at` is `None`) or a click on the bar at `at` adds
/// to `fill` (criterion 18): its position, and the colour and opacity the ramp
/// has there, so adding it changes nothing on screen. With no stops: one at 50 %
/// in the stored fill colour at full opacity.
#[must_use]
pub fn new_stop_values(fill: &Fill, at: Option<f64>) -> NewStop {
    let sorted = sorted_stops(&fill.stops);
    if sorted.is_empty() {
        return NewStop {
            position: rounded_position(at.unwrap_or(0.5)),
            color: fill.color,
            opacity: Opacity::OPAQUE,
        };
    }
    let position = rounded_position(at.unwrap_or_else(|| widest_gap_midpoint(&sorted)));
    let (color, opacity) =
        ramp_at(&sorted, position.get()).unwrap_or((fill.color, Opacity::OPAQUE));
    NewStop {
        position,
        color,
        opacity,
    }
}

/// The stop of rank `rank` of every object in `ids`, as `(object, stop)` pairs;
/// empty if any object has no stop of that rank. Rank is position order, ties
/// in list order (criterion 34).
#[must_use]
pub fn stop_targets(
    objects: &[ObjectSnapshot],
    ids: &[NodeId],
    rank: usize,
) -> Vec<(NodeId, StopId)> {
    let found: Option<Vec<(NodeId, StopId)>> = edited(objects, ids)
        .into_iter()
        .map(|object| {
            sorted_of(object)
                .get(rank)
                .map(|stop| (object.id(), stop.id))
        })
        .collect();
    found.unwrap_or_default()
}

/// The rank, in the first of `ids`, of the stop `targets` names for that
/// object; `None` when nothing is selected or the stop is gone. The selection
/// holds ids, so an edit that re-sorts the list moves the selection with its
/// stop.
#[must_use]
pub fn selected_rank(
    objects: &[ObjectSnapshot],
    ids: &[NodeId],
    targets: &[(NodeId, StopId)],
) -> Option<usize> {
    let object = edited(objects, ids).into_iter().next()?;
    let (_, stop) = targets.iter().find(|(id, _)| *id == object.id())?;
    sorted_of(object).iter().position(|s| s.id == *stop)
}

/// One [`StopChange`] for each of `targets`.
#[must_use]
pub fn stop_edits(targets: &[(NodeId, StopId)], change: StopChange) -> Vec<StopEdit> {
    targets
        .iter()
        .map(|&(id, stop)| StopEdit { id, stop, change })
        .collect()
}
