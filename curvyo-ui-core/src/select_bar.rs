//! What the Select tool's top bar shows and which objects each of its kind
//! controls acts on (`specs/0009-unified-object-editing/specification.md`, criteria
//! 21, 21a and 22): one rule for all of them. A control is shown when the
//! selection contains at least one object of the kind it acts on, it acts on
//! exactly those objects, and it is enabled when it would change something.
//! The state is a pure function of the document snapshots, the selection and
//! a pending slider edit, so the DOM holds no editing logic.

use curvyo_document_core::{
    Corner, CornerRadii, Length, NodeId, ObjectSnapshot, PrimitiveSnapshot, RectBounds,
    SHARP_CORNER_EPSILON_MM, Shape, effective_corner_radii,
};

use crate::object_selection::ObjectSelection;
use crate::param_edit::{PARAM_EQUAL_EPSILON, ParamValue};

/// The kinds of object a bar control acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    /// A rectangle: "Radius" and "Remove rounding".
    Rectangle,
    /// A polygon or a star: "Points".
    PolygonOrStar,
    /// A star: "Ratio".
    Star,
    /// A rectangle, ellipse, polygon or star: "Object to path".
    Primitive,
}

impl ObjectKind {
    fn matches(self, object: &ObjectSnapshot) -> bool {
        let ObjectSnapshot::Primitive(PrimitiveSnapshot { shape, .. }) = object else {
            return false;
        };
        match self {
            Self::Rectangle => matches!(shape, Shape::Rect { .. }),
            Self::PolygonOrStar => matches!(shape, Shape::Polygon { .. } | Shape::Star { .. }),
            Self::Star => matches!(shape, Shape::Star { .. }),
            Self::Primitive => true,
        }
    }
}

/// The selected objects of `kind`, in selection order: the objects a bar
/// control acts on and the ones that make it show. Ids the document no longer
/// holds are dropped.
#[must_use]
pub fn ids_of_kind(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    kind: ObjectKind,
) -> Vec<NodeId> {
    selection
        .ids()
        .iter()
        .filter_map(|id| objects.iter().find(|object| object.id() == *id))
        .filter(|object| kind.matches(object))
        .map(ObjectSnapshot::id)
        .collect()
}

/// A bar value over the objects a control acts on: one shared value, or
/// "Mixed" (the field is empty with a muted placeholder and a typed value
/// applies to all of them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarValue<T> {
    /// Every acted-on object holds this value.
    Uniform(T),
    /// They differ.
    Mixed,
}

/// A slider edit in flight: the value being previewed and the objects it was
/// started against. It is flushed (committed once) before the selection can
/// change, so it never lands on another object.
#[derive(Debug, Clone, PartialEq)]
pub struct BarPreview {
    /// The objects the edit acts on, fixed when it started.
    pub ids: Vec<NodeId>,
    /// The previewed value.
    pub value: ParamValue,
}

/// What the Select bar shows for the current selection. The two switches are
/// session state, not part of it.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // independent shown/enabled flags, read by name
pub struct SelectBarState {
    /// "Radius" (criterion 21a): the effective radius of the selected
    /// rectangles, `None` when the selection holds none. Always enabled.
    pub radius: Option<BarValue<Length>>,
    /// For a uniform "Radius": the stored value when it exceeds what the
    /// rectangle allows, so the field can tag "limited" and name it.
    pub radius_limited: Option<Length>,
    /// For one selected rectangle whose four effective radii differ (the field
    /// reads "Mixed"): those radii in the rectangle's own frame, for the tooltip
    /// ("Top-left 12, top-right 0, ...", `specs/0013-rectangle-corner-radii/`
    /// criterion 22). `None` otherwise.
    pub radius_corners: Option<CornerRadii>,
    /// "Remove rounding" is shown: the selection holds a rectangle.
    pub remove_rounding_shown: bool,
    /// "Remove rounding" would change something: a selected rectangle has a
    /// stored radius above 0.
    pub remove_rounding_enabled: bool,
    /// "Points" (3 to 1024), `None` when the selection holds no polygon or star.
    pub points: Option<BarValue<u32>>,
    /// "Ratio" (0.01 to 0.99), `None` when the selection holds no star.
    pub ratio: Option<BarValue<f64>>,
    /// "Object to path" is shown: the selection holds a rectangle, ellipse,
    /// polygon or star. Always enabled.
    pub object_to_path: bool,
}

/// The one value of `values` if they all agree within `eps`, else `Mixed`.
fn uniform<T: Copy>(values: &[T], same: impl Fn(T, T) -> bool) -> Option<BarValue<T>> {
    let (first, rest) = values.split_first()?;
    Some(if rest.iter().all(|v| same(*first, *v)) {
        BarValue::Uniform(*first)
    } else {
        BarValue::Mixed
    })
}

fn shapes_of<'a>(
    objects: &'a [ObjectSnapshot],
    ids: &'a [NodeId],
) -> impl Iterator<Item = &'a Shape> + 'a {
    ids.iter()
        .filter_map(|id| objects.iter().find(|object| object.id() == *id))
        .filter_map(|object| match object {
            ObjectSnapshot::Primitive(primitive) => Some(&primitive.shape),
            ObjectSnapshot::Path(_) => None,
        })
}

/// What the bar's rectangle controls show for the selected rectangles `ids`.
struct RadiusState {
    value: Option<BarValue<Length>>,
    limited: Option<Length>,
    corners: Option<CornerRadii>,
    /// Some stored corner radius is above the sharp tolerance.
    rounded: bool,
}

/// The "Radius" field and "Remove rounding" state of the rectangles `ids`
/// (criteria 21a, 22): Uniform when every effective radius of every rectangle
/// (four each) is equal, Mixed otherwise; the "limited" tag for a Uniform value
/// only; the four effective radii for one rectangle that reads Mixed.
fn radius_state(objects: &[ObjectSnapshot], ids: &[NodeId]) -> RadiusState {
    let rectangles: Vec<(RectBounds, CornerRadii)> = shapes_of(objects, ids)
        .filter_map(|shape| match *shape {
            Shape::Rect {
                bounds,
                corner_radii,
            } => Some((bounds, corner_radii)),
            _ => None,
        })
        .collect();
    // One (effective, stored) pair per corner of every selected rectangle.
    let pairs: Vec<(f64, f64)> = rectangles
        .iter()
        .flat_map(|&(bounds, corner_radii)| {
            let effective = effective_corner_radii(bounds, corner_radii);
            Corner::ALL.map(|corner| {
                (
                    effective.get(corner).as_mm(),
                    corner_radii.get(corner).as_mm(),
                )
            })
        })
        .collect();
    let effective: Vec<f64> = pairs.iter().map(|(effective, _)| *effective).collect();
    let value =
        uniform(&effective, |a, b| (a - b).abs() <= PARAM_EQUAL_EPSILON).map(|value| match value {
            BarValue::Uniform(mm) => BarValue::Uniform(Length::from_mm(mm)),
            BarValue::Mixed => BarValue::Mixed,
        });
    let limited = match value {
        Some(BarValue::Uniform(_)) => pairs
            .iter()
            .find(|(effective, stored)| stored - effective > SHARP_CORNER_EPSILON_MM)
            .map(|(_, stored)| Length::from_mm(*stored)),
        _ => None,
    };
    let corners = match (value, &rectangles[..]) {
        (Some(BarValue::Mixed), &[(bounds, corner_radii)]) => {
            Some(effective_corner_radii(bounds, corner_radii))
        }
        _ => None,
    };
    RadiusState {
        value,
        limited,
        corners,
        rounded: pairs
            .iter()
            .any(|(_, stored)| *stored > SHARP_CORNER_EPSILON_MM),
    }
}

/// The Select bar's state for `selection`, showing a pending slider edit
/// `preview` as if it were committed.
#[must_use]
pub fn select_bar_state(
    objects: &[ObjectSnapshot],
    selection: &ObjectSelection,
    preview: Option<&BarPreview>,
) -> SelectBarState {
    let rectangles = ids_of_kind(objects, selection, ObjectKind::Rectangle);
    let polygons_and_stars = ids_of_kind(objects, selection, ObjectKind::PolygonOrStar);
    let stars = ids_of_kind(objects, selection, ObjectKind::Star);

    let radius = radius_state(objects, &rectangles);

    let counts: Vec<u32> = shapes_of(objects, &polygons_and_stars)
        .filter_map(|shape| match *shape {
            Shape::Polygon { point_count, .. } | Shape::Star { point_count, .. } => {
                Some(point_count.get())
            }
            _ => None,
        })
        .collect();
    let ratios: Vec<f64> = shapes_of(objects, &stars)
        .filter_map(|shape| match *shape {
            Shape::Star { inner_ratio, .. } => Some(inner_ratio.get()),
            _ => None,
        })
        .collect();
    let mut points = uniform(&counts, |a, b| a == b);
    let mut ratio = uniform(&ratios, |a, b| (a - b).abs() <= PARAM_EQUAL_EPSILON);
    if let Some(preview) = preview {
        match preview.value {
            ParamValue::PointCount(count) if points.is_some() => {
                points = Some(BarValue::Uniform(count.get()));
            }
            ParamValue::Ratio(value) if ratio.is_some() => {
                ratio = Some(BarValue::Uniform(value.get()));
            }
            _ => {}
        }
    }

    SelectBarState {
        radius: radius.value,
        radius_limited: radius.limited,
        radius_corners: radius.corners,
        remove_rounding_shown: !rectangles.is_empty(),
        remove_rounding_enabled: radius.rounded,
        points,
        ratio,
        object_to_path: !ids_of_kind(objects, selection, ObjectKind::Primitive).is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curvyo_document_core::{
        AnchorId, Angle, CornerRadii, Document, InnerRatio, NewAnchor, Point, PointCount,
        RectBounds, StarFrame,
    };

    struct Scene {
        document: Document,
    }

    impl Scene {
        fn new() -> Self {
            Self {
                document: Document::new(1),
            }
        }

        fn rect(&self, size: f64, radius: f64) -> NodeId {
            let id = self.document.create_rect(RectBounds {
                origin: Point::new(0.0, 0.0),
                width: Length::from_mm(size),
                height: Length::from_mm(size),
            });
            self.document
                .set_corner_radius(&[id], Length::from_mm(radius))
                .unwrap();
            id
        }

        fn star(&self, points: u32, ratio: f64) -> NodeId {
            self.document.create_star(
                StarFrame {
                    center: Point::new(0.0, 0.0),
                    radius: Length::from_mm(10.0),
                    angle: Angle::from_radians(0.0),
                },
                PointCount::new(points).unwrap(),
                InnerRatio::new(ratio).unwrap(),
            )
        }

        fn polygon(&self, points: u32) -> NodeId {
            self.document.create_polygon(
                StarFrame {
                    center: Point::new(0.0, 0.0),
                    radius: Length::from_mm(10.0),
                    angle: Angle::from_radians(0.0),
                },
                PointCount::new(points).unwrap(),
            )
        }

        fn path(&self) -> NodeId {
            self.document.create_path(
                &[
                    NewAnchor::corner(AnchorId::new(1, 1), Point::new(0.0, 0.0)),
                    NewAnchor::corner(AnchorId::new(1, 2), Point::new(5.0, 0.0)),
                ],
                false,
            )
        }

        fn ellipse(&self) -> NodeId {
            self.document
                .create_ellipse(curvyo_document_core::EllipseFrame {
                    center: Point::new(0.0, 0.0),
                    rx: Length::from_mm(4.0),
                    ry: Length::from_mm(3.0),
                })
        }

        fn state(&self, ids: &[NodeId]) -> SelectBarState {
            let objects: Vec<ObjectSnapshot> = self
                .document
                .object_ids()
                .into_iter()
                .filter_map(|id| self.document.object(id))
                .collect();
            let mut selection = ObjectSelection::new();
            for id in ids {
                selection.toggle(*id);
            }
            select_bar_state(&objects, &selection, None)
        }
    }

    #[test]
    fn an_empty_selection_shows_no_kind_control() {
        let scene = Scene::new();
        scene.rect(10.0, 0.0);
        let state = scene.state(&[]);
        assert_eq!(state.radius, None);
        assert!(!state.remove_rounding_shown && !state.object_to_path);
        assert_eq!((state.points, state.ratio), (None, None));
    }

    /// Criteria 21, 21a, 22: each control shows when the selection contains
    /// its kind; with a rectangle and a star the bar shows Radius, Remove
    /// rounding, Points and Ratio; a path in the selection is left alone.
    #[test]
    fn a_control_shows_when_the_selection_contains_its_kind() {
        let scene = Scene::new();
        let (rect, star, polygon, path, ellipse) = (
            scene.rect(20.0, 0.0),
            scene.star(5, 0.5),
            scene.polygon(6),
            scene.path(),
            scene.ellipse(),
        );
        let only_rect = scene.state(&[rect]);
        assert!(only_rect.radius.is_some() && only_rect.remove_rounding_shown);
        assert!(only_rect.points.is_none() && only_rect.ratio.is_none());
        assert!(only_rect.object_to_path);

        let both = scene.state(&[rect, star]);
        assert!(both.radius.is_some() && both.remove_rounding_shown);
        assert!(both.points.is_some() && both.ratio.is_some());

        let polygon_state = scene.state(&[polygon]);
        assert!(polygon_state.points.is_some() && polygon_state.ratio.is_none());
        assert!(polygon_state.radius.is_none());

        let with_path = scene.state(&[path, star]);
        assert!(with_path.object_to_path, "the star in it converts");
        assert!(scene.state(&[path]).radius.is_none());
        assert!(!scene.state(&[path]).object_to_path, "a lone path has none");
        assert!(scene.state(&[ellipse]).object_to_path);
        assert!(scene.state(&[ellipse]).radius.is_none());
    }

    #[test]
    fn remove_rounding_is_enabled_only_when_a_selected_rectangle_has_a_radius() {
        let scene = Scene::new();
        let (sharp, round) = (scene.rect(20.0, 0.0), scene.rect(20.0, 3.0));
        assert!(!scene.state(&[sharp]).remove_rounding_enabled);
        assert!(
            scene.state(&[sharp]).remove_rounding_shown,
            "shown, disabled"
        );
        assert!(scene.state(&[round]).remove_rounding_enabled);
        assert!(scene.state(&[sharp, round]).remove_rounding_enabled);
    }

    #[test]
    fn differing_values_read_mixed_and_equal_ones_read_uniform() {
        let scene = Scene::new();
        let (a, b, c) = (
            scene.rect(20.0, 2.0),
            scene.rect(20.0, 2.0),
            scene.rect(20.0, 4.0),
        );
        assert_eq!(
            scene.state(&[a, b]).radius,
            Some(BarValue::Uniform(Length::from_mm(2.0)))
        );
        assert_eq!(scene.state(&[a, c]).radius, Some(BarValue::Mixed));
        let (s1, s2) = (scene.star(5, 0.4), scene.star(6, 0.4));
        assert_eq!(scene.state(&[s1, s2]).points, Some(BarValue::Mixed));
        assert_eq!(scene.state(&[s1, s2]).ratio, Some(BarValue::Uniform(0.4)));
    }

    /// A rectangle whose four effective radii differ (a file can hold one) reads
    /// Mixed, and the limited tag looks at every corner.
    #[test]
    fn one_rectangle_with_unequal_corners_reads_mixed() {
        let scene = Scene::new();
        let id = scene.rect(20.0, 2.0);
        scene
            .document
            .set_corner_radii(&[(
                id,
                CornerRadii {
                    tl: Length::from_mm(2.0),
                    tr: Length::from_mm(0.0),
                    br: Length::from_mm(2.0),
                    bl: Length::from_mm(2.0),
                },
            )])
            .unwrap();
        let state = scene.state(&[id]);
        assert_eq!(state.radius, Some(BarValue::Mixed));
        assert_eq!(state.radius_limited, None);
        assert!(state.remove_rounding_enabled);
        assert_eq!(
            state
                .radius_corners
                .map(|c| [c.tl, c.tr, c.br, c.bl].map(Length::as_mm)),
            Some([2.0, 0.0, 2.0, 2.0])
        );
        // Equal corners, or several rectangles: no corner list.
        let other = scene.rect(20.0, 4.0);
        assert_eq!(scene.state(&[id, other]).radius_corners, None);
        assert_eq!(scene.state(&[other]).radius_corners, None);
    }

    /// Criterion 21a: a stored radius larger than the rectangle allows shows
    /// the effective value and carries the stored one for the tooltip.
    #[test]
    fn a_limited_radius_shows_the_effective_value_and_names_the_stored_one() {
        let scene = Scene::new();
        let limited = scene.rect(20.0, 15.0); // half the side is 10
        let state = scene.state(&[limited]);
        assert_eq!(state.radius, Some(BarValue::Uniform(Length::from_mm(10.0))));
        assert_eq!(state.radius_limited, Some(Length::from_mm(15.0)));
        let fine = scene.rect(20.0, 4.0);
        assert_eq!(scene.state(&[fine]).radius_limited, None);
    }

    #[test]
    fn ids_of_kind_keeps_selection_order_and_only_the_kind() {
        let scene = Scene::new();
        let (a, star, b, path) = (
            scene.rect(10.0, 0.0),
            scene.star(5, 0.5),
            scene.rect(10.0, 0.0),
            scene.path(),
        );
        let objects: Vec<ObjectSnapshot> = [a, star, b, path]
            .iter()
            .filter_map(|id| scene.document.object(*id))
            .collect();
        let mut selection = ObjectSelection::new();
        for id in [b, star, path, a] {
            selection.toggle(id);
        }
        assert_eq!(
            ids_of_kind(&objects, &selection, ObjectKind::Rectangle),
            vec![b, a]
        );
        assert_eq!(
            ids_of_kind(&objects, &selection, ObjectKind::Star),
            vec![star]
        );
        assert_eq!(
            ids_of_kind(&objects, &selection, ObjectKind::Primitive),
            vec![b, star, a]
        );
    }

    #[test]
    fn a_pending_slider_edit_is_shown_as_if_committed() {
        let scene = Scene::new();
        let star = scene.star(5, 0.4);
        let objects = vec![scene.document.object(star).unwrap()];
        let mut selection = ObjectSelection::new();
        selection.select_single(star);
        let preview = BarPreview {
            ids: vec![star],
            value: ParamValue::Ratio(InnerRatio::new(0.7).unwrap()),
        };
        let state = select_bar_state(&objects, &selection, Some(&preview));
        assert_eq!(state.ratio, Some(BarValue::Uniform(0.7)));
    }
}
