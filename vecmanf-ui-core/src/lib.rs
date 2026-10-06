//! The vecmanf interaction layer (ADR 0001 §1, §2): pen- and node-tool
//! state machines, hit-testing, selection and command dispatch, as plain
//! state and pure functions (`specs/0002-path-node-editing/adrs.md`).
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads or UI. The frontend renders this crate's state and
//! forwards input events into it; it holds no editing logic of its own.
//! Depends on `vecmanf-document-core` (the commands these tools dispatch)
//! and `vecmanf-geometry-core` (hit-testing a curved segment, subdividing
//! one for an insert) — never on `vecmanf-render-core` or the wasm
//! facade (ADR 0011 §3).

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod anchor_id_minter;
mod angle_snap;
mod conversion;
mod ellipse_tool;
mod handle_layout;
mod hit_test;
mod hit_test_object;
mod node_tool;
mod object_bounds;
mod object_selection;
mod oriented_box;
mod pen_tool;
mod poly_star_tool;
mod rectangle_tool;
mod select_tool;
mod selection;
mod shape_hit_test;
mod shape_tool_common;
mod skew_math;
mod transform_commit;
mod transform_drag;
mod transform_entry;
mod transform_handle_layout;
mod transform_math;
mod transform_primitive;
mod viewport;

pub use anchor_id_minter::AnchorIdMinter;
pub use angle_snap::{MAX_SKEW_SNAP_DEG, snap_angle, snap_skew_angle};
pub use conversion::build_primitive_conversions;
pub use ellipse_tool::{EllipsePointerDownOutcome, EllipsePointerUpOutcome, EllipseTool};
pub use handle_layout::{HandleKind, ResizeDirection, ShapeHandle, handles_for};
pub use hit_test::{Hit, hit_test};
pub use hit_test_object::hit_test_object;
pub use node_tool::{
    HitTolerances, LiveNodeDrag, NodeTool, NodeToolbarState,
    PointerDownOutcome as NodePointerDownOutcome, PointerUpOutcome as NodePointerUpOutcome,
};
pub use object_bounds::object_bounds;
pub use object_selection::ObjectSelection;
pub use oriented_box::{OrientedBox, oriented_bounds};
pub use pen_tool::{PenTool, PointerUpOutcome as PenPointerUpOutcome};
pub use poly_star_tool::{
    PolyStarMode, PolyStarPointerDownOutcome, PolyStarPointerUpOutcome, PolygonStarTool,
};
pub use rectangle_tool::{RectPointerDownOutcome, RectPointerUpOutcome, RectangleTool};
pub use select_tool::{
    SelectDoubleClickOutcome, SelectPointerDownOutcome, SelectTool, TransformHandleTolerances,
    double_click,
};
pub use selection::NodeSelection;
pub use shape_hit_test::{hit_test_handle, hit_test_primitive};
pub use shape_tool_common::{LiveShape, ShapeHitTolerances};
pub use skew_math::{SkewFrame, skew_angle, skew_factor, skew_frame};
pub use transform_drag::StrokeScaling;
pub use transform_entry::{
    EntryField, EntryKind, EntryOutcome, InvalidReason, TransformEntry, format_degrees,
    parse_entry_number,
};
pub use transform_handle_layout::{
    ALL_EIGHT, CORNERS_FOUR, HandleSpec, Side, TransformHandle, hit_transform_handle, is_corner,
    is_drawn_handle, resize_cursor_angle_degrees, resize_handle_local_position,
    skew_cursor_angle_degrees, transform_handles,
};
pub use transform_math::{
    ResizedBox, opposite_direction, polygon_star_resize_factor, resize_anchor_local_position,
    resize_local_box, rotate_delta_angle, rotate_pivot, scaled_and_floored,
    stroke_or_radius_factor,
};
pub use viewport::{PX_PER_MM_AT_100, Viewport, Zoom};
