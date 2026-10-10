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
/// Alpha is not part of it: stroke and fill carry their
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
    pub(crate) fn all_anchors_mut(&mut self) -> impl Iterator<Item = &mut AnchorSnapshot> {
        self.anchors.iter_mut().chain(
            self.extra_subpaths
                .iter_mut()
                .flat_map(|s| s.anchors.iter_mut()),
        )
    }
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
    /// `Document::extend_path`, `connect_paths` or `close_paths` refused: the path to grow is
    /// closed, compound, the same object as the one to absorb, or the absorbed one is closed or
    /// compound (`specs/0034-pen-path-extension`).
    #[error("this path cannot be extended or connected")]
    NotExtendable,
    /// `Document::close_paths` refused: the closed result would have fewer than three anchors.
    #[error("a closed path needs at least three nodes")]
    TooFewToClose,
    /// A path command carried an added anchor id that is used twice, or is already an id of one of
    /// the paths it changes. Only those paths are checked: the caller mints ids (`AnchorIdMinter`),
    /// so a clash with an unrelated object cannot arise in a session.
    #[error("an anchor id is used twice")]
    DuplicateAnchorId,
}
