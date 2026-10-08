import { useCallback, useEffect, useMemo, useRef, useState } from "react";

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
  fillMode: "none",
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
    fillMode: raw.fill_mode as StyleView["fillMode"],
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
}

interface EditorHandle {
  getSession: () => WasmSession | null;
  syncRevision: number;
}

type Preview =
  | { kind: "color"; field: StyleFieldName; rgb: number }
  | { kind: "opacity"; field: StyleFieldName; percent: number };

/**
 * The Style panel's state and commands. Everything with a rule is the
 * session's (`curvyo-ui-core::style_panel`, `style_edit`); this holds the
 * snapshot the panel renders and the gesture bookkeeping of a drag: ticks are
 * coalesced to one session update per animation frame, the first tick
 * installs window listeners for the release, Escape drops the preview, and
 * the release commits once, even outside the control.
 */
export function useStylePanel(editor: EditorHandle): StylePanelApi {
  const { getSession, syncRevision } = editor;
  const [local, setLocal] = useState(0);
  const refresh = useCallback(() => setLocal((n) => n + 1), []);
  // The snapshot is re-read whenever the session says its UI state changed
  // (`syncRevision`) or this panel changed something (`local`).
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const view = useMemo(() => readView(getSession()), [getSession, syncRevision, local]);

  const pending = useRef<Preview | null>(null);
  const frame = useRef(0);
  const endGesture = useRef<(() => void) | null>(null);

  const flushPending = useCallback(() => {
    window.cancelAnimationFrame(frame.current);
    frame.current = 0;
    const tick = pending.current;
    pending.current = null;
    const session = getSession();
    if (!tick || !session) {
      return;
    }
    if (tick.kind === "color") {
      session.preview_style_color(tick.field, tick.rgb);
    } else {
      session.preview_style_opacity(tick.field, tick.percent);
    }
    refresh();
  }, [getSession, refresh]);

  /** The release: flushes the last tick, commits once, removes the listeners. */
  const commit = useCallback(() => {
    flushPending();
    endGesture.current?.();
    endGesture.current = null;
    getSession()?.commit_style_preview();
    refresh();
  }, [flushPending, getSession, refresh]);

  const beginGesture = useCallback(() => {
    if (endGesture.current) {
      return;
    }
    const onRelease = () => commit();
    const onArrowUp = (event: KeyboardEvent) => {
      if (event.key.startsWith("Arrow")) {
        commit();
      }
    };
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      // Escape during a drag drops the preview; the release then writes
      // nothing. It never reaches the popover or the canvas.
      event.preventDefault();
      event.stopPropagation();
      window.cancelAnimationFrame(frame.current);
      frame.current = 0;
      pending.current = null;
      getSession()?.cancel_style_preview();
      refresh();
    };
    window.addEventListener("pointerup", onRelease, true);
    window.addEventListener("pointercancel", onRelease, true);
    window.addEventListener("blur", onRelease);
    window.addEventListener("keyup", onArrowUp, true);
    window.addEventListener("keydown", onEscape, true);
    endGesture.current = () => {
      window.removeEventListener("pointerup", onRelease, true);
      window.removeEventListener("pointercancel", onRelease, true);
      window.removeEventListener("blur", onRelease);
      window.removeEventListener("keyup", onArrowUp, true);
      window.removeEventListener("keydown", onEscape, true);
    };
  }, [commit, getSession, refresh]);

  const queue = useCallback(
    (tick: Preview) => {
      beginGesture();
      pending.current = tick;
      if (frame.current === 0) {
        frame.current = window.requestAnimationFrame(flushPending);
      }
    },
    [beginGesture, flushPending],
  );

  // A panel that goes away mid-gesture must not leave listeners behind.
  useEffect(
    () => () => {
      window.cancelAnimationFrame(frame.current);
      endGesture.current?.();
      endGesture.current = null;
    },
    [],
  );

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
    setText,
    previewColor: (field, rgb) => queue({ kind: "color", field, rgb }),
    previewOpacity: (field, percent) => queue({ kind: "opacity", field, percent }),
    setStrokePaint: (on) => act((s) => s.set_stroke_paint(on)),
    setStrokeDash: (name) => act((s) => s.set_stroke_dash(name)),
    setStrokeJoin: (name) => act((s) => s.set_stroke_join(name)),
    setStrokeCap: (name) => act((s) => s.set_stroke_cap(name)),
    setFillMode: (name) => act((s) => s.set_fill_mode(name)),
  };
}
