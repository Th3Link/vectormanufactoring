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
  fillPaint: "on" | "off" | "mixed";
  fillColorMixed: boolean;
  fillColor: number;
  fillOpacityMixed: boolean;
  fillOpacity: number;
}

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
  fillPaint: "off",
  fillColorMixed: false,
  fillColor: 0,
  fillOpacityMixed: false,
  fillOpacity: 100,
};

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
    fillPaint: raw.fill_paint as StyleView["fillPaint"],
    fillColorMixed: raw.fill_color_mixed,
    fillColor: raw.fill_color,
    fillOpacityMixed: raw.fill_opacity_mixed,
    fillOpacity: raw.fill_opacity,
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
  setFillPaint: (on: boolean) => void;
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
    setFillPaint: (on) => act((s) => s.set_fill_paint(on)),
  };
}
