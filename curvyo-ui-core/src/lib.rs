//! The Curvyo interaction layer (ADR 0001 §1, §2): pen- and node-tool
//! state machines, hit-testing, selection and command dispatch, as plain
//! state and pure functions (`specs/0002-path-node-editing/adrs.md`).
//!
//! Pure and wasm-compatible (`CLAUDE.md` §6): no filesystem, network,
//! clock, threads or UI. The frontend renders this crate's state and
//! forwards input events into it; it holds no editing logic of its own.
//! Depends on `curvyo-document-core` (the commands these tools dispatch)
//! and `curvyo-geometry-core` (hit-testing a curved segment, subdividing
//! one for an insert) — never on `curvyo-render-core` or the wasm
//! facade (ADR 0011 §3).

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod anchor_id_minter;
mod angle_snap;
mod boolean;
mod colour_hsv;
mod colour_pick;
mod conversion;
mod dash_text;
mod display_unit_text;
mod document_fit;
mod ellipse_tool;
mod hit_test;
mod hit_test_object;
mod marquee;
mod modifiers;
mod move_entry;
mod node_tool;
mod object_bounds;
mod object_selection;
mod oriented_box;
mod panel_content;
mod param_edit;
mod param_entry;
mod param_handles;
mod pen_tool;
mod poly_star_tool;
mod rectangle_tool;
mod resize_direction;
mod ruler;
mod select_bar;
mod select_tool;
mod selection;
mod shape_tool_common;
mod skew_entry;
mod skew_math;
mod style_edit;
mod style_entry;
mod style_panel;
mod style_scope;
mod transform_commit;
mod transform_drag;
mod transform_entry;
mod transform_handle_layout;
mod transform_math;
mod transform_primitive;
mod value_scale;
mod viewport;

pub use anchor_id_minter::AnchorIdMinter;
pub use angle_snap::{MAX_SKEW_SNAP_DEG, snap_angle, snap_skew_angle};
pub use boolean::{
    BooleanAvailability, BooleanOp, BooleanPlan, BooleanRefusal, boolean_availability, plan_boolean,
};
pub use colour_hsv::{Hsv, hsv_to_rgb, rgb_to_hsv};
pub use colour_pick::{PaintTarget, PickedColour, pick_colour};
pub use conversion::build_primitive_conversions;
pub use dash_text::{MAX_DASH_NUMBER, MAX_DASH_NUMBERS, dash_text, parse_dash_text};
pub use display_unit_text::{
    content_too_large_message, document_side_message, format_cursor, format_field_length,
    format_size, format_status_length, parse_document_side,
};
pub use document_fit::fit_document_to_content;
pub use ellipse_tool::EllipseTool;
pub use hit_test::{Hit, hit_test};
pub use hit_test_object::{hit_test_object, hit_test_objects, hit_test_objects_along};
pub use marquee::{MarqueeMode, objects_in_marquee};
pub use modifiers::Modifiers;
pub use move_entry::MoveEntry;
pub use node_tool::{
    HitTolerances, LiveNodeDrag, NodeTool, NodeToolbarState,
    PointerDownOutcome as NodePointerDownOutcome, PointerUpOutcome as NodePointerUpOutcome,
};
pub use object_bounds::{content_bounds, object_bounds, object_outline_bounds};
pub use object_selection::{ObjectSelection, SelectionCombine};
pub use oriented_box::{OrientedBox, oriented_bounds};
pub use panel_content::{PanelContent, panel_content};
pub use param_edit::{
    MAX_INNER_RATIO, MIN_INNER_RATIO, ParamValue, apply_param, clamped_ratio, commit_param_batch,
    value_from_pointer,
};
pub use param_entry::ParamEntry;
pub use param_handles::{
    Corner, HandleTiers, KNOB_DIAMETER_PX, KNOB_INSET_PX, KNOB_PITCH_PX, PARAM_HIT_PX,
    PARAM_MIN_SIDE_PX, ParamHandle, centre_drawn, corner_local_position, handle_tiers, knob_rho,
    param_handles, radius_gain, radius_travel,
};
pub use pen_tool::{PenTool, PointerUpOutcome as PenPointerUpOutcome};
pub use poly_star_tool::{PolyStarMode, PolygonStarTool};
pub use rectangle_tool::RectangleTool;
pub use resize_direction::ResizeDirection;
pub use ruler::{RulerAxis, RulerLabel, RulerLayout, RulerMajor, ruler_layout};
pub use select_bar::{
    BarPreview, BarValue, ObjectKind, SelectBarState, ids_of_kind, select_bar_state,
};
pub use select_tool::{
    Axis, EntryKey, GestureKind, GestureShape, KeyEntryRefusal, LiveEdit, LiveGesture,
    MoveEntryMode, MoveResolution, PressTarget, SelectDoubleClickOutcome, SelectPointerDownOutcome,
    SelectTool, TransformHandleTolerances, classify_press, double_click, entry_anchor,
};
pub use selection::NodeSelection;
pub use shape_tool_common::{CreateOutcome, CreatePreview};
pub use skew_entry::SkewEntry;
pub use skew_math::{SkewFrame, skew_angle, skew_factor, skew_frame};
pub use style_edit::StyleEditor;
pub use style_entry::{
    HexColour, MAX_STROKE_WIDTH_MM, MarkerSlot, StyleEntryError, StyleField, cap_from_name,
    hex_text, join_from_name, marker_place_from_name, marker_shape_from_name, opacity_from_percent,
    parse_hex, parse_marker_count, parse_opacity_percent, parse_stroke_width,
};
pub use style_panel::{
    DashChoice, DashShown, FillPanel, MarkersPanel, Rgba, StrokePanel, StylePanelState,
    style_panel_state,
};
pub use style_scope::{StyleScope, StyleTool, style_scope};
pub use transform_drag::{
    CornerLinking, CornerRadiusScaling, ParamDragInfo, ScaleModes, StrokeScaling,
};
pub use transform_entry::{
    EntryField, EntryKind, EntryOutcome, InvalidReason, TransformEntry, format_degrees,
    parse_entry_number,
};
pub use transform_handle_layout::{
    ALL_EIGHT, CORNERS_FOUR, EditHandle, HandleSpec, Side, hit_transform_handle, is_corner,
    is_drawn_handle, resize_cursor_angle_degrees, resize_handle_local_position,
    skew_cursor_angle_degrees, transform_handles,
};
pub use transform_math::{
    ResizedBox, opposite_direction, polygon_star_resize_factor, resize_anchor_local_position,
    resize_local_box, rotate_delta_angle, rotate_delta_for, rotate_pivot, scaled_and_floored,
    stroke_or_radius_factor,
};
pub use value_scale::{Grid, ValueField, ValueScale};
pub use viewport::{DOCUMENT_INSET_PX, PX_PER_MM_AT_100, Viewport, Zoom};
