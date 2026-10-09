import { useCallback, useMemo, useState } from "react";

import type { WasmSession } from "@/lib/editorSession";

/** What the Properties panel shows (`curvyo-ui-core::panel_content`). */
export type PanelContent = "document" | "style" | "empty";

/** The unit symbols the document stores (`DisplayUnit`). */
export type UnitSymbol = "mm" | "cm" | "in";

/** What Fit to content did. */
export type FitResult =
  | { kind: "fitted" }
  | { kind: "already-fits" }
  | { kind: "empty" }
  | { kind: "blocked" }
  | { kind: "too-large"; message: string };

/** One preset button (`curvyo-ui-core::document_presets_view`). */
export interface PresetButton {
  /** The index of its group in `groupNames`. */
  group: number;
  id: string;
  name: string;
  /** "Paper A4". */
  accessible: string;
  /** The tooltip: lines separated by a newline. */
  tooltip: string;
  pressed: boolean;
}

/** What the presets part of the Document section shows. */
export interface PresetsView {
  /** "A4, portrait", "Custom". */
  subject: string;
  orientation: "portrait" | "landscape" | "none";
  groupNames: string[];
  presets: PresetButton[];
}

/** A plain-JS copy of what the session says about the document: the panel's
 * content, the size fields, the unit and the status bar's size text. Every
 * text is formatted in Rust. */
export interface DocumentView {
  content: PanelContent;
  unit: UnitSymbol;
  widthText: string;
  heightText: string;
  /** The message of a refused size, the limits in the display unit. */
  sideMessage: string;
  hasObjects: boolean;
  /** The status bar's size readout, e.g. "210.0 × 297.0 mm". */
  sizeText: string;
  presets: PresetsView;
}

/** Before a session exists: nothing is shown, no text is formatted here (the
 * formats live in Rust). */
const INITIAL: DocumentView = {
  content: "document",
  unit: "mm",
  widthText: "",
  heightText: "",
  sideMessage: "",
  hasObjects: false,
  sizeText: "",
  presets: { subject: "", orientation: "none", groupNames: [], presets: [] },
};

function readPresets(session: WasmSession): PresetsView {
  const raw = session.document_presets_view();
  const groups = raw.preset_group;
  const ids = raw.preset_ids;
  const names = raw.preset_names;
  const accessible = raw.preset_accessible;
  const tooltips = raw.preset_tooltips;
  const pressed = raw.preset_pressed;
  const view: PresetsView = {
    subject: raw.subject,
    orientation: raw.orientation as PresetsView["orientation"],
    groupNames: raw.group_names,
    presets: Array.from(ids, (id, index) => ({
      group: groups[index],
      id,
      name: names[index],
      accessible: accessible[index],
      tooltip: tooltips[index],
      pressed: pressed[index] === 1,
    })),
  };
  raw.free();
  return view;
}

function readView(session: WasmSession | null): DocumentView {
  if (!session) {
    return INITIAL;
  }
  return {
    content: session.panel_content() as PanelContent,
    unit: session.display_unit() as UnitSymbol,
    widthText: session.document_side_text("width"),
    heightText: session.document_side_text("height"),
    sideMessage: session.document_side_message(),
    hasObjects: session.document_has_objects(),
    sizeText: session.size_text(),
    presets: readPresets(session),
  };
}

export interface DocumentPanelApi {
  view: DocumentView;
  /** The status bar's cursor readout for a document point in millimetres. */
  cursorText: (xMm: number, yMm: number) => string;
  /** Enter, Tab or leaving a size field: `"committed"`, `"unchanged"` or
   * `"invalid:number"`. */
  setSide: (side: "width" | "height", text: string) => string;
  setUnit: (unit: UnitSymbol) => void;
  /** A press on a preset button; Rust decides whether anything is written. */
  pickPreset: (id: string) => void;
  /** A press on an orientation item. */
  setOrientation: (orientation: "portrait" | "landscape") => void;
  fit: () => FitResult;
}

interface EditorHandle {
  getSession: () => WasmSession | null;
  syncRevision: number;
  /** Re-reads the cursor readout after a command moved the document. */
  refreshCursor: () => void;
}

/**
 * The document's panel state and commands (`specs/0015-document-size-and-
 * rulers/`, criteria 12, 14 to 27a, 33 to 36). The snapshot is re-read
 * whenever the session says its UI state changed (`syncRevision`: a New, an
 * Open, a selection, a pen click) or a command here changed something. Every
 * rule is the session's; this forwards each control's command.
 */
export function useDocumentPanel(editor: EditorHandle): DocumentPanelApi {
  const { getSession, syncRevision, refreshCursor } = editor;
  const [local, setLocal] = useState(0);
  const refresh = useCallback(() => setLocal((n) => n + 1), []);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const view = useMemo(() => readView(getSession()), [getSession, syncRevision, local]);

  const setSide = useCallback(
    (side: "width" | "height", text: string): string => {
      const session = getSession();
      if (!session) {
        return "unchanged";
      }
      const outcome = session.set_document_side(side, text);
      refresh();
      if (outcome === "committed") {
        refreshCursor();
      }
      return outcome;
    },
    [getSession, refresh, refreshCursor],
  );

  const setUnit = useCallback(
    (unit: UnitSymbol) => {
      getSession()?.set_display_unit(unit);
      refresh();
    },
    [getSession, refresh],
  );

  const pickPreset = useCallback(
    (id: string) => {
      const outcome = getSession()?.apply_document_preset(id);
      refresh();
      if (outcome === "committed") {
        refreshCursor();
      }
    },
    [getSession, refresh, refreshCursor],
  );

  const setOrientation = useCallback(
    (orientation: "portrait" | "landscape") => {
      const outcome = getSession()?.set_document_orientation(orientation);
      refresh();
      if (outcome === "committed") {
        refreshCursor();
      }
    },
    [getSession, refresh, refreshCursor],
  );

  const fit = useCallback((): FitResult => {
    const session = getSession();
    if (!session) {
      return { kind: "empty" };
    }
    const outcome = session.fit_document();
    refresh();
    if (outcome === "fitted") {
      refreshCursor();
    }
    if (outcome.startsWith("too-large:")) {
      return { kind: "too-large", message: outcome.slice("too-large:".length) };
    }
    return { kind: outcome as "fitted" | "already-fits" | "empty" | "blocked" };
  }, [getSession, refresh, refreshCursor]);

  const cursorText = useCallback(
    (xMm: number, yMm: number) => getSession()?.cursor_text(xMm, yMm) ?? "",
    [getSession],
  );

  return { view, cursorText, setSide, setUnit, pickPreset, setOrientation, fit };
}
