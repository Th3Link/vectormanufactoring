//! The Loro value shapes and keys for the primitive-shape schema
//! (`specs/0003-primitive-shapes/adrs.md`, "the primitive schema"). Mirrors
//! [`crate::path_codec`]'s split for paths: this module owns every
//! read/write against a primitive node's meta map, so [`crate::shapes`]'s
//! `Document` methods stay command-shaped logic with no Loro value-shape
//! details of their own.
//!
//! On-disk/in-CRDT shape, one Loro tree node per primitive, sharing the
//! same tree as paths (ADR 0002 §5):
//!
//! ```text
//! meta map:
//!   shape         : "rect" | "ellipse" | "polygon" | "star"   written
//!                   once at creation, deleted by "object to path"
//!   (style keys    same keys/defaults as a path's, see `crate::style_codec`)
//!   rect:     rect_bounds   [x, y, w, h] mm        ONE LWW register
//!             corner_radius_tl/_tr/_br/_bl  mm, stored raw, one LWW
//!                           register per corner (`crate::corner_radii_codec`;
//!                           the legacy `corner_radius` of format_version <= 5
//!                           is read as a fallback, never written)
//!   ellipse:  ellipse_frame  [cx, cy, rx, ry]       ONE LWW register
//!   polygon:  star_frame     [cx, cy, r, theta]     ONE LWW register
//!             point_count    integer >= 3           LWW register
//!   star:     star_frame, point_count as polygon, plus
//!             inner_ratio    0 < r < 1               LWW register
//! ```
//!
//! **Unknown extra keys are tolerated** on both a path and a primitive
//! node (`adrs.md`'s "object to path" decision): a concurrent parameter
//! edit racing a conversion can leave a stray `rect_bounds` key on a
//! node that is now a path (its `shape` key deleted). That key is never
//! read again — [`validate_primitive_node`] is only ever asked to
//! validate a node whose `shape` tag is actually present, and a path's
//! own [`crate::path_codec::validate_path_tree`] never looks at
//! primitive keys at all.

use loro::{LoroMap, LoroValue};

use crate::corner_radii_codec::{
    KEY_CORNER_RADIUS_BL, KEY_CORNER_RADIUS_BR, KEY_CORNER_RADIUS_TL, KEY_CORNER_RADIUS_TR,
    KEY_LEGACY_CORNER_RADIUS, corner_radii_are_valid, read_corner_radii,
};
use crate::path_codec;
use crate::path_model::NewAnchor;
use crate::primitive_model::{
    EllipseFrame, InnerRatio, PointCount, PrimitiveSnapshot, RectBounds, Shape, StarFrame,
};
use crate::units::{Angle, Length, Point};

pub(crate) const KEY_SHAPE: &str = "shape";
pub(crate) const KEY_RECT_BOUNDS: &str = "rect_bounds";
pub(crate) const KEY_ELLIPSE_FRAME: &str = "ellipse_frame";
pub(crate) const KEY_STAR_FRAME: &str = "star_frame";
pub(crate) const KEY_POINT_COUNT: &str = "point_count";
pub(crate) const KEY_INNER_RATIO: &str = "inner_ratio";

pub(crate) const SHAPE_RECT: &str = "rect";
pub(crate) const SHAPE_ELLIPSE: &str = "ellipse";
pub(crate) const SHAPE_POLYGON: &str = "polygon";
pub(crate) const SHAPE_STAR: &str = "star";

/// Every key a primitive node can carry (the legacy `corner_radius` included,
/// which is read but no longer written) — used by "object to path" to strip
/// them all before the node becomes a path (`adrs.md`: "deletes `shape` and
/// every primitive parameter key").
pub(crate) const ALL_PRIMITIVE_KEYS: &[&str] = &[
    KEY_SHAPE,
    KEY_RECT_BOUNDS,
    KEY_LEGACY_CORNER_RADIUS,
    KEY_CORNER_RADIUS_TL,
    KEY_CORNER_RADIUS_TR,
    KEY_CORNER_RADIUS_BR,
    KEY_CORNER_RADIUS_BL,
    KEY_ELLIPSE_FRAME,
    KEY_STAR_FRAME,
    KEY_POINT_COUNT,
    KEY_INNER_RATIO,
];

/// This node's `shape` tag, or `None` if absent (a path) or mistyped —
/// the one dispatch point `adrs.md` names ("dispatch on `shape`
/// alone"). Used by ordinary reads on an already-[`validate_primitive_
/// node`]-checked document, where "mistyped" cannot actually occur
/// (every writer here only ever inserts a plain string); open-file
/// validation itself uses [`read_shape_tag_checked`] instead, which
/// does not collapse "mistyped" into "absent" (architect review: a
/// `shape` field that isn't a string must refuse as `Damaged`, not
/// silently open as a plain path).
pub(crate) fn read_shape_tag(meta: &LoroMap) -> Option<String> {
    match read_shape_tag_checked(meta) {
        ShapeTag::Present(shape) => Some(shape),
        ShapeTag::Absent | ShapeTag::Mistyped => None,
    }
}

/// Whether a node's `shape` key is absent, present with a valid string,
/// or present but not a string at all — the three cases open-file
/// validation must tell apart (`adrs.md`: "a missing or mistyped
/// parameter for the given `shape`" is `Damaged`, and that rule applies
/// to the `shape` key itself, not just a shape's own parameters).
pub(crate) enum ShapeTag {
    /// No `shape` key at all — a path.
    Absent,
    /// A `shape` key holding this string.
    Present(String),
    /// A `shape` key present but not a string — refused as `Damaged`.
    Mistyped,
}

pub(crate) fn read_shape_tag_checked(meta: &LoroMap) -> ShapeTag {
    match meta.get(KEY_SHAPE) {
        None => ShapeTag::Absent,
        Some(value) => match value.get_deep_value() {
            LoroValue::String(s) => ShapeTag::Present(s.to_string()),
            _ => ShapeTag::Mistyped,
        },
    }
}

pub(crate) fn write_shape_tag(meta: &LoroMap, shape: &str) {
    // invariant: inserting a plain string under a known key on an
    // attached, freshly created meta map cannot fail.
    #[allow(clippy::unwrap_used)]
    meta.insert(KEY_SHAPE, shape).unwrap();
}

fn write_mm_list(map: &LoroMap, key: &str, values: &[f64]) {
    // invariant: see `write_shape_tag`.
    #[allow(clippy::unwrap_used)]
    map.insert(key, values.to_vec()).unwrap();
}

pub(crate) fn write_rect_bounds(meta: &LoroMap, bounds: RectBounds) {
    write_mm_list(
        meta,
        KEY_RECT_BOUNDS,
        &[
            bounds.origin.x,
            bounds.origin.y,
            bounds.width.as_mm(),
            bounds.height.as_mm(),
        ],
    );
}

pub(crate) fn read_rect_bounds(meta: &LoroMap) -> Option<RectBounds> {
    let values = read_f64_list(meta, KEY_RECT_BOUNDS, 4)?;
    Some(RectBounds {
        origin: Point::new(values[0], values[1]),
        width: Length::from_mm(values[2]),
        height: Length::from_mm(values[3]),
    })
}

pub(crate) fn write_ellipse_frame(meta: &LoroMap, frame: EllipseFrame) {
    write_mm_list(
        meta,
        KEY_ELLIPSE_FRAME,
        &[
            frame.center.x,
            frame.center.y,
            frame.rx.as_mm(),
            frame.ry.as_mm(),
        ],
    );
}

pub(crate) fn read_ellipse_frame(meta: &LoroMap) -> Option<EllipseFrame> {
    let values = read_f64_list(meta, KEY_ELLIPSE_FRAME, 4)?;
    Some(EllipseFrame {
        center: Point::new(values[0], values[1]),
        rx: Length::from_mm(values[2]),
        ry: Length::from_mm(values[3]),
    })
}

pub(crate) fn write_star_frame(meta: &LoroMap, frame: StarFrame) {
    write_mm_list(
        meta,
        KEY_STAR_FRAME,
        &[
            frame.center.x,
            frame.center.y,
            frame.radius.as_mm(),
            frame.angle.as_radians(),
        ],
    );
}

pub(crate) fn read_star_frame(meta: &LoroMap) -> Option<StarFrame> {
    let values = read_f64_list(meta, KEY_STAR_FRAME, 4)?;
    Some(StarFrame {
        center: Point::new(values[0], values[1]),
        radius: Length::from_mm(values[2]),
        angle: Angle::from_radians(values[3]),
    })
}

/// Writes whichever frame register `shape` has (`rect_bounds`,
/// `ellipse_frame` or `star_frame`) — the one place that maps a [`Shape`]
/// to its stored frame, shared by move and rotate commits. Parameters
/// that are not part of the frame (corner radius, point count, inner
/// ratio) are never rewritten here.
pub(crate) fn write_shape_frame(meta: &LoroMap, shape: &Shape) {
    match *shape {
        Shape::Rect { bounds, .. } => write_rect_bounds(meta, bounds),
        Shape::Ellipse { frame } => write_ellipse_frame(meta, frame),
        Shape::Polygon { frame, .. } | Shape::Star { frame, .. } => write_star_frame(meta, frame),
    }
}

pub(crate) fn write_point_count(meta: &LoroMap, point_count: PointCount) {
    // invariant: see `write_shape_tag`.
    #[allow(clippy::unwrap_used)]
    meta.insert(KEY_POINT_COUNT, i64::from(point_count.get()))
        .unwrap();
}

pub(crate) fn read_point_count(meta: &LoroMap) -> Option<PointCount> {
    let raw = read_i64(meta, KEY_POINT_COUNT)?;
    let n = u32::try_from(raw).ok()?;
    PointCount::new(n).ok()
}

pub(crate) fn write_inner_ratio(meta: &LoroMap, ratio: InnerRatio) {
    // invariant: see `write_shape_tag`.
    #[allow(clippy::unwrap_used)]
    meta.insert(KEY_INNER_RATIO, ratio.get()).unwrap();
}

pub(crate) fn read_inner_ratio(meta: &LoroMap) -> Option<InnerRatio> {
    read_f64(meta, KEY_INNER_RATIO).and_then(|r| InnerRatio::new(r).ok())
}

fn read_f64(meta: &LoroMap, key: &str) -> Option<f64> {
    path_codec::as_f64(&meta.get(key)?.get_deep_value())
}

/// `point_count` must be a genuine integer — a decimal value (e.g.
/// `5.7`) is a mistyped parameter, refused as `Damaged` by
/// [`validate_polygon`]/[`validate_star`] below, not silently truncated
/// (architect review: `adrs.md` names "a missing or mistyped parameter"
/// as a refusal case, and truncating `5.7` to `5` would be exactly the
/// silent coercion that rule exists to rule out).
fn read_i64(meta: &LoroMap, key: &str) -> Option<i64> {
    match meta.get(key)?.get_deep_value() {
        LoroValue::I64(n) => Some(n),
        _ => None,
    }
}

fn read_f64_list(meta: &LoroMap, key: &str, expected_len: usize) -> Option<Vec<f64>> {
    match meta.get(key)?.get_deep_value() {
        LoroValue::List(list) if list.len() == expected_len => {
            list.iter().map(path_codec::as_f64).collect()
        }
        _ => None,
    }
}

/// Reads a primitive node's full [`Shape`] and style, trusting the
/// invariants [`validate_primitive_node`] already confirmed for a
/// document read back from bytes (the same trust model
/// [`crate::path_codec::read_path_snapshot`] uses for a path).
///
/// # Panics
/// Does not panic in practice: see this module's own validation, called
/// before any freshly opened `Document` is handed to a caller, and the
/// fact that every write helper above always writes the shape it
/// matching-reads here.
pub(crate) fn read_primitive_snapshot(
    id: crate::path_model::NodeId,
    meta: &LoroMap,
    shape_tag: &str,
) -> PrimitiveSnapshot {
    // invariant: see above.
    #[allow(clippy::unwrap_used)]
    let shape = read_shape(meta, shape_tag).unwrap();
    PrimitiveSnapshot {
        id,
        shape,
        style: crate::style_codec::read_style(meta),
        rotation: path_codec::read_rotation(meta),
    }
}

pub(crate) fn read_shape(meta: &LoroMap, shape_tag: &str) -> Option<Shape> {
    match shape_tag {
        SHAPE_RECT => Some(Shape::Rect {
            bounds: read_rect_bounds(meta)?,
            corner_radii: read_corner_radii(meta)?,
        }),
        SHAPE_ELLIPSE => Some(Shape::Ellipse {
            frame: read_ellipse_frame(meta)?,
        }),
        SHAPE_POLYGON => Some(Shape::Polygon {
            frame: read_star_frame(meta)?,
            point_count: read_point_count(meta)?,
        }),
        SHAPE_STAR => Some(Shape::Star {
            frame: read_star_frame(meta)?,
            point_count: read_point_count(meta)?,
            inner_ratio: read_inner_ratio(meta)?,
        }),
        _ => None,
    }
}

/// Validates one primitive node against every refusal case
/// `specs/0003-primitive-shapes/adrs.md` names: an unknown `shape` value, a
/// missing or mistyped parameter for that shape, a non-finite number, a
/// negative size or radius, a `point_count` outside `3..=1024`, or an
/// `inner_ratio` outside `(0, 1)`. Reuses each parameter's own validated
/// newtype for its range check rather than duplicating the bound.
pub(crate) fn validate_primitive_node(meta: &LoroMap, shape: &str) -> bool {
    if !path_codec::rotation_is_valid(meta) || !crate::style_codec::style_is_valid(meta) {
        return false;
    }
    match shape {
        SHAPE_RECT => validate_rect(meta),
        SHAPE_ELLIPSE => validate_ellipse(meta),
        SHAPE_POLYGON => validate_polygon(meta),
        SHAPE_STAR => validate_star(meta),
        _ => false,
    }
}

fn finite_non_negative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn validate_rect(meta: &LoroMap) -> bool {
    let Some(bounds) = read_rect_bounds(meta) else {
        return false;
    };
    bounds.origin.x.is_finite()
        && bounds.origin.y.is_finite()
        && finite_non_negative(bounds.width.as_mm())
        && finite_non_negative(bounds.height.as_mm())
        && corner_radii_are_valid(meta)
}

fn validate_ellipse(meta: &LoroMap) -> bool {
    let Some(frame) = read_ellipse_frame(meta) else {
        return false;
    };
    frame.center.x.is_finite()
        && frame.center.y.is_finite()
        && finite_non_negative(frame.rx.as_mm())
        && finite_non_negative(frame.ry.as_mm())
}

fn validate_star_frame(frame: StarFrame) -> bool {
    frame.center.x.is_finite()
        && frame.center.y.is_finite()
        && finite_non_negative(frame.radius.as_mm())
        && frame.angle.as_radians().is_finite()
}

fn validate_polygon(meta: &LoroMap) -> bool {
    let Some(frame) = read_star_frame(meta) else {
        return false;
    };
    // `read_point_count` already applies `PointCount::new`'s own
    // `3..=1024` range check (acceptance criterion 10's file-open
    // refusal) and `None`s out on a mistyped or missing value.
    read_point_count(meta).is_some() && validate_star_frame(frame)
}

fn validate_star(meta: &LoroMap) -> bool {
    let Some(frame) = read_star_frame(meta) else {
        return false;
    };
    read_point_count(meta).is_some()
        && read_inner_ratio(meta).is_some()
        && validate_star_frame(frame)
}

/// Builds the movable `anchors` list and `closed = true` flag "object to
/// path" writes, reusing [`crate::path_codec`]'s own anchor writer so a
/// converted primitive's path shape is byte-for-byte what
/// [`crate::Document::create_path`] would have written for the same
/// anchors.
pub(crate) fn write_converted_path_fields(meta: &LoroMap, anchors: &[NewAnchor]) {
    // invariant: see `write_shape_tag`; `closed` is a plain bool insert.
    #[allow(clippy::unwrap_used)]
    meta.insert(path_codec::KEY_CLOSED, true).unwrap();
    let list = path_codec::insert_anchors_container(meta);
    for anchor in anchors {
        path_codec::push_anchor(&list, anchor);
    }
}

/// Removes every primitive-only key a node might carry — a no-op for
/// any key that was never present (no error either way).
pub(crate) fn strip_primitive_keys(meta: &LoroMap) {
    for key in ALL_PRIMITIVE_KEYS {
        if meta.get(key).is_some() {
            // invariant: deleting a key just confirmed present on an
            // attached map cannot fail.
            #[allow(clippy::unwrap_used)]
            meta.delete(key).unwrap();
        }
    }
}
