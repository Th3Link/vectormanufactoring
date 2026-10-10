import { useCallback, useEffect, useRef, useState } from "react";

import { ModifierTracker } from "@/lib/keyModifiers";
import { createSession, openSession } from "@/lib/editorSession";
import type { WasmSession } from "@/lib/editorSession";
import type { EditHint } from "@/components/EditHintChip";
import type { BooleanOp, BooleanResult } from "@/lib/booleanText";

/** The canvas's backing-buffer (physical pixel) size for a given CSS
 * (layout) size, plus the `devicePixelRatio` that relates the two —
 * `curvyo-editor-wasm`'s `Gpu` needs both: the buffer itself sized to
 * the display's actual resolution (correctness fix: a HiDPI display
 * previously got a buffer sized 1:1 to CSS pixels, then upscaled and
 * softened by the browser compositor — the same class of bug as this
 * slice's `wgpu` surface-resize requirement, but for resolution rather
 * than staleness), and the ratio itself so `Gpu::render` can map back to
 * CSS pixels for the clip-space transform. `devicePixelRatio` can be a
 * fractional, non-integer value (Windows/Linux fractional scaling, e.g.
 * 1.25 or 1.5), so the buffer size is rounded, not assumed to divide
 * evenly. */
function backingBufferSize(
  cssWidth: number,
  cssHeight: number,
): { width: number; height: number; devicePixelRatio: number } {
  const devicePixelRatio = window.devicePixelRatio > 0 ? window.devicePixelRatio : 1;
  return {
    width: Math.max(1, Math.round(cssWidth * devicePixelRatio)),
    height: Math.max(1, Math.round(cssHeight * devicePixelRatio)),
    devicePixelRatio,
  };
}

/** Which tool is active (`specification.md`'s tool rail: Select is first
 * — `canvas-navigation-and-selection`'s launch default, acceptance
 * criterion 13 — then Pen, Node, Rectangle, Ellipse, Polygon/Star). */
export type Tool = "select" | "pen" | "node" | "rectangle" | "ellipse" | "polygon-star";

/** The polygon/star tool-options bar's mode toggle (acceptance criteria
 * 11 vs. 12). */
export type PolyStarMode = "polygon" | "star";

/** A plain-JS copy of the Rust `NodeToolbarState` — read out of the
 * wasm-bindgen struct instance once, immediately, so the instance itself
 * can be `free()`d rather than held onto (see `readToolbarState` below). */
export interface NodeToolbarState {
  canInsert: boolean;
  canDelete: boolean;
  canConvertToCorner: boolean;
  canConvertToSymmetric: boolean;
  canConvertToAsymmetric: boolean;
  canMakeLine: boolean;
  canMakeCurve: boolean;
  /** Join (acceptance criterion 8). */
  canJoin: boolean;
  /** Split (acceptance criterion 12). */
  canSplit: boolean;
  /** The only selected object is a compound path: the bar says its nodes
   * cannot be edited yet (`0016-boolean-operations` criterion 38). */
  compoundOnly: boolean;
}

const EMPTY_TOOLBAR_STATE: NodeToolbarState = {
  canInsert: false,
  canDelete: false,
  canConvertToCorner: false,
  canConvertToSymmetric: false,
  canConvertToAsymmetric: false,
  canMakeLine: false,
  canMakeCurve: false,
  canJoin: false,
  canSplit: false,
  compoundOnly: false,
};

/** A plain-JS copy of the Rust `SelectBarView` (`specs/0009-unified-object-editing/`
 * criteria 21, 21a, 22): which kind controls the Select bar shows and their
 * values. A `*Mixed` flag means the selected objects differ (the field is
 * empty with the placeholder "Mixed"). */
export interface SelectBarState {
  radiusShown: boolean;
  radiusMixed: boolean;
  /** The effective radius, millimetres. */
  radius: number;
  /** The stored radius exceeds what the rectangle allows ("limited"). */
  radiusLimited: boolean;
  /** The stored radius, millimetres, for the tooltip of "limited". */
  radiusStored: number;
  /** One rectangle with unequal corners: its four effective radii in its own
   * frame, millimetres (top-left, top-right, bottom-right, bottom-left) for the
   * "Mixed" tooltip (`specs/0013-rectangle-corner-radii/` criterion 22); `null`
   * otherwise. */
  radiusCorners: [number, number, number, number] | null;
  removeRoundingShown: boolean;
  removeRoundingEnabled: boolean;
  pointsShown: boolean;
  pointsMixed: boolean;
  points: number;
  ratioShown: boolean;
  ratioMixed: boolean;
  ratio: number;
  objectToPathShown: boolean;
}

const EMPTY_SELECT_BAR_STATE: SelectBarState = {
  radiusShown: false,
  radiusMixed: false,
  radius: 0,
  radiusLimited: false,
  radiusStored: 0,
  radiusCorners: null,
  removeRoundingShown: false,
  removeRoundingEnabled: false,
  pointsShown: false,
  pointsMixed: false,
  points: 0,
  ratioShown: false,
  ratioMixed: false,
  ratio: 0,
  objectToPathShown: false,
};

function readSelectBar(raw: {
  radius_shown: boolean;
  radius_mixed: boolean;
  radius: number;
  radius_limited: boolean;
  radius_stored: number;
  radius_corners_shown: boolean;
  radius_tl: number;
  radius_tr: number;
  radius_br: number;
  radius_bl: number;
  remove_rounding_shown: boolean;
  remove_rounding_enabled: boolean;
  points_shown: boolean;
  points_mixed: boolean;
  points: number;
  ratio_shown: boolean;
  ratio_mixed: boolean;
  ratio: number;
  object_to_path_shown: boolean;
  free(): void;
}): SelectBarState {
  const state: SelectBarState = {
    radiusShown: raw.radius_shown,
    radiusMixed: raw.radius_mixed,
    radius: raw.radius,
    radiusLimited: raw.radius_limited,
    radiusStored: raw.radius_stored,
    radiusCorners: raw.radius_corners_shown
      ? [raw.radius_tl, raw.radius_tr, raw.radius_br, raw.radius_bl]
      : null,
    removeRoundingShown: raw.remove_rounding_shown,
    removeRoundingEnabled: raw.remove_rounding_enabled,
    pointsShown: raw.points_shown,
    pointsMixed: raw.points_mixed,
    points: raw.points,
    ratioShown: raw.ratio_shown,
    ratioMixed: raw.ratio_mixed,
    ratio: raw.ratio,
    objectToPathShown: raw.object_to_path_shown,
  };
  raw.free();
  return state;
}

function sameSelectBar(a: SelectBarState, b: SelectBarState): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** A hand-rolled double-click detector (specs/0002-path-node-editing/
 * adrs.md's `PenTool` doc comment: "double-click detection itself is the
 * frontend's job"). A real DOM `dblclick` event fires only after BOTH
 * clicks' own `pointerdown`/`pointerup` pairs have already run — relaying
 * both to the session would place an unwanted extra node (acceptance
 * criterion 3: a finishing double-click is "instead of placing another
 * node", not in addition to it). This tracks press timing/position
 * itself instead, so the second press of a double-click can be withheld
 * from the session entirely and read as one `double_click(x, y)` call
 * once its matching release arrives
 * (`specs/0004-canvas-navigation-and-selection/adrs.md`: "the host
 * detects a double-click... and calls `double_click(x, y)`. `Session`
 * dispatches it" — no more per-tool branching here). The first press
 * still committed normally — exactly the node the maker expects to end
 * up with (matching Inkscape).
 */
/** How long the "max" notice of a limited typed corner radius stays at its knob
 * (`specs/0013-rectangle-corner-radii/` criterion 6). */
const LIMIT_NOTICE_MS = 1500;

const DOUBLE_CLICK_MS = 400;
/** Canvas-relative CSS pixels now (`canvas-navigation-and-selection`: all
 * screen↔document conversion moved into Rust), not document millimetres
 * — comparing in screen space is also exactly right for "did the two
 * presses land close enough to read as one double-click", independent
 * of zoom. */
const DOUBLE_CLICK_PX = 5;

/** How many wheel-delta pixels correspond to one print-sized "line" of
 * scroll (`deltaMode === 1`) or one "page" (`deltaMode === 2`) — most
 * browsers never actually send these modes for a mouse wheel/trackpad,
 * but normalizing them here means `Session::wheel` only ever has to
 * reason about pixels (`adrs.md`'s frontend requirement: "normalize
 * `deltaMode` to pixels"). */
const WHEEL_LINE_HEIGHT_PX = 16;
const WHEEL_PAGE_HEIGHT_PX = 800;

function normalizedWheelDelta(event: WheelEvent): { deltaX: number; deltaY: number } {
  let factor = 1;
  if (event.deltaMode === 1) {
    factor = WHEEL_LINE_HEIGHT_PX;
  } else if (event.deltaMode === 2) {
    factor = WHEEL_PAGE_HEIGHT_PX;
  }
  return { deltaX: event.deltaX * factor, deltaY: event.deltaY * factor };
}

function readToolbarState(raw: {
  can_insert: boolean;
  can_delete: boolean;
  can_convert_to_corner: boolean;
  can_convert_to_symmetric: boolean;
  can_convert_to_asymmetric: boolean;
  can_make_line: boolean;
  can_make_curve: boolean;
  can_join: boolean;
  can_split: boolean;
  compound_only: boolean;
  free(): void;
}): NodeToolbarState {
  const state: NodeToolbarState = {
    canInsert: raw.can_insert,
    canDelete: raw.can_delete,
    canConvertToCorner: raw.can_convert_to_corner,
    canConvertToSymmetric: raw.can_convert_to_symmetric,
    canConvertToAsymmetric: raw.can_convert_to_asymmetric,
    canMakeLine: raw.can_make_line,
    canMakeCurve: raw.can_make_curve,
    canJoin: raw.can_join,
    canSplit: raw.can_split,
    compoundOnly: raw.compound_only,
  };
  raw.free();
  return state;
}

/** Whether a keyboard event's target is an interactive control (a switch,
 * button, input, slider, radio, select) rather than the canvas itself —
 * canvas shortcuts must ignore those. The Style panel sits outside the canvas
 * container, so its keys never get here; the roles are a second guard
 * (`specs/0007-stroke-and-fill-styling` criterion 38). */
function isFormControl(target: EventTarget): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest(
      'input, textarea, select, button, [role="switch"], [role="slider"], [role="radio"], [role="combobox"], [role="option"], [role="dialog"], [contenteditable]:not([contenteditable="false"])',
    ) !== null
  );
}

/** Reads the wasm-bindgen `LiveReadout` instance once, immediately, so
 * it can be `free()`d rather than held onto — same reasoning as
 * `readToolbarState`. */
function readLiveReadout(
  raw: { text: string; anchor_x: number; anchor_y: number; free(): void } | undefined,
): LiveReadout | null {
  if (!raw) {
    return null;
  }
  const readout: LiveReadout = { text: raw.text, x: raw.anchor_x, y: raw.anchor_y };
  raw.free();
  return readout;
}

/** Reads a wasm-bindgen `DocumentPoint` instance once, immediately, so
 * it can be `free()`d rather than held onto — same reasoning as
 * `readToolbarState`. */
function readDocumentPoint(raw: { x: number; y: number; free(): void }): {
  x: number;
  y: number;
} {
  const point = { x: raw.x, y: raw.y };
  raw.free();
  return point;
}

/** The on-canvas numeric readout shown during a shape tool's
 * create-drag (`specs/0003-primitive-shapes/specification.md`'s "Live
 * creation feedback"). `x`/`y` are already canvas-relative CSS pixels
 * (`specs/0004-canvas-navigation-and-selection/adrs.md`: "Anything the
 * DOM positions... is returned already converted") — position the
 * overlay directly from these, no conversion math here. */
export interface LiveReadout {
  text: string;
  x: number;
  y: number;
}

/** One field of the typed numeric entry chip (`object-transform-
 * refinements` criteria 18-32), as `curvyo-editor-wasm` describes it. */
export interface TransformEntryField {
  /** Visible label ("W", "H", "r"; empty for the angle). */
  label: string;
  /** Accessible name ("Width", "Height", "Outer radius", "Angle",
   * "Corner radius", "Inner ratio"). */
  name: string;
  /** The text the field opens with. */
  prefill: string;
  editable: boolean;
}

/** The open numeric entry: what to show and where. Positions are canvas-
 * relative CSS pixels, already converted by Rust. */
export interface TransformEntryState {
  kind: "angle" | "size" | "radius" | "corner-radius" | "inner-ratio" | "skew";
  fields: TransformEntryField[];
  /** The muted second row of a corner radius entry ("All four corners" or "This
   * corner only", `specs/0013-rectangle-corner-radii/` criterion 6); empty for every
   * other entry. */
  scope: string;
  /** Whether the two fields are linked (Ctrl at the second press). */
  linked: boolean;
  handle: { x: number; y: number };
  center: { x: number; y: number };
  /** How far past the handle's position its outermost glyph reaches, in
   * CSS pixels: 6 normally, 22 for an edge resize handle with a skew arrow
   * on the same side (the chip must clear the arrow). */
  glyphReach: number;
  /** The chip goes 16 px right of and below the box centre, as the move chip
   * does, instead of outward from the handle: the key S, whose fixed point is
   * the centre (`edit-interaction-polish` criterion 59). */
  atCentre: boolean;
  /** The chip edits a multi-selection (`specs/0019-multi-object-transform/`):
   * the size chip is named "Resize selection". */
  selection: boolean;
}

/** Reads the wasm-bindgen `TransformEntryView` once, immediately, so it can
 * be `free()`d rather than held onto — same reasoning as `readToolbarState`. */
function readTransformEntry(
  raw:
    | {
        kind: string;
        field_count: number;
        field_label(index: number): string;
        field_name(index: number): string;
        field_prefill(index: number): string;
        field_editable(index: number): boolean;
        linked: boolean;
        scope: string;
        handle_x: number;
        handle_y: number;
        center_x: number;
        center_y: number;
        glyph_reach: number;
        at_centre: boolean;
        selection: boolean;
        free(): void;
      }
    | undefined,
): TransformEntryState | null {
  if (!raw) {
    return null;
  }
  const fields: TransformEntryField[] = [];
  for (let index = 0; index < raw.field_count; index += 1) {
    fields.push({
      label: raw.field_label(index),
      name: raw.field_name(index),
      prefill: raw.field_prefill(index),
      editable: raw.field_editable(index),
    });
  }
  const entry: TransformEntryState = {
    kind: raw.kind as TransformEntryState["kind"],
    fields,
    linked: raw.linked,
    scope: raw.scope,
    handle: { x: raw.handle_x, y: raw.handle_y },
    center: { x: raw.center_x, y: raw.center_y },
    glyphReach: raw.glyph_reach,
    atCentre: raw.at_centre,
    selection: raw.selection,
  };
  raw.free();
  return entry;
}

function sameEntry<T>(a: T | null, b: T | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** The open typed move (`specs/0010-edit-interaction-polish/` criteria 15 to 25):
 * where to put the chip and what an untouched field shows in each mode.
 * Positions are canvas-relative CSS pixels, already converted by Rust. */
export interface MoveEntryState {
  /** The box centre: the centre handle is there, or would be. */
  center: { x: number; y: number };
  /** What an untouched X and Y show in Relative mode. */
  relative: [string, string];
  /** What an untouched X and Y show in Absolute mode (the current top-left). */
  absolute: [string, string];
  /** Whether the Copy check opens on (Ctrl at the second press). */
  copyPreset: boolean;
  /** The chip moves a multi-selection: Absolute measures from the top-left
   * corner of the group box (`specs/0019-multi-object-transform/` criterion 35). */
  selection: boolean;
}

/** Reads the wasm-bindgen `MoveEntryChip` once, immediately, so it can be
 * `free()`d rather than held onto. */
function readMoveEntry(
  raw:
    | {
        center_x: number;
        center_y: number;
        relative_prefill(axis: number): string;
        absolute_prefill(axis: number): string;
        copy_preset(): boolean;
        selection(): boolean;
        free(): void;
      }
    | undefined,
): MoveEntryState | null {
  if (!raw) {
    return null;
  }
  const entry: MoveEntryState = {
    center: { x: raw.center_x, y: raw.center_y },
    relative: [raw.relative_prefill(0), raw.relative_prefill(1)],
    absolute: [raw.absolute_prefill(0), raw.absolute_prefill(1)],
    copyPreset: raw.copy_preset(),
    selection: raw.selection(),
  };
  raw.free();
  return entry;
}

/** The modifier badges of a move (`specs/0010-edit-interaction-polish/` criteria
 * 26, 27, 33): which show, and where the pointer is, canvas-relative CSS
 * pixels. Rust decides everything but the placement
 * (`Session::move_indicators`). */
export interface MoveBadgeState {
  /** The plus badge: a copy drag runs, or a press with Ctrl would start a move. */
  copy: boolean;
  /** The minus badge: a Ctrl marquee or lasso runs, or a press with Ctrl would
   * arm one (`specs/0014-advanced-selection/`): it removes from the selection. */
  remove: boolean;
  /** The lock badge's axis: `""` (none), `"x"` or `"y"`. */
  lock: "" | "x" | "y";
  /** The pointer hotspot. */
  x: number;
  y: number;
}

const NO_BADGES: MoveBadgeState = { copy: false, remove: false, lock: "", x: 0, y: 0 };

/** A key that could not act, shown for 2 s next to the pointer
 * (`specs/0010-edit-interaction-polish/` criterion 59). */
export interface KeyHint {
  text: string;
  /** Changes on every message, so the 2 s life restarts. */
  id: number;
}

/** What `WasmSession.key_down` returns for a refused key. */
const KEY_HINT_TEXT: Record<string, string> = {
  "hint-select-first": "Select an object first",
  "hint-path-only": "Skew works on paths only",
};

/** How long a key hint stays. */
const KEY_HINT_MS = 2000;

export interface EditorSession {
  /** Attach to the `<canvas>` element the host renders. */
  canvasRef: React.RefObject<HTMLCanvasElement | null>;
  /** Attach to the canvas's focusable container (canvas-focus shortcut
   * scope, docs/design-system.md's keyboard shortcut table). */
  containerRef: React.RefObject<HTMLDivElement | null>;
  tool: Tool;
  nodeToolbarState: NodeToolbarState;
  /** The polygon/star tool-options bar's current mode. */
  polyStarMode: PolyStarMode;
  /** The polygon/star tool-options bar's current point count. */
  polyStarPointCount: number;
  /** The polygon/star tool-options bar's current ratio. */
  polyStarRatio: number;
  /** The live numeric readout for an in-progress create-drag, or `null`
   * outside one (ux-engineer review item 2). */
  liveReadout: LiveReadout | null;
  /** The edit hint after a double-click on a primitive, or `null`. */
  editHint: EditHint | null;
  /** Closes the edit hint (3 s, a press, a key, the pointer leaving). */
  dismissEditHint: () => void;
  /** `curvyo-editor-wasm`'s `cursor_hint()` — `"default"`, `"rotate"`
   * or `"resize:<degrees>"` (`object-transform`'s transform-handle
   * cursors); `Canvas` turns it into a CSS cursor via `lib/cursors`. */
  cursorHint: string;
  /** Which hint the handle under the pointer earns (`""`, `"resize-edge"`,
   * `"resize-corner"`, `"resize-corner-uniform"`, `"rotate-corner"`,
   * `"rotate-side"`, `"skew"` (top and bottom handle), `"skew-y"` (left and
   * right), `"move"`, `"param-radius"`, `"param-inner"`): the hover chip's content
   * (`object-transform-refinements` criterion 54). */
  handleHint: string;
  /** The lines of the hint chip of a corner radius knob under the pointer
   * (they depend on the "Link corners" switch and on a limited corner, see
   * `Session::corner_hint_lines`); empty on any other handle. */
  cornerHintLines: string[];
  /** The typed numeric entry to show, or `null`. */
  transformEntry: TransformEntryState | null;
  /** The open typed move, if any. */
  moveEntry: MoveEntryState | null;
  /** The plus and lock badges by the pointer. */
  moveBadges: MoveBadgeState;
  /** Enter in the move chip: `"committed"`, `"unchanged"` or
   * `"invalid:<field>:number"` (the chip stays open). `copy` is the state of
   * the chip's Copy check. */
  commitMoveEntry: (
    first: string,
    second: string,
    absolute: boolean,
    copy: boolean,
  ) => string;
  /** Enter in the entry chip: `"committed"`, `"unchanged"` (both close it)
   * or `"invalid:<field>:number|positive|negative|ratio-range|skew-range|too-large"`
   * (it stays
   * open). */
  commitTransformEntry: (first: string, second: string, lastEdited: number) => string;
  /** Closes the entry without writing (Escape, blur). Idempotent. */
  cancelTransformEntry: () => void;
  /** For linked fields: the text the other field takes. */
  transformEntryLinked: (field: number, text: string) => string | undefined;
  /** Acceptance criterion 5's cursor cue: whether the live cursor is
   * over the in-progress pen path's own close target — `Canvas` swaps
   * to the "pen-with-small-circle" cursor variant while this is `true`. */
  isHoveringPenCloseTarget: boolean;
  /** The current zoom level's integer percentage read-out (acceptance
   * criterion 9) — `StatusBar`'s new center segment. */
  zoomPercent: number;
  /** Whether a drag-pan gesture (middle-mouse or Space+primary,
   * acceptance criteria 3, 4) is currently in flight — `Canvas`'s own
   * grabbing-cursor convention. */
  isPanning: boolean;
  /** Whether Space is currently held — `Canvas`'s own open-hand cursor
   * convention, the moment Space is held but before any drag motion
   * (`docs/design-system.md`'s "Pan cursor"). */
  isSpaceHeld: boolean;
  setTool: (tool: Tool) => void;
  deleteSelected: () => void;
  /** How many objects the session has selected: the rail's tooltips name
   * "Esc, R" while the Select tool has one (criterion 62). */
  selectionCount: number;
  /** What a screen reader says about a multi-selection: the count and the group
   * box size, empty for fewer than two objects. */
  selectionAnnouncement: string;
  /** The one-line message of a key that could not act ("Select one object to
   * type a value"), or `null`; it clears itself after 2 s. */
  keyHint: KeyHint | null;
  convertSelected: (kind: "corner" | "symmetric" | "asymmetric") => void;
  makeLine: () => void;
  makeCurve: () => void;
  /** Join: merges the two selected endpoint nodes into one (acceptance
   * criteria 8-11). No keyboard shortcut (specification.md's "Out of
   * scope"). */
  joinSelected: () => void;
  /** Split: breaks a path apart at the one selected node (acceptance
   * criteria 12-15). No keyboard shortcut, same reasoning as Join. */
  splitSelected: () => void;
  insertSelected: () => void;
  /** Acceptance criterion 6's "remove rounding" action. */
  removeCornerRounding: () => void;
  /** The Select tool's "Scale stroke width" switch
   * (`object-transform` criteria 8, 26-31): whether a resize scales the
   * stroke. Session state, off in every new session, never saved. */
  scaleStrokeWidth: boolean;
  setScaleStrokeWidth: (on: boolean) => void;
  /** The Select tool's "Scale corner radius" switch
   * (`unified-object-editing` criterion 23): whether a resize scales a
   * rectangle's corner radius. Session state, off in every new session. */
  scaleCornerRadius: boolean;
  setScaleCornerRadius: (on: boolean) => void;
  /** The "Link corners" switch (`specs/0013-rectangle-corner-radii/` criterion 2):
   * on, a corner handle sets all four radii. On in every new or opened project. */
  linkCorners: boolean;
  setLinkCorners: (on: boolean) => void;
  /** What the Select bar shows for the current selection (criteria 21-22). */
  selectBar: SelectBarState;
  /** Counts every re-read of the session's UI state: the Style panel reads
   * its own state from the session whenever this changes. */
  syncRevision: number;
  /** The live wasm session, or `null` before the first attach. */
  getSession: () => WasmSession | null;
  /** Registers a function that runs right after every canvas resize has
   * reached the session (window resize, Properties panel toggle), in the
   * same task as the canvas's own redraw: whatever is drawn beside the canvas
   * from the view repaints there instead of one frame late. Returns the
   * function that removes it. */
  addViewResizedListener: (listener: () => void) => () => void;
  /** Re-reads the cursor readout at the last pointer position: a resize or a
   * fit moves the document under a still pointer. */
  refreshCursor: () => void;
  /** Enter in the bar's "Radius" field: `"committed"`, `"unchanged"`,
   * `"invalid:number"` or `"invalid:negative"`. */
  setSelectedRadius: (text: string) => string;
  /** The bar's "Points" field and stepper: one commit. */
  setSelectedPointCount: (count: number) => void;
  /** The bar's "Ratio" slider: a live preview on every tick (nothing is
   * written), one commit on release. */
  previewSelectedRatio: (ratio: number) => void;
  commitSelectedRatio: () => void;
  /** The mode toggle (acceptance criteria 11 vs. 12). */
  setPolyStarMode: (mode: PolyStarMode) => void;
  /** The point-count setting for the next shape (acceptance criterion 10). */
  setPolyStarPointCount: (count: number) => void;
  /** The ratio setting for the next star (it never changes a selected
   * star: that is the Select bar's Ratio). */
  setPolyStarRatio: (ratio: number) => void;
  /** "Object to path" (acceptance criteria 17, 21, 22). */
  convertSelectedToPaths: () => void;
  /** A boolean operation on the selection, in one commit (`0016-boolean-operations`);
   * `"ignored"` when no session is live or the Select tool is not active. */
  applyBoolean: (op: BooleanOp) => BooleanResult;
  onPointerDown: (event: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerMove: (event: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerUp: (event: React.PointerEvent<HTMLCanvasElement>) => void;
  onPointerLeave: () => void;
  /** The browser took the pointer away (a system gesture, an alert):
   * cancel any in-flight drag instead of leaving it half-done. */
  onPointerCancel: () => void;
  /** The canvas container lost focus: clears the Shift/Ctrl reveal state. */
  onContainerBlur: (event: React.FocusEvent<HTMLDivElement>) => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
  onKeyUp: (event: React.KeyboardEvent<HTMLDivElement>) => void;
  /** File → New: swaps in a brand-new, empty session. */
  newProject: () => void;
  /** File → Open / the OS file association: parses `bytes` as a `.curvyo`
   * and swaps it in as the live session.
   * @throws the exact user-facing sentence naming why `bytes` could not
   * be opened (see `@/lib/editorSession`'s `openSession`) — the caller
   * shows it, this hook does not. */
  openProject: (bytes: Uint8Array) => Promise<void>;
  /** File → Save / Save As: packs the live session's document into
   * `.curvyo` container bytes for the host to write.
   * @throws a plain `Error` if there is no live session yet, or the
   * `JsValue` `curvyo-editor-wasm`'s `pack` throws if the document could
   * not be serialized — the caller shows it, this hook does not. */
  packProject: (appVersion: string) => Uint8Array;
}

/** Owns one `WasmSession` for the app's lifetime: creates it, attaches
 * it to the host's `<canvas>`, keeps its `wgpu` surface sized to the
 * canvas (specs/0002-path-node-editing/adrs.md's PASS note, requirement 2),
 * runs its per-frame render loop, and forwards pointer/keyboard/wheel
 * input — "the frontend renders state and forwards input events into it;
 * it holds no editing logic of its own" (same file, ADR 0001 §1/§2).
 * `canvas-navigation-and-selection` moves every screen↔document
 * conversion into Rust (`Session::screen_to_document`) — this hook now
 * only ever passes canvas-relative CSS pixels across the wasm boundary,
 * never a document-space point it computed itself.
 *
 * The session itself is swappable: `newProject`/`openProject` free
 * whatever is currently attached and attach a different one in its
 * place (`specs/0002-path-node-editing/adrs.md`'s PR review: "the host does
 * byte I/O only" — Open/New used to recreate the native `Document`;
 * now they recreate this hook's `WasmSession`), without tearing down
 * the mount effect's resize observer or render loop, which both always
 * read `sessionRef.current` fresh rather than closing over one instance. */
export function useEditorSession(
  onCursorMove: (point: { x: number; y: number }) => void,
): EditorSession {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<WasmSession | null>(null);
  /** Functions to run after each canvas resize reached the session. */
  const viewResizedListenersRef = useRef(new Set<() => void>());
  /** The tail of the session queue (`enqueueSessionTask`). */
  const sessionQueueRef = useRef<Promise<void>>(Promise.resolve());
  const lastPressRef = useRef<{ time: number; x: number; y: number } | null>(
    null,
  );
  const suppressedPressRef = useRef(false);
  /** The second press of a double-click: its position and modifiers, the
   * ones `double_click` is called with when its release arrives (a handle
   * is picked at the second press, `object-transform-refinements`). */
  const doubleClickPressRef = useRef<{
    x: number;
    y: number;
    shift: boolean;
    ctrl: boolean;
  } | null>(null);
  /** Whether a drag-pan gesture is in flight — a ref (not just the
   * mirrored `isPanning` state below) so `onPointerMove`/`onPointerUp`
   * read the current value synchronously within the same event, not a
   * stale one from before the latest `setState` re-render. */
  /** A heavy release is waiting for its `wait` cursor to paint: pointer events
   * are ignored until the commit has run (`specs/0019-multi-object-transform/`
   * criteria 28, 46, 49). */
  const releasePendingRef = useRef(false);
  const panningRef = useRef(false);

  const [tool, setToolState] = useState<Tool>("select");
  const [nodeToolbarState, setNodeToolbarState] = useState<NodeToolbarState>(
    EMPTY_TOOLBAR_STATE,
  );
  const [polyStarMode, setPolyStarModeState] = useState<PolyStarMode>("polygon");
  const [polyStarPointCount, setPolyStarPointCountState] = useState(6);
  const [polyStarRatio, setPolyStarRatioState] = useState(0.5);
  const [isHoveringPenCloseTarget, setIsHoveringPenCloseTarget] =
    useState(false);
  const [liveReadout, setLiveReadout] = useState<LiveReadout | null>(null);
  const [scaleStrokeWidth, setScaleStrokeWidthState] = useState(false);
  const [scaleCornerRadius, setScaleCornerRadiusState] = useState(false);
  const [linkCorners, setLinkCornersState] = useState(true);
  const [selectBar, setSelectBar] = useState<SelectBarState>(EMPTY_SELECT_BAR_STATE);
  const [syncRevision, setSyncRevision] = useState(0);
  const [cursorHint, setCursorHint] = useState("default");
  const [editHint, setEditHint] = useState<EditHint | null>(null);
  const editHintCounter = useRef(0);
  const [handleHint, setHandleHint] = useState("");
  const [cornerHintLines, setCornerHintLines] = useState<string[]>([]);
  const limitNoticeTimer = useRef<number | undefined>(undefined);
  const [transformEntry, setTransformEntry] = useState<TransformEntryState | null>(null);
  const [moveEntry, setMoveEntry] = useState<MoveEntryState | null>(null);
  const [moveBadges, setMoveBadges] = useState<MoveBadgeState>(NO_BADGES);
  const entryOpenRef = useRef(false);
  /** The last pointer position over the canvas (CSS px) — lets a
   * modifier key press/release re-run the hover, so the Select tool's
   * pivot marker and live preview react the instant Shift/Ctrl change,
   * with no pointer motion needed. */
  const lastPointerRef = useRef<{ x: number; y: number } | null>(null);
  /** The Shift, Ctrl and Alt keys that are down (`lib/keyModifiers.ts`). */
  const modifierTrackerRef = useRef(new ModifierTracker());
  /** A pointer event's flags are right: they correct the tracked keys, so a
   * key-up that never arrived is forgotten. */
  const syncTrackedModifiers = useCallback(
    (event: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean; altKey: boolean }) => {
      modifierTrackerRef.current.pointerFlags({
        shift: event.shiftKey,
        ctrl: event.ctrlKey || event.metaKey,
        alt: event.altKey,
      });
    },
    [],
  );
  const [zoomPercent, setZoomPercent] = useState(100);
  const [isPanning, setIsPanning] = useState(false);
  const [isSpaceHeld, setIsSpaceHeld] = useState(false);
  const [selectionCount, setSelectionCount] = useState(0);
  const [selectionAnnouncement, setSelectionAnnouncement] = useState("");
  const [keyHint, setKeyHint] = useState<KeyHint | null>(null);
  const keyHintCounter = useRef(0);

  /** Re-reads the typed entry (position follows zoom, pan and resize; it
   * closes on a tool switch or a selection change). */
  const syncEntry = useCallback((session: WasmSession) => {
    const next = readTransformEntry(session.transform_entry());
    const nextMove = readMoveEntry(session.move_entry());
    entryOpenRef.current = next !== null || nextMove !== null;
    setTransformEntry((previous) => (sameEntry(previous, next) ? previous : next));
    setMoveEntry((previous) => (sameEntry(previous, nextMove) ? previous : nextMove));
  }, []);

  /** Re-reads the modifier badges: a pure read of Rust's state, called after
   * every pointer event and every modifier change so a badge follows its key
   * in the same frame, with the pointer at rest. */
  const syncBadges = useCallback((session: WasmSession) => {
    const last = lastPointerRef.current;
    const raw = session.move_indicators();
    const next: MoveBadgeState = {
      copy: last !== null && raw.copy_badge,
      remove: last !== null && raw.remove_badge,
      lock: last !== null && (raw.lock === "x" || raw.lock === "y") ? raw.lock : "",
      x: last?.x ?? 0,
      y: last?.y ?? 0,
    };
    raw.free();
    setMoveBadges((previous) =>
      previous.copy === next.copy &&
      previous.remove === next.remove &&
      previous.lock === next.lock &&
      (!next.copy && !next.remove && next.lock === ""
        ? true
        : previous.x === next.x && previous.y === next.y)
        ? previous
        : next,
    );
  }, []);

  /** Re-reads every bit of session-owned UI state after any call that
   * might have changed it — cheap, and simpler than having every call
   * site know which ones to refresh. */
  const syncFromSession = useCallback(() => {
    const session = sessionRef.current;
    if (!session) {
      return;
    }
    setToolState(session.tool() as Tool);
    setNodeToolbarState(readToolbarState(session.node_toolbar_state()));
    setIsHoveringPenCloseTarget(session.is_hovering_pen_close_target());
    setPolyStarModeState(session.poly_star_mode() as PolyStarMode);
    setPolyStarPointCountState(session.poly_star_point_count());
    setPolyStarRatioState(session.poly_star_ratio());
    // A new or opened project's session starts with the switch off
    // (criterion 27); reading it back here is what resets the UI.
    setScaleStrokeWidthState(session.scale_stroke_width());
    setScaleCornerRadiusState(session.scale_corner_radius());
    setLinkCornersState(session.link_corners());
    const nextBar = readSelectBar(session.select_bar_state());
    setSelectBar((previous) => (sameSelectBar(previous, nextBar) ? previous : nextBar));
    setZoomPercent(session.zoom_percent());
    setSelectionCount(session.selection_count());
    setSelectionAnnouncement(session.selection_announcement());
    setSyncRevision((revision) => revision + 1);
    syncEntry(session);
    syncBadges(session);
  }, [syncEntry, syncBadges]);

  const getSession = useCallback(() => sessionRef.current, []);

  const refreshCursor = useCallback(() => {
    const session = sessionRef.current;
    const at = lastPointerRef.current;
    if (session && at) {
      onCursorMove(readDocumentPoint(session.screen_to_document(at.x, at.y)));
    }
  }, [onCursorMove]);

  const addViewResizedListener = useCallback((listener: () => void) => {
    viewResizedListenersRef.current.add(listener);
    return () => {
      viewResizedListenersRef.current.delete(listener);
    };
  }, []);

  /** Runs `task` once every earlier queued task has settled. Everything
   * that frees, replaces or publishes the live session goes through this
   * one queue, so none of it can overlap an in-flight `attach_canvas`:
   * that call is async (it negotiates a `wgpu` adapter/device) and holds
   * the session's `Gpu` slot while it runs, so freeing the session
   * underneath it is a use-after-free (`memory access out of bounds` in
   * `drop_glue<Option<Gpu>>`). A task that throws does not stop later
   * ones; its error goes to the caller of `enqueueSessionTask` only. */
  const enqueueSessionTask = useCallback((task: () => Promise<void> | void) => {
    const run = sessionQueueRef.current.then(task);
    sessionQueueRef.current = run.catch(() => {});
    return run;
  }, []);

  /** Frees whatever session is currently live, then makes `session` the
   * live one and attaches it to the host's `<canvas>` — the mount
   * effect's own first attach and every later New/Open swap (including a
   * file passed at launch, which arrives while the first attach may still
   * be in flight) all go through this one path.
   *
   * Runs on the session queue: it starts only after any earlier attach
   * has finished, so the session it frees is never mid-`attach_canvas`.
   * `sessionRef.current` holds a session only while it is safe to use
   * (null during the attach itself), so the render loop and input
   * handlers never call into a session that has an `attach_canvas` in
   * flight. The returned promise rejects if `attach_canvas` does; the
   * session is still published then, as a session without a canvas. */
  const attachSession = useCallback(
    (session: WasmSession) =>
      enqueueSessionTask(async () => {
        sessionRef.current?.free();
        sessionRef.current = null;

        const canvas = canvasRef.current;
        if (!canvas) {
          sessionRef.current = session;
          return;
        }
        const { width, height, devicePixelRatio } = backingBufferSize(
          canvas.clientWidth,
          canvas.clientHeight,
        );
        canvas.width = width;
        canvas.height = height;
        try {
          // `attach_canvas` also records the canvas's CSS size on the new
          // session's viewport (acceptance criterion 10's "resize keeps
          // the center" starts from a correctly-sized viewport on attach,
          // not just on the first later resize).
          await session.attach_canvas(canvas, width, height, devicePixelRatio);
        } finally {
          sessionRef.current = session;
        }
        syncFromSession();
      }),
    [enqueueSessionTask, syncFromSession],
  );

  useEffect(() => {
    let cancelled = false;

    createSession().then((session) => {
      if (cancelled) {
        session.free();
        return;
      }
      void attachSession(session);
    });

    const canvas = canvasRef.current;
    let resizeObserver: ResizeObserver | null = null;
    if (canvas) {
      resizeObserver = new ResizeObserver((entries) => {
        const entry = entries[0];
        if (!entry) {
          return;
        }
        const { width, height, devicePixelRatio } = backingBufferSize(
          entry.contentRect.width,
          entry.contentRect.height,
        );
        canvas.width = width;
        canvas.height = height;
        try {
          // Reconfigures the wgpu surface, updates the viewport's own
          // CSS size (acceptance criterion 10), and renders the next
          // frame — all in this one call (see `WasmSession::resize`'s
          // own doc comment) — a resize never leaves a stale/empty
          // frame on screen waiting for the render loop's next
          // animation-frame tick.
          sessionRef.current?.resize(width, height, devicePixelRatio);
          if (entryOpenRef.current && sessionRef.current) {
            syncEntry(sessionRef.current);
          }
        } catch {
          // A dropped frame on resize (e.g. a momentarily lost surface)
          // is not fatal — the render loop's own next tick recovers it;
          // swallow rather than let it break the ResizeObserver callback
          // for every future resize.
        }
        for (const listener of viewResizedListenersRef.current) {
          listener();
        }
      });
      resizeObserver.observe(canvas);
    }

    // React's `onWheel` is passive and cannot `preventDefault`, so
    // Ctrl+wheel would zoom the whole webview page itself rather than
    // just the canvas (`specs/0004-canvas-navigation-and-selection/
    // adrs.md`'s frontend requirement) — a native, non-passive listener
    // is the only way to stop that.
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const session = sessionRef.current;
      if (!session || !canvas) {
        return;
      }
      const bounds = canvas.getBoundingClientRect();
      const x = event.clientX - bounds.left;
      const y = event.clientY - bounds.top;
      const { deltaX, deltaY } = normalizedWheelDelta(event);
      session.wheel(deltaX, deltaY, x, y, event.shiftKey, event.ctrlKey || event.metaKey);
      setZoomPercent(session.zoom_percent());
      // A pan or zoom moves the document under a still pointer.
      onCursorMove(readDocumentPoint(session.screen_to_document(x, y)));
      if (entryOpenRef.current) {
        syncEntry(session);
      }
    };
    canvas?.addEventListener("wheel", onWheel, { passive: false });

    // Reads `sessionRef.current` fresh every frame (rather than closing
    // over one session instance) so a New/Open swap underneath it is
    // picked up on the very next frame, with no need to restart the
    // loop itself.
    let frame = requestAnimationFrame(function renderLoop() {
      sessionRef.current?.render();
      frame = requestAnimationFrame(renderLoop);
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
      resizeObserver?.disconnect();
      canvas?.removeEventListener("wheel", onWheel);
      // Queued, not immediate: an `attach_canvas` still in flight must
      // finish before its session is freed. StrictMode's remount queues
      // its own attach behind this, so it never sees the freed session.
      void enqueueSessionTask(() => {
        sessionRef.current?.free();
        sessionRef.current = null;
      });
    };
  }, [attachSession, enqueueSessionTask, onCursorMove, syncEntry]);

  const newProject = useCallback(() => {
    void createSession().then(attachSession);
  }, [attachSession]);

  const openProject = useCallback(
    async (bytes: Uint8Array) => {
      const session = await openSession(bytes);
      await attachSession(session);
    },
    [attachSession],
  );

  const packProject = useCallback((appVersion: string): Uint8Array => {
    const session = sessionRef.current;
    if (!session) {
      throw new Error("no editing session is attached yet");
    }
    return session.pack(appVersion);
  }, []);

  const setTool = useCallback(
    (next: Tool) => {
      sessionRef.current?.set_tool(next);
      syncFromSession();
    },
    [syncFromSession],
  );

  const deleteSelected = useCallback(() => {
    sessionRef.current?.delete_selected();
    syncFromSession();
  }, [syncFromSession]);

  const convertSelected = useCallback(
    (kind: "corner" | "symmetric" | "asymmetric") => {
      sessionRef.current?.convert_selected(kind);
      syncFromSession();
    },
    [syncFromSession],
  );

  const makeLine = useCallback(() => {
    sessionRef.current?.make_line();
    syncFromSession();
  }, [syncFromSession]);

  const makeCurve = useCallback(() => {
    sessionRef.current?.make_curve();
    syncFromSession();
  }, [syncFromSession]);

  const joinSelected = useCallback(() => {
    sessionRef.current?.join_selected();
    syncFromSession();
  }, [syncFromSession]);

  const splitSelected = useCallback(() => {
    sessionRef.current?.split_selected();
    syncFromSession();
  }, [syncFromSession]);

  const insertSelected = useCallback(() => {
    sessionRef.current?.insert_selected();
    syncFromSession();
  }, [syncFromSession]);

  const removeCornerRounding = useCallback(() => {
    sessionRef.current?.remove_corner_rounding();
    syncFromSession();
  }, [syncFromSession]);

  const setScaleStrokeWidth = useCallback(
    (on: boolean) => {
      sessionRef.current?.set_scale_stroke_width(on);
      syncFromSession();
    },
    [syncFromSession],
  );

  const setScaleCornerRadius = useCallback(
    (on: boolean) => {
      sessionRef.current?.set_scale_corner_radius(on);
      syncFromSession();
    },
    [syncFromSession],
  );

  const setLinkCorners = useCallback(
    (on: boolean) => {
      sessionRef.current?.set_link_corners(on);
      syncFromSession();
    },
    [syncFromSession],
  );

  const setSelectedRadius = useCallback(
    (text: string): string => {
      const session = sessionRef.current;
      if (!session) {
        return "unchanged";
      }
      const outcome = session.set_selected_radius(text);
      syncFromSession();
      return outcome;
    },
    [syncFromSession],
  );

  const setSelectedPointCount = useCallback(
    (count: number) => {
      sessionRef.current?.set_selected_point_count(count);
      syncFromSession();
    },
    [syncFromSession],
  );

  const previewSelectedRatio = useCallback(
    (ratio: number) => {
      sessionRef.current?.preview_selected_ratio(ratio);
      syncFromSession();
    },
    [syncFromSession],
  );

  const commitSelectedRatio = useCallback(() => {
    sessionRef.current?.commit_selected_ratio();
    syncFromSession();
  }, [syncFromSession]);

  const setPolyStarMode = useCallback(
    (mode: PolyStarMode) => {
      sessionRef.current?.set_poly_star_mode(mode);
      syncFromSession();
    },
    [syncFromSession],
  );

  const setPolyStarPointCount = useCallback(
    (count: number) => {
      sessionRef.current?.set_poly_star_point_count(count);
      syncFromSession();
    },
    [syncFromSession],
  );

  const setPolyStarRatio = useCallback(
    (ratio: number) => {
      sessionRef.current?.set_poly_star_ratio(ratio);
      syncFromSession();
    },
    [syncFromSession],
  );

  const convertSelectedToPaths = useCallback(() => {
    sessionRef.current?.convert_selected_to_paths();
    syncFromSession();
  }, [syncFromSession]);

  const applyBoolean = useCallback(
    (op: BooleanOp): BooleanResult => {
      const raw = sessionRef.current?.apply_boolean(op);
      syncFromSession();
      if (!raw) {
        return { kind: "ignored", count: 0, of: 0, compound: false };
      }
      const result = { kind: raw.kind, count: raw.count, of: raw.of, compound: raw.compound };
      raw.free();
      return result;
    },
    [syncFromSession],
  );

  /** The event's canvas-relative CSS pixel position — pure DOM geometry,
   * no wasm call and no document-space conversion (that happens inside
   * `Session` now, `specs/0004-canvas-navigation-and-selection/adrs.md`).
   */
  const canvasPoint = useCallback((event: { clientX: number; clientY: number }) => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return { x: 0, y: 0 };
    }
    const bounds = canvas.getBoundingClientRect();
    return {
      x: event.clientX - bounds.left,
      y: event.clientY - bounds.top,
    };
  }, []);

  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      if (releasePendingRef.current) {
        return;
      }
      syncTrackedModifiers(event);
      // A press on the canvas takes the focus from a field of the panel, and a
      // field that commits when it is left (the Document size) must do so
      // before the tool sees the press: a resize moves every object and the
      // view, so a press handled first would be stored in the old coordinates.
      const focused = document.activeElement;
      if (
        focused instanceof HTMLInputElement &&
        !containerRef.current?.contains(focused)
      ) {
        focused.blur();
      }
      const session = sessionRef.current;
      if (!session) {
        return;
      }
      const { x, y } = canvasPoint(event);

      // Middle-mouse or Space+primary: a drag-pan gesture (acceptance
      // criteria 3, 4), never forwarded to the active tool — captured so
      // the gesture continues even if the cursor leaves the canvas.
      const isMiddleButton = event.button === 1;
      const isSpacePrimary = event.button === 0 && isSpaceHeld;
      if (isMiddleButton || isSpacePrimary) {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        panningRef.current = true;
        setIsPanning(true);
        session.begin_pan(x, y);
        return;
      }

      const now = performance.now();
      const last = lastPressRef.current;
      // Alt-clicks step through overlapping objects (`specs/0014-advanced-selection/`
      // criterion 5), quickly and at one point: never a double-click.
      const isDoubleClick =
        !event.altKey &&
        last !== null &&
        now - last.time < DOUBLE_CLICK_MS &&
        Math.hypot(x - last.x, y - last.y) < DOUBLE_CLICK_PX;
      lastPressRef.current = { time: now, x, y };
      suppressedPressRef.current = isDoubleClick;
      if (isDoubleClick) {
        doubleClickPressRef.current = {
          x,
          y,
          shift: event.shiftKey,
          ctrl: event.ctrlKey || event.metaKey,
        };
        return;
      }
      setHandleHint("");
      // Capture the pointer for every tool, not only for panning: a
      // transform drag (a rotate swings the pointer in a wide arc) must
      // keep receiving moves and the release when the pointer leaves the
      // canvas, instead of losing the release and clearing its readout.
      if (event.button === 0) {
        try {
          event.currentTarget.setPointerCapture(event.pointerId);
        } catch {
          // Not capturable (e.g. a synthetic event) — the gesture still
          // works while the pointer stays over the canvas.
        }
      }
      session.pointer_down(
        x,
        y,
        event.shiftKey,
        event.ctrlKey || event.metaKey,
        event.altKey,
      );
      setCursorHint(session.cursor_hint());
      syncFromSession();
    },
    [canvasPoint, isSpaceHeld, syncFromSession, syncTrackedModifiers],
  );

  const onPointerMove = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      if (releasePendingRef.current) {
        return;
      }
      syncTrackedModifiers(event);
      const session = sessionRef.current;
      const { x, y } = canvasPoint(event);
      lastPointerRef.current = { x, y };
      if (session) {
        onCursorMove(readDocumentPoint(session.screen_to_document(x, y)));
      }
      if (panningRef.current) {
        session?.pan_to(x, y);
        if (session) {
          // The pan moved the document under the pointer: read it again.
          onCursorMove(readDocumentPoint(session.screen_to_document(x, y)));
        }
        if (entryOpenRef.current && session) {
          syncEntry(session);
        }
        return;
      }
      session?.pointer_hover(
        x,
        y,
        event.shiftKey,
        event.ctrlKey || event.metaKey,
        event.altKey,
      );
      setIsHoveringPenCloseTarget(
        session?.is_hovering_pen_close_target() ?? false,
      );
      setLiveReadout(readLiveReadout(session?.live_readout()));
      setCursorHint(session?.cursor_hint() ?? "default");
      setHandleHint(session?.handle_hint() ?? "");
      setCornerHintLines(session?.corner_hint_lines() ?? []);
      if (session) {
        syncBadges(session);
      }
    },
    [canvasPoint, onCursorMove, syncEntry, syncBadges, syncTrackedModifiers],
  );

  const onPointerUp = useCallback(
    (event: React.PointerEvent<HTMLCanvasElement>) => {
      if (releasePendingRef.current) {
        return;
      }
      syncTrackedModifiers(event);
      const session = sessionRef.current;
      if (!session) {
        return;
      }
      const { x, y } = canvasPoint(event);

      if (panningRef.current) {
        panningRef.current = false;
        setIsPanning(false);
        session.end_pan();
        try {
          event.currentTarget.releasePointerCapture(event.pointerId);
        } catch {
          // Already released (e.g. the pointer was already up) — not an
          // error worth surfacing.
        }
        return;
      }

      try {
        event.currentTarget.releasePointerCapture(event.pointerId);
      } catch {
        // Not captured (e.g. a non-primary button) — nothing to release.
      }

      if (suppressedPressRef.current) {
        suppressedPressRef.current = false;
        // One dispatch point for every tool (acceptance criteria 3, 12,
        // 22, 23) — `Session` itself decides what a double-click does. A
        // transform handle is picked at the *second press*: its position
        // and modifiers, not the release's.
        const press = doubleClickPressRef.current ?? {
          x,
          y,
          shift: event.shiftKey,
          ctrl: event.ctrlKey || event.metaKey,
        };
        doubleClickPressRef.current = null;
        const hintCode = session.double_click(press.x, press.y, press.shift, press.ctrl);
        if (hintCode) {
          // A double-click on a primitive changes nothing; the hint says how
          // to edit it (criterion 32), on every such double-click. On a
          // compound path it says why there are no nodes (`0016` criterion 38).
          editHintCounter.current += 1;
          setEditHint({
            x: press.x,
            y: press.y,
            id: editHintCounter.current,
            compound: hintCode === "compound_path",
          });
        }
        // Re-run the hover so the cursor describes the handle under the
        // pointer right away (a skew double-click changes nothing else).
        session.pointer_hover(
          x,
          y,
          event.shiftKey,
          event.ctrlKey || event.metaKey,
          event.altKey,
        );
      } else {
        const [shift, ctrl, alt] = [event.shiftKey, event.ctrlKey || event.metaKey, event.altKey];
        if (session.release_is_slow()) {
          // A move or transform of many objects: the commit can take seconds.
          // Show the `wait` cursor with the preview unchanged, let it paint,
          // and only then release (`specs/0019-multi-object-transform/`
          // criteria 28, 46, 49). Pointer events are ignored meanwhile.
          releasePendingRef.current = true;
          setCursorHint("wait");
          window.requestAnimationFrame(() => {
            window.setTimeout(() => {
              try {
                session.pointer_up(x, y, shift, ctrl, alt);
                setLiveReadout(null);
                setCursorHint(session.cursor_hint());
                syncFromSession();
              } finally {
                releasePendingRef.current = false;
              }
            }, 0);
          });
          return;
        }
        session.pointer_up(x, y, shift, ctrl, alt);
      }
      setLiveReadout(null);
      setCursorHint(session.cursor_hint());
      syncFromSession();
    },
    [canvasPoint, syncFromSession, syncTrackedModifiers],
  );

  const onPointerCancel = useCallback(() => {
    sessionRef.current?.pointer_cancelled();
    setLiveReadout(null);
    setCursorHint("default");
    syncFromSession();
  }, [syncFromSession]);

  const dismissEditHint = useCallback(() => setEditHint(null), []);

  const onPointerLeave = useCallback(() => {
    sessionRef.current?.pointer_leave();
    setIsHoveringPenCloseTarget(false);
    setLiveReadout(null);
    setCursorHint("default");
    setHandleHint("");
    lastPointerRef.current = null;
    setMoveBadges(NO_BADGES);
  }, []);

  /** Tells the session the modifier state changed — from window-level key
   * events, so Shift reveals the side rotate handles with no pointer
   * movement and while the canvas is not focused — and re-runs the hover
   * at the last pointer position, so the pivot marker, live preview, cursor
   * and hint follow the key in the same frame
   * (`object-transform-refinements` criteria 6, 14). */
  const applyModifiers = useCallback((shift: boolean, ctrl: boolean, alt: boolean) => {
    const session = sessionRef.current;
    if (!session) {
      return;
    }
    session.modifiers_changed(shift, ctrl, alt);
    const last = lastPointerRef.current;
    if (last) {
      session.pointer_hover(last.x, last.y, shift, ctrl, alt);
      setLiveReadout(readLiveReadout(session.live_readout()));
      setHandleHint("");
    }
    setCursorHint(session.cursor_hint());
    syncBadges(session);
  }, [syncBadges]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // A lone Alt press or release would focus the menu bar in some browsers
      // and on Windows; with the pointer on the canvas and the Select tool
      // active Alt belongs to the lasso and the Alt-click cycle
      // (`specs/0014-advanced-selection/`).
      if (
        event.key === "Alt" &&
        lastPointerRef.current !== null &&
        sessionRef.current?.tool() === "select"
      ) {
        event.preventDefault();
      }
      // Tracked by key, not read from the event's flags (`lib/keyModifiers.ts`).
      const held = modifierTrackerRef.current.keyEvent(event);
      applyModifiers(held.shift, held.ctrl, held.alt);
    };
    // A key released outside the window must not leave Shift stuck: the
    // side rotate handles and the pivot follow `modifiers_changed`.
    const onWindowBlur = () => {
      sessionRef.current?.pointer_cancelled();
      modifierTrackerRef.current.reset();
      applyModifiers(false, false, false);
    };
    // A hidden page gets no key-up either.
    const onVisibility = () => {
      if (document.visibilityState === "hidden") {
        modifierTrackerRef.current.reset();
        applyModifiers(false, false, false);
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);
    window.addEventListener("blur", onWindowBlur);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKey);
      window.removeEventListener("blur", onWindowBlur);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [applyModifiers]);

  /** The canvas container lost focus to something outside it. */
  const onContainerBlur = useCallback(
    (event: React.FocusEvent<HTMLDivElement>) => {
      const next = event.relatedTarget;
      if (next instanceof Node && event.currentTarget.contains(next)) {
        return;
      }
      modifierTrackerRef.current.reset();
      applyModifiers(false, false, false);
    },
    [applyModifiers],
  );

  /** A key pressed on the canvas. Every rule (the gate of criterion 55, the
   * key table of criterion 54, the Escape cascade of criterion 42) is
   * `Session::key_down`'s: this forwards the key, the modifiers, the key
   * repeat and the one fact only the DOM has (an IME composition is running
   * or Space is held), and prevents the page's default for everything the
   * session handled, so Ctrl+R and Ctrl+S keep their page and menu behaviour.
   * Keys typed into a control are that control's, not the canvas's. */
  const onKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (isFormControl(event.target)) {
        return;
      }
      if (event.key === " ") {
        // Space+drag pans (acceptance criterion 4) — `preventDefault`
        // so it never activates a focused button
        // (`specs/0004-canvas-navigation-and-selection/adrs.md`'s
        // frontend requirement); the open-hand cursor shows from this
        // moment, before any drag motion (`docs/design-system.md`'s
        // "Pan cursor").
        event.preventDefault();
        setIsSpaceHeld(true);
        return;
      }
      const session = sessionRef.current;
      if (!session) {
        return;
      }
      const outcome = session.key_down(
        event.key,
        event.shiftKey,
        event.ctrlKey || event.metaKey,
        event.altKey,
        event.repeat,
        event.nativeEvent.isComposing || isSpaceHeld,
      );
      if (outcome === "ignored") {
        return;
      }
      event.preventDefault();
      const hint = KEY_HINT_TEXT[outcome];
      if (hint !== undefined) {
        keyHintCounter.current += 1;
        setKeyHint({ text: hint, id: keyHintCounter.current });
      }
      setLiveReadout(readLiveReadout(session.live_readout()));
      setCursorHint(session.cursor_hint());
      syncFromSession();
    },
    [isSpaceHeld, syncFromSession],
  );

  // A key hint clears itself after 2 s; a newer message restarts the clock.
  useEffect(() => {
    if (!keyHint) {
      return undefined;
    }
    const timer = window.setTimeout(() => setKeyHint(null), KEY_HINT_MS);
    return () => window.clearTimeout(timer);
  }, [keyHint]);

  const onKeyUp = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (isFormControl(event.target)) {
        return;
      }
      if (event.key === " ") {
        setIsSpaceHeld(false);
      }
    },
    [],
  );

  const commitTransformEntry = useCallback(
    (first: string, second: string, lastEdited: number): string => {
      const session = sessionRef.current;
      if (!session) {
        return "unchanged";
      }
      const outcome = session.commit_transform_entry(first, second, lastEdited);
      syncFromSession();
      // A typed corner radius past its limit is committed limited and says
      // "r 12.0 mm max" at the knob for 1.5 s: a limit is never silent.
      const notice = readLiveReadout(session.live_readout());
      if (outcome === "committed" && notice) {
        setLiveReadout(notice);
        window.clearTimeout(limitNoticeTimer.current);
        limitNoticeTimer.current = window.setTimeout(() => {
          sessionRef.current?.clear_limit_notice();
          setLiveReadout(null);
        }, LIMIT_NOTICE_MS);
      }
      return outcome;
    },
    [syncFromSession],
  );

  const commitMoveEntry = useCallback(
    (first: string, second: string, absolute: boolean, copy: boolean): string => {
      const session = sessionRef.current;
      if (!session) {
        return "unchanged";
      }
      const outcome = session.commit_move_entry(first, second, absolute, copy);
      syncFromSession();
      return outcome;
    },
    [syncFromSession],
  );

  const cancelTransformEntry = useCallback(() => {
    const session = sessionRef.current;
    if (!session || !entryOpenRef.current) {
      return;
    }
    session.cancel_transform_entry();
    syncFromSession();
  }, [syncFromSession]);

  const transformEntryLinked = useCallback(
    (field: number, text: string): string | undefined =>
      sessionRef.current?.transform_entry_linked(field, text) ?? undefined,
    [],
  );

  return {
    canvasRef,
    containerRef,
    tool,
    nodeToolbarState,
    polyStarMode,
    polyStarPointCount,
    polyStarRatio,
    liveReadout,
    cursorHint,
    editHint,
    dismissEditHint,
    handleHint,
    cornerHintLines,
    transformEntry,
    moveEntry,
    moveBadges,
    commitMoveEntry,
    commitTransformEntry,
    cancelTransformEntry,
    transformEntryLinked,
    onContainerBlur,
    isHoveringPenCloseTarget,
    zoomPercent,
    isPanning,
    isSpaceHeld,
    setTool,
    deleteSelected,
    selectionCount,
    selectionAnnouncement,
    keyHint,
    convertSelected,
    makeLine,
    makeCurve,
    joinSelected,
    splitSelected,
    insertSelected,
    removeCornerRounding,
    scaleStrokeWidth,
    setScaleStrokeWidth,
    scaleCornerRadius,
    setScaleCornerRadius,
    linkCorners,
    setLinkCorners,
    selectBar,
    syncRevision,
    getSession,
    addViewResizedListener,
    refreshCursor,
    setSelectedRadius,
    setSelectedPointCount,
    previewSelectedRatio,
    commitSelectedRatio,
    setPolyStarMode,
    setPolyStarPointCount,
    setPolyStarRatio,
    convertSelectedToPaths,
    applyBoolean,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerLeave,
    onPointerCancel,
    onKeyDown,
    onKeyUp,
    newProject,
    openProject,
    packProject,
  };
}
