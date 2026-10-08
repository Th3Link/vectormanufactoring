import { useCallback, useMemo, useState } from "react";

import { usePreviewGesture } from "@/hooks/usePreviewGesture";
import type { WasmSession } from "@/lib/editorSession";

/** A property edited with typed text or a colour area; the names
 * `WasmSession.set_style_text` and the preview calls take. */
export type StyleFieldName =
  | "stroke-width"
  | "stroke-color"
  | "stroke-opacity"
  | "fill-color"
  | "fill-opacity";

export type DashName = "solid" | "dash" | "dot" | "dash-dot";
export type JoinName = "miter" | "round" | "bevel";
export type CapName = "butt" | "round" | "square";
export type FillModeName = "none" | "solid" | "linear" | "radial";

/** A plain-JS copy of the Rust `StylePanelView` (`specs/0007-stroke-and-fill-
 * styling` criteria 5, 13, 24, 37): what the Style panel shows. A `*Mixed`
 * flag means the edited objects differ (the field is empty with the
 * placeholder "Mixed"). Colours are packed `0xRRGGBB`; opacities are
 * percents, shown rounded. */
export interface StyleView {
  subject: string;
  /** Changes when the edited objects change. */
  scopeKey: string;
  /** There is something to edit; otherwise every control is disabled. */
  enabled: boolean;
  strokePaint: "on" | "off" | "mixed";
  /** Every edited stroke is off: Dash, Join and Cap are disabled. */
  strokeAllOff: boolean;
  strokeColorMixed: boolean;
  strokeColor: number;
  strokeOpacityMixed: boolean;
  strokeOpacity: number;
  strokeWidthMixed: boolean;
  strokeWidth: number;
  strokeDash: DashName | "custom" | "mixed";
  strokeJoin: JoinName | "mixed";
  strokeCap: CapName | "mixed";
  fillMode: FillModeName | "mixed";
  fillColorMixed: boolean;
  fillColor: number;
  fillOpacityMixed: boolean;
  fillOpacity: number;
  /** The gradient stop editor: nothing, a message, or the editor. */
  stopsState: "hidden" | "different-counts" | "editor";
  /** How many objects the editor edits; Add and Remove are for one. */
  stopsObjects: number;
  stopsCanAdd: boolean;
  stopsCanRemove: boolean;
  /** The selection holds a polygon or star (the gradient box note). */
  stopsBoxNote: boolean;
  /** The bar shows its ramp and thumbs; otherwise a neutral hatched track. */
  stopsBarShown: boolean;
  /** One row per stop, in position order. */
  stopRows: StopRow[];
  /** The bar's stops in position order (only when `stopsBarShown`). */
  stopBar: BarStop[];
  /** The selected stop's rank, or -1. */
  selectedStop: number;
}

/** A row of the stop list: the stop of one rank in every edited object. */
export interface StopRow {
  /** Percent. */
  position: number;
  positionMixed: boolean;
  color: number;
  colorMixed: boolean;
  /** Percent. */
  opacity: number;
  opacityMixed: boolean;
}

/** A stop of the gradient bar. */
export interface BarStop {
  /** Percent. */
  position: number;
  color: number;
  /** Percent. */
  opacity: number;
}

export type StopFieldName = "position" | "color" | "opacity";

const DISABLED_VIEW: StyleView = {
  subject: "Nothing selected",
  scopeKey: "",
  enabled: false,
  strokePaint: "on",
  strokeAllOff: false,
  strokeColorMixed: false,
  strokeColor: 0,
  strokeOpacityMixed: false,
  strokeOpacity: 100,
  strokeWidthMixed: false,
  strokeWidth: 0.25,
  strokeDash: "solid",
  strokeJoin: "miter",
  strokeCap: "butt",
  fillMode: "none",
  fillColorMixed: false,
  fillColor: 0,
  fillOpacityMixed: false,
  fillOpacity: 100,
  stopsState: "hidden",
  stopsObjects: 0,
  stopsCanAdd: false,
  stopsCanRemove: false,
  stopsBoxNote: false,
  stopsBarShown: false,
  stopRows: [],
  stopBar: [],
  selectedStop: -1,
};

/** Splits a flat list of numbers into records of `width`. */
function chunks(flat: ArrayLike<number>, width: number): number[][] {
  const out: number[][] = [];
  for (let i = 0; i + width <= flat.length; i += width) {
    out.push(Array.from({ length: width }, (_, k) => flat[i + k]));
  }
  return out;
}

/** Reads the wasm-bindgen `StylePanelView` once, immediately, so the instance
 * can be `free()`d rather than held onto. */
function readView(session: WasmSession | null): StyleView {
  if (!session) {
    return DISABLED_VIEW;
  }
  const raw = session.style_panel_view();
  const view: StyleView = {
    subject: raw.subject,
    scopeKey: raw.scope_key,
    enabled: raw.enabled,
    strokePaint: raw.stroke_paint as StyleView["strokePaint"],
    strokeAllOff: raw.stroke_all_off,
    strokeColorMixed: raw.stroke_color_mixed,
    strokeColor: raw.stroke_color,
    strokeOpacityMixed: raw.stroke_opacity_mixed,
    strokeOpacity: raw.stroke_opacity,
    strokeWidthMixed: raw.stroke_width_mixed,
    strokeWidth: raw.stroke_width,
    strokeDash: raw.stroke_dash as StyleView["strokeDash"],
    strokeJoin: raw.stroke_join as StyleView["strokeJoin"],
    strokeCap: raw.stroke_cap as StyleView["strokeCap"],
    fillMode: raw.fill_mode as StyleView["fillMode"],
    fillColorMixed: raw.fill_color_mixed,
    fillColor: raw.fill_color,
    fillOpacityMixed: raw.fill_opacity_mixed,
    fillOpacity: raw.fill_opacity,
    stopsState: raw.stops_state as StyleView["stopsState"],
    stopsObjects: raw.stops_objects,
    stopsCanAdd: raw.stops_can_add,
    stopsCanRemove: raw.stops_can_remove,
    stopsBoxNote: raw.stops_box_note,
    stopsBarShown: raw.stops_bar_shown,
    stopRows: chunks(raw.stop_rows, 6).map(([p, pm, c, cm, o, om]) => ({
      position: p,
      positionMixed: pm === 1,
      color: c,
      colorMixed: cm === 1,
      opacity: o,
      opacityMixed: om === 1,
    })),
    stopBar: chunks(raw.stop_bar, 3).map(([p, c, o]) => ({ position: p, color: c, opacity: o })),
    selectedStop: raw.selected_stop,
  };
  raw.free();
  return view;
}

/** What `setText` answers: `"committed"`, `"unchanged"`, or
 * `"invalid:<code>"` with the code `hex`, `hex8`, `percent` or `width`. */
export type TextOutcome = string;

export interface StylePanelApi {
  view: StyleView;
  /** A panel drag is running; the colour popover keeps its own state meanwhile. */
  previewing: boolean;
  /** Escapes that dropped a preview; a colour popover restarts on each. */
  cancels: number;
  /** Enter or Tab in a typed field. */
  setText: (field: StyleFieldName, text: string) => TextOutcome;
  /** A colour area or hue tick: shown on the canvas, nothing written. The
   * gesture ends, with one commit, at the next pointer release or arrow
   * key-up anywhere (react-colorful reports neither). */
  previewColor: (field: StyleFieldName, rgb: number) => void;
  /** An opacity slider tick (percent), same gesture rules. */
  previewOpacity: (field: StyleFieldName, percent: number) => void;
  setStrokePaint: (on: boolean) => void;
  setStrokeDash: (name: DashName) => void;
  setStrokeJoin: (name: JoinName) => void;
  setStrokeCap: (name: CapName) => void;
  setFillMode: (name: FillModeName) => void;
  /** A row got focus or a thumb was pressed. */
  selectStop: (rank: number) => void;
  /** Enter or Tab in a stop field. */
  setStopText: (rank: number, field: StopFieldName, text: string) => TextOutcome;
  /** A tick of a drag on the stop of `rank` (see `usePreviewGesture`): a
   * percent for a position and an opacity, `0xRRGGBB` for a colour. The stop is
   * fixed by the first tick. */
  previewStop: (rank: number, field: StopFieldName, value: number) => void;
  /** The Add stop button; `false` when refused. */
  addStop: () => boolean;
  /** A click on the bar at a fraction 0 to 1. */
  addStopAt: (fraction: number) => boolean;
  removeStop: (rank: number) => boolean;
}

interface EditorHandle {
  getSession: () => WasmSession | null;
  syncRevision: number;
}

/**
 * The Style panel's state and commands. Everything with a rule is the
 * session's (`curvyo-ui-core::style_panel`, `style_edit`); this holds the
 * snapshot the panel renders and forwards each control's command. A drag's
 * gesture bookkeeping is `usePreviewGesture`.
 */
export function useStylePanel(editor: EditorHandle): StylePanelApi {
  const { getSession, syncRevision } = editor;
  const [local, setLocal] = useState(0);
  const refresh = useCallback(() => setLocal((n) => n + 1), []);
  // The snapshot is re-read whenever the session says its UI state changed
  // (`syncRevision`) or this panel changed something (`local`).
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const view = useMemo(() => readView(getSession()), [getSession, syncRevision, local]);
  const { queue: preview, previewing, cancels } = usePreviewGesture(getSession, refresh);

  const setText = useCallback(
    (field: StyleFieldName, text: string): TextOutcome => {
      const session = getSession();
      if (!session) {
        return "unchanged";
      }
      const outcome = session.set_style_text(field, text);
      refresh();
      return outcome;
    },
    [getSession, refresh],
  );

  const act = useCallback(
    (run: (session: WasmSession) => void) => {
      const session = getSession();
      if (session) {
        run(session);
        refresh();
      }
    },
    [getSession, refresh],
  );

  /** Runs a command that answers whether it was accepted. */
  const ask = (run: (session: WasmSession) => boolean): boolean => {
    const session = getSession();
    if (!session) {
      return false;
    }
    const accepted = run(session);
    refresh();
    return accepted;
  };

  return {
    view,
    previewing,
    cancels,
    setText,
    previewColor: (field, rgb) => preview((s) => s.preview_style_color(field, rgb)),
    previewOpacity: (field, percent) => preview((s) => s.preview_style_opacity(field, percent)),
    setStrokePaint: (on) => act((s) => s.set_stroke_paint(on)),
    setStrokeDash: (name) => act((s) => s.set_stroke_dash(name)),
    setStrokeJoin: (name) => act((s) => s.set_stroke_join(name)),
    setStrokeCap: (name) => act((s) => s.set_stroke_cap(name)),
    setFillMode: (name) => act((s) => s.set_fill_mode(name)),
    selectStop: (rank) => act((s) => s.select_stop(rank)),
    setStopText: (rank, field, text) => {
      const session = getSession();
      if (!session) {
        return "unchanged";
      }
      const outcome = session.set_stop_text(rank, field, text);
      refresh();
      return outcome;
    },
    previewStop: (rank, field, value) => preview((s) => s.preview_stop(rank, field, value)),
    addStop: () => ask((s) => s.add_stop()),
    addStopAt: (fraction) => ask((s) => s.add_stop_at(fraction)),
    removeStop: (rank) => ask((s) => s.remove_stop(rank)),
  };
}

/** What the colour picker needs of the panel, with the fields as plain names so
 * a stop row can stand in for a stroke or fill property. */
export interface ColourPanel {
  setText: (field: string, text: string) => TextOutcome;
  previewColor: (field: string, rgb: number) => void;
  previewOpacity: (field: string, percent: number) => void;
  previewing: boolean;
  cancels: number;
}

/** The picker's view of the panel for a stroke or fill property. */
export function colourPanelOf(panel: StylePanelApi): ColourPanel {
  return {
    setText: (field, text) => panel.setText(field as StyleFieldName, text),
    previewColor: (field, rgb) => panel.previewColor(field as StyleFieldName, rgb),
    previewOpacity: (field, percent) => panel.previewOpacity(field as StyleFieldName, percent),
    previewing: panel.previewing,
    cancels: panel.cancels,
  };
}

/** The picker's view of the panel for the stop of `rank`: the fields it names
 * are `"color"` and `"opacity"`. */
export function stopColourPanel(panel: StylePanelApi, rank: number): ColourPanel {
  return {
    setText: (field, text) => panel.setStopText(rank, field as StopFieldName, text),
    previewColor: (field, rgb) => panel.previewStop(rank, field as StopFieldName, rgb),
    previewOpacity: (field, percent) => panel.previewStop(rank, field as StopFieldName, percent),
    previewing: panel.previewing,
    cancels: panel.cancels,
  };
}
