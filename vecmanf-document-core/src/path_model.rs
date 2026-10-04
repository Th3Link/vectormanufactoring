//! The path/anchor schema's pure data types (`specs/0002-path-node-editing/
//! adrs.md`, "the anchor schema, and the three merge choices inside it"):
//! identities, the per-anchor data a caller builds or reads, and the
//! typed refusal reasons for an edit. No Loro type appears here — the CRDT
//! wiring lives in [`crate::paths`] — so these types are what
//! `vecmanf-ui-core` and `vecmanf-render-core` actually depend on.

use serde::{Deserialize, Serialize};

use crate::units::{Length, Point, Vec2};

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
/// `vecmanf-document-core`, never minted there").
///
/// Built from a `(peer, counter)` pair the same shape as a Loro peer id
/// plus a per-session monotonic counter — both already available to
/// `vecmanf-ui-core` without this crate (or its caller) touching an
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnchorKind {
    /// Each handle moves independently (acceptance criterion 1's plain
    /// click, or a smooth node converted to corner).
    Corner,
    /// Dragging one handle keeps the other collinear through the anchor at
    /// the same distance (acceptance criterion 2's click-drag).
    Smooth,
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
/// Minimal on purpose: this slice writes exactly one stroke color (black,
/// acceptance criterion 6) and no fill. Gradients, alpha and the maker's
/// own color choice are `stroke-and-fill-styling` (slice 4,
/// `specs/index.md`) and grow this type then, not now.
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
    /// Solid black — this slice's one stroke color (acceptance criterion
    /// 6).
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

/// A path's full data as read from the document — the ADR 0002 §5 "derived
/// local read model" `vecmanf-ui-core` and `vecmanf-render-core` consume
/// instead of touching Loro themselves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathSnapshot {
    /// This path's identity.
    pub id: NodeId,
    /// Whether the last anchor connects back to the first.
    pub closed: bool,
    /// Stroke width — this slice's one placeholder default is 0.25 mm
    /// (acceptance criterion 6); no command here changes it.
    pub stroke_width: Length,
    /// Stroke color — this slice's one placeholder default is solid black
    /// (acceptance criterion 6).
    pub stroke: Color,
    /// Fill — always `None` in this slice (acceptance criterion 6).
    pub fill: Option<Color>,
    /// Anchors in traversal order (ADR 0009 §3's movable-list order, never
    /// an array index — `specs/0002-path-node-editing/adrs.md`).
    pub anchors: Vec<AnchorSnapshot>,
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
}
