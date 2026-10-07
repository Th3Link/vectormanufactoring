//! The Curvyo document model (ADR 0002): units, the document root
//! record, and the `.curvyo` project container (ADR 0004 §1).
//!
//! This crate is pure and wasm-compatible (`CLAUDE.md` §6): no filesystem,
//! network, clock, threads or UI. Everything a caller needs — bytes in,
//! bytes out — is passed in explicitly; `curvyo-storage-io` and
//! `curvyo-app` own the impure edges (reading/writing files, native
//! dialogs).
//!
//! `project-file-foundation` (slice 1) implemented the thinnest possible
//! document: no nodes, just a `format_version` and a page `size`.
//! `path-node-editing` (slice 2) grows the node tree (ADR 0002 §5) with the
//! first geometry — paths and their anchors — without changing the shape
//! slice 1 fixed.

#![forbid(unsafe_code)]
// `CLAUDE.md` §5 allows unwrap/expect in tests; only production code is held
// to the stricter rule.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod container;
mod document;
mod error;
mod objects;
mod path_codec;
mod path_model;
mod path_topology;
mod paths;
mod primitive_model;
mod primitive_outline;
mod shape_codec;
mod shape_radii;
mod shapes;
mod units;

pub use container::{CURRENT_LORO_SNAPSHOT_VERSION, pack, unpack};
pub use document::{CURRENT_FORMAT_VERSION, Document};
pub use error::{OpenError, SaveError};
pub use objects::{CopySource, ObjectEditError};
pub use path_model::{
    AnchorId, AnchorKind, AnchorSnapshot, Color, HandleSlot, NewAnchor, NodeId, PathEditError,
    PathSnapshot,
};
pub use paths::resolve_handle_pair;
pub use primitive_model::{
    EllipseFrame, InnerRatio, ObjectSnapshot, PointCount, PrimitiveSnapshot, RectBounds, Shape,
    ShapeParamError, StarFrame, shape_center, shape_frame_bounds, translate_shape,
};
pub use primitive_outline::{
    KAPPA, OutlineAnchor, effective_corner_radius, ellipse_outline, outline_of, outline_of_rotated,
    polygon_outline, rect_outline, star_outline,
};
pub use shapes::ShapeEditError;
pub use units::{Angle, DocumentSize, Length, Point, Tolerance, Vec2, ViewTransform};
