//! What the Style panel shows and which objects it edits
//! (`specs/0007-stroke-and-fill-styling` criteria 1, 5, 13, 24, 37): the scope
//! per tool, the subject line, and every property as one shared value or
//! "Mixed". A pure function of the snapshots and the selections, so the DOM
//! holds no editing logic.

use curvyo_document_core::{
    Color, DashPattern, FillKind, FillMode, Length, LineCap, LineJoin, NodeId, ObjectSnapshot,
    Opacity, PrimitiveSnapshot, Shape, Style,
};

use crate::hit_test_object::style_of;
use crate::object_selection::ObjectSelection;
use crate::select_bar::BarValue;
use crate::selection::NodeSelection;

/// Two stroke widths closer than this are one value, millimetres.
const WIDTH_EQUAL_EPSILON_MM: f64 = 1e-9;
/// Two dash lengths (multiples of the width) closer than this are one value.
const DASH_EQUAL_EPSILON: f64 = 1e-9;

/// The tool the panel is scoped by (criterion 37). The creation tools and
/// Select all edit the object selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleTool {
    /// The pen: the path being drawn does not exist yet; nothing to edit.
    Pen,
    /// The node tool: the paths that own the selected nodes.
    Node,
    /// Select and the rectangle, ellipse and polygon/star tools.
    Other,
}

/// The objects the panel edits and the line that says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleScope {
    /// The objects an edit goes to; empty when the panel is disabled.
    pub ids: Vec<NodeId>,
    /// "Nothing selected", "Pen: finish the path to style it", a kind
    /// ("Rectangle") or a count ("3 rectangles", "4 objects", "2 paths").
    pub subject: String,
}

/// The scope of the panel for `tool` (criterion 37). Ids the document no
/// longer holds are dropped.
#[must_use]
pub fn style_scope(
    tool: StyleTool,
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    nodes: &NodeSelection,
) -> StyleScope {
    if tool == StyleTool::Pen {
        return StyleScope {
            ids: Vec::new(),
            subject: "Pen: finish the path to style it".to_string(),
        };
    }
    let wanted: Vec<NodeId> = if tool == StyleTool::Node {
        let owners = node_owners(nodes);
        if owners.is_empty() {
            // The path being edited: the paths of the object selection.
            selection
                .ids()
                .iter()
                .copied()
                .filter(|id| {
                    objects
                        .iter()
                        .any(|o| o.id() == *id && matches!(o, ObjectSnapshot::Path(_)))
                })
                .collect()
        } else {
            owners
        }
    } else {
        selection.ids().to_vec()
    };
    let held: Vec<&ObjectSnapshot> = wanted
        .iter()
        .filter_map(|id| objects.iter().find(|object| object.id() == *id))
        .collect();
    StyleScope {
        ids: held.iter().map(|object| object.id()).collect(),
        subject: subject_line(&held),
    }
}

/// The paths owning the selected nodes or the selected segment, in selection
/// order, once each.
fn node_owners(nodes: &NodeSelection) -> Vec<NodeId> {
    let mut owners: Vec<NodeId> = Vec::new();
    let paths = nodes.node_pairs().iter().map(|&(path, _)| path);
    let segment = nodes.segment_with_path().map(|(path, _, _)| path);
    for path in paths.chain(segment) {
        if !owners.contains(&path) {
            owners.push(path);
        }
    }
    owners
}

fn kind_name(object: &ObjectSnapshot) -> (&'static str, &'static str) {
    match object {
        ObjectSnapshot::Path(_) => ("Path", "paths"),
        ObjectSnapshot::Primitive(PrimitiveSnapshot { shape, .. }) => match shape {
            Shape::Rect { .. } => ("Rectangle", "rectangles"),
            Shape::Ellipse { .. } => ("Ellipse", "ellipses"),
            Shape::Polygon { .. } => ("Polygon", "polygons"),
            Shape::Star { .. } => ("Star", "stars"),
        },
    }
}

fn subject_line(objects: &[&ObjectSnapshot]) -> String {
    let Some(first) = objects.first() else {
        return "Nothing selected".to_string();
    };
    let (singular, plural) = kind_name(first);
    let same_kind = objects.iter().all(|object| kind_name(object).0 == singular);
    match (objects.len(), same_kind) {
        (1, _) => singular.to_string(),
        (count, true) => format!("{count} {plural}"),
        (count, false) => format!("{count} objects"),
    }
}

/// The named dash patterns the panel offers, plus "Custom" for a stored
/// pattern that is none of them (criterion 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashChoice {
    /// No dashes.
    Solid,
    /// `[6, 4]`.
    Dash,
    /// `[1, 3]`.
    Dot,
    /// `[6, 3, 1, 3]`.
    DashDot,
    /// Any other stored pattern: shown, never offered as a choice.
    Custom,
}

impl DashChoice {
    const PRESETS: [(Self, &'static [f64]); 4] = [
        (Self::Solid, &[]),
        (Self::Dash, &[6.0, 4.0]),
        (Self::Dot, &[1.0, 3.0]),
        (Self::DashDot, &[6.0, 3.0, 1.0, 3.0]),
    ];

    /// The choice a stored pattern is.
    #[must_use]
    pub fn from_pattern(pattern: &DashPattern) -> Self {
        Self::PRESETS
            .iter()
            .find(|(_, lengths)| same_dash(pattern.as_slice(), lengths))
            .map_or(Self::Custom, |(choice, _)| *choice)
    }

    /// The pattern a choice stores, `None` for [`DashChoice::Custom`].
    #[must_use]
    pub fn pattern(self) -> Option<DashPattern> {
        let (_, lengths) = Self::PRESETS.iter().find(|(choice, _)| *choice == self)?;
        DashPattern::new(lengths.to_vec()).ok()
    }

    /// The choice named by the host (`"solid"`, `"dash"`, `"dot"`,
    /// `"dash-dot"`); `Custom` is not a choice.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "solid" => Some(Self::Solid),
            "dash" => Some(Self::Dash),
            "dot" => Some(Self::Dot),
            "dash-dot" => Some(Self::DashDot),
            _ => None,
        }
    }
}

fn same_dash(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= DASH_EQUAL_EPSILON)
}

/// Parses the host's join word.
#[must_use]
pub fn join_from_name(name: &str) -> Option<LineJoin> {
    match name {
        "miter" => Some(LineJoin::Miter),
        "round" => Some(LineJoin::Round),
        "bevel" => Some(LineJoin::Bevel),
        _ => None,
    }
}

/// Parses the host's cap word.
#[must_use]
pub fn cap_from_name(name: &str) -> Option<LineCap> {
    match name {
        "butt" => Some(LineCap::Butt),
        "round" => Some(LineCap::Round),
        "square" => Some(LineCap::Square),
        _ => None,
    }
}

/// Parses the host's fill-mode word.
#[must_use]
pub fn fill_mode_from_name(name: &str) -> Option<FillMode> {
    match name {
        "none" => Some(FillMode::None),
        "solid" => Some(FillMode::Solid),
        "linear" => Some(FillMode::Linear),
        "radial" => Some(FillMode::Radial),
        _ => None,
    }
}

/// The stroke rows of the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokePanel {
    /// The Paint switch: on, off, or "no state pressed".
    pub paint: BarValue<bool>,
    /// Every edited object has its stroke off: Dash, Join and Cap are disabled
    /// because they would have no visible effect (criterion 5).
    pub all_off: bool,
    /// The colour.
    pub color: BarValue<Color>,
    /// The opacity.
    pub opacity: BarValue<Opacity>,
    /// The stored width, shown also while the stroke is off.
    pub width: BarValue<Length>,
    /// The dash pattern.
    pub dash: BarValue<DashChoice>,
    /// The join.
    pub join: BarValue<LineJoin>,
    /// The cap.
    pub cap: BarValue<LineCap>,
}

/// The fill rows of the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct FillPanel {
    /// The Fill type row. A mode the panel has no button for yet still reads
    /// as itself.
    pub mode: BarValue<FillMode>,
    /// The solid colour.
    pub color: BarValue<Color>,
    /// The solid opacity.
    pub opacity: BarValue<Opacity>,
}

/// Everything the panel shows.
#[derive(Debug, Clone, PartialEq)]
pub struct StylePanelState {
    /// The subject line.
    pub subject: String,
    /// Whether there is anything to edit; when not, every control is disabled
    /// and shows the frozen defaults.
    pub enabled: bool,
    /// The stroke rows.
    pub stroke: StrokePanel,
    /// The fill rows.
    pub fill: FillPanel,
}

/// The one value of `values` if they all agree, else `Mixed`. The caller
/// always passes at least one value (the disabled panel passes the defaults).
fn shared_by<T: Copy>(values: &[T], same: impl Fn(T, T) -> bool) -> BarValue<T> {
    match values.split_first() {
        Some((first, rest)) if rest.iter().all(|v| same(*first, *v)) => BarValue::Uniform(*first),
        _ => BarValue::Mixed,
    }
}

fn shared<T: Copy + PartialEq>(values: &[T]) -> BarValue<T> {
    shared_by(values, |a, b| a == b)
}

fn fill_mode(style: &Style) -> FillMode {
    match (style.fill.enabled, style.fill.kind) {
        (false, _) => FillMode::None,
        (true, FillKind::Solid) => FillMode::Solid,
        (true, FillKind::Linear) => FillMode::Linear,
        (true, FillKind::Radial) => FillMode::Radial,
    }
}

/// What the panel shows for the objects of `scope` out of `objects`. With
/// nothing to edit it shows the frozen defaults, disabled.
#[must_use]
pub fn style_panel_state(objects: &[ObjectSnapshot], scope: &StyleScope) -> StylePanelState {
    let defaults = Style::default();
    let mut styles: Vec<&Style> = scope
        .ids
        .iter()
        .filter_map(|id| objects.iter().find(|object| object.id() == *id))
        .map(style_of)
        .collect();
    let enabled = !styles.is_empty();
    if !enabled {
        styles.push(&defaults);
    }
    StylePanelState {
        subject: scope.subject.clone(),
        enabled,
        stroke: StrokePanel {
            paint: shared(&column(&styles, |s| s.stroke.enabled)),
            all_off: enabled && styles.iter().all(|s| !s.stroke.enabled),
            color: shared(&column(&styles, |s| s.stroke.color)),
            opacity: shared(&column(&styles, |s| s.stroke.opacity)),
            width: shared_by(&column(&styles, |s| s.stroke.width), |a: Length, b| {
                (a.as_mm() - b.as_mm()).abs() <= WIDTH_EQUAL_EPSILON_MM
            }),
            dash: dash_of(&styles),
            join: shared(&column(&styles, |s| s.stroke.join)),
            cap: shared(&column(&styles, |s| s.stroke.cap)),
        },
        fill: FillPanel {
            mode: shared(&column(&styles, fill_mode)),
            color: shared(&column(&styles, |s| s.fill.color)),
            opacity: shared(&column(&styles, |s| s.fill.opacity)),
        },
    }
}

fn column<T>(styles: &[&Style], pick: impl Fn(&Style) -> T) -> Vec<T> {
    styles.iter().map(|style| pick(style)).collect()
}

fn dash_of(styles: &[&Style]) -> BarValue<DashChoice> {
    let patterns: Vec<&DashPattern> = styles.iter().map(|s| &s.stroke.dash).collect();
    match shared_by(&patterns, |a, b| same_dash(a.as_slice(), b.as_slice())) {
        BarValue::Uniform(pattern) => BarValue::Uniform(DashChoice::from_pattern(pattern)),
        BarValue::Mixed => BarValue::Mixed,
    }
}
