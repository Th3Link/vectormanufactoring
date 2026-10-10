import { useCallback, useMemo, useState } from "react";

import { usePreviewGesture, type GestureEnds } from "@/hooks/usePreviewGesture";
import type { GridName, HsvTriple, TextOutcome } from "@/hooks/useStylePanel";
import type { WasmSession } from "@/lib/editorSession";

/** A plain-JS copy of the Rust `BackgroundView` (`specs/0040-document-background`):
 * what the Background block shows. `paint` is the stored one and decides which rows
 * exist; the colour and opacity follow a drag in flight. The colour is packed
 * `0xRRGGBB`, the hex is `#RRGGBBAA`, the opacity a percent. */
export interface BackgroundView {
  paint: "none" | "solid";
  color: number;
  hex: string;
  opacity: number;
  opacityText: string;
  opacityBar: number;
  opacityResettable: boolean;
  opacityTypedMax: number;
  opacityDefaultText: string;
  /** The eyedropper is picking for the background. */
  picking: boolean;
}

const DISABLED_VIEW: BackgroundView = {
  paint: "solid",
  color: 0xe8e8eb,
  hex: "#E8E8EBFF",
  opacity: 100,
  opacityText: "100",
  opacityBar: 1,
  opacityResettable: false,
  opacityTypedMax: 100,
  opacityDefaultText: "100 %",
  picking: false,
};

function readView(session: WasmSession | null): BackgroundView {
  if (!session) {
    return DISABLED_VIEW;
  }
  const raw = session.background_view();
  const view: BackgroundView = {
    paint: raw.paint as BackgroundView["paint"],
    color: raw.color,
    hex: raw.hex,
    opacity: raw.opacity,
    opacityText: raw.opacity_text,
    opacityBar: raw.opacity_bar,
    opacityResettable: raw.opacity_resettable,
    opacityTypedMax: raw.opacity_typed_max,
    opacityDefaultText: raw.opacity_default_text,
    picking: raw.picking,
  };
  raw.free();
  return view;
}

export interface BackgroundPanelApi {
  view: BackgroundView;
  /** The wasm session exists. Before it, `hsvOf` and `rgbOf` are placeholders. */
  ready: boolean;
  /** A press on the Paint group: one commit. */
  setPaint: (paint: "none" | "solid") => void;
  /** Enter or Tab in the hex field. */
  setHex: (text: string) => TextOutcome;
  /** A tick of a drag in the colour area or the hue slider: shown on the canvas,
   * nothing written; one commit at the release. */
  previewHsv: (hue: number, saturation: number, value: number) => void;
  hsvOf: (rgb: number) => HsvTriple;
  rgbOf: (hue: number, saturation: number, value: number) => number;
  /** Ticks, arrow keys, reset and typed text of the Opacity field. */
  previewOpacity: (p: number, grid: GridName) => void;
  stepOpacity: (steps: number, grid: GridName) => void;
  resetOpacity: () => void;
  setOpacityText: (text: string) => TextOutcome;
  /** The eyedropper button: starts picking for the background, or ends it. */
  beginPick: () => void;
  /** Ends picking and writes nothing (a press elsewhere in the panel, Escape). */
  endPick: () => void;
}

interface EditorHandle {
  getSession: () => WasmSession | null;
  syncRevision: number;
}

const BACKGROUND_ENDS: GestureEnds = {
  commit: (session) => session.commit_background_preview(),
  cancel: (session) => session.cancel_background_preview(),
};

/**
 * The Background block's state and commands. Every rule is the session's
 * (`curvyo-editor-wasm::session::background`); this holds the snapshot the block
 * renders and forwards each control's command. It reads the document the block
 * edits, so it exists whatever the selection is (`0040` criterion 23).
 */
export function useBackgroundPanel(editor: EditorHandle): BackgroundPanelApi {
  const { getSession, syncRevision } = editor;
  const [local, setLocal] = useState(0);
  const refresh = useCallback(() => setLocal((n) => n + 1), []);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const view = useMemo(() => readView(getSession()), [getSession, syncRevision, local]);
  const { queue: preview } = usePreviewGesture(getSession, refresh, BACKGROUND_ENDS);

  const act = useCallback(
    <T>(run: (session: WasmSession) => T, fallback: T): T => {
      const session = getSession();
      if (!session) {
        return fallback;
      }
      const result = run(session);
      refresh();
      return result;
    },
    [getSession, refresh],
  );

  // The block is the first thing shown, so it renders before the session exists. The
  // converters change identity when the session appears, which makes the picker read the
  // colour again and repaint its canvases (UX review B1).
  const ready = getSession() !== null;
  const hsvOf = useCallback(
    (rgb: number): HsvTriple => {
      const [hue, saturation, value] = (ready ? getSession()?.colour_hsv(rgb) : null) ?? [
        -1, 0, 0,
      ];
      return [hue, saturation, value];
    },
    [getSession, ready],
  );
  const rgbOf = useCallback(
    (hue: number, saturation: number, value: number) =>
      (ready ? getSession()?.hsv_colour(hue, saturation, value) : null) ?? 0,
    [getSession, ready],
  );

  return {
    view,
    ready,
    setPaint: (paint) => act((s) => s.set_background_paint(paint), false),
    setHex: (text) => act((s) => s.set_background_hex(text), "unchanged"),
    previewHsv: (hue, saturation, value) =>
      preview((s) => s.preview_background_hsv(hue, saturation, value)),
    hsvOf,
    rgbOf,
    previewOpacity: (p, grid) => preview((s) => s.preview_background_opacity(p, grid)),
    stepOpacity: (steps, grid) => preview((s) => s.step_background_opacity(steps, grid)),
    resetOpacity: () => act((s) => s.reset_background_opacity(), false),
    setOpacityText: (text) => act((s) => s.set_background_opacity_text(text), "unchanged"),
    beginPick: () => act((s) => s.begin_colour_pick("background"), undefined),
    endPick: () => act((s) => s.end_colour_pick(), undefined),
  };
}
