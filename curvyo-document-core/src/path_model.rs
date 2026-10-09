//! The path/anchor schema's pure data types (`specs/0002-path-node-editing/
//! adrs.md`, "the anchor schema, and the three merge choices inside it"):
//! identities, the per-anchor data a caller builds or reads, and the
//! typed refusal reasons for an edit. No Loro type appears here — the CRDT
//! wiring lives in [`crate::paths`] — so these types are what
//! `curvyo-ui-core` and `curvyo-render-core` actually depend on.

use serde::{Deserialize, Serialize};

use crate::style_model::Style;
use crate::units::{Angle, Point, Vec2};

/// A path's identity — one tree node (ADR 0002 §5).
///
/// Wraps a Loro `TreeID` without exposing it: ADR 0004 §3 forbids any Loro
/// type in this crate's public API. Two `NodeId`s are equal exactly when
/// they name the same tree node, regardless of when or in what order each
/// was observed — the same guarantee `TreeID` itself makes.
///
/// `(De)Serialize` are hand-written, not derived (see `NodeIdWire` below):
/// `peer` is a `u64` that routinely exceeds `document.json`'s readers'
/// safe-integer range (any JSON number above 2^53), so a plain derive
/// would write it as a JSON number and silently lose precision for any
/// non-Rust reader. `counter` is a plain `i32`, always within that range,
/// and is left as a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
    pub(crate) peer: u64,
    pub(crate) counter: i32,
}

impl NodeId {
    /// Builds a [`NodeId`] from the `(peer, counter)` pair Loro's own
    /// `TreeID` carries — used only by [`crate::paths`]'s conversions to
    /// and from `loro::TreeID`, which never crosses this crate's public
    /// API (ADR 0004 §3).
    pub(crate) const fn from_parts(peer: u64, counter: i32) -> Self {
        Self { peer, counter }
    }
}

/// `document.json`'s on-the-wire shape for a [`NodeId`]: `peer` written as
/// a decimal string so a non-Rust reader never has to parse a JSON number
/// wider than it can represent exactly.
#[derive(Serialize, Deserialize)]
struct NodeIdWire {
    peer: String,
    counter: i32,
}

impl Serialize for NodeId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        NodeIdWire {
            peer: self.peer.to_string(),
            counter: self.counter,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = NodeIdWire::deserialize(deserializer)?;
        let peer = wire.peer.parse().map_err(serde::de::Error::custom)?;
        Ok(Self {
            peer,
            counter: wire.counter,
        })
    }
}

/// An anchor's identity within its path's movable list
/// (`specs/0002-path-node-editing/adrs.md`: "`AnchorId` is minted by the
/// creating peer and is globally unique... passed into
/// `curvyo-document-core`, never minted there").
///
/// Built from a `(peer, counter)` pair the same shape as a Loro peer id
/// plus a per-session monotonic counter — both already available to
/// `curvyo-ui-core` without this crate (or its caller) touching an
/// entropy source (`CLAUDE.md` §6).
///
/// `(De)Serialize` are hand-written as a lowercase, zero-padded 32-digit
/// hex string — the same encoding `crate::paths` already stores this id
/// as inside the Loro CRDT itself (its `id` field) — rather than derived,
/// which would write the raw `u128` as a JSON number and silently lose
/// precision for any non-Rust reader of `document.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnchorId(u128);

impl AnchorId {
    /// Builds an [`AnchorId`] from a peer id and a counter that peer never
    /// reuses within one open session. Stays globally unique across
    /// sessions because the peer id itself is freshly minted per session
    /// (`specs/0001-project-file-foundation/adrs.md`, amended 2026-10-03).
    #[must_use]
    pub const fn new(peer: u64, counter: u64) -> Self {
        Self(((peer as u128) << 64) | counter as u128)
    }

    /// The raw value, for callers that need to serialize an `AnchorId`
    /// outside this crate's own (de)serialization (e.g. a wasm boundary
    /// that prefers a plain number).
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    /// This id's lowercase, zero-padded 32-digit hex encoding — the one
    /// shape it is ever written as, in Loro storage (`crate::paths`) and
    /// in `document.json` alike.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("{:032x}", self.0)
    }

    /// Parses [`AnchorId::to_hex`]'s own encoding back into an id.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        u128::from_str_radix(hex, 16).ok().map(Self)
    }
}

impl Serialize for AnchorId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for AnchorId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid AnchorId hex string"))
    }
}

/// Whether an anchor's two handles move together or independently
/// (`specs/0002-path-node-editing/adrs.md` decision 3: stored, not derived from
/// geometry — a corner node whose handles happen to be mirrored must still
/// behave as a corner).
///
/// Renamed and extended by `specs/0006-path-merge-split-and-node-types/
/// adrs.md`: the slice-2 `Smooth` variant is renamed `Symmetric` (a
/// mechanical rename — every call site matches on the variant, never on
/// its name) and a third variant, `Asymmetric`, is added alongside it.
/// The three names are deliberately distinct from Inkscape's own
/// confusingly-overloaded "smooth"/"symmetric" pair — see that spec's own
/// naming note — and no code may reuse "Smooth" for anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorKind {
    /// Each handle moves independently (acceptance criterion 1's plain
    /// click, or a symmetric/asymmetric node converted to corner).
    Corner,
    /// Dragging one handle keeps the other collinear through the anchor at
    /// the same distance, mirrored (acceptance criterion 2's click-drag;
    /// `specs/0002-path-node-editing/specification.md` called this kind
    /// "smooth").
    Symmetric,
    /// Tangent-continuous like [`AnchorKind::Symmetric`], but each
    /// handle's length is independently adjustable: dragging one handle
    /// rotates the opposite one to stay collinear without changing its
    /// own length (`specs/0006-path-merge-split-and-node-types/
    /// specification.md` acceptance criterion 3).
    Asymmetric,
}

/// Which of an anchor's two handles an operation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandleSlot {
    /// The handle toward the previous anchor in the path.
    In,
    /// The handle toward the next anchor in the path.
    Out,
}

/// An RGB color, 8 bits per channel.
///
/// Alpha is not part of it: stroke, fill and each gradient stop carry their
/// own [`crate::Opacity`] (`specs/0007-stroke-and-fill-styling/adrs.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Color {
    /// Solid black, the default stroke and fill colour.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
}

/// One anchor's full data, as a caller builds it before a write (pen-tool
/// finish, or an insert) and as [`AnchorSnapshot`] reads it back.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NewAnchor {
    /// This anchor's identity.
    pub id: AnchorId,
    /// Its absolute document-space position.
    pub point: Point,
    /// Its handle toward the previous anchor, relative to `point`.
    pub handle_in: Vec2,
    /// Its handle toward the next anchor, relative to `point`.
    pub handle_out: Vec2,
    /// Corner or smooth.
    pub kind: AnchorKind,
}

impl NewAnchor {
    /// A corner anchor with both handles retracted — acceptance criterion
    /// 1's plain click.
    #[must_use]
    pub const fn corner(id: AnchorId, point: Point) -> Self {
        Self {
            id,
            point,
            handle_in: Vec2::ZERO,
            handle_out: Vec2::ZERO,
            kind: AnchorKind::Corner,
        }
    }
}

/// One anchor as read back from the document — identical shape to
/// [`NewAnchor`], kept as a distinct type so a read and a write are never
/// accidentally interchanged at a call site.
pub type AnchorSnapshot = NewAnchor;

/// One further outline of a compound path: the same two fields as a path's
/// first outline (`specs/0016-boolean-operations/adrs.md`, "compound path").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubpathSnapshot {
    /// Whether the last anchor connects back to the first.
    pub closed: bool,
    /// Anchors in traversal order. Every anchor id is unique within the
    /// whole document, like those of the first outline.
    pub anchors: Vec<AnchorSnapshot>,
}

/// One outline of a path as a borrowed view: its `closed` flag and anchors.
/// [`PathSnapshot::subpaths`] yields one per outline, the first included.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubpathRef<'a> {
    /// Whether the last anchor connects back to the first.
    pub closed: bool,
    /// The outline's anchors in traversal order.
    pub anchors: &'a [AnchorSnapshot],
}

/// A path's full data as read from the document — the ADR 0002 §5 "derived
/// local read model" `curvyo-ui-core` and `curvyo-render-core` consume
/// instead of touching Loro themselves.
///
/// A path has one outline (`closed`, `anchors`) or, as the result of a
/// boolean operation, several: the first outline stays in those two fields
/// and the others are `extra_subpaths` (empty for an ordinary path). The
/// fill rule is nonzero over all outlines together, so an outline wound
/// against its surrounding one is a hole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathSnapshot {
    /// This path's identity.
    pub id: NodeId,
    /// Whether the last anchor of the first outline connects back to its
    /// first.
    pub closed: bool,
    /// This path's whole style: stroke and fill (`specs/0007-stroke-and-fill-
    /// styling/adrs.md`). The same type a primitive carries.
    pub style: Style,
    /// The first outline's anchors in traversal order (ADR 0009 §3's
    /// movable-list order, never an array index — `specs/0002-path-node-
    /// editing/adrs.md`).
    pub anchors: Vec<AnchorSnapshot>,
    /// The outlines after the first, in order; empty for an ordinary path.
    /// Skipped in `document.json` when empty, so an ordinary path exports as
    /// before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_subpaths: Vec<SubpathSnapshot>,
    /// This path's own cumulative rotation (`specs/0005-object-transform/
    /// adrs.md`: "used only to keep its selection box oriented on a later
    /// reselect... never consulted to render, hit-test, export or
    /// otherwise reconstruct the path's geometry, which is already
    /// correct in the baked anchors"). `0` for every path this crate
    /// creates directly; a Select-tool rotate drag is the only writer,
    /// and it also bakes the same rotation into `anchors` via
    /// [`PathSnapshot::rotated`] in the same commit.
    pub rotation: Angle,
}

impl PathSnapshot {
    /// Whether this path has more than one outline.
    #[must_use]
    pub fn is_compound(&self) -> bool {
        !self.extra_subpaths.is_empty()
    }

    /// Every outline, the first included, in storage order.
    pub fn subpaths(&self) -> impl Iterator<Item = SubpathRef<'_>> {
        std::iter::once(SubpathRef {
            closed: self.closed,
            anchors: &self.anchors,
        })
        .chain(self.extra_subpaths.iter().map(|subpath| SubpathRef {
            closed: subpath.closed,
            anchors: &subpath.anchors,
        }))
    }

    /// Every anchor of every outline, in [`PathSnapshot::subpaths`] order.
    pub fn all_anchors(&self) -> impl Iterator<Item = &AnchorSnapshot> {
        self.anchors
            .iter()
            .chain(self.extra_subpaths.iter().flat_map(|s| s.anchors.iter()))
    }

    /// Every anchor of every outline, mutably, in the same order.
    pub fn all_anchors_mut(&mut self) -> impl Iterator<Item = &mut AnchorSnapshot> {
        self.anchors.iter_mut().chain(
            self.extra_subpaths
                .iter_mut()
                .flat_map(|s| s.anchors.iter_mut()),
        )
    }

    /// This path rotated by `angle` about `pivot`: every anchor's `point`
    /// rotates about `pivot`; every `handle_in`/`handle_out` rotates by
    /// `angle` alone (already relative to its own anchor, so it needs no
    /// pivot) — acceptance criterion 20 of `specs/0005-object-transform/
    /// specification.md`. `rotation` advances by `angle`, normalized, so
    /// a later reselect still knows this path's own orientation
    /// (criterion 18) even though the anchors themselves are now the
    /// sole source of truth for its actual geometry.
    #[must_use]
    pub fn rotated(&self, pivot: Point, angle: Angle) -> Self {
        let mut rotated = self.clone();
        for anchor in rotated.all_anchors_mut() {
            anchor.point = anchor.point.rotated_around(pivot, angle);
            anchor.handle_in = anchor.handle_in.rotated(angle);
            anchor.handle_out = anchor.handle_out.rotated(angle);
        }
        rotated.rotation =
            Angle::from_radians(self.rotation.as_radians() + angle.as_radians()).normalized();
        rotated
    }

    /// This path resized by `(sx, sy)` about `pivot`, measured along the
    /// path's own local axes (acceptance criterion 12): every anchor's
    /// point and handle vectors are mapped into the path's local frame
    /// (rotated by `-rotation` about `pivot`), scaled per axis, then
    /// mapped back out (rotated by `+rotation`) — "into the local frame,
    /// per-axis scale, back out" (`adrs.md`). Identical to a plain
    /// anisotropic scale when `rotation` is zero. `rotation` itself is
    /// untouched: a resize never changes an object's orientation
    /// (`specification.md` acceptance criterion 23's "a move is always a
    /// pure translation", extended here to "a resize never rotates").
    #[must_use]
    pub fn scaled(&self, pivot: Point, sx: f64, sy: f64) -> Self {
        let into_local = Angle::from_radians(-self.rotation.as_radians());
        let out_of_local = self.rotation;
        let mut scaled = self.clone();
        for anchor in scaled.all_anchors_mut() {
            anchor.point =
                scale_point_in_local_frame(anchor.point, pivot, into_local, out_of_local, sx, sy);
            anchor.handle_in =
                scale_vec_in_local_frame(anchor.handle_in, into_local, out_of_local, sx, sy);
            anchor.handle_out =
                scale_vec_in_local_frame(anchor.handle_out, into_local, out_of_local, sx, sy);
        }
        scaled
    }

    /// This path sheared about the line through `pivot` along the path's own
    /// local axes (`specs/0008-object-transform-refinements/adrs.md`, "skew
    /// (Part B) writes anchors only"): in local coordinates the linear part
    /// is `L = [[1, ku], [kv, 1]]` (an x skew sets `ku`, a y skew `kv`; the
    /// caller passes one of them as zero), applied to every anchor's offset
    /// from `pivot`; in document space `M = R(θ) · L · R(−θ)` with θ the
    /// path's `rotation`. Handle vectors are relative to their anchor, so
    /// they take the linear part only, without a translation. An affine map
    /// sends a cubic's control points to the control points of its image, so
    /// every segment stays exact. `rotation` is untouched: a shear is baked
    /// into the anchors and handles, the register keeps naming the box's
    /// orientation.
    #[must_use]
    pub fn sheared(&self, pivot: Point, ku: f64, kv: f64) -> Self {
        let into_local = Angle::from_radians(-self.rotation.as_radians());
        let out_of_local = self.rotation;
        let shear = |x: f64, y: f64| (x + ku * y, y + kv * x);
        let mut sheared = self.clone();
        for anchor in sheared.all_anchors_mut() {
            let local = anchor.point.rotated_around(pivot, into_local);
            let (dx, dy) = shear(local.x - pivot.x, local.y - pivot.y);
            anchor.point =
                Point::new(pivot.x + dx, pivot.y + dy).rotated_around(pivot, out_of_local);
            for handle in [&mut anchor.handle_in, &mut anchor.handle_out] {
                let local = handle.rotated(into_local);
                let (hx, hy) = shear(local.x, local.y);
                *handle = Vec2::new(hx, hy).rotated(out_of_local);
            }
        }
        sheared
    }
}

/// `point`, mapped into the local frame about `pivot` (rotate by
/// `into_local`), scaled per axis relative to `pivot`, then mapped back
/// out (rotate by `out_of_local`) — [`PathSnapshot::scaled`]'s one rule
/// for an anchor's absolute position.
fn scale_point_in_local_frame(
    point: Point,
    pivot: Point,
    into_local: Angle,
    out_of_local: Angle,
    sx: f64,
    sy: f64,
) -> Point {
    let local = point.rotated_around(pivot, into_local);
    let scaled_local = Point::new(
        pivot.x + (local.x - pivot.x) * sx,
        pivot.y + (local.y - pivot.y) * sy,
    );
    scaled_local.rotated_around(pivot, out_of_local)
}

/// A handle vector (relative to its own anchor, so no pivot is involved)
/// mapped into the local frame, scaled per axis, then mapped back out —
/// [`PathSnapshot::scaled`]'s one rule for a handle.
fn scale_vec_in_local_frame(
    v: Vec2,
    into_local: Angle,
    out_of_local: Angle,
    sx: f64,
    sy: f64,
) -> Vec2 {
    let local = v.rotated(into_local);
    let scaled_local = Vec2::new(local.x * sx, local.y * sy);
    scaled_local.rotated(out_of_local)
}

/// Why a path-editing [`crate::Document`] method refused to apply.
///
/// Every variant describes a caller error against a `PathSnapshot` that is
/// already stale (ADR 0009 §2: a `NodeId`/`AnchorId` held in local state
/// can dangle when a peer deletes it) rather than a defect in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathEditError {
    /// No path with this [`NodeId`] exists (deleted locally or by a
    /// collaborator since the caller last read a snapshot).
    #[error("no such path")]
    NoSuchPath,
    /// No anchor with this [`AnchorId`] exists on the named path, for the
    /// same reason.
    #[error("no such anchor")]
    NoSuchAnchor,
    /// An operation needs two distinct, path-adjacent anchors (e.g. a
    /// segment) and did not get them.
    #[error("the given anchors are not an adjacent segment on this path")]
    NotAnAdjacentSegment,
    /// `Document::join_endpoints` refused: the two given anchors are not
    /// each the first-or-last anchor of an open path, are the same
    /// anchor, or (for two ends of the same path) that path has only two
    /// anchors, so joining them would collapse it to a one-anchor closed
    /// path `render-core` cannot draw
    /// (`specs/0006-path-merge-split-and-node-types/specification.md`
    /// acceptance criterion 8).
    #[error("the given anchors cannot be joined")]
    NotJoinable,
    /// `Document::split_at_anchor` refused: the given anchor is the first
    /// or last anchor of an open path, which has nothing on one side to
    /// split off (acceptance criterion 12).
    #[error("the given anchor cannot be split")]
    NotSplittable,
    /// `Document::resize_path` carried a stroke width that is not a finite
    /// number greater than zero. Nothing is written: a stored width of zero
    /// or less would make the file refuse to open.
    #[error("a stroke width must be a finite number greater than zero")]
    InvalidStrokeWidth,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_path(a: Point, b: Point) -> PathSnapshot {
        PathSnapshot {
            id: NodeId::from_parts(1, 1),
            closed: false,
            style: Style::default(),
            anchors: vec![
                NewAnchor::corner(AnchorId::new(1, 1), a),
                NewAnchor {
                    id: AnchorId::new(1, 2),
                    point: b,
                    handle_in: Vec2::new(-2.0, 0.0),
                    handle_out: Vec2::new(2.0, 0.0),
                    kind: AnchorKind::Symmetric,
                },
            ],
            extra_subpaths: Vec::new(),
            rotation: Angle::from_radians(0.0),
        }
    }

    /// Acceptance criterion 20: every anchor point rotates about the
    /// pivot; handle vectors rotate by the angle alone (no pivot); the
    /// `rotation` register advances by the same angle.
    #[test]
    fn path_rotated_bakes_points_and_handles_and_advances_rotation() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let pivot = Point::new(0.0, 0.0);
        let rotated = path.rotated(pivot, Angle::from_radians(std::f64::consts::FRAC_PI_2));
        // (10, 0) rotated 90 degrees about the origin -> (0, 10).
        assert!((rotated.anchors[1].point.x - 0.0).abs() < 1e-9);
        assert!((rotated.anchors[1].point.y - 10.0).abs() < 1e-9);
        // handle_out (2, 0) rotates to (0, 2), no pivot involved.
        assert!((rotated.anchors[1].handle_out.x - 0.0).abs() < 1e-9);
        assert!((rotated.anchors[1].handle_out.y - 2.0).abs() < 1e-9);
        assert!(
            (rotated.rotation.as_radians() - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "rotation register advances"
        );
    }

    /// A rotate about a point other than the origin still only rotates
    /// the points, never the (already-relative) handle vectors.
    #[test]
    fn path_rotated_about_an_off_origin_pivot() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let pivot = Point::new(5.0, 0.0);
        let rotated = path.rotated(pivot, Angle::from_radians(std::f64::consts::PI));
        // (0,0) rotated 180 degrees about (5,0) -> (10, 0).
        assert!((rotated.anchors[0].point.x - 10.0).abs() < 1e-9);
        assert!(rotated.anchors[0].point.y.abs() < 1e-9);
        // (10,0) rotated 180 degrees about (5,0) -> (0, 0).
        assert!(rotated.anchors[1].point.x.abs() < 1e-9);
        assert!(rotated.anchors[1].point.y.abs() < 1e-9);
    }

    /// Acceptance criterion 12: a plain (unrotated) scale is the
    /// ordinary per-axis scale about the pivot.
    #[test]
    fn path_scaled_with_zero_rotation_scales_plainly_about_the_pivot() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0));
        let scaled = path.scaled(Point::new(0.0, 0.0), 2.0, 3.0);
        assert!((scaled.anchors[1].point.x - 20.0).abs() < 1e-9);
        assert!(scaled.anchors[1].point.y.abs() < 1e-9);
        // handle_out (2,0) scales by sx=2 -> (4, 0).
        assert!((scaled.anchors[1].handle_out.x - 4.0).abs() < 1e-9);
        assert!(scaled.anchors[1].handle_out.y.abs() < 1e-9);
        // rotation register is untouched by a resize.
        assert!(scaled.rotation.as_radians().abs() < 1e-9);
    }

    /// A rotated path's resize happens along its own local axes: scaling
    /// a path that is already rotated 90 degrees by sx along what is now
    /// the document's Y axis (its own local X) stretches it along Y, not
    /// X — proving the "into local frame, scale, back out" round trip.
    #[test]
    fn path_scaled_respects_its_own_rotation_local_axes() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0)).rotated(
            Point::new(0.0, 0.0),
            Angle::from_radians(std::f64::consts::FRAC_PI_2),
        );
        // After the 90-degree rotation, anchor[1] sits at (0, 10) (its
        // local +X axis now points along document +Y).
        assert!((path.anchors[1].point.x - 0.0).abs() < 1e-6);
        assert!((path.anchors[1].point.y - 10.0).abs() < 1e-6);

        let scaled = path.scaled(Point::new(0.0, 0.0), 2.0, 1.0);
        // Scaling by sx=2 along the path's own local X axis (now
        // document Y) doubles the Y extent, leaving X untouched.
        assert!(scaled.anchors[1].point.x.abs() < 1e-6);
        assert!((scaled.anchors[1].point.y - 20.0).abs() < 1e-6);
    }
    /// Skew: an x shear about the line `y = 0` moves each point along x in
    /// proportion to its distance from that line; the line itself stays.
    #[test]
    fn path_sheared_moves_points_in_proportion_to_their_distance_from_the_line() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 4.0));
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.5, 0.0);
        assert!(
            sheared.anchors[0].point.x.abs() < 1e-12,
            "on the fixed line"
        );
        assert!((sheared.anchors[1].point.x - 12.0).abs() < 1e-12);
        assert!((sheared.anchors[1].point.y - 4.0).abs() < 1e-12);
        // Handle vector (2, 0) has no y component: unchanged by an x shear
        // (linear part only, no translation).
        assert!((sheared.anchors[1].handle_out.x - 2.0).abs() < 1e-12);
        assert!(sheared.anchors[1].handle_out.y.abs() < 1e-12);
    }

    /// A y shear moves along y in proportion to the distance from the
    /// vertical line, and a handle with an x component picks up a y part.
    #[test]
    fn path_sheared_in_y_shears_handles_by_the_linear_part_only() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 4.0));
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.0, 0.25);
        assert!((sheared.anchors[1].point.x - 10.0).abs() < 1e-12);
        assert!((sheared.anchors[1].point.y - 6.5).abs() < 1e-12);
        assert!((sheared.anchors[1].handle_out.x - 2.0).abs() < 1e-12);
        assert!((sheared.anchors[1].handle_out.y - 0.5).abs() < 1e-12);
        assert!((sheared.anchors[1].handle_in.y + 0.5).abs() < 1e-12);
    }

    /// Criterion 44: the shear acts along the path's local axes, and the
    /// `rotation` register is not touched.
    #[test]
    fn path_sheared_acts_along_local_axes_and_keeps_rotation() {
        let theta = Angle::from_radians(std::f64::consts::FRAC_PI_2);
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 0.0))
            .rotated(Point::new(0.0, 0.0), theta);
        // The path now runs along document +y; its local x axis is +y. A
        // local y shear (kv) therefore moves points along local y, which is
        // document -x. Anchor 1 at local (10, 0): local y' = 0 + kv * 10.
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.0, 0.3);
        assert!((sheared.anchors[1].point.x + 3.0).abs() < 1e-9);
        assert!((sheared.anchors[1].point.y - 10.0).abs() < 1e-9);
        assert!((sheared.rotation.as_radians() - theta.as_radians()).abs() < 1e-15);
    }

    /// Criterion 43: a skew by k followed by one by −k restores every
    /// anchor and handle (the fixed line is unchanged by the first skew).
    #[test]
    fn path_sheared_by_k_then_minus_k_is_the_identity() {
        let original = simple_path(Point::new(3.0, -2.0), Point::new(10.0, 4.0))
            .rotated(Point::new(1.0, 1.0), Angle::from_radians(0.7));
        let pivot = Point::new(5.0, 5.0);
        for (ku, kv) in [(0.8, 0.0), (0.0, -1.3)] {
            let round_trip = original.sheared(pivot, ku, kv).sheared(pivot, -ku, -kv);
            for (a, b) in round_trip.anchors.iter().zip(&original.anchors) {
                assert!((a.point.x - b.point.x).abs() < 1e-9);
                assert!((a.point.y - b.point.y).abs() < 1e-9);
                assert!((a.handle_in.x - b.handle_in.x).abs() < 1e-9);
                assert!((a.handle_out.y - b.handle_out.y).abs() < 1e-9);
            }
        }
    }

    /// Node kinds, node count and everything but anchors are unchanged
    /// (criterion 42).
    #[test]
    fn path_sheared_changes_only_points_and_handle_vectors() {
        let path = simple_path(Point::new(0.0, 0.0), Point::new(10.0, 4.0));
        let sheared = path.sheared(Point::new(0.0, 0.0), 0.5, 0.0);
        assert_eq!(sheared.anchors.len(), path.anchors.len());
        for (a, b) in sheared.anchors.iter().zip(&path.anchors) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.kind, b.kind);
        }
        assert_eq!(sheared.style, path.style);
        assert_eq!(sheared.closed, path.closed);
    }
}
